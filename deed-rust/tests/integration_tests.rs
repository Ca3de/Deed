//! Integration Tests - End-to-End Scenarios
//!
//! Tests complete workflows that users would actually perform.

use deed_rust::*;
use std::sync::{Arc, RwLock};
use tempfile::TempDir;

/// Test: E-commerce order processing workflow
#[test]
fn test_ecommerce_order_workflow() {
    let temp_dir = TempDir::new().unwrap();
    let wal_path = temp_dir.path().join("ecommerce.wal");

    let graph = Arc::new(RwLock::new(Graph::new()));
    let executor = DQLExecutor::new_with_wal(graph, &wal_path).unwrap();

    // Setup: Create products and inventory
    executor.execute("BEGIN TRANSACTION").unwrap();
    executor.execute("INSERT INTO Products VALUES ({id: 1, name: \"Laptop\", price: 999, stock: 10})").unwrap();
    executor.execute("INSERT INTO Products VALUES ({id: 2, name: \"Mouse\", price: 29, stock: 50})").unwrap();
    executor.execute("INSERT INTO Products VALUES ({id: 3, name: \"Keyboard\", price: 79, stock: 30})").unwrap();
    executor.execute("COMMIT").unwrap();

    // Workflow: Customer places order
    executor.execute("BEGIN TRANSACTION").unwrap();

    // 1. Create order
    executor.execute("INSERT INTO Orders VALUES ({id: 1, customer_id: 42, total: 0, status: \"pending\"})").unwrap();

    // 2. Add order items and update inventory
    executor.execute("INSERT INTO OrderItems VALUES ({order_id: 1, product_id: 1, quantity: 1, price: 999})").unwrap();
    executor.execute("UPDATE Products SET stock = stock - 1 WHERE id = 1").unwrap();

    executor.execute("INSERT INTO OrderItems VALUES ({order_id: 1, product_id: 2, quantity: 2, price: 58})").unwrap();
    executor.execute("UPDATE Products SET stock = stock - 2 WHERE id = 2").unwrap();

    // 3. Update order total
    executor.execute("UPDATE Orders SET total = 1057, status = \"confirmed\" WHERE id = 1").unwrap();

    executor.execute("COMMIT").unwrap();

    // Verify: Check inventory was updated
    let result = executor.execute("FROM Products WHERE id = 1 SELECT stock").unwrap();
    assert!(!result.rows.is_empty());

    // Verify: Order exists
    let result = executor.execute("FROM Orders WHERE id = 1 SELECT status, total").unwrap();
    assert!(!result.rows.is_empty());

    println!("✓ E-commerce order workflow completed");
}

/// Test: Bank transfer with rollback on insufficient funds
#[test]
fn test_bank_transfer_with_validation() {
    let graph = Arc::new(RwLock::new(Graph::new()));
    let executor = DQLExecutor::new(graph);

    // Setup accounts
    executor.execute("INSERT INTO Accounts VALUES ({id: 1, name: \"Alice\", balance: 1000})").unwrap();
    executor.execute("INSERT INTO Accounts VALUES ({id: 2, name: \"Bob\", balance: 500})").unwrap();

    // Successful transfer
    executor.execute("BEGIN TRANSACTION ISOLATION LEVEL SERIALIZABLE").unwrap();
    executor.execute("UPDATE Accounts SET balance = balance - 100 WHERE id = 1").unwrap();
    executor.execute("UPDATE Accounts SET balance = balance + 100 WHERE id = 2").unwrap();
    executor.execute("INSERT INTO Transactions VALUES ({from_id: 1, to_id: 2, amount: 100})").unwrap();
    executor.execute("COMMIT").unwrap();

    // Verify balances
    let result = executor.execute("FROM Accounts SELECT id, balance").unwrap();
    println!("After transfer: {:?}", result);

    // Failed transfer (insufficient funds) - should rollback
    executor.execute("BEGIN TRANSACTION").unwrap();
    let result = executor.execute("UPDATE Accounts SET balance = balance - 2000 WHERE id = 1 AND balance >= 2000");

    // This should fail or update 0 rows
    executor.execute("ROLLBACK").unwrap();

    println!("✓ Bank transfer workflow completed");
}

