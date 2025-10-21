//! Network Partition Handling and Split-Brain Resolution
//!
//! Implements mechanisms to detect and resolve network partitions in the
//! distributed Deed database cluster.
//!
//! Features:
//! - Partition detection using heartbeats and quorum
//! - Split-brain resolution using majority quorum
//! - Automatic partition healing
//! - Read/write quorum enforcement

use crate::distributed_topology::NodeId;
use crate::distributed_consensus::RaftNode;
use serde::{Serialize, Deserialize};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

/// Partition detection state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PartitionState {
    /// Normal operation - cluster healthy
    Healthy,
    /// Partition detected
    Partitioned,
    /// Partition healing in progress
    Healing,
}

/// Consistency level for operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConsistencyLevel {
    /// Must reach majority of nodes
    Quorum,
    /// Must reach all nodes
    All,
    /// Can succeed with any node
    One,
    /// Local node only (fastest, least consistent)
    Local,
}

impl ConsistencyLevel {
    /// Calculate required number of nodes for this consistency level
    pub fn required_nodes(&self, total_nodes: usize, replication_factor: usize) -> usize {
        match self {
            ConsistencyLevel::Quorum => (replication_factor / 2) + 1,
            ConsistencyLevel::All => replication_factor,
            ConsistencyLevel::One => 1,
            ConsistencyLevel::Local => 1,
        }
    }
}

/// Node health status
#[derive(Debug, Clone)]
struct NodeHealth {
    node_id: NodeId,
    last_seen: Instant,
    is_reachable: bool,
    consecutive_failures: usize,
}

/// Partition detector and handler
pub struct PartitionManager {
    /// This node's ID
    local_id: NodeId,

    /// Current partition state
    state: Arc<RwLock<PartitionState>>,

    /// Health status of all nodes
    node_health: Arc<RwLock<HashMap<NodeId, NodeHealth>>>,

    /// Nodes in our partition
    our_partition: Arc<RwLock<HashSet<NodeId>>>,

    /// Total number of nodes in cluster
    total_nodes: usize,

    /// Replication factor
    replication_factor: usize,

    /// Health check interval
    health_check_interval: Duration,

    /// Failure threshold (consecutive failures before marking unreachable)
    failure_threshold: usize,
}

impl PartitionManager {
    /// Create new partition manager
    pub fn new(
        local_id: NodeId,
        total_nodes: usize,
        replication_factor: usize,
        health_check_interval_secs: u64,
    ) -> Self {
        Self {
            local_id,
            state: Arc::new(RwLock::new(PartitionState::Healthy)),
            node_health: Arc::new(RwLock::new(HashMap::new())),
            our_partition: Arc::new(RwLock::new(HashSet::new())),
            total_nodes,
            replication_factor,
            health_check_interval: Duration::from_secs(health_check_interval_secs),
            failure_threshold: 3,
        }
    }

    /// Register a node
    pub fn add_node(&self, node_id: NodeId) {
        let health = NodeHealth {
            node_id,
            last_seen: Instant::now(),
            is_reachable: true,
            consecutive_failures: 0,
        };

        self.node_health.write().unwrap().insert(node_id, health);
        self.our_partition.write().unwrap().insert(node_id);
    }

    /// Record successful contact with a node
    pub fn record_heartbeat(&self, node_id: NodeId) {
        let mut health_map = self.node_health.write().unwrap();

        if let Some(health) = health_map.get_mut(&node_id) {
            health.last_seen = Instant::now();
            health.is_reachable = true;
            health.consecutive_failures = 0;
        }
    }

    /// Record failed contact with a node
    pub fn record_failure(&self, node_id: NodeId) {
        let mut health_map = self.node_health.write().unwrap();

        if let Some(health) = health_map.get_mut(&node_id) {
            health.consecutive_failures += 1;

            if health.consecutive_failures >= self.failure_threshold {
                health.is_reachable = false;
            }
        }
    }

    /// Check for network partition
    pub fn check_partition(&self) -> PartitionState {
        let health_map = self.node_health.read().unwrap();

        let reachable_count = health_map
            .values()
            .filter(|h| h.is_reachable)
            .count() + 1; // +1 for self

        let unreachable_count = health_map
            .values()
            .filter(|h| !h.is_reachable)
            .count();

        // Update our partition membership
        let mut our_partition = self.our_partition.write().unwrap();
        our_partition.clear();
        our_partition.insert(self.local_id);

        for health in health_map.values() {
            if health.is_reachable {
                our_partition.insert(health.node_id);
            }
        }

        drop(our_partition);

        // Determine if we're in a partition
        let new_state = if unreachable_count > 0 {
            // We have unreachable nodes
            let quorum = (self.total_nodes / 2) + 1;

            if reachable_count >= quorum {
                // We have quorum - we're in the majority partition
                println!("Partition detected but we have quorum ({}/{})",
                    reachable_count, self.total_nodes);
                PartitionState::Partitioned
            } else {
                // We're in the minority partition - should stop accepting writes
                println!("WARNING: In minority partition ({}/{}) - read-only mode",
                    reachable_count, self.total_nodes);
                PartitionState::Partitioned
            }
        } else {
            // All nodes reachable
            PartitionState::Healthy
        };

        *self.state.write().unwrap() = new_state;
        new_state
    }

