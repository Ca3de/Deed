use deed_rust::*;
use std::sync::{Arc, RwLock};

fn main() {
    println!("🎉 My First Deed Database!\n");

    // Step 1: Create a graph (database)
    let graph = Arc::new(RwLock::new(Graph::new()));
    let executor = DQLExecutor::new(graph.clone());

    // Step 2: Insert some data
    println!("📝 Inserting data...");

    executor.execute(r#"
        INSERT INTO Users VALUES ({
            name: "Alice",
            age: 30,
            email: "alice@example.com",
            city: "San Francisco"
        })
    "#).unwrap();

    executor.execute(r#"
        INSERT INTO Users VALUES ({
            name: "Bob",
            age: 25,
            email: "bob@example.com",
            city: "New York"
        })
    "#).unwrap();

    executor.execute(r#"
        INSERT INTO Users VALUES ({
            name: "Charlie",
            age: 35,
            email: "charlie@example.com",
            city: "San Francisco"
        })
    "#).unwrap();

    println!("✅ Inserted 3 users\n");

    // Step 3: Query the data
    println!("🔍 Query 1: All users in San Francisco");
    let result = executor.execute(r#"
        FROM Users
        WHERE city = "San Francisco"
        SELECT name, age, email
    "#).unwrap();
    println!("   Result: {} rows affected\n", result.rows_affected);

    println!("🔍 Query 2: Users older than 25");
    let result = executor.execute(r#"
        FROM Users
        WHERE age > 25
        SELECT name, age, city
    "#).unwrap();
    println!("   Result: {} rows affected\n", result.rows_affected);

    // Step 4: Aggregation
    println!("🔍 Query 3: Count users by city");
    let result = executor.execute(r#"
        FROM Users
        SELECT city, COUNT(*)
        GROUP BY city
    "#).unwrap();
    println!("   Result: {} rows affected\n", result.rows_affected);

    // Step 5: Update data
    println!("📝 Updating Bob's age...");
    executor.execute(r#"
        UPDATE Users
        SET age = 26
        WHERE name = "Bob"
    "#).unwrap();
    println!("✅ Updated\n");

    // Step 6: Delete data
    println!("🗑️  Deleting users over 30...");
    executor.execute(r#"
        DELETE FROM Users
        WHERE age > 30
    "#).unwrap();
    println!("✅ Deleted\n");

    // Step 7: Final count
    println!("🔍 Final query: All remaining users");
    let result = executor.execute(r#"
        FROM Users
        SELECT name, age, city
    "#).unwrap();
    println!("   Remaining: {} users", result.rows_affected);

    println!("\n🎉 Success! Your first Deed database is working!");
}
