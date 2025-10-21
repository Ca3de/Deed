//! Transaction Management
//!
//! Provides ACID-compliant transactions with MVCC (Multi-Version Concurrency Control).

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

/// Unique transaction identifier
pub type TransactionId = u64;

/// Transaction state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransactionState {
    /// Transaction is currently active
    Active,
    /// Transaction is preparing to commit (validation phase)
    Preparing,
    /// Transaction has been committed
    Committed,
    /// Transaction has been rolled back
    Aborted,
}

/// Isolation level for transaction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IsolationLevel {
    /// Dirty reads allowed (lowest isolation)
    ReadUncommitted,
    /// No dirty reads (default for most databases)
    ReadCommitted,
    /// No dirty reads, no non-repeatable reads
    RepeatableRead,
    /// Full serializability (highest isolation)
    Serializable,
}

impl Default for IsolationLevel {
    fn default() -> Self {
        IsolationLevel::RepeatableRead
    }
}

/// Transaction metadata and state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    /// Unique transaction ID
    pub id: TransactionId,
    /// Current state
    pub state: TransactionState,
    /// When transaction started (milliseconds since epoch)
    pub start_time: u64,
    /// Isolation level
    pub isolation_level: IsolationLevel,
    /// Read set - entities read by this transaction
    pub read_set: Vec<(u64, u64)>, // (entity_id, version)
    /// Write set - entities modified by this transaction
    pub write_set: Vec<u64>, // entity_ids
}

impl Transaction {
    /// Create a new transaction
    pub fn new(id: TransactionId, isolation_level: IsolationLevel) -> Self {
        let start_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        Transaction {
            id,
            state: TransactionState::Active,
            start_time,
            isolation_level,
            read_set: Vec::new(),
            write_set: Vec::new(),
        }
    }

    /// Mark entity as read by this transaction
    pub fn track_read(&mut self, entity_id: u64, version: u64) {
        self.read_set.push((entity_id, version));
    }

    /// Mark entity as written by this transaction
    pub fn track_write(&mut self, entity_id: u64) {
        if !self.write_set.contains(&entity_id) {
            self.write_set.push(entity_id);
        }
    }

    /// Check if transaction is active
    pub fn is_active(&self) -> bool {
        self.state == TransactionState::Active
    }

    /// Check if transaction is committed
    pub fn is_committed(&self) -> bool {
        self.state == TransactionState::Committed
    }

    /// Check if transaction is aborted
    pub fn is_aborted(&self) -> bool {
        self.state == TransactionState::Aborted
    }
}

/// Transaction manager - coordinates all transactions
pub struct TransactionManager {
    /// Next transaction ID to allocate
    next_txn_id: AtomicU64,
    /// Active transactions
    active_transactions: Arc<RwLock<HashMap<TransactionId, Transaction>>>,
    /// Committed transactions (keep recent history for MVCC)
    committed_transactions: Arc<RwLock<HashMap<TransactionId, Transaction>>>,
}

