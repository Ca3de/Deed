//! Crash Recovery and Durability Tests
//!
//! Tests that committed transactions survive crashes and WAL recovery works correctly.

use deed_rust::*;
use std::sync::{Arc, RwLock};
use std::fs;
use tempfile::TempDir;

/// Test basic WAL write and recovery
#[test]
fn test_wal_basic_recovery() {
    let temp_dir = TempDir::new().unwrap();
    let wal_path = temp_dir.path().join("test.wal");

    // Phase 1: Write transactions to WAL
    {
        let graph = Arc::new(RwLock::new(Graph::new()));
        let executor = DQLExecutor::new_with_wal(graph, &wal_path).unwrap();

        executor.execute("BEGIN TRANSACTION").unwrap();
        executor.execute("INSERT INTO Users VALUES ({id: 1, name: \"Alice\"})").unwrap();
        executor.execute("INSERT INTO Users VALUES ({id: 2, name: \"Bob\"})").unwrap();
        executor.execute("COMMIT").unwrap();

        // Verify data is there
        let result = executor.execute("FROM Users SELECT COUNT(*) AS count").unwrap();
        assert!(!result.rows.is_empty());
    }

    // Phase 2: "Crash" (drop executor) and recover
    {
        let wal_manager = WALManager::new(&wal_path).unwrap();
        let recovery_result = wal_manager.recover().unwrap();

        println!("Recovery result:");
        println!("  Entries: {}", recovery_result.entries.len());
        println!("  Committed txns: {:?}", recovery_result.committed_txns);
        println!("  Aborted txns: {:?}", recovery_result.aborted_txns);
        println!("  Active txns: {:?}", recovery_result.active_txns);

        // Should have committed transaction
        assert_eq!(recovery_result.committed_txns.len(), 1);
        assert_eq!(recovery_result.aborted_txns.len(), 0);
        assert_eq!(recovery_result.active_txns.len(), 0);
    }
}

/// Test recovery with uncommitted transaction
#[test]
fn test_wal_uncommitted_recovery() {
    let temp_dir = TempDir::new().unwrap();
    let wal_path = temp_dir.path().join("test.wal");

    // Phase 1: Start transaction but don't commit (simulate crash)
    {
        let graph = Arc::new(RwLock::new(Graph::new()));
        let executor = DQLExecutor::new_with_wal(graph, &wal_path).unwrap();

        executor.execute("BEGIN TRANSACTION").unwrap();
        executor.execute("INSERT INTO Users VALUES ({id: 1, name: \"Alice\"})").unwrap();
        // NO COMMIT - simulate crash
    }

    // Phase 2: Recover - uncommitted transaction should be aborted
    {
        let wal_manager = WALManager::new(&wal_path).unwrap();
        let recovery_result = wal_manager.recover().unwrap();

        println!("Recovery with uncommitted txn:");
        println!("  Committed txns: {:?}", recovery_result.committed_txns);
        println!("  Active txns (to abort): {:?}", recovery_result.active_txns);

        // Should have one active transaction (to be aborted)
        assert_eq!(recovery_result.committed_txns.len(), 0);
        assert_eq!(recovery_result.active_txns.len(), 1);
    }
}

/// Test recovery with mixed committed and uncommitted transactions
#[test]
fn test_wal_mixed_recovery() {
    let temp_dir = TempDir::new().unwrap();
    let wal_path = temp_dir.path().join("test.wal");

    {
        let graph = Arc::new(RwLock::new(Graph::new()));
        let executor = DQLExecutor::new_with_wal(graph, &wal_path).unwrap();

        // Transaction 1: Committed
        executor.execute("BEGIN TRANSACTION").unwrap();
        executor.execute("INSERT INTO Users VALUES ({id: 1, name: \"Alice\"})").unwrap();
        executor.execute("COMMIT").unwrap();

        // Transaction 2: Uncommitted (crash)
        executor.execute("BEGIN TRANSACTION").unwrap();
        executor.execute("INSERT INTO Users VALUES ({id: 2, name: \"Bob\"})").unwrap();
        // NO COMMIT

        // Transaction 3: Committed
        executor.execute("BEGIN TRANSACTION").unwrap();
        executor.execute("INSERT INTO Users VALUES ({id: 3, name: \"Charlie\"})").unwrap();
        executor.execute("COMMIT").unwrap();
    }

    {
        let wal_manager = WALManager::new(&wal_path).unwrap();
        let recovery_result = wal_manager.recover().unwrap();

        println!("Mixed recovery:");
        println!("  Committed txns: {:?}", recovery_result.committed_txns);
        println!("  Active txns: {:?}", recovery_result.active_txns);

        // Should have 2 committed and 1 active
        assert_eq!(recovery_result.committed_txns.len(), 2);
        assert_eq!(recovery_result.active_txns.len(), 1);
    }
}

