//! Write-Ahead Log (WAL)
//!
//! Ensures durability - committed transactions survive crashes.

use crate::graph::Entity;
use crate::transaction::{TransactionId, IsolationLevel};
use crate::types::{EntityId, EdgeId, Properties};
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// WAL file magic number
const WAL_MAGIC: u32 = 0xDEED_0001;

/// WAL format version
const WAL_VERSION: u32 = 1;

/// Write-Ahead Log entry types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WALEntry {
    /// Transaction begin
    BeginTransaction {
        txn_id: TransactionId,
        isolation_level: IsolationLevel,
        timestamp: u64,
    },

    /// Insert entity
    InsertEntity {
        txn_id: TransactionId,
        entity_id: u64,
        entity_type: String,
        properties: Properties,
    },

    /// Update entity
    UpdateEntity {
        txn_id: TransactionId,
        entity_id: u64,
        old_properties: Properties,
        new_properties: Properties,
    },

    /// Delete entity
    DeleteEntity {
        txn_id: TransactionId,
        entity_id: u64,
        entity_type: String,
        properties: Properties,
    },

    /// Create edge
    CreateEdge {
        txn_id: TransactionId,
        edge_id: u64,
        source_id: u64,
        target_id: u64,
        edge_type: String,
        properties: Properties,
    },

    /// Delete edge
    DeleteEdge {
        txn_id: TransactionId,
        edge_id: u64,
    },

    /// Transaction commit
    Commit {
        txn_id: TransactionId,
        timestamp: u64,
    },

    /// Transaction rollback
    Rollback {
        txn_id: TransactionId,
        timestamp: u64,
    },

    /// Checkpoint marker
    Checkpoint {
        txn_id: TransactionId,
        timestamp: u64,
    },
}

impl WALEntry {
    /// Get transaction ID from entry
    pub fn txn_id(&self) -> TransactionId {
        match self {
            WALEntry::BeginTransaction { txn_id, .. } => *txn_id,
            WALEntry::InsertEntity { txn_id, .. } => *txn_id,
            WALEntry::UpdateEntity { txn_id, .. } => *txn_id,
            WALEntry::DeleteEntity { txn_id, .. } => *txn_id,
            WALEntry::CreateEdge { txn_id, .. } => *txn_id,
            WALEntry::DeleteEdge { txn_id, .. } => *txn_id,
            WALEntry::Commit { txn_id, .. } => *txn_id,
            WALEntry::Rollback { txn_id, .. } => *txn_id,
            WALEntry::Checkpoint { txn_id, .. } => *txn_id,
        }
    }

    /// Check if this is a commit entry
    pub fn is_commit(&self) -> bool {
        matches!(self, WALEntry::Commit { .. })
    }

    /// Check if this is a rollback entry
    pub fn is_rollback(&self) -> bool {
        matches!(self, WALEntry::Rollback { .. })
    }
}

/// WAL file header
#[derive(Debug, Clone, Serialize, Deserialize)]
struct WALHeader {
    magic: u32,
    version: u32,
    created_at: u64,
}

impl WALHeader {
    fn new() -> Self {
        WALHeader {
            magic: WAL_MAGIC,
            version: WAL_VERSION,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }

    fn is_valid(&self) -> bool {
        self.magic == WAL_MAGIC && self.version == WAL_VERSION
    }
}

/// Write-Ahead Log writer
pub struct WALWriter {
    file: Arc<Mutex<BufWriter<File>>>,
    path: PathBuf,
    entry_count: usize,
}

impl WALWriter {
    /// Create a new WAL writer
    pub fn new<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let path = path.as_ref().to_path_buf();

        // Create or open WAL file
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)?;

        let mut writer = BufWriter::new(file);

        // Write header if file is empty
        if writer.get_ref().metadata()?.len() == 0 {
            let header = WALHeader::new();
            let header_bytes = bincode::serialize(&header)
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

            writer.write_all(&header_bytes)?;
            writer.flush()?;
        }

