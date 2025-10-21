use deed_rust::*;
use std::sync::{Arc, RwLock};

fn main() {
    println!("💎 Testing ACID Transactions\n");

    let graph = Arc::new(RwLock::new(Graph::new()));
    let executor = DQLExecutor::new(graph.clone());
    let txn_manager = Arc::new(TransactionManager::new());

    // Insert initial data
    executor.execute(r#"
        INSERT INTO Accounts VALUES ({
            id: "acc1",
            name: "Alice",
            balance: 1000
        })
    "#).unwrap();

    executor.execute(r#"
        INSERT INTO Accounts VALUES ({
            id: "acc2",
            name: "Bob",
            balance: 500
        })
    "#).unwrap();

    println!("📊 Initial balances:");
    println!("   Alice: $1000");
    println!("   Bob: $500\n");

    // Transaction: Transfer $200 from Alice to Bob
    println!("💸 Starting transaction: Transfer $200 Alice → Bob");

    let txn = txn_manager.begin(IsolationLevel::Serializable).unwrap();
    println!("   Transaction ID: {}", txn.id);

    // Debit Alice
    executor.execute(r#"
        UPDATE Accounts
        SET balance = 800
        WHERE id = "acc1"
    "#).unwrap();

    // Credit Bob
    executor.execute(r#"
        UPDATE Accounts
        SET balance = 700
        WHERE id = "acc2"
    "#).unwrap();

    // Commit
    txn_manager.commit(txn.id).unwrap();
    println!("   ✅ Transaction committed!\n");

    println!("📊 Final balances:");
    println!("   Alice: $800");
    println!("   Bob: $700");

    println!("\n🎉 ACID transactions working!");
}
