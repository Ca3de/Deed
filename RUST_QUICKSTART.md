# Deed Database - Rust Quick Start Guide

Get started testing all production features in 5 minutes!

---

## Prerequisites

```bash
# Rust (if not installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Navigate to Rust implementation
cd Deed/deed-rust
```

---

## Quick Test: Run the E-Commerce Demo

**Complete real-world example - recommended for first-time users**

```bash
cargo run --example ecommerce_demo
```

This runs a complete e-commerce application demonstrating ALL features:

✅ **Authentication** - 3 users with different roles
✅ **Connection Pooling** - 5 concurrent queries
✅ **Transactions** - ACID-compliant inserts
✅ **Indexes** - 3 B-tree indexes
✅ **Complex Queries** - Aggregations, filters
✅ **Replication** - Master node setup
✅ **Backup/Restore** - Full backup creation
✅ **Admin Dashboard** - Real-time stats

**Expected runtime:** ~2-3 seconds
**Output:** Formatted dashboard with statistics

---

## What You'll See

```
╔══════════════════════════════════════════════════════════════╗
║       DEED DATABASE - E-COMMERCE DEMO                       ║
╚══════════════════════════════════════════════════════════════╝

📦 Step 1: Setting up database infrastructure...
✓ Connection pool created (min: 2, max: 10)
✓ Authentication configured
✓ Replication master initialized
✓ Backup manager configured
✓ Admin dashboard ready

📋 Step 2: Creating schema and indexes...
✓ Created indexes (user_email, product_price, order_status)

🎲 Step 3: Loading sample data...
✓ Inserted 3 users, 4 products, 3 orders

🔍 Step 4: Running business queries...
Query 1: Users in New York → Found 2 users
Query 2: Products under $50 → Found 2 products
Query 3: Order statistics → 2 groups

🔗 Step 5: Testing connection pooling...
  Thread 1-5 completed queries
✓ All concurrent queries successful

💾 Step 6: Creating database backup...
✓ Backup created and verified

📊 Step 7: Admin Dashboard
[Beautiful Unicode dashboard with real-time stats]

🎉 Demo complete!
```

---

## Step-by-Step: Build Your Own Test

### 1. Create a Simple Test

Create `deed-rust/examples/my_first_test.rs`:

```rust
use deed_rust::*;
use std::sync::{Arc, RwLock};

fn main() {
    println!("🚀 My First Deed Test\n");

    // Create database
    let graph = Arc::new(RwLock::new(Graph::new()));
    let executor = DQLExecutor::new(graph.clone());

    // Insert data
    println!("📥 Inserting data...");
    executor.execute(r#"
        INSERT INTO Users VALUES ({
            name: "Alice",
            age: 30,
            email: "alice@example.com"
        })
    "#).unwrap();

    executor.execute(r#"
        INSERT INTO Users VALUES ({
            name: "Bob",
            age: 25,
            email: "bob@example.com"
        })
    "#).unwrap();

    // Query data
    println!("🔍 Querying data...");
    let result = executor.execute(
        "FROM Users WHERE age > 20 SELECT name, age"
    ).unwrap();

    println!("✅ Found {} users\n", result.rows_affected);

    // Use aggregation
    println!("📊 Running aggregation...");
    let result = executor.execute(
        "FROM Users SELECT COUNT(*), AVG(age)"
    ).unwrap();

    println!("✅ Aggregation complete\n");
    println!("🎉 Test passed!");
}
```

### 2. Run It

```bash
cargo run --example my_first_test
```

---

## Test Individual Features

### Authentication

```rust
use deed_rust::*;

fn test_authentication() {
    let auth = AuthManager::new();

    // Create user
    auth.create_user(
        "alice".to_string(),
        "password123",
        Role::Admin
    ).unwrap();

    // Login
    let session = auth.login("alice", "password123").unwrap();
    println!("✓ Login successful: {}", session);

    // Check permissions
    auth.check_admin_permission(&session).unwrap();
    println!("✓ Has admin permission");

    // Logout
    auth.logout(&session).unwrap();
    println!("✓ Logged out");
}
```

### Connection Pool

```rust
use deed_rust::*;
use std::sync::Arc;

fn test_connection_pool() {
    let pool = ConnectionPool::with_defaults(
        graph, optimizer, cache, tx_mgr, None
    ).unwrap();

    // Get connection
    let mut conn = pool.get_connection().unwrap();
    let executor = conn.executor().unwrap();

    // Use it
    executor.execute("FROM Users SELECT name").unwrap();

    // Check stats
    let stats = pool.stats();
    println!("Pool: {} active, {} idle",
        stats.active_connections,
        stats.idle_connections
    );
}
```

### Indexes