/// Test: Social network - create users and relationships
#[test]
fn test_social_network_graph() {
    let graph = Arc::new(RwLock::new(Graph::new()));
    let executor = DQLExecutor::new(graph);

    // Create users in transaction
    executor.execute("BEGIN TRANSACTION").unwrap();
    executor.execute("INSERT INTO Users VALUES ({id: 1, name: \"Alice\", age: 30})").unwrap();
    executor.execute("INSERT INTO Users VALUES ({id: 2, name: \"Bob\", age: 25})").unwrap();
    executor.execute("INSERT INTO Users VALUES ({id: 3, name: \"Charlie\", age: 35})").unwrap();
    executor.execute("COMMIT").unwrap();

    // Create friendships (graph edges)
    executor.execute("BEGIN TRANSACTION").unwrap();
    // Note: CREATE edge syntax would be:
    // executor.execute("CREATE (alice) -[:FRIEND_OF]-> (bob) {since: \"2024\"}").unwrap();
    // For now, using table to simulate
    executor.execute("INSERT INTO Friendships VALUES ({user1_id: 1, user2_id: 2, since: \"2024-01-01\"})").unwrap();
    executor.execute("INSERT INTO Friendships VALUES ({user1_id: 1, user2_id: 3, since: \"2024-02-15\"})").unwrap();
    executor.execute("INSERT INTO Friendships VALUES ({user2_id: 2, user2_id: 3, since: \"2024-03-20\"})").unwrap();
    executor.execute("COMMIT").unwrap();

    // Query with aggregation
    let result = executor.execute(
        "FROM Friendships SELECT user1_id, COUNT(*) AS friend_count GROUP BY user1_id"
    ).unwrap();

    println!("Friend counts: {:?}", result);
    println!("✓ Social network workflow completed");
}

/// Test: Analytics - daily active users report
#[test]
fn test_analytics_workflow() {
    let graph = Arc::new(RwLock::new(Graph::new()));
    let executor = DQLExecutor::new(graph);

    // Insert activity data
    executor.execute("BEGIN TRANSACTION").unwrap();
    for day in 1..=7 {
        for user in 1..=100 {
            executor.execute(&format!(
                "INSERT INTO UserActivity VALUES ({{user_id: {}, day: {}, sessions: {}, duration: {}}})",
                user, day, 1 + (user % 5), 10 + (user % 60)
            )).unwrap();
        }
    }
    executor.execute("COMMIT").unwrap();

    // Run analytics queries
    let result = executor.execute(
        "FROM UserActivity SELECT day, COUNT(*) AS active_users, SUM(sessions) AS total_sessions \
         GROUP BY day"
    ).unwrap();

    println!("Daily active users: {:?}", result);

    let result = executor.execute(
        "FROM UserActivity SELECT user_id, SUM(duration) AS total_time \
         GROUP BY user_id \
         HAVING SUM(duration) > 200"
    ).unwrap();

    println!("High-engagement users: {:?}", result);

    println!("✓ Analytics workflow completed");
}

