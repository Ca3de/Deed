//! Performance Benchmarks for Deed Database
//!
//! Measures transaction overhead, throughput, and latency.

use deed_rust::*;
use std::sync::Arc;
use std::sync::RwLock;
use std::time::{Duration, Instant};
use std::thread;

/// Benchmark result
#[derive(Debug, Clone)]
pub struct BenchmarkResult {
    pub name: String,
    pub operations: usize,
    pub duration: Duration,
    pub ops_per_sec: f64,
    pub avg_latency_ms: f64,
}

impl BenchmarkResult {
    pub fn new(name: &str, operations: usize, duration: Duration) -> Self {
        let secs = duration.as_secs_f64();
        let ops_per_sec = operations as f64 / secs;
        let avg_latency_ms = (secs * 1000.0) / operations as f64;

        BenchmarkResult {
            name: name.to_string(),
            operations,
            duration,
            ops_per_sec,
            avg_latency_ms,
        }
    }

    pub fn print(&self) {
        println!("=== {} ===", self.name);
        println!("  Operations:    {}", self.operations);
        println!("  Duration:      {:?}", self.duration);
        println!("  Throughput:    {:.2} ops/sec", self.ops_per_sec);
        println!("  Avg Latency:   {:.3} ms", self.avg_latency_ms);
        println!();
    }
}

/// Benchmark: Single INSERT without transactions
pub fn bench_insert_no_transaction(count: usize) -> BenchmarkResult {
    let graph = Arc::new(RwLock::new(Graph::new()));
    let executor = DQLExecutor::new(graph);

    let start = Instant::now();

    for i in 0..count {
        let query = format!(
            "INSERT INTO Users VALUES ({{name: \"User{}\", age: {}}})",
            i, 20 + (i % 50)
        );
        executor.execute(&query).unwrap();
    }

    let duration = start.elapsed();
    BenchmarkResult::new("INSERT (auto-commit)", count, duration)
}

/// Benchmark: Batch INSERT in explicit transaction
pub fn bench_insert_batched_transaction(count: usize) -> BenchmarkResult {
    let graph = Arc::new(RwLock::new(Graph::new()));
    let executor = DQLExecutor::new(graph);

    let start = Instant::now();

    executor.execute("BEGIN TRANSACTION").unwrap();

    for i in 0..count {
        let query = format!(
            "INSERT INTO Users VALUES ({{name: \"User{}\", age: {}}})",
            i, 20 + (i % 50)
        );
        executor.execute(&query).unwrap();
    }

    executor.execute("COMMIT").unwrap();

    let duration = start.elapsed();
    BenchmarkResult::new("INSERT (batched in 1 txn)", count, duration)
}

/// Benchmark: SELECT queries (read-only)
pub fn bench_select(count: usize) -> BenchmarkResult {
    let graph = Arc::new(RwLock::new(Graph::new()));
    let executor = DQLExecutor::new(graph);

    // Insert test data
    executor.execute("BEGIN TRANSACTION").unwrap();
    for i in 0..1000 {
        let query = format!(
            "INSERT INTO Users VALUES ({{name: \"User{}\", age: {}}})",
            i, 20 + (i % 50)
        );
        executor.execute(&query).unwrap();
    }
    executor.execute("COMMIT").unwrap();

    let start = Instant::now();

    for _ in 0..count {
        executor.execute("FROM Users SELECT name, age").unwrap();
    }

    let duration = start.elapsed();
    BenchmarkResult::new("SELECT (read-only)", count, duration)
}

/// Benchmark: UPDATE with transactions
pub fn bench_update_transaction(count: usize) -> BenchmarkResult {
    let graph = Arc::new(RwLock::new(Graph::new()));
    let executor = DQLExecutor::new(graph);

    // Insert test data
    executor.execute("BEGIN TRANSACTION").unwrap();
    for i in 0..100 {
        let query = format!(
            "INSERT INTO Users VALUES ({{id: {}, name: \"User{}\", age: {}}})",
            i, i, 20 + (i % 50)
        );
        executor.execute(&query).unwrap();
    }
    executor.execute("COMMIT").unwrap();

    let start = Instant::now();

    for i in 0..count {
        executor.execute("BEGIN TRANSACTION").unwrap();
        let query = format!("UPDATE Users SET age = {} WHERE id = {}", 30, i % 100);
        executor.execute(&query).unwrap();
        executor.execute("COMMIT").unwrap();
    }

    let duration = start.elapsed();
    BenchmarkResult::new("UPDATE (with txn)", count, duration)
}

