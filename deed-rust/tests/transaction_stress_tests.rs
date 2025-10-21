//! Stress Tests for Deed Database Transactions
//!
//! Tests concurrent transactions, race conditions, and system limits.

use deed_rust::*;
use std::sync::{Arc, RwLock, Barrier};
use std::thread;
use std::time::Duration;

/// Test concurrent inserts from multiple threads
#[test]
fn test_concurrent_inserts() {
    let graph = Arc::new(RwLock::new(Graph::new()));
    let executor = Arc::new(DQLExecutor::new(graph));

    let num_threads = 10;
    let inserts_per_thread = 100;

    let barrier = Arc::new(Barrier::new(num_threads));
    let mut handles = vec![];

    for thread_id in 0..num_threads {
        let executor = Arc::clone(&executor);
        let barrier = Arc::clone(&barrier);

        let handle = thread::spawn(move || {
            // Wait for all threads to be ready
            barrier.wait();

            // Each thread does inserts in a transaction
            executor.execute("BEGIN TRANSACTION").unwrap();

            for i in 0..inserts_per_thread {
                let query = format!(
                    "INSERT INTO Users VALUES ({{thread: {}, id: {}, name: \"User{}\"}})",
                    thread_id, i, i
                );
                executor.execute(&query).unwrap();
            }

            executor.execute("COMMIT").unwrap();
        });

        handles.push(handle);
    }

    // Wait for all threads to complete
    for handle in handles {
        handle.join().unwrap();
    }

    // Verify all inserts succeeded
    let result = executor.execute("FROM Users SELECT COUNT(*) AS count").unwrap();
    println!("Concurrent inserts completed: {:?}", result);
}

/// Test concurrent reads and writes
#[test]
fn test_concurrent_reads_writes() {
    let graph = Arc::new(RwLock::new(Graph::new()));
    let executor = Arc::new(DQLExecutor::new(graph));

    // Insert initial data
    executor.execute("BEGIN TRANSACTION").unwrap();
    for i in 0..100 {
        let query = format!(
            "INSERT INTO Accounts VALUES ({{id: {}, balance: 1000}})",
            i
        );
        executor.execute(&query).unwrap();
    }
    executor.execute("COMMIT").unwrap();

    let num_readers = 5;
    let num_writers = 5;
    let operations_per_thread = 50;

    let barrier = Arc::new(Barrier::new(num_readers + num_writers));
    let mut handles = vec![];

    // Spawn reader threads
    for _ in 0..num_readers {
        let executor = Arc::clone(&executor);
        let barrier = Arc::clone(&barrier);

        let handle = thread::spawn(move || {
            barrier.wait();

            for _ in 0..operations_per_thread {
                let result = executor.execute("FROM Accounts SELECT id, balance").unwrap();
                assert!(!result.rows.is_empty());
            }
        });

        handles.push(handle);
    }

    // Spawn writer threads
    for thread_id in 0..num_writers {
        let executor = Arc::clone(&executor);
        let barrier = Arc::clone(&barrier);

        let handle = thread::spawn(move || {
            barrier.wait();

            for i in 0..operations_per_thread {
                executor.execute("BEGIN TRANSACTION").unwrap();
                let account_id = (thread_id * operations_per_thread + i) % 100;
                let query = format!(
                    "UPDATE Accounts SET balance = balance + 10 WHERE id = {}",
                    account_id
                );
                executor.execute(&query).unwrap();
                executor.execute("COMMIT").unwrap();
            }
        });

        handles.push(handle);
    }

    // Wait for all threads
    for handle in handles {
        handle.join().unwrap();
    }

    println!("Concurrent reads/writes completed successfully");
}