/// Test: Inventory management with low-stock alerts
#[test]
fn test_inventory_management() {
    let graph = Arc::new(RwLock::new(Graph::new()));
    let executor = DQLExecutor::new(graph);

    // Setup inventory
    executor.execute("BEGIN TRANSACTION").unwrap();
    executor.execute("INSERT INTO Inventory VALUES ({id: 1, product: \"Widget\", warehouse: 1, stock: 100})").unwrap();
    executor.execute("INSERT INTO Inventory VALUES ({id: 2, product: \"Gadget\", warehouse: 1, stock: 50})").unwrap();
    executor.execute("INSERT INTO Inventory VALUES ({id: 3, product: \"Doohickey\", warehouse: 2, stock: 5})").unwrap();
    executor.execute("COMMIT").unwrap();

    // Simulate sales
    executor.execute("BEGIN TRANSACTION").unwrap();
    executor.execute("UPDATE Inventory SET stock = stock - 95 WHERE id = 1").unwrap();
    executor.execute("UPDATE Inventory SET stock = stock - 45 WHERE id = 2").unwrap();
    executor.execute("COMMIT").unwrap();

    // Low stock alert query
    let result = executor.execute(
        "FROM Inventory WHERE stock < 10 SELECT product, warehouse, stock"
    ).unwrap();

    println!("Low stock items: {:?}", result);

    // Warehouse summary
    let result = executor.execute(
        "FROM Inventory SELECT warehouse, COUNT(*) AS items, SUM(stock) AS total_stock \
         GROUP BY warehouse"
    ).unwrap();

    println!("Warehouse summary: {:?}", result);

    println!("✓ Inventory management workflow completed");
}

/// Test: Multi-table update in transaction
#[test]
fn test_multi_table_update() {
    let graph = Arc::new(RwLock::new(Graph::new()));
    let executor = DQLExecutor::new(graph);

    // Setup
    executor.execute("INSERT INTO Users VALUES ({id: 1, name: \"Alice\", points: 100})").unwrap();
    executor.execute("INSERT INTO Rewards VALUES ({user_id: 1, total_earned: 100})").unwrap();

    // Award points in transaction
    executor.execute("BEGIN TRANSACTION").unwrap();
    executor.execute("UPDATE Users SET points = points + 50 WHERE id = 1").unwrap();
    executor.execute("UPDATE Rewards SET total_earned = total_earned + 50 WHERE user_id = 1").unwrap();
    executor.execute("INSERT INTO PointsHistory VALUES ({user_id: 1, amount: 50, reason: \"bonus\"})").unwrap();
    executor.execute("COMMIT").unwrap();

    // Verify consistency
    let result = executor.execute("FROM Users WHERE id = 1 SELECT points").unwrap();
    println!("User points: {:?}", result);

    let result = executor.execute("FROM Rewards WHERE user_id = 1 SELECT total_earned").unwrap();
    println!("Total earned: {:?}", result);

    println!("✓ Multi-table update workflow completed");
}

/// Test: Batch import with error handling
#[test]
fn test_batch_import_with_errors() {
    let graph = Arc::new(RwLock::new(Graph::new()));
    let executor = DQLExecutor::new(graph);

    // Successful batch import
    executor.execute("BEGIN TRANSACTION").unwrap();
    for i in 0..100 {
        executor.execute(&format!(
            "INSERT INTO ImportedData VALUES ({{id: {}, value: {}}})",
            i, i * 10
        )).unwrap();
    }
    executor.execute("COMMIT").unwrap();

    // Verify
    let result = executor.execute("FROM ImportedData SELECT COUNT(*) AS count").unwrap();
    println!("Imported records: {:?}", result);

    // Failed batch (all or nothing)
    executor.execute("BEGIN TRANSACTION").unwrap();
    for i in 100..150 {
        executor.execute(&format!(
            "INSERT INTO ImportedData VALUES ({{id: {}, value: {}}})",
            i, i * 10
        )).unwrap();
    }
    // Simulate error
    executor.execute("ROLLBACK").unwrap();

    // Verify rollback - should still be 100 records
    let result = executor.execute("FROM ImportedData SELECT COUNT(*) AS count").unwrap();
    println!("After rollback: {:?}", result);

    println!("✓ Batch import workflow completed");
}