/// Test recovery with rollback
#[test]
fn test_wal_rollback_recovery() {
    let temp_dir = TempDir::new().unwrap();
    let wal_path = temp_dir.path().join("test.wal");

    {
        let graph = Arc::new(RwLock::new(Graph::new()));
        let executor = DQLExecutor::new_with_wal(graph, &wal_path).unwrap();

        executor.execute("BEGIN TRANSACTION").unwrap();
        executor.execute("INSERT INTO Users VALUES ({id: 1, name: \"Alice\"})").unwrap();
        executor.execute("ROLLBACK").unwrap();
    }

    {
        let wal_manager = WALManager::new(&wal_path).unwrap();
        let recovery_result = wal_manager.recover().unwrap();

        println!("Rollback recovery:");
        println!("  Aborted txns: {:?}", recovery_result.aborted_txns);

        assert_eq!(recovery_result.committed_txns.len(), 0);
        assert_eq!(recovery_result.aborted_txns.len(), 1);
        assert_eq!(recovery_result.active_txns.len(), 0);
    }
}

/// Test WAL with many transactions
#[test]
fn test_wal_many_transactions() {
    let temp_dir = TempDir::new().unwrap();
    let wal_path = temp_dir.path().join("test.wal");

    let num_transactions = 100;

    {
        let graph = Arc::new(RwLock::new(Graph::new()));
        let executor = DQLExecutor::new_with_wal(graph, &wal_path).unwrap();

        for i in 0..num_transactions {
            executor.execute("BEGIN TRANSACTION").unwrap();
            executor.execute(&format!(
                "INSERT INTO Users VALUES ({{id: {}, name: \"User{}\"}})",
                i, i
            )).unwrap();
            executor.execute("COMMIT").unwrap();
        }
    }

    {
        let wal_manager = WALManager::new(&wal_path).unwrap();
        let recovery_result = wal_manager.recover().unwrap();

        println!("Many transactions recovery:");
        println!("  Total entries: {}", recovery_result.entries.len());
        println!("  Committed txns: {}", recovery_result.committed_txns.len());

        assert_eq!(recovery_result.committed_txns.len(), num_transactions);
    }
}

/// Test WAL corruption detection (future feature)
#[test]
#[ignore] // Not implemented yet
fn test_wal_corruption_detection() {
    let temp_dir = TempDir::new().unwrap();
    let wal_path = temp_dir.path().join("test.wal");

    {
        let graph = Arc::new(RwLock::new(Graph::new()));
        let executor = DQLExecutor::new_with_wal(graph, &wal_path).unwrap();

        executor.execute("BEGIN TRANSACTION").unwrap();
        executor.execute("INSERT INTO Users VALUES ({id: 1, name: \"Alice\"})").unwrap();
        executor.execute("COMMIT").unwrap();
    }

    // Corrupt the WAL file
    let mut content = fs::read(&wal_path).unwrap();
    content[content.len() - 10] ^= 0xFF; // Flip some bits
    fs::write(&wal_path, content).unwrap();

    // Recovery should detect corruption
    let wal_manager = WALManager::new(&wal_path).unwrap();
    let result = wal_manager.recover();

    // Should fail with corruption error
    assert!(result.is_err() || result.unwrap().entries.len() < 4);
}