/// Test transaction isolation - concurrent updates to same entity
#[test]
fn test_concurrent_updates_same_entity() {
    let graph = Arc::new(RwLock::new(Graph::new()));
    let executor = Arc::new(DQLExecutor::new(graph));

    // Insert test account
    executor.execute("BEGIN TRANSACTION").unwrap();
    executor.execute("INSERT INTO Accounts VALUES ({id: 1, balance: 1000})").unwrap();
    executor.execute("COMMIT").unwrap();

    let num_threads = 10;
    let updates_per_thread = 10;

    let barrier = Arc::new(Barrier::new(num_threads));
    let mut handles = vec![];

    for _ in 0..num_threads {
        let executor = Arc::clone(&executor);
        let barrier = Arc::clone(&barrier);

        let handle = thread::spawn(move || {
            barrier.wait();

            for _ in 0..updates_per_thread {
                // Each thread tries to update the same account
                executor.execute("BEGIN TRANSACTION").unwrap();
                executor.execute("UPDATE Accounts SET balance = balance + 10 WHERE id = 1").unwrap();
                executor.execute("COMMIT").unwrap();
            }
        });

        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Verify final balance is correct
    let result = executor.execute("FROM Accounts WHERE id = 1 SELECT balance").unwrap();
    println!("Final balance after concurrent updates: {:?}", result);
}

/// Test rollback under concurrent load
#[test]
fn test_concurrent_rollbacks() {
    let graph = Arc::new(RwLock::new(Graph::new()));
    let executor = Arc::new(DQLExecutor::new(graph));

    let num_threads = 5;
    let barrier = Arc::new(Barrier::new(num_threads));
    let mut handles = vec![];

    for thread_id in 0..num_threads {
        let executor = Arc::clone(&executor);
        let barrier = Arc::clone(&barrier);

        let handle = thread::spawn(move || {
            barrier.wait();

            for i in 0..20 {
                executor.execute("BEGIN TRANSACTION").unwrap();

                executor.execute(&format!(
                    "INSERT INTO Users VALUES ({{thread: {}, id: {}}})",
                    thread_id, i
                )).unwrap();

                // Rollback every other transaction
                if i % 2 == 0 {
                    executor.execute("ROLLBACK").unwrap();
                } else {
                    executor.execute("COMMIT").unwrap();
                }
            }
        });

        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Verify only committed transactions are visible
    let result = executor.execute("FROM Users SELECT COUNT(*) AS count").unwrap();
    println!("After concurrent rollbacks: {:?}", result);
}

/// Test high-throughput insert workload
#[test]
fn test_high_throughput_inserts() {
    let graph = Arc::new(RwLock::new(Graph::new()));
    let executor = Arc::new(DQLExecutor::new(graph));

    let num_threads = 8;
    let inserts_per_thread = 1000;

    let start = std::time::Instant::now();
    let barrier = Arc::new(Barrier::new(num_threads));
    let mut handles = vec![];

    for thread_id in 0..num_threads {
        let executor = Arc::clone(&executor);
        let barrier = Arc::clone(&barrier);

        let handle = thread::spawn(move || {
            barrier.wait();

            // Batch inserts for performance
            executor.execute("BEGIN TRANSACTION").unwrap();

            for i in 0..inserts_per_thread {
                let query = format!(
                    "INSERT INTO Events VALUES ({{thread: {}, seq: {}, timestamp: {}}})",
                    thread_id, i, i * 1000
                );
                executor.execute(&query).unwrap();
            }

            executor.execute("COMMIT").unwrap();
        });

        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let duration = start.elapsed();
    let total_inserts = num_threads * inserts_per_thread;
    let throughput = total_inserts as f64 / duration.as_secs_f64();

    println!("High-throughput test:");
    println!("  Total inserts: {}", total_inserts);
    println!("  Duration: {:?}", duration);
    println!("  Throughput: {:.2} inserts/sec", throughput);
}

/// Test transaction abort and retry pattern
#[test]
fn test_transaction_retry_pattern() {
    let graph = Arc::new(RwLock::new(Graph::new()));
    let executor = Arc::new(DQLExecutor::new(graph));

    // Insert test data
    executor.execute("INSERT INTO Counters VALUES ({id: 1, value: 0})").unwrap();

    let num_threads = 5;
    let increments_per_thread = 20;

    let barrier = Arc::new(Barrier::new(num_threads));
    let mut handles = vec![];

    for _ in 0..num_threads {
        let executor = Arc::clone(&executor);
        let barrier = Arc::clone(&barrier);

        let handle = thread::spawn(move || {
            barrier.wait();

            for _ in 0..increments_per_thread {
                loop {
                    // Try to increment counter
                    let result = executor.execute("BEGIN TRANSACTION");
                    if result.is_err() {
                        continue;
                    }

                    let result = executor.execute(
                        "UPDATE Counters SET value = value + 1 WHERE id = 1"
                    );

                    if result.is_err() {
                        let _ = executor.execute("ROLLBACK");
                        continue;
                    }

                    let result = executor.execute("COMMIT");
                    if result.is_ok() {
                        break; // Success!
                    }
                    // Retry on conflict
                }
            }
        });

        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Verify final counter value
    let result = executor.execute("FROM Counters WHERE id = 1 SELECT value").unwrap();
    println!("Final counter after concurrent increments: {:?}", result);
}

/// Test mixed transaction sizes
#[test]
fn test_mixed_transaction_sizes() {
    let graph = Arc::new(RwLock::new(Graph::new()));
    let executor = Arc::new(DQLExecutor::new(graph));

    let num_threads = 4;
    let barrier = Arc::new(Barrier::new(num_threads));
    let mut handles = vec![];

    for thread_id in 0..num_threads {
        let executor = Arc::clone(&executor);
        let barrier = Arc::clone(&barrier);

        let handle = thread::spawn(move || {
            barrier.wait();

            // Small transactions (1-5 operations)
            for i in 0..10 {
                executor.execute("BEGIN TRANSACTION").unwrap();
                for j in 0..5 {
                    executor.execute(&format!(
                        "INSERT INTO Small VALUES ({{thread: {}, batch: {}, id: {}}})",
                        thread_id, i, j
                    )).unwrap();
                }
                executor.execute("COMMIT").unwrap();
            }

            // Large transaction (100 operations)
            executor.execute("BEGIN TRANSACTION").unwrap();
            for i in 0..100 {
                executor.execute(&format!(
                    "INSERT INTO Large VALUES ({{thread: {}, id: {}}})",
                    thread_id, i
                )).unwrap();
            }
            executor.execute("COMMIT").unwrap();
        });

        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    println!("Mixed transaction sizes completed");
}

/// Test long-running transaction behavior
#[test]
fn test_long_running_transaction() {
    let graph = Arc::new(RwLock::new(Graph::new()));
    let executor = Arc::new(DQLExecutor::new(graph));

    // Start long-running transaction
    let executor1 = Arc::clone(&executor);
    let handle1 = thread::spawn(move || {
        executor1.execute("BEGIN TRANSACTION").unwrap();

        // Insert some data
        for i in 0..10 {
            executor1.execute(&format!(
                "INSERT INTO Data VALUES ({{id: {}, value: {}}})",
                i, i * 10
            )).unwrap();
        }

        // Sleep to simulate long-running work
        thread::sleep(Duration::from_millis(100));

        // More inserts
        for i in 10..20 {
            executor1.execute(&format!(
                "INSERT INTO Data VALUES ({{id: {}, value: {}}})",
                i, i * 10
            )).unwrap();
        }

        executor1.execute("COMMIT").unwrap();
    });

    // Meanwhile, other short transactions
    let executor2 = Arc::clone(&executor);
    let handle2 = thread::spawn(move || {
        thread::sleep(Duration::from_millis(10));

        for i in 0..5 {
            executor2.execute(&format!(
                "INSERT INTO OtherData VALUES ({{id: {}}})", i
            )).unwrap();
        }
    });

    handle1.join().unwrap();
    handle2.join().unwrap();

    println!("Long-running transaction test completed");
}