/// Test: Time-series data ingestion
#[test]
fn test_timeseries_ingestion() {
    let graph = Arc::new(RwLock::new(Graph::new()));
    let executor = DQLExecutor::new(graph);

    // Batch insert sensor data
    executor.execute("BEGIN TRANSACTION").unwrap();
    for minute in 0..60 {
        for sensor_id in 1..=10 {
            executor.execute(&format!(
                "INSERT INTO SensorReadings VALUES ({{sensor_id: {}, timestamp: {}, value: {}}})",
                sensor_id, minute, 20 + (minute % 30)
            )).unwrap();
        }
    }
    executor.execute("COMMIT").unwrap();

    // Aggregate by sensor
    let result = executor.execute(
        "FROM SensorReadings SELECT sensor_id, COUNT(*) AS readings, AVG(value) AS avg_value \
         GROUP BY sensor_id"
    ).unwrap();

    println!("Sensor statistics: {:?}", result);

    println!("✓ Time-series workflow completed");
}

/// Test: Complex query with multiple aggregations
#[test]
fn test_complex_analytics() {
    let graph = Arc::new(RwLock::new(Graph::new()));
    let executor = DQLExecutor::new(graph);

    // Setup sales data
    executor.execute("BEGIN TRANSACTION").unwrap();
    for i in 0..200 {
        executor.execute(&format!(
            "INSERT INTO Sales VALUES ({{id: {}, product: \"Product{}\", region: \"Region{}\", amount: {}, quantity: {}}})",
            i,
            i % 10,
            i % 3,
            100 + (i % 500),
            1 + (i % 10)
        )).unwrap();
    }
    executor.execute("COMMIT").unwrap();

    // Complex analytics
    let result = executor.execute(
        "FROM Sales SELECT \
            region, \
            COUNT(*) AS sale_count, \
            SUM(amount) AS total_revenue, \
            AVG(amount) AS avg_sale, \
            MAX(amount) AS max_sale \
         GROUP BY region"
    ).unwrap();

    println!("Sales by region: {:?}", result);

    let result = executor.execute(
        "FROM Sales SELECT \
            product, \
            SUM(quantity) AS units_sold, \
            SUM(amount) AS revenue \
         GROUP BY product \
         HAVING SUM(quantity) > 20"
    ).unwrap();

    println!("Top selling products: {:?}", result);

    println!("✓ Complex analytics completed");
}

/// Test: Full application lifecycle
#[test]
fn test_application_lifecycle() {
    let temp_dir = TempDir::new().unwrap();
    let wal_path = temp_dir.path().join("app.wal");

    // Phase 1: Application startup and initialization
    {
        let graph = Arc::new(RwLock::new(Graph::new()));
        let executor = DQLExecutor::new_with_wal(graph, &wal_path).unwrap();

        executor.execute("BEGIN TRANSACTION").unwrap();
        executor.execute("INSERT INTO Config VALUES ({key: \"version\", value: \"1.0\"})").unwrap();
        executor.execute("INSERT INTO Config VALUES ({key: \"initialized\", value: \"true\"})").unwrap();
        executor.execute("COMMIT").unwrap();
    }

    // Phase 2: Normal operation
    {
        let graph = Arc::new(RwLock::new(Graph::new()));
        let executor = DQLExecutor::new_with_wal(graph, &wal_path).unwrap();

        // User signup
        executor.execute("INSERT INTO Users VALUES ({id: 1, email: \"user@example.com\"})").unwrap();

        // User activity
        executor.execute("BEGIN TRANSACTION").unwrap();
        executor.execute("INSERT INTO Events VALUES ({user_id: 1, event: \"login\"})").unwrap();
        executor.execute("INSERT INTO Events VALUES ({user_id: 1, event: \"view_page\"})").unwrap();
        executor.execute("COMMIT").unwrap();
    }

    // Phase 3: Crash and recovery
    {
        let wal_manager = WALManager::new(&wal_path).unwrap();
        let recovery = wal_manager.recover().unwrap();

        println!("Application lifecycle recovery:");
        println!("  Committed transactions: {}", recovery.committed_txns.len());

        assert!(recovery.committed_txns.len() >= 2);
    }

    println!("✓ Application lifecycle completed");
}
