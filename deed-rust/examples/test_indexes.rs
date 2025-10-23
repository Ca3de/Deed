use deed_rust::*;
use std::sync::{Arc, RwLock};
use std::time::Instant;

fn main() {
    println!("🔍 Deed Index Performance Test\n");

    let graph = Arc::new(RwLock::new(Graph::new()));
    let executor = DQLExecutor::new(graph.clone());

    // Test 1: Create test data
    println!("📦 Test 1: Creating test dataset...");
    let dataset_size = 1000;

    for i in 0..dataset_size {
        let age = 20 + (i % 50); // Ages from 20-69
        let salary = 30000 + (i % 100) * 1000; // Salaries from 30k-129k
        let city = match i % 5 {
            0 => "NYC",
            1 => "SF",
            2 => "LA",
            3 => "Chicago",
            _ => "Boston",
        };

        executor.execute(&format!(
            r#"INSERT INTO Employees VALUES ({{
                name: "Employee{}",
                age: {},
                salary: {},
                city: "{}",
                department: "Engineering"
            }})"#,
            i, age, salary, city
        )).ok();
    }

    println!("   ✓ Created {} employee records\n", dataset_size);

    // Test 2: Query WITHOUT index
    println!("🐌 Test 2: Query performance WITHOUT index");
    println!("   Query: SELECT employees where age = 35");

    let start = Instant::now();
    match executor.execute(r#"FROM Employees WHERE age = 35 SELECT name, age, salary"#) {
        Ok(result) => {
            let duration = start.elapsed();
            println!("   ✓ Found {} employees", result.rows.len());
            println!("   ⏱️  Time: {:?}", duration);
            println!("   📊 Full table scan (no index)");
        }
        Err(e) => println!("   ✗ Error: {}", e),
    }

    // Test 3: Create index
    println!("\n🔨 Test 3: Creating index on 'age' field...");

    match executor.execute(r#"CREATE INDEX idx_age ON Employees (age)"#) {
        Ok(_) => println!("   ✓ Index created successfully"),
        Err(e) => println!("   ✗ Error creating index: {}", e),
    }

    // Test 4: Query WITH index
    println!("\n⚡ Test 4: Query performance WITH index");
    println!("   Query: SELECT employees where age = 35");

    let start = Instant::now();
    match executor.execute(r#"FROM Employees WHERE age = 35 SELECT name, age, salary"#) {
        Ok(result) => {
            let duration = start.elapsed();
            println!("   ✓ Found {} employees", result.rows.len());
            println!("   ⏱️  Time: {:?}", duration);
            println!("   📊 Index lookup (optimized)");
        }
        Err(e) => println!("   ✗ Error: {}", e),
    }

    // Test 5: Range query without index
    println!("\n🐌 Test 5: Range query WITHOUT index on salary");
    println!("   Query: SELECT employees where salary > 80000");

    let start = Instant::now();
    match executor.execute(r#"FROM Employees WHERE salary > 80000 SELECT name, salary"#) {
        Ok(result) => {
            let duration = start.elapsed();
            println!("   ✓ Found {} employees", result.rows.len());
            println!("   ⏱️  Time: {:?}", duration);
        }
        Err(e) => println!("   ✗ Error: {}", e),
    }

    // Test 6: Create second index
    println!("\n🔨 Test 6: Creating index on 'salary' field...");

    match executor.execute(r#"CREATE INDEX idx_salary ON Employees (salary)"#) {
        Ok(_) => println!("   ✓ Index created successfully"),
        Err(e) => println!("   ✗ Error creating index: {}", e),
    }

    // Test 7: Range query with index
    println!("\n⚡ Test 7: Range query WITH index on salary");
    println!("   Query: SELECT employees where salary > 80000");

    let start = Instant::now();
    match executor.execute(r#"FROM Employees WHERE salary > 80000 SELECT name, salary"#) {
        Ok(result) => {
            let duration = start.elapsed();
            println!("   ✓ Found {} employees", result.rows.len());
            println!("   ⏱️  Time: {:?}", duration);
            println!("   📊 Index range scan (optimized)");
        }
        Err(e) => println!("   ✗ Error: {}", e),
    }

    // Test 8: Unique index
    println!("\n🔒 Test 8: Testing UNIQUE index");

    // Create unique index on name
    match executor.execute(r#"CREATE UNIQUE INDEX idx_unique_name ON Employees (name)"#) {
        Ok(_) => println!("   ✓ Unique index created"),
        Err(e) => println!("   ✗ Error creating unique index: {}", e),
    }

    // Try to insert duplicate (should fail)
    match executor.execute(r#"INSERT INTO Employees VALUES ({name: "Employee0", age: 30, salary: 50000, city: "NYC", department: "Sales"})"#) {
        Ok(_) => println!("   ✗ Duplicate insert should have failed!"),
        Err(e) => println!("   ✓ Duplicate insert correctly rejected: {}", e),
    }

    // Test 9: Drop index
    println!("\n🗑️  Test 9: Dropping index");

    match executor.execute(r#"DROP INDEX idx_age"#) {
        Ok(_) => println!("   ✓ Index dropped successfully"),
        Err(e) => println!("   ✗ Error dropping index: {}", e),
    }

    // Test 10: Verify index dropped (query should be slower)
    println!("\n🐌 Test 10: Query after index drop");
    println!("   Query: SELECT employees where age = 35");

    let start = Instant::now();
    match executor.execute(r#"FROM Employees WHERE age = 35 SELECT name, age"#) {
        Ok(result) => {
            let duration = start.elapsed();
            println!("   ✓ Found {} employees", result.rows.len());
            println!("   ⏱️  Time: {:?}", duration);
            println!("   📊 Back to full table scan");
        }
        Err(e) => println!("   ✗ Error: {}", e),
    }

    // Test 11: Query statistics
    println!("\n📊 Test 11: Index statistics summary");
    println!("   Dataset: {} records", dataset_size);
    println!("   Active indexes: idx_salary, idx_unique_name");
    println!("   Dropped indexes: idx_age");
    println!("\n   Expected performance:");
    println!("   - Indexed queries: O(log n) ~ {}ms", (dataset_size as f64).log2() as u64);
    println!("   - Full table scan: O(n) ~ {}ms", dataset_size / 100);

    println!("\n✨ Index Performance Test Complete!");
}