        Ok(WALWriter {
            file: Arc::new(Mutex::new(writer)),
            path,
            entry_count: 0,
        })
    }

    /// Write a WAL entry
    pub fn write_entry(&mut self, entry: &WALEntry) -> io::Result<()> {
        let mut file = self.file.lock().unwrap();

        // Serialize entry
        let entry_bytes = bincode::serialize(entry)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        // Write length prefix
        let len = entry_bytes.len() as u32;
        file.write_all(&len.to_le_bytes())?;

        // Write entry
        file.write_all(&entry_bytes)?;

        // Flush to ensure durability (fsync)
        file.flush()?;
        file.get_mut().sync_all()?;

        self.entry_count += 1;

        Ok(())
    }

    /// Get entry count
    pub fn entry_count(&self) -> usize {
        self.entry_count
    }

    /// Flush all pending writes
    pub fn flush(&self) -> io::Result<()> {
        let mut file = self.file.lock().unwrap();
        file.flush()?;
        file.get_mut().sync_all()?;
        Ok(())
    }
}

/// Write-Ahead Log reader
pub struct WALReader {
    file: BufReader<File>,
}

impl WALReader {
    /// Open a WAL file for reading
    pub fn new<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);

        // Read and validate header
        let mut header_bytes = vec![0u8; std::mem::size_of::<WALHeader>()];
        reader.read_exact(&mut header_bytes)?;

        let header: WALHeader = bincode::deserialize(&header_bytes)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        if !header.is_valid() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid WAL header",
            ));
        }

        Ok(WALReader { file: reader })
    }

    /// Read next entry from WAL
    pub fn read_entry(&mut self) -> io::Result<Option<WALEntry>> {
        // Read length prefix
        let mut len_bytes = [0u8; 4];
        match self.file.read_exact(&mut len_bytes) {
            Ok(_) => {},
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
            Err(e) => return Err(e),
        }

        let len = u32::from_le_bytes(len_bytes) as usize;

        // Read entry
        let mut entry_bytes = vec![0u8; len];
        self.file.read_exact(&mut entry_bytes)?;

        // Deserialize entry
        let entry: WALEntry = bincode::deserialize(&entry_bytes)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        Ok(Some(entry))
    }

    /// Read all entries from WAL
    pub fn read_all(&mut self) -> io::Result<Vec<WALEntry>> {
        let mut entries = Vec::new();

        while let Some(entry) = self.read_entry()? {
            entries.push(entry);
        }

        Ok(entries)
    }
}

/// WAL Manager - coordinates WAL operations
pub struct WALManager {
    writer: Arc<Mutex<WALWriter>>,
    path: PathBuf,
}

impl WALManager {
    /// Create a new WAL manager
    pub fn new<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let path = path.as_ref().to_path_buf();
        let writer = WALWriter::new(&path)?;

