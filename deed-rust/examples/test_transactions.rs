use deed_rust::*;
use std::sync::{Arc, RwLock};

fn main() {
    println!("💳 Deed Transaction & MVCC Test\n");

    let graph = Arc::new(RwLock::new(Graph::new()));
    let executor = DQLExecutor::new(graph.clone());

    // Setup: Create bank accounts
    println!("🏦 Setup: Creating bank accounts...");

    executor.execute(r#"INSERT INTO Accounts VALUES ({holder: "Alice", balance: 1000})"#).ok();
    executor.execute(r#"INSERT INTO Accounts VALUES ({holder: "Bob", balance: 500})"#).ok();
    executor.execute(r#"INSERT INTO Accounts VALUES ({holder: "Carol", balance: 750})"#).ok();

    println!("   ✓ Created 3 accounts\n");

    // Test 1: Basic Transaction with BEGIN/COMMIT
    println!("📝 Test 1: Basic Transaction (BEGIN/COMMIT)");

    match executor.execute("BEGIN TRANSACTION") {
        Ok(_) => println!("   ✓ Transaction started"),
        Err(e) => println!("   ✗ Error starting transaction: {}", e),
    }

    // Update Alice's balance
    match executor.execute(r#"UPDATE Accounts SET balance = 1200 WHERE holder = "Alice""#) {
        Ok(result) => println!("   ✓ Updated {} account (Alice: 1000 → 1200)", result.rows_affected),
        Err(e) => println!("   ✗ Error: {}", e),
    }

    // Verify the change
    match executor.execute(r#"FROM Accounts WHERE holder = "Alice" SELECT holder, balance"#) {
        Ok(result) => {
            println!("   ✓ Within transaction, Alice's balance:");
            for row in &result.rows {
                println!("     {:?}", row);
            }
        }
        Err(e) => println!("   ✗ Error: {}", e),
    }

    match executor.execute("COMMIT") {
        Ok(_) => println!("   ✓ Transaction committed\n"),
        Err(e) => println!("   ✗ Error committing: {}\n", e),
    }

    // Test 2: Transaction Rollback
    println!("🔄 Test 2: Transaction Rollback");

    match executor.execute("BEGIN TRANSACTION") {
        Ok(_) => println!("   ✓ Transaction started"),
        Err(e) => println!("   ✗ Error: {}", e),
    }

    // Update Bob's balance
    match executor.execute(r#"UPDATE Accounts SET balance = 1000 WHERE holder = "Bob""#) {
        Ok(result) => println!("   ✓ Updated {} account (Bob: 500 → 1000)", result.rows_affected),
        Err(e) => println!("   ✗ Error: {}", e),
    }

    // Check balance within transaction
    match executor.execute(r#"FROM Accounts WHERE holder = "Bob" SELECT holder, balance"#) {
        Ok(result) => {
            println!("   ✓ Within transaction, Bob's balance:");
            for row in &result.rows {
                println!("     {:?}", row);
            }
        }
        Err(e) => println!("   ✗ Error: {}", e),
    }

    // Rollback the transaction
    match executor.execute("ROLLBACK") {
        Ok(_) => println!("   ✓ Transaction rolled back"),
        Err(e) => println!("   ✗ Error: {}", e),
    }

    // Verify rollback - Bob should still have 500
    match executor.execute(r#"FROM Accounts WHERE holder = "Bob" SELECT holder, balance"#) {
        Ok(result) => {
            println!("   ✓ After rollback, Bob's balance (should be 500):");
            for row in &result.rows {
                println!("     {:?}", row);
            }
        }
        Err(e) => println!("   ✗ Error: {}", e),
    }

    println!();

    // Test 3: Isolation Levels
    println!("🔒 Test 3: Isolation Levels");

    // Test Read Committed
    println!("\n   Testing READ COMMITTED:");
    match executor.execute("BEGIN TRANSACTION ISOLATION LEVEL READ COMMITTED") {
        Ok(_) => println!("     ✓ Started with READ COMMITTED isolation"),
        Err(e) => println!("     ✗ Error: {}", e),
    }

    match executor.execute(r#"FROM Accounts SELECT holder, balance"#) {
        Ok(result) => {
            println!("     ✓ Read {} accounts", result.rows.len());
        }
        Err(e) => println!("     ✗ Error: {}", e),
    }

    match executor.execute("COMMIT") {
        Ok(_) => println!("     ✓ Committed"),
        Err(e) => println!("     ✗ Error: {}", e),
    }

    // Test Repeatable Read
    println!("\n   Testing REPEATABLE READ:");
    match executor.execute("BEGIN TRANSACTION ISOLATION LEVEL REPEATABLE READ") {
        Ok(_) => println!("     ✓ Started with REPEATABLE READ isolation"),
        Err(e) => println!("     ✗ Error: {}", e),
    }

    match executor.execute(r#"FROM Accounts WHERE holder = "Alice" SELECT holder, balance"#) {
        Ok(result) => {
            println!("     ✓ First read of Alice's account:");
            for row in &result.rows {
                println!("       {:?}", row);
            }
        }
        Err(e) => println!("     ✗ Error: {}", e),
    }

    // In a real concurrent scenario, another transaction would modify this
    // For now, just demonstrate that we can read again
    match executor.execute(r#"FROM Accounts WHERE holder = "Alice" SELECT holder, balance"#) {
        Ok(result) => {
            println!("     ✓ Second read (should be same in REPEATABLE READ):");
            for row in &result.rows {
                println!("       {:?}", row);
            }
        }
        Err(e) => println!("     ✗ Error: {}", e),
    }

    match executor.execute("COMMIT") {
        Ok(_) => println!("     ✓ Committed"),
        Err(e) => println!("     ✗ Error: {}", e),
    }

    // Test Serializable
    println!("\n   Testing SERIALIZABLE:");
    match executor.execute("BEGIN TRANSACTION ISOLATION LEVEL SERIALIZABLE") {
        Ok(_) => println!("     ✓ Started with SERIALIZABLE isolation"),
        Err(e) => println!("     ✗ Error: {}", e),
    }

    match executor.execute(r#"FROM Accounts SELECT holder, balance"#) {
        Ok(result) => {
            println!("     ✓ Read {} accounts with SERIALIZABLE isolation", result.rows.len());
        }
        Err(e) => println!("     ✗ Error: {}", e),
    }

    match executor.execute("COMMIT") {
        Ok(_) => println!("     ✓ Committed"),
        Err(e) => println!("     ✗ Error: {}", e),
    }

    // Test 4: Multiple Operations in One Transaction
    println!("\n💸 Test 4: Bank Transfer (Multiple Operations)");
    println!("   Scenario: Transfer $200 from Alice to Carol");

    match executor.execute("BEGIN TRANSACTION") {
        Ok(_) => println!("   ✓ Transaction started"),
        Err(e) => println!("   ✗ Error: {}", e),
    }

    // Check initial balances
    match executor.execute(r#"FROM Accounts WHERE holder = "Alice" OR holder = "Carol" SELECT holder, balance"#) {
        Ok(result) => {
            println!("   Initial balances:");
            for row in &result.rows {
                println!("     {:?}", row);
            }
        }
        Err(e) => println!("   ✗ Error: {}", e),
    }

    // Deduct from Alice (1200 - 200 = 1000)
    match executor.execute(r#"UPDATE Accounts SET balance = 1000 WHERE holder = "Alice""#) {
        Ok(result) => println!("   ✓ Deducted $200 from Alice ({} updated)", result.rows_affected),
        Err(e) => println!("   ✗ Error: {}", e),
    }

    // Add to Carol (750 + 200 = 950)
    match executor.execute(r#"UPDATE Accounts SET balance = 950 WHERE holder = "Carol""#) {
        Ok(result) => println!("   ✓ Added $200 to Carol ({} updated)", result.rows_affected),
        Err(e) => println!("   ✗ Error: {}", e),
    }

    // Verify within transaction
    match executor.execute(r#"FROM Accounts WHERE holder = "Alice" OR holder = "Carol" SELECT holder, balance"#) {
        Ok(result) => {
            println!("   Balances within transaction:");
            for row in &result.rows {
                println!("     {:?}", row);
            }
        }
        Err(e) => println!("   ✗ Error: {}", e),
    }

    match executor.execute("COMMIT") {
        Ok(_) => println!("   ✓ Transfer committed"),
        Err(e) => println!("   ✗ Error: {}", e),
    }

    // Verify after commit
    match executor.execute(r#"FROM Accounts SELECT holder, balance"#) {
        Ok(result) => {
            println!("\n   Final account balances:");
            for row in &result.rows {
                println!("     {:?}", row);
            }
        }
        Err(e) => println!("   ✗ Error: {}", e),
    }

    println!("\n✨ Transaction & MVCC Test Complete!");
}