```rust
fn test_indexes() {
    let executor = DQLExecutor::new(graph);

    // Create index
    executor.execute(
        "CREATE INDEX idx_user_age ON Users(age)"
    ).unwrap();
    println!("✓ Index created");

    // Queries now faster
    executor.execute(
        "FROM Users WHERE age = 25 SELECT name"
    ).unwrap();

    // Drop index
    executor.execute("DROP INDEX idx_user_age").unwrap();
    println!("✓ Index dropped");
}
```

### Transactions

```rust
fn test_transactions() {
    let executor = DQLExecutor::new(graph);

    // Manual transaction
    executor.execute("BEGIN TRANSACTION").unwrap();

    executor.execute(
        "INSERT INTO Users VALUES ({name: 'Alice'})"
    ).unwrap();

    executor.execute(
        "INSERT INTO Users VALUES ({name: 'Bob'})"
    ).unwrap();

    executor.execute("COMMIT").unwrap();
    println!("✓ Transaction committed");

    // Rollback example
    executor.execute("BEGIN TRANSACTION").unwrap();
    executor.execute("DELETE FROM Users").unwrap();
    executor.execute("ROLLBACK").unwrap();
    println!("✓ Changes rolled back");
}
```

### Backups

```rust
fn test_backups() {
    let mut backup_mgr = BackupManager::with_defaults().unwrap();

    // Create backup
    let metadata = backup_mgr.create_full_backup(&graph).unwrap();
    println!("✓ Backup created: {}", metadata.backup_id);

    // Verify
    let is_valid = backup_mgr.verify_backup(&metadata.backup_id).unwrap();
    println!("✓ Backup valid: {}", is_valid);

    // List backups
    let backups = backup_mgr.list_backups().unwrap();
    println!("✓ Total backups: {}", backups.len());

    // Restore (WARNING: clears current data)
    // backup_mgr.restore_backup(&metadata.backup_id, &mut graph).unwrap();
}
```

### Replication

```rust
fn test_replication() {
    // Master
    let master = ReplicationManager::new_master("master-1".to_string());

    // Log operation
    let mut props = HashMap::new();
    props.insert("name".to_string(), PropertyValue::String("Alice".to_string()));

    let seq = master.log_insert(1, "User".to_string(), props).unwrap();
    println!("✓ Logged at sequence: {}", seq);

    // Create slave
    let slave = ReplicationManager::new_slave(
        "slave-1".to_string(),
        "localhost:9000".to_string()
    );

    // Register slave
    master.register_slave("slave-1".to_string()).unwrap();
    println!("✓ Slave registered");

    // Check stats
    let stats = master.stats();
    println!("✓ Slaves: {}", stats.slave_count);
}
```

### Admin Dashboard

```rust
fn test_dashboard() {
    let dashboard = AdminDashboard::new();

    let stats = dashboard.get_stats(
        &graph,
        &auth,
        Some(&pool),
        Some(&replication),
        &tx_mgr
    );

    // Display full dashboard
    println!("{}", dashboard.format_dashboard(&stats));

    // Or just specific stats
    println!("Entities: {}", stats.database.entity_count);
    println!("Pool utilization: {:.1}%",
        stats.pool.as_ref().map(|p| p.utilization()).unwrap_or(0.0)
    );
}
```

---

## Run Tests

### Unit Tests

```bash
# All tests
cargo test

# Specific feature
cargo test transaction
cargo test production_features
cargo test auth
cargo test replication
cargo test backup

# With output
cargo test -- --nocapture
```

### Benchmarks

```bash
cargo bench
```

Expected output:
```
test bench_insert_no_transaction       ... bench:     125 ns/iter
test bench_insert_batched_transaction  ... bench:      45 ns/iter
test bench_aggregation                 ... bench:   1,234 ns/iter
```

---

## Real-World Scenarios

### Scenario 1: Social Network

```rust
fn social_network_demo() {
    let graph = Arc::new(RwLock::new(Graph::new()));
    let executor = DQLExecutor::new(graph.clone());

    // Users
    executor.execute("INSERT INTO Users VALUES ({name: 'Alice', city: 'NYC'})").unwrap();
    executor.execute("INSERT INTO Users VALUES ({name: 'Bob', city: 'SF'})").unwrap();

    // Friendships
    executor.execute("CREATE (1)-[:FOLLOWS]->(2)").unwrap();

    // Graph query
    let result = executor.execute(
        "FROM Users WHERE name = 'Alice'
         TRAVERSE -[:FOLLOWS]-> Users
         SELECT name"
    ).unwrap();

    println!("Alice follows {} people", result.rows_affected);
}
```

### Scenario 2: Analytics

