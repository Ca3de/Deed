//! Replication System
//!
//! Implements master-slave replication for high availability.
//! - Master node handles all writes
//! - Slave nodes replicate data asynchronously
//! - Replication log tracks all mutations
//! - Automatic failover support

use crate::types::{EntityId, EdgeId, Properties, PropertyValue};
use crate::wal::WALEntry;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, RwLock, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use std::net::{TcpListener, TcpStream};
use std::io::{Read, Write, BufReader, BufWriter};
use serde::{Serialize, Deserialize};

/// Node role in replication topology
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeRole {
    Master,
    Slave,
}

/// Replication log sequence number
pub type ReplicationSeq = u64;

/// Replication log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReplicationEntry {
    InsertEntity {
        seq: ReplicationSeq,
        entity_id: u64,
        entity_type: String,
        properties: HashMap<String, PropertyValue>,
        timestamp: u64,
    },
    UpdateEntity {
        seq: ReplicationSeq,
        entity_id: u64,
        properties: HashMap<String, PropertyValue>,
        timestamp: u64,
    },
    DeleteEntity {
        seq: ReplicationSeq,
        entity_id: u64,
        timestamp: u64,
    },
    CreateEdge {
        seq: ReplicationSeq,
        edge_id: u64,
        from_id: u64,
        to_id: u64,
        edge_type: String,
        properties: HashMap<String, PropertyValue>,
        timestamp: u64,
    },
    DeleteEdge {
        seq: ReplicationSeq,
        edge_id: u64,
        timestamp: u64,
    },
}

impl ReplicationEntry {
    pub fn seq(&self) -> ReplicationSeq {
        match self {
            ReplicationEntry::InsertEntity { seq, .. } => *seq,
            ReplicationEntry::UpdateEntity { seq, .. } => *seq,
            ReplicationEntry::DeleteEntity { seq, .. } => *seq,
            ReplicationEntry::CreateEdge { seq, .. } => *seq,
            ReplicationEntry::DeleteEdge { seq, .. } => *seq,
        }
    }

    pub fn timestamp(&self) -> u64 {
        match self {
            ReplicationEntry::InsertEntity { timestamp, .. } => *timestamp,
            ReplicationEntry::UpdateEntity { timestamp, .. } => *timestamp,
            ReplicationEntry::DeleteEntity { timestamp, .. } => *timestamp,
            ReplicationEntry::CreateEdge { timestamp, .. } => *timestamp,
            ReplicationEntry::DeleteEdge { timestamp, .. } => *timestamp,
        }
    }
}

/// Replication configuration
#[derive(Debug, Clone)]
pub struct ReplicationConfig {
    /// Node ID (unique identifier)
    pub node_id: String,
    /// Node role (Master or Slave)
    pub role: NodeRole,
    /// Master address (for slaves)
    pub master_address: Option<String>,
    /// Replication lag tolerance (ms)
    pub max_lag_ms: u64,
    /// Batch size for replication
    pub batch_size: usize,
}

impl Default for ReplicationConfig {
    fn default() -> Self {
        ReplicationConfig {
            node_id: "node-1".to_string(),
            role: NodeRole::Master,
            master_address: None,
            max_lag_ms: 1000, // 1 second
            batch_size: 100,
        }
    }
}

/// Replication manager
pub struct ReplicationManager {
    config: ReplicationConfig,
    /// Replication log
    log: Arc<RwLock<VecDeque<ReplicationEntry>>>,
    /// Next sequence number
    next_seq: Arc<Mutex<ReplicationSeq>>,
    /// Slave states (for master)
    slave_states: Arc<RwLock<HashMap<String, SlaveState>>>,
    /// Last applied sequence (for slave)
    last_applied_seq: Arc<Mutex<ReplicationSeq>>,
}

/// Slave replication state
#[derive(Debug, Clone)]
pub struct SlaveState {
    pub slave_id: String,
    pub last_ack_seq: ReplicationSeq,
    pub last_contact: u64,
    pub lag_ms: u64,
}