    /// Check if we have quorum
    pub fn has_quorum(&self) -> bool {
        let our_partition = self.our_partition.read().unwrap();
        let quorum = (self.total_nodes / 2) + 1;
        our_partition.len() >= quorum
    }

    /// Check if we can perform writes
    pub fn can_write(&self) -> bool {
        self.has_quorum()
    }

    /// Check if we can perform reads
    pub fn can_read(&self, consistency: ConsistencyLevel) -> bool {
        let our_partition = self.our_partition.read().unwrap();
        let required = consistency.required_nodes(self.total_nodes, self.replication_factor);
        our_partition.len() >= required
    }

    /// Get current partition state
    pub fn get_state(&self) -> PartitionState {
        *self.state.read().unwrap()
    }

    /// Get nodes in our partition
    pub fn get_our_partition(&self) -> Vec<NodeId> {
        self.our_partition.read().unwrap().iter().copied().collect()
    }

    /// Get unreachable nodes
    pub fn get_unreachable_nodes(&self) -> Vec<NodeId> {
        self.node_health
            .read()
            .unwrap()
            .values()
            .filter(|h| !h.is_reachable)
            .map(|h| h.node_id)
            .collect()
    }

    /// Start automatic partition detection
    pub fn start_monitoring(&self) {
        let manager = self.clone_for_async();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(manager.health_check_interval);

            loop {
                interval.tick().await;
                manager.check_partition();
            }
        });
    }

    /// Attempt to heal partition
    pub fn attempt_healing(&self) {
        *self.state.write().unwrap() = PartitionState::Healing;

        println!("Attempting to heal partition...");

        // Reset failure counters to give nodes a chance
        let mut health_map = self.node_health.write().unwrap();
        for health in health_map.values_mut() {
            if !health.is_reachable {
                health.consecutive_failures = 0;
            }
        }
    }

    /// Get statistics
    pub fn get_statistics(&self) -> PartitionStats {
        let health_map = self.node_health.read().unwrap();
        let our_partition = self.our_partition.read().unwrap();

        PartitionStats {
            state: self.get_state(),
            total_nodes: self.total_nodes,
            reachable_nodes: our_partition.len(),
            unreachable_nodes: health_map.values().filter(|h| !h.is_reachable).count(),
            has_quorum: self.has_quorum(),
            can_write: self.can_write(),
        }
    }

    /// Clone for async tasks
    fn clone_for_async(&self) -> Arc<Self> {
        Arc::new(Self {
            local_id: self.local_id,
            state: Arc::clone(&self.state),
            node_health: Arc::clone(&self.node_health),
            our_partition: Arc::clone(&self.our_partition),
            total_nodes: self.total_nodes,
            replication_factor: self.replication_factor,
            health_check_interval: self.health_check_interval,
            failure_threshold: self.failure_threshold,
        })
    }
}

/// Quorum-based read/write manager
pub struct QuorumManager {
    /// This node's ID
    local_id: NodeId,

    /// Replication factor
    replication_factor: usize,

    /// Default read consistency
    read_consistency: ConsistencyLevel,

    /// Default write consistency
    write_consistency: ConsistencyLevel,

    /// Partition manager
    partition_manager: Arc<PartitionManager>,
}

impl QuorumManager {
    /// Create new quorum manager
    pub fn new(
        local_id: NodeId,
        replication_factor: usize,
        partition_manager: Arc<PartitionManager>,
    ) -> Self {
        Self {
            local_id,
            replication_factor,
            read_consistency: ConsistencyLevel::Quorum,
            write_consistency: ConsistencyLevel::Quorum,
            partition_manager,
        }
    }

    /// Set read consistency level
    pub fn set_read_consistency(&mut self, level: ConsistencyLevel) {
        self.read_consistency = level;
    }

    /// Set write consistency level
    pub fn set_write_consistency(&mut self, level: ConsistencyLevel) {
        self.write_consistency = level;
    }

    /// Check if read can proceed
    pub fn can_read(&self) -> Result<(), String> {
        if !self.partition_manager.can_read(self.read_consistency) {
            return Err(format!(
                "Cannot satisfy read consistency {:?} - not enough reachable nodes",
                self.read_consistency
            ));
        }

        Ok(())
    }

    /// Check if write can proceed
    pub fn can_write(&self) -> Result<(), String> {
        if !self.partition_manager.can_write() {
            return Err("Cannot write - no quorum".to_string());
        }

        // Additional check for write consistency
        if !self.partition_manager.can_read(self.write_consistency) {
            return Err(format!(
                "Cannot satisfy write consistency {:?} - not enough reachable nodes",
                self.write_consistency
            ));
        }

        Ok(())
    }