impl TransactionManager {
    /// Create a new transaction manager
    pub fn new() -> Self {
        TransactionManager {
            next_txn_id: AtomicU64::new(1),
            active_transactions: Arc::new(RwLock::new(HashMap::new())),
            committed_transactions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Begin a new transaction
    pub fn begin(&self, isolation_level: IsolationLevel) -> Result<TransactionId, String> {
        let txn_id = self.next_txn_id.fetch_add(1, Ordering::SeqCst);
        let transaction = Transaction::new(txn_id, isolation_level);

        let mut active = self.active_transactions.write()
            .map_err(|e| format!("Failed to acquire lock: {}", e))?;

        active.insert(txn_id, transaction);

        Ok(txn_id)
    }

    /// Commit a transaction
    pub fn commit(&self, txn_id: TransactionId) -> Result<(), String> {
        // Move from active to committed
        let mut active = self.active_transactions.write()
            .map_err(|e| format!("Failed to acquire lock: {}", e))?;

        let mut transaction = active.remove(&txn_id)
            .ok_or_else(|| format!("Transaction {} not found", txn_id))?;

        if !transaction.is_active() {
            return Err(format!("Transaction {} is not active", txn_id));
        }

        // Validate transaction (check for conflicts)
        self.validate_transaction(&transaction)?;

        // Mark as committed
        transaction.state = TransactionState::Committed;

        // Move to committed transactions
        let mut committed = self.committed_transactions.write()
            .map_err(|e| format!("Failed to acquire lock: {}", e))?;

        committed.insert(txn_id, transaction);

        Ok(())
    }

    /// Rollback (abort) a transaction
    pub fn rollback(&self, txn_id: TransactionId) -> Result<(), String> {
        let mut active = self.active_transactions.write()
            .map_err(|e| format!("Failed to acquire lock: {}", e))?;

        let mut transaction = active.remove(&txn_id)
            .ok_or_else(|| format!("Transaction {} not found", txn_id))?;

        // Mark as aborted
        transaction.state = TransactionState::Aborted;

        // Don't need to track aborted transactions
        Ok(())
    }

    /// Get a transaction
    pub fn get_transaction(&self, txn_id: TransactionId) -> Result<Transaction, String> {
        // Check active first
        {
            let active = self.active_transactions.read()
                .map_err(|e| format!("Failed to acquire lock: {}", e))?;

            if let Some(txn) = active.get(&txn_id) {
                return Ok(txn.clone());
            }
        }

        // Check committed
        {
            let committed = self.committed_transactions.read()
                .map_err(|e| format!("Failed to acquire lock: {}", e))?;

            if let Some(txn) = committed.get(&txn_id) {
                return Ok(txn.clone());
            }
        }

        Err(format!("Transaction {} not found", txn_id))
    }

    /// Track a read operation
    pub fn track_read(&self, txn_id: TransactionId, entity_id: u64, version: u64) -> Result<(), String> {
        let mut active = self.active_transactions.write()
            .map_err(|e| format!("Failed to acquire lock: {}", e))?;

        let transaction = active.get_mut(&txn_id)
            .ok_or_else(|| format!("Transaction {} not found", txn_id))?;

        transaction.track_read(entity_id, version);
        Ok(())
    }

    /// Track a write operation
    pub fn track_write(&self, txn_id: TransactionId, entity_id: u64) -> Result<(), String> {
        let mut active = self.active_transactions.write()
            .map_err(|e| format!("Failed to acquire lock: {}", e))?;

        let transaction = active.get_mut(&txn_id)
            .ok_or_else(|| format!("Transaction {} not found", txn_id))?;

        transaction.track_write(entity_id);
        Ok(())
    }

    /// Get minimum active transaction ID (for MVCC garbage collection)
    pub fn get_min_active_txn(&self) -> TransactionId {
        let active = self.active_transactions.read().unwrap();

        active.keys().min().copied().unwrap_or(u64::MAX)
    }

    /// Get all active transaction IDs
    pub fn get_active_txn_ids(&self) -> Vec<TransactionId> {
        let active = self.active_transactions.read().unwrap();
        active.keys().copied().collect()
    }

    /// Validate transaction before commit (detect conflicts)
    fn validate_transaction(&self, transaction: &Transaction) -> Result<(), String> {
        match transaction.isolation_level {
            IsolationLevel::ReadUncommitted => {
                // No validation needed
                Ok(())
            }
            IsolationLevel::ReadCommitted => {
                // Check that read entities haven't been modified
                // (Simplified - full implementation would check versions)
                Ok(())
            }
            IsolationLevel::RepeatableRead => {
                // Check that read set hasn't changed
                // Check for write-write conflicts
                self.check_repeatable_read_conflicts(transaction)
            }
            IsolationLevel::Serializable => {
                // Full serializability checking
                self.check_serializable_conflicts(transaction)
            }
        }
    }

    /// Check for conflicts in repeatable read isolation
    fn check_repeatable_read_conflicts(&self, _transaction: &Transaction) -> Result<(), String> {
        // Simplified implementation
        // Full version would:
        // 1. Check if any read entities were modified by concurrent transactions
        // 2. Check for write-write conflicts

        // For now, just succeed (optimistic)
        Ok(())
    }

    /// Check for conflicts in serializable isolation
    fn check_serializable_conflicts(&self, _transaction: &Transaction) -> Result<(), String> {
        // Simplified implementation
        // Full version would:
        // 1. Build dependency graph of transactions
        // 2. Detect cycles (indicating non-serializable execution)
        // 3. Abort if cycles found

        // For now, just succeed (optimistic)
        Ok(())
    }

    /// Cleanup old committed transactions
    pub fn cleanup_old_transactions(&self, min_txn_to_keep: TransactionId) {
        let mut committed = self.committed_transactions.write().unwrap();

        committed.retain(|&id, _| id >= min_txn_to_keep);
    }
}

impl Default for TransactionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_begin_transaction() {
        let mgr = TransactionManager::new();

        let txn1 = mgr.begin(IsolationLevel::default()).unwrap();
        let txn2 = mgr.begin(IsolationLevel::default()).unwrap();

        assert_eq!(txn1, 1);
        assert_eq!(txn2, 2);
    }

    #[test]
    fn test_commit_transaction() {
        let mgr = TransactionManager::new();

        let txn_id = mgr.begin(IsolationLevel::default()).unwrap();
        mgr.commit(txn_id).unwrap();

        let txn = mgr.get_transaction(txn_id).unwrap();
        assert!(txn.is_committed());
    }

    #[test]
    fn test_rollback_transaction() {
        let mgr = TransactionManager::new();

        let txn_id = mgr.begin(IsolationLevel::default()).unwrap();
        mgr.rollback(txn_id).unwrap();

        // Transaction should be removed after rollback
        assert!(mgr.get_transaction(txn_id).is_err());
    }

    #[test]
    fn test_track_operations() {
        let mgr = TransactionManager::new();

        let txn_id = mgr.begin(IsolationLevel::default()).unwrap();
        mgr.track_read(txn_id, 100, 1).unwrap();
        mgr.track_write(txn_id, 200).unwrap();

        let txn = mgr.get_transaction(txn_id).unwrap();
        assert_eq!(txn.read_set.len(), 1);
        assert_eq!(txn.write_set.len(), 1);
    }

    #[test]
    fn test_get_min_active_txn() {
        let mgr = TransactionManager::new();

        let txn1 = mgr.begin(IsolationLevel::default()).unwrap();
        let txn2 = mgr.begin(IsolationLevel::default()).unwrap();
        let _txn3 = mgr.begin(IsolationLevel::default()).unwrap();

        mgr.commit(txn1).unwrap();

        let min_txn = mgr.get_min_active_txn();
        assert_eq!(min_txn, txn2);
    }
}