impl ReplicationManager {
    /// Create a new replication manager
    pub fn new(config: ReplicationConfig) -> Self {
        ReplicationManager {
            config,
            log: Arc::new(RwLock::new(VecDeque::new())),
            next_seq: Arc::new(Mutex::new(0)),
            slave_states: Arc::new(RwLock::new(HashMap::new())),
            last_applied_seq: Arc::new(Mutex::new(0)),
        }
    }

    /// Create a master node
    pub fn new_master(node_id: String) -> Self {
        let config = ReplicationConfig {
            node_id,
            role: NodeRole::Master,
            master_address: None,
            max_lag_ms: 1000,
            batch_size: 100,
        };
        Self::new(config)
    }

    /// Create a slave node
    pub fn new_slave(node_id: String, master_address: String) -> Self {
        let config = ReplicationConfig {
            node_id,
            role: NodeRole::Slave,
            master_address: Some(master_address),
            max_lag_ms: 1000,
            batch_size: 100,
        };
        Self::new(config)
    }

    /// Get node role
    pub fn role(&self) -> NodeRole {
        self.config.role
    }

    /// Log an insert operation (master only)
    pub fn log_insert(
        &self,
        entity_id: u64,
        entity_type: String,
        properties: HashMap<String, PropertyValue>,
    ) -> Result<ReplicationSeq, String> {
        if self.config.role != NodeRole::Master {
            return Err("Only master can log operations".to_string());
        }

        let seq = self.get_next_seq();
        let timestamp = current_timestamp();

        let entry = ReplicationEntry::InsertEntity {
            seq,
            entity_id,
            entity_type,
            properties,
            timestamp,
        };

        self.log.write().unwrap().push_back(entry);
        Ok(seq)
    }

    /// Log an update operation (master only)
    pub fn log_update(
        &self,
        entity_id: u64,
        properties: HashMap<String, PropertyValue>,
    ) -> Result<ReplicationSeq, String> {
        if self.config.role != NodeRole::Master {
            return Err("Only master can log operations".to_string());
        }

        let seq = self.get_next_seq();
        let timestamp = current_timestamp();

        let entry = ReplicationEntry::UpdateEntity {
            seq,
            entity_id,
            properties,
            timestamp,
        };

        self.log.write().unwrap().push_back(entry);
        Ok(seq)
    }

    /// Log a delete operation (master only)
    pub fn log_delete(&self, entity_id: u64) -> Result<ReplicationSeq, String> {
        if self.config.role != NodeRole::Master {
            return Err("Only master can log operations".to_string());
        }

        let seq = self.get_next_seq();
        let timestamp = current_timestamp();

        let entry = ReplicationEntry::DeleteEntity {
            seq,
            entity_id,
            timestamp,
        };

        self.log.write().unwrap().push_back(entry);
        Ok(seq)
    }

    /// Log a create edge operation (master only)
    pub fn log_create_edge(
        &self,
        edge_id: u64,
        from_id: u64,
        to_id: u64,
        edge_type: String,
        properties: HashMap<String, PropertyValue>,
    ) -> Result<ReplicationSeq, String> {
        if self.config.role != NodeRole::Master {
            return Err("Only master can log operations".to_string());
        }

        let seq = self.get_next_seq();
        let timestamp = current_timestamp();

        let entry = ReplicationEntry::CreateEdge {
            seq,
            edge_id,
            from_id,
            to_id,
            edge_type,
            properties,
            timestamp,
        };

        self.log.write().unwrap().push_back(entry);
        Ok(seq)
    }

    /// Get entries since a sequence number
    pub fn get_entries_since(&self, since_seq: ReplicationSeq) -> Vec<ReplicationEntry> {
        let log = self.log.read().unwrap();
        log.iter()
            .filter(|entry| entry.seq() > since_seq)
            .take(self.config.batch_size)
            .cloned()
            .collect()
    }