        Ok(WALManager {
            writer: Arc::new(Mutex::new(writer)),
            path,
        })
    }

    /// Log a begin transaction
    pub fn log_begin(
        &self,
        txn_id: TransactionId,
        isolation_level: IsolationLevel,
    ) -> io::Result<()> {
        let entry = WALEntry::BeginTransaction {
            txn_id,
            isolation_level,
            timestamp: Self::current_timestamp(),
        };

        self.writer.lock().unwrap().write_entry(&entry)
    }

    /// Log an insert operation
    pub fn log_insert(
        &self,
        txn_id: TransactionId,
        entity: &Entity,
    ) -> io::Result<()> {
        let entry = WALEntry::InsertEntity {
            txn_id,
            entity_id: entity.id.as_u64(),
            entity_type: entity.entity_type.clone(),
            properties: entity.properties.clone(),
        };

        self.writer.lock().unwrap().write_entry(&entry)
    }

    /// Log an update operation
    pub fn log_update(
        &self,
        txn_id: TransactionId,
        entity_id: EntityId,
        old_props: Properties,
        new_props: Properties,
    ) -> io::Result<()> {
        let entry = WALEntry::UpdateEntity {
            txn_id,
            entity_id: entity_id.as_u64(),
            old_properties: old_props,
            new_properties: new_props,
        };

        self.writer.lock().unwrap().write_entry(&entry)
    }

    /// Log a delete operation
    pub fn log_delete(
        &self,
        txn_id: TransactionId,
        entity: &Entity,
    ) -> io::Result<()> {
        let entry = WALEntry::DeleteEntity {
            txn_id,
            entity_id: entity.id.as_u64(),
            entity_type: entity.entity_type.clone(),
            properties: entity.properties.clone(),
        };

        self.writer.lock().unwrap().write_entry(&entry)
    }

    /// Log a commit
    pub fn log_commit(&self, txn_id: TransactionId) -> io::Result<()> {
        let entry = WALEntry::Commit {
            txn_id,
            timestamp: Self::current_timestamp(),
        };

        self.writer.lock().unwrap().write_entry(&entry)
    }

    /// Log a rollback
    pub fn log_rollback(&self, txn_id: TransactionId) -> io::Result<()> {
        let entry = WALEntry::Rollback {
            txn_id,
            timestamp: Self::current_timestamp(),
        };

        self.writer.lock().unwrap().write_entry(&entry)
    }

    /// Log a checkpoint
    pub fn log_checkpoint(&self, txn_id: TransactionId) -> io::Result<()> {
        let entry = WALEntry::Checkpoint {
            txn_id,
            timestamp: Self::current_timestamp(),
        };

        self.writer.lock().unwrap().write_entry(&entry)
    }

    /// Flush WAL to disk
    pub fn flush(&self) -> io::Result<()> {
        self.writer.lock().unwrap().flush()
    }

    /// Recover from WAL
    pub fn recover(&self) -> io::Result<RecoveryResult> {
        let mut reader = WALReader::new(&self.path)?;
        let entries = reader.read_all()?;

        let mut result = RecoveryResult::new();

        for entry in entries {
            match &entry {
                WALEntry::BeginTransaction { txn_id, .. } => {
                    result.active_txns.insert(*txn_id);
                }
                WALEntry::Commit { txn_id, .. } => {
                    result.active_txns.remove(txn_id);
                    result.committed_txns.push(*txn_id);
                }
                WALEntry::Rollback { txn_id, .. } => {
                    result.active_txns.remove(txn_id);
                    result.aborted_txns.push(*txn_id);
                }
                _ => {
                    // Data operation - belongs to a transaction
                }
            }

            result.entries.push(entry);
        }

        Ok(result)
    }

    fn current_timestamp() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
    }
}

/// Result of WAL recovery
pub struct RecoveryResult {
    /// All WAL entries
    pub entries: Vec<WALEntry>,
    /// Transactions that need to be committed
    pub committed_txns: Vec<TransactionId>,
    /// Transactions that need to be aborted
    pub aborted_txns: Vec<TransactionId>,
    /// Transactions still active (need to abort on recovery)
    pub active_txns: std::collections::HashSet<TransactionId>,
}

impl RecoveryResult {
    fn new() -> Self {
        RecoveryResult {
            entries: Vec::new(),
            committed_txns: Vec::new(),
            aborted_txns: Vec::new(),
            active_txns: std::collections::HashSet::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_wal_write_read() {
        let temp_dir = TempDir::new().unwrap();
        let wal_path = temp_dir.path().join("test.wal");

        // Write entries
        {
            let mut writer = WALWriter::new(&wal_path).unwrap();

            let entry = WALEntry::BeginTransaction {
                txn_id: 1,
                isolation_level: IsolationLevel::ReadCommitted,
                timestamp: 1000,
            };

            writer.write_entry(&entry).unwrap();

            let entry2 = WALEntry::Commit {
                txn_id: 1,
                timestamp: 2000,
            };

            writer.write_entry(&entry2).unwrap();
        }

        // Read entries
        {
            let mut reader = WALReader::new(&wal_path).unwrap();
            let entries = reader.read_all().unwrap();

            assert_eq!(entries.len(), 2);
            assert!(entries[0].txn_id() == 1);
            assert!(entries[1].is_commit());
        }
    }

    #[test]
    fn test_wal_recovery() {
        let temp_dir = TempDir::new().unwrap();
        let wal_path = temp_dir.path().join("test.wal");

        let manager = WALManager::new(&wal_path).unwrap();

        manager.log_begin(1, IsolationLevel::ReadCommitted).unwrap();
        manager.log_commit(1).unwrap();

        manager.log_begin(2, IsolationLevel::ReadCommitted).unwrap();
        // Transaction 2 never commits

        let result = manager.recover().unwrap();

        assert_eq!(result.committed_txns.len(), 1);
        assert_eq!(result.active_txns.len(), 1); // Transaction 2 is still active
    }
}