```rust
fn analytics_demo() {
    let executor = DQLExecutor::new(graph);

    // Sales by category
    executor.execute(r#"
        FROM Orders
        SELECT category, COUNT(*), SUM(total), AVG(total)
        GROUP BY category
    "#).unwrap();

    // Top customers
    executor.execute(r#"
        FROM Orders
        SELECT user_id, SUM(total) as revenue
        GROUP BY user_id
        HAVING revenue > 1000
    "#).unwrap();
}
```

### Scenario 3: High Availability

```rust
fn ha_setup() {
    // Master
    let master = ReplicationManager::new_master("master-1".to_string());

    // Slaves
    let slave1 = ReplicationManager::new_slave("slave-1".to_string(), "master:9000".to_string());
    let slave2 = ReplicationManager::new_slave("slave-2".to_string(), "master:9000".to_string());

    master.register_slave("slave-1".to_string()).unwrap();
    master.register_slave("slave-2".to_string()).unwrap();

    // Log operations
    master.log_insert(1, "User".to_string(), props).unwrap();

    // Slaves fetch and apply
    let entries = master.get_entries_since(0);
    for entry in entries {
        slave1.apply_entry(entry.clone()).unwrap();
        slave2.apply_entry(entry).unwrap();
    }

    println!("Replicated to {} slaves", master.get_slave_states().len());
}
```

---

## DQL Query Reference

```sql
-- INSERT
INSERT INTO Users VALUES ({name: "Alice", age: 30})

-- SELECT
FROM Users SELECT name, age
FROM Users WHERE age > 25 SELECT name

-- AGGREGATIONS
FROM Orders SELECT COUNT(*), SUM(total)
FROM Orders SELECT category, AVG(price) GROUP BY category
FROM Orders SELECT status, COUNT(*) GROUP BY status HAVING COUNT(*) > 10

-- GRAPH TRAVERSAL
FROM Users WHERE name = 'Alice'
TRAVERSE -[:FOLLOWS]-> Users
SELECT name

-- TRANSACTIONS
BEGIN TRANSACTION
BEGIN TRANSACTION ISOLATION LEVEL SERIALIZABLE
COMMIT
ROLLBACK

-- INDEXES
CREATE INDEX idx_name ON Users(name)
CREATE UNIQUE INDEX idx_email ON Users(email)
DROP INDEX idx_name
```

---

## Performance Testing

### Measure Index Impact

```rust
use std::time::Instant;

// Without index
let start = Instant::now();
executor.execute("FROM Users WHERE age = 30 SELECT name").unwrap();
let without = start.elapsed();

// Create index
executor.execute("CREATE INDEX idx_age ON Users(age)").unwrap();

// With index
let start = Instant::now();
executor.execute("FROM Users WHERE age = 30 SELECT name").unwrap();
let with = start.elapsed();

println!("Without index: {:?}", without);
println!("With index: {:?}", with);
println!("Speedup: {:.1}x",
    without.as_nanos() as f64 / with.as_nanos() as f64
);
```

### Measure Transaction Throughput

```rust
let start = Instant::now();
executor.execute("BEGIN TRANSACTION").unwrap();

for i in 0..1000 {
    executor.execute(
        &format!("INSERT INTO Users VALUES ({{id: {}}})", i)
    ).unwrap();
}

executor.execute("COMMIT").unwrap();
let duration = start.elapsed();

println!("1000 inserts in {:?}", duration);
println!("Throughput: {:.0} inserts/sec",
    1000.0 / duration.as_secs_f64()
);
```

---

## Troubleshooting

### Compilation Errors

```bash
# Update dependencies
cargo update

# Clean build
cargo clean
cargo build
```

### Test Failures

```bash
# Run with output
cargo test -- --nocapture

# Single test
cargo test test_name -- --nocapture
```

### Backup Issues

```bash
# Create directory
mkdir -p /tmp/deed_backups

# Or use local directory
let config = BackupConfig {
    backup_dir: PathBuf::from("./backups"),
    ...
};
```

---

## Next Steps

1. ✅ Run the e-commerce demo
2. ✅ Read the guides:
   - `PRODUCTION_FEATURES_GUIDE.md`
   - `HIGH_AVAILABILITY_GUIDE.md`
3. ✅ Try your own examples
4. ✅ Run benchmarks
5. ✅ Deploy with replication and backups

---

## Common Commands

```bash
# Run e-commerce demo
cargo run --example ecommerce_demo

# Run tests
cargo test
cargo test --nocapture

# Run benchmarks
cargo bench

# Build release
cargo build --release

# Check code
cargo clippy
cargo fmt --check
```

---

**Happy testing! 🎉**

Questions? Check the comprehensive guides:
- `PRODUCTION_FEATURES_GUIDE.md` - Indexes, Auth, Connection Pooling
- `HIGH_AVAILABILITY_GUIDE.md` - Replication, Backup, Dashboard