/// Benchmark: Mixed workload
pub fn bench_mixed_workload(iterations: usize) -> BenchmarkResult {
    let graph = Arc::new(RwLock::new(Graph::new()));
    let executor = DQLExecutor::new(graph);

    let start = Instant::now();
    let mut total_ops = 0;

    for i in 0..iterations {
        // INSERT
        executor.execute("BEGIN TRANSACTION").unwrap();
        for j in 0..10 {
            let query = format!(
                "INSERT INTO Users VALUES ({{name: \"User{}\", age: {}}})",
                i * 10 + j,
                20 + (j % 50)
            );
            executor.execute(&query).unwrap();
            total_ops += 1;
        }
        executor.execute("COMMIT").unwrap();

        // SELECT
        executor.execute("FROM Users SELECT name, age").unwrap();
        total_ops += 1;

        // UPDATE
        executor.execute(&format!("UPDATE Users SET age = 25 WHERE id = {}", i % 50)).unwrap();
        total_ops += 1;
    }

    let duration = start.elapsed();
    BenchmarkResult::new("Mixed workload", total_ops, duration)
}

/// Benchmark: Transaction commit overhead
pub fn bench_transaction_overhead() -> BenchmarkResult {
    let graph = Arc::new(RwLock::new(Graph::new()));
    let executor = DQLExecutor::new(graph);

    let count = 1000;
    let start = Instant::now();

    for _ in 0..count {
        executor.execute("BEGIN TRANSACTION").unwrap();
        executor.execute("COMMIT").unwrap();
    }

    let duration = start.elapsed();
    BenchmarkResult::new("Transaction overhead (BEGIN/COMMIT)", count, duration)
}

/// Benchmark: Aggregation performance
pub fn bench_aggregation(row_count: usize) -> BenchmarkResult {
    let graph = Arc::new(RwLock::new(Graph::new()));
    let executor = DQLExecutor::new(graph);

    // Insert test data
    executor.execute("BEGIN TRANSACTION").unwrap();
    for i in 0..row_count {
        let query = format!(
            "INSERT INTO Orders VALUES ({{customer_id: {}, amount: {}, city: \"City{}\"}})",
            i % 100,
            100 + (i % 500),
            i % 10
        );
        executor.execute(&query).unwrap();
    }
    executor.execute("COMMIT").unwrap();

    let start = Instant::now();

    executor.execute(
        "FROM Orders SELECT city, COUNT(*) AS order_count, SUM(amount) AS total \
         GROUP BY city"
    ).unwrap();

    let duration = start.elapsed();
    BenchmarkResult::new(
        &format!("Aggregation (GROUP BY on {} rows)", row_count),
        1,
        duration,
    )
}

/// Run all benchmarks
pub fn run_all_benchmarks() {
    println!("\n╔════════════════════════════════════════════════╗");
    println!("║   Deed Database Performance Benchmarks        ║");
    println!("╚════════════════════════════════════════════════╝\n");

    // 1. INSERT benchmarks
    println!("━━━ INSERT Performance ━━━\n");
    bench_insert_no_transaction(1000).print();
    bench_insert_batched_transaction(1000).print();

    // 2. Transaction overhead
    println!("━━━ Transaction Overhead ━━━\n");
    bench_transaction_overhead().print();

    // 3. SELECT benchmarks
    println!("━━━ SELECT Performance ━━━\n");
    bench_select(1000).print();

    // 4. UPDATE benchmarks
    println!("━━━ UPDATE Performance ━━━\n");
    bench_update_transaction(500).print();

    // 5. Aggregation benchmarks
    println!("━━━ Aggregation Performance ━━━\n");
    bench_aggregation(1000).print();
    bench_aggregation(10000).print();

    // 6. Mixed workload
    println!("━━━ Mixed Workload ━━━\n");
    bench_mixed_workload(100).print();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_benchmarks() {
        // Just verify benchmarks run without errors
        bench_insert_no_transaction(10);
        bench_insert_batched_transaction(10);
        bench_select(10);
        bench_transaction_overhead();
    }
}

fn main() {
    run_all_benchmarks();
}