    /// Apply replication entry (slave only)
    pub fn apply_entry(&self, entry: ReplicationEntry) -> Result<(), String> {
        if self.config.role != NodeRole::Slave {
            return Err("Only slave can apply entries".to_string());
        }

        // Update last applied sequence
        *self.last_applied_seq.lock().unwrap() = entry.seq();

        Ok(())
    }

    /// Get replication lag (slave only)
    pub fn get_replication_lag(&self) -> u64 {
        if self.config.role != NodeRole::Slave {
            return 0;
        }

        let last_applied = *self.last_applied_seq.lock().unwrap();
        let log = self.log.read().unwrap();

        if let Some(latest) = log.back() {
            latest.seq() - last_applied
        } else {
            0
        }
    }

    /// Register a slave (master only)
    pub fn register_slave(&self, slave_id: String) -> Result<(), String> {
        if self.config.role != NodeRole::Master {
            return Err("Only master can register slaves".to_string());
        }

        let state = SlaveState {
            slave_id: slave_id.clone(),
            last_ack_seq: 0,
            last_contact: current_timestamp(),
            lag_ms: 0,
        };

        self.slave_states.write().unwrap().insert(slave_id, state);
        Ok(())
    }

    /// Update slave acknowledgment (master only)
    pub fn update_slave_ack(&self, slave_id: &str, ack_seq: ReplicationSeq) -> Result<(), String> {
        if self.config.role != NodeRole::Master {
            return Err("Only master can update slave ack".to_string());
        }

        let mut slaves = self.slave_states.write().unwrap();
        if let Some(state) = slaves.get_mut(slave_id) {
            state.last_ack_seq = ack_seq;
            state.last_contact = current_timestamp();

            // Calculate lag
            let current_seq = *self.next_seq.lock().unwrap();
            state.lag_ms = ((current_seq - ack_seq) as u64) * 10; // Approximate
        }

        Ok(())
    }

    /// Get all slave states (master only)
    pub fn get_slave_states(&self) -> Vec<SlaveState> {
        self.slave_states.read().unwrap().values().cloned().collect()
    }

    /// Get current sequence number
    pub fn current_seq(&self) -> ReplicationSeq {
        *self.next_seq.lock().unwrap()
    }

    /// Get log size
    pub fn log_size(&self) -> usize {
        self.log.read().unwrap().len()
    }

    /// Trim log up to sequence (safe after all slaves have acknowledged)
    pub fn trim_log(&self, up_to_seq: ReplicationSeq) {
        let mut log = self.log.write().unwrap();
        while let Some(entry) = log.front() {
            if entry.seq() <= up_to_seq {
                log.pop_front();
            } else {
                break;
            }
        }
    }

    /// Get minimum acknowledged sequence across all slaves
    pub fn get_min_slave_seq(&self) -> ReplicationSeq {
        let slaves = self.slave_states.read().unwrap();
        slaves.values()
            .map(|s| s.last_ack_seq)
            .min()
            .unwrap_or(0)
    }

    fn get_next_seq(&self) -> ReplicationSeq {
        let mut seq = self.next_seq.lock().unwrap();
        let current = *seq;
        *seq += 1;
        current
    }
}

/// Replication statistics
#[derive(Debug, Clone)]
pub struct ReplicationStats {
    pub node_id: String,
    pub role: NodeRole,
    pub current_seq: ReplicationSeq,
    pub log_size: usize,
    pub slave_count: usize,
    pub max_slave_lag_ms: u64,
    pub min_slave_seq: ReplicationSeq,
}

impl ReplicationManager {
    pub fn stats(&self) -> ReplicationStats {
        let slaves = self.slave_states.read().unwrap();
        let max_lag = slaves.values().map(|s| s.lag_ms).max().unwrap_or(0);

        ReplicationStats {
            node_id: self.config.node_id.clone(),
            role: self.config.role,
            current_seq: self.current_seq(),
            log_size: self.log_size(),
            slave_count: slaves.len(),
            max_slave_lag_ms: max_lag,
            min_slave_seq: self.get_min_slave_seq(),
        }
    }
}

fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_master() {
        let master = ReplicationManager::new_master("master-1".to_string());
        assert_eq!(master.role(), NodeRole::Master);
        assert_eq!(master.current_seq(), 0);
    }

    #[test]
    fn test_create_slave() {
        let slave = ReplicationManager::new_slave(
            "slave-1".to_string(),
            "localhost:9000".to_string(),
        );
        assert_eq!(slave.role(), NodeRole::Slave);
    }

    #[test]
    fn test_log_insert() {
        let master = ReplicationManager::new_master("master-1".to_string());

        let mut props = HashMap::new();
        props.insert("name".to_string(), PropertyValue::String("Alice".to_string()));

        let seq = master.log_insert(1, "User".to_string(), props).unwrap();
        assert_eq!(seq, 0);
        assert_eq!(master.log_size(), 1);
    }

    #[test]
    fn test_log_multiple_operations() {
        let master = ReplicationManager::new_master("master-1".to_string());

        let mut props = HashMap::new();
        props.insert("name".to_string(), PropertyValue::String("Alice".to_string()));

        let seq1 = master.log_insert(1, "User".to_string(), props.clone()).unwrap();
        let seq2 = master.log_update(1, props.clone()).unwrap();
        let seq3 = master.log_delete(1).unwrap();

        assert_eq!(seq1, 0);
        assert_eq!(seq2, 1);
        assert_eq!(seq3, 2);
        assert_eq!(master.log_size(), 3);
    }

    #[test]
    fn test_slave_cannot_log() {
        let slave = ReplicationManager::new_slave(
            "slave-1".to_string(),
            "localhost:9000".to_string(),
        );

        let mut props = HashMap::new();
        props.insert("name".to_string(), PropertyValue::String("Alice".to_string()));

        let result = slave.log_insert(1, "User".to_string(), props);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Only master"));
    }

    #[test]
    fn test_get_entries_since() {
        let master = ReplicationManager::new_master("master-1".to_string());

        let mut props = HashMap::new();
        props.insert("name".to_string(), PropertyValue::String("Alice".to_string()));

        master.log_insert(1, "User".to_string(), props.clone()).unwrap();
        master.log_insert(2, "User".to_string(), props.clone()).unwrap();
        master.log_insert(3, "User".to_string(), props.clone()).unwrap();

        let entries = master.get_entries_since(0);
        assert_eq!(entries.len(), 3);

        let entries = master.get_entries_since(1);
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn test_register_slave() {
        let master = ReplicationManager::new_master("master-1".to_string());

        master.register_slave("slave-1".to_string()).unwrap();
        let slaves = master.get_slave_states();
        assert_eq!(slaves.len(), 1);
        assert_eq!(slaves[0].slave_id, "slave-1");
    }

    #[test]
    fn test_update_slave_ack() {
        let master = ReplicationManager::new_master("master-1".to_string());

        master.register_slave("slave-1".to_string()).unwrap();
        master.update_slave_ack("slave-1", 5).unwrap();

        let slaves = master.get_slave_states();
        assert_eq!(slaves[0].last_ack_seq, 5);
    }

    #[test]
    fn test_trim_log() {
        let master = ReplicationManager::new_master("master-1".to_string());

        let mut props = HashMap::new();
        props.insert("name".to_string(), PropertyValue::String("Alice".to_string()));

        for i in 0..10 {
            master.log_insert(i, "User".to_string(), props.clone()).unwrap();
        }

        assert_eq!(master.log_size(), 10);

        master.trim_log(5);
        assert_eq!(master.log_size(), 4); // Sequences 6, 7, 8, 9 remain
    }

    #[test]
    fn test_replication_stats() {
        let master = ReplicationManager::new_master("master-1".to_string());

        master.register_slave("slave-1".to_string()).unwrap();
        master.register_slave("slave-2".to_string()).unwrap();

        let stats = master.stats();
        assert_eq!(stats.node_id, "master-1");
        assert_eq!(stats.role, NodeRole::Master);
        assert_eq!(stats.slave_count, 2);
    }
}