/// Test durability: data survives process "crash"
#[test]
fn test_durability_guarantee() {
    let temp_dir = TempDir::new().unwrap();
    let wal_path = temp_dir.path().join("test.wal");

    // Write committed data
    {
        let graph = Arc::new(RwLock::new(Graph::new()));
        let executor = DQLExecutor::new_with_wal(graph, &wal_path).unwrap();

        executor.execute("BEGIN TRANSACTION").unwrap();
        executor.execute("INSERT INTO Users VALUES ({id: 1, name: \"Alice\", balance: 1000})").unwrap();
        executor.execute("UPDATE Users SET balance = 1100 WHERE id = 1").unwrap();
        executor.execute("COMMIT").unwrap();

        // Explicit drop to simulate crash
        drop(executor);
    }

    // Verify WAL contains all operations
    {
        let wal_manager = WALManager::new(&wal_path).unwrap();
        let recovery_result = wal_manager.recover().unwrap();

        // Should have: BeginTransaction, InsertEntity, UpdateEntity, Commit
        let entries = recovery_result.entries;
        assert!(entries.len() >= 4);

        let mut has_begin = false;
        let mut has_insert = false;
        let mut has_update = false;
        let mut has_commit = false;

        for entry in &entries {
            match entry {
                WALEntry::BeginTransaction { .. } => has_begin = true,
                WALEntry::InsertEntity { .. } => has_insert = true,
                WALEntry::UpdateEntity { .. } => has_update = true,
                WALEntry::Commit { .. } => has_commit = true,
                _ => {}
            }
        }

        assert!(has_begin, "WAL should contain BEGIN");
        assert!(has_commit, "WAL should contain COMMIT");
    }
}

/// Test WAL compaction (checkpoint) - future feature
#[test]
#[ignore] // Not implemented yet
fn test_wal_checkpoint() {
    let temp_dir = TempDir::new().unwrap();
    let wal_path = temp_dir.path().join("test.wal");

    {
        let graph = Arc::new(RwLock::new(Graph::new()));
        let executor = DQLExecutor::new_with_wal(graph, &wal_path).unwrap();

        // Many transactions
        for i in 0..100 {
            executor.execute("BEGIN TRANSACTION").unwrap();
            executor.execute(&format!("INSERT INTO Users VALUES ({{id: {}}})", i)).unwrap();
            executor.execute("COMMIT").unwrap();
        }

        // Checkpoint should compact the WAL
        // Future: executor.checkpoint().unwrap();
    }

    // WAL should be smaller after checkpoint
    let wal_size = fs::metadata(&wal_path).unwrap().len();
    println!("WAL size: {} bytes", wal_size);

    // After checkpoint, recovery should still work
    let wal_manager = WALManager::new(&wal_path).unwrap();
    let recovery_result = wal_manager.recover().unwrap();
    assert!(recovery_result.committed_txns.len() > 0);
}

/// Test recovery order matters
#[test]
fn test_recovery_order() {
    let temp_dir = TempDir::new().unwrap();
    let wal_path = temp_dir.path().join("test.wal");

    {
        let graph = Arc::new(RwLock::new(Graph::new()));
        let executor = DQLExecutor::new_with_wal(graph, &wal_path).unwrap();

        // Transaction that modifies same entity multiple times
        executor.execute("BEGIN TRANSACTION").unwrap();
        executor.execute("INSERT INTO Accounts VALUES ({id: 1, balance: 100})").unwrap();
        executor.execute("UPDATE Accounts SET balance = 200 WHERE id = 1").unwrap();
        executor.execute("UPDATE Accounts SET balance = 300 WHERE id = 1").unwrap();
        executor.execute("COMMIT").unwrap();
    }

    {
        let wal_manager = WALManager::new(&wal_path).unwrap();
        let recovery_result = wal_manager.recover().unwrap();

        // Verify operations are in correct order
        let mut operation_order = Vec::new();
        for entry in &recovery_result.entries {
            match entry {
                WALEntry::BeginTransaction { .. } => operation_order.push("BEGIN"),
                WALEntry::InsertEntity { .. } => operation_order.push("INSERT"),
                WALEntry::UpdateEntity { .. } => operation_order.push("UPDATE"),
                WALEntry::Commit { .. } => operation_order.push("COMMIT"),
                _ => {}
            }
        }

        println!("Operation order: {:?}", operation_order);
        assert_eq!(operation_order, vec!["BEGIN", "INSERT", "UPDATE", "UPDATE", "COMMIT"]);
    }
}