    /// Coordinate a quorum read
    pub async fn quorum_read(&self, key: &str, replicas: Vec<NodeId>) -> Result<Vec<u8>, String> {
        self.can_read()?;

        let required = self.read_consistency.required_nodes(
            replicas.len(),
            self.replication_factor,
        );

        // Read from required number of replicas
        let mut responses = Vec::new();

        for (i, &node_id) in replicas.iter().enumerate() {
            if i >= required {
                break;
            }

            // In production, would send read request via P2P
            // For now, simplified
            let value = vec![1, 2, 3]; // Dummy value
            responses.push(value);
        }

        // Return most recent value (would use version numbers in production)
        responses.into_iter().next().ok_or_else(|| "No responses".to_string())
    }

    /// Coordinate a quorum write
    pub async fn quorum_write(&self, key: &str, value: Vec<u8>, replicas: Vec<NodeId>) -> Result<(), String> {
        self.can_write()?;

        let required = self.write_consistency.required_nodes(
            replicas.len(),
            self.replication_factor,
        );

        let mut successful_writes = 0;

        for (i, &node_id) in replicas.iter().enumerate() {
            if i >= required {
                break;
            }

            // In production, would send write request via P2P
            // For now, simplified
            successful_writes += 1;
        }

        if successful_writes >= required {
            Ok(())
        } else {
            Err(format!(
                "Write failed - only {} of {} required nodes succeeded",
                successful_writes, required
            ))
        }
    }

    /// Get statistics
    pub fn get_statistics(&self) -> QuorumStats {
        QuorumStats {
            read_consistency: self.read_consistency,
            write_consistency: self.write_consistency,
            replication_factor: self.replication_factor,
        }
    }
}

/// Partition statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartitionStats {
    pub state: PartitionState,
    pub total_nodes: usize,
    pub reachable_nodes: usize,
    pub unreachable_nodes: usize,
    pub has_quorum: bool,
    pub can_write: bool,
}

/// Quorum statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuorumStats {
    pub read_consistency: ConsistencyLevel,
    pub write_consistency: ConsistencyLevel,
    pub replication_factor: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_partition_manager_creation() {
        let manager = PartitionManager::new(1, 5, 3, 10);
        assert_eq!(manager.get_state(), PartitionState::Healthy);
    }

    #[test]
    fn test_add_nodes() {
        let manager = PartitionManager::new(1, 5, 3, 10);
        manager.add_node(2);
        manager.add_node(3);
        manager.add_node(4);
        manager.add_node(5);

        let partition = manager.get_our_partition();
        assert_eq!(partition.len(), 5); // Including self
    }

    #[test]
    fn test_quorum_with_majority() {
        let manager = PartitionManager::new(1, 5, 3, 10);
        manager.add_node(2);
        manager.add_node(3);
        manager.add_node(4);
        manager.add_node(5);

        // Mark 2 nodes as failed
        manager.record_failure(4);
        manager.record_failure(4);
        manager.record_failure(4);
        manager.record_failure(5);
        manager.record_failure(5);
        manager.record_failure(5);

        manager.check_partition();

        // Should still have quorum (3 out of 5)
        assert!(manager.has_quorum());
        assert!(manager.can_write());
    }

    #[test]
    fn test_partition_without_quorum() {
        let manager = PartitionManager::new(1, 5, 3, 10);
        manager.add_node(2);
        manager.add_node(3);
        manager.add_node(4);
        manager.add_node(5);

        // Mark 3 nodes as failed (minority partition)
        for _ in 0..3 {
            manager.record_failure(3);
            manager.record_failure(4);
            manager.record_failure(5);
        }

        manager.check_partition();

        // Should not have quorum (2 out of 5)
        assert!(!manager.has_quorum());
        assert!(!manager.can_write());
    }

    #[test]
    fn test_consistency_level_required_nodes() {
        assert_eq!(ConsistencyLevel::Quorum.required_nodes(5, 3), 2);
        assert_eq!(ConsistencyLevel::All.required_nodes(5, 3), 3);
        assert_eq!(ConsistencyLevel::One.required_nodes(5, 3), 1);
        assert_eq!(ConsistencyLevel::Local.required_nodes(5, 3), 1);
    }

    #[test]
    fn test_quorum_manager() {
        let partition_manager = Arc::new(PartitionManager::new(1, 5, 3, 10));
        partition_manager.add_node(2);
        partition_manager.add_node(3);
        partition_manager.add_node(4);
        partition_manager.add_node(5);

        let quorum_manager = QuorumManager::new(1, 3, partition_manager);

        assert!(quorum_manager.can_read().is_ok());
        assert!(quorum_manager.can_write().is_ok());
    }

    #[test]
    fn test_stats() {
        let manager = PartitionManager::new(1, 5, 3, 10);
        manager.add_node(2);
        manager.add_node(3);

        let stats = manager.get_statistics();
        assert_eq!(stats.total_nodes, 5);
        assert_eq!(stats.reachable_nodes, 3);
        assert!(stats.has_quorum);
    }
}
