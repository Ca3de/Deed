# Getting Started with Deed on macOS

Complete walkthrough from installation to running your first distributed database.

---

## Part 1: Prerequisites & Installation (5 minutes)

### Step 1: Install Rust

```bash
# Install Rust using rustup
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Follow prompts (default installation is fine)
# Then reload your shell
source $HOME/.cargo/env

# Verify installation
rustc --version  # Should show: rustc 1.70+ or higher
cargo --version  # Should show: cargo 1.70+ or higher
```

### Step 2: Clone Deed Repository

```bash
# Clone the repository
git clone https://github.com/yourusername/Deed.git
cd Deed

# Check directory structure
ls -la
# You should see:
#   deed-rust/     <- Rust core engine
#   examples/      <- Python examples
#   README.md
```

### Step 3: Build the Rust Core

```bash
cd deed-rust

# Build in release mode (optimized)
cargo build --release

# This will:
# - Download dependencies
# - Compile the Rust code
# - Create optimized binaries
# Takes 2-5 minutes on first build
```

**Expected output:**
```
   Compiling deed-rust v0.1.0
   ...
   Finished release [optimized] target(s) in 2m 34s
```

---

## Part 2: Your First Deed Database (10 minutes)

### Example 1: Simple In-Memory Database

Create a new file: `deed-rust/examples/my_first_deed.rs`

```rust
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
```

### Run it:

```bash
cargo run --example my_first_deed
```

**Expected output:**
```
🎉 My First Deed Database!

📝 Inserting data...
✅ Inserted 3 users

🔍 Query 1: All users in San Francisco
   Result: 2 rows affected

🔍 Query 2: Users older than 25
   Result: 2 rows affected

🔍 Query 3: Count users by city
   Result: 2 rows affected

📝 Updating Bob's age...
✅ Updated

🗑️  Deleting users over 30...
✅ Deleted

🔍 Final query: All remaining users
   Remaining: 2 users

🎉 Success! Your first Deed database is working!
```

---

## Part 3: Production Features Demo (15 minutes)

### Example 2: Database with ACID Transactions

Create: `deed-rust/examples/test_transactions.rs`

```rust
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
```

Run it:
```bash
cargo run --example test_transactions
```

---

## Part 4: REST API Server (20 minutes)

### Step 1: Start the REST API Server

```bash
# In one terminal
cd deed-rust
cargo run --example rest_api_server
```

**Expected output:**
```
🚀 Deed REST API Server v0.2.0
================================

📊 Initializing database...
👥 Creating demo users...
   ✓ admin / admin123 (Admin)
   ✓ user / user123 (ReadWrite)

📝 Loading sample data...
   ✓ 2 users inserted
   ✓ 2 products inserted

🌐 Server running at http://localhost:8080

📡 API Endpoints:
   POST http://localhost:8080/api/login
   POST http://localhost:8080/api/query
   POST http://localhost:8080/api/logout

⏳ Server starting...
```

**Keep this running!** Open a new terminal for the next steps.

### Step 2: Test with curl

```bash
# Terminal 2: Test API calls

# 1. Login
curl -X POST http://localhost:8080/api/login \
  -H "Content-Type: application/json" \
  -d '{"username": "admin", "password": "admin123"}'

# Response:
# {"success":true,"session_id":"abc123...","message":"Successfully logged in as admin"}

# Copy the session_id for next commands
export SESSION_ID="<paste_your_session_id_here>"

# 2. Query data
curl -X POST http://localhost:8080/api/query \
  -H "Content-Type: application/json" \
  -d "{\"session_id\": \"$SESSION_ID\", \"query\": \"FROM Users SELECT name\"}"

# 3. Insert data
curl -X POST http://localhost:8080/api/query \
  -H "Content-Type: application/json" \
  -d "{\"session_id\": \"$SESSION_ID\", \"query\": \"INSERT INTO Users VALUES ({name: \\\"Carol\\\", age: 28})\"}"

# 4. Logout
curl -X POST http://localhost:8080/api/logout \
  -H "Content-Type: application/json" \
  -d "{\"session_id\": \"$SESSION_ID\"}"
```

### Step 3: Test with Python Client

```bash
# Install Python dependency
pip install requests

# Run Python client
cd deed-rust/examples
python python_client.py
```

**Expected output:**
```
============================================================
Deed Database - Python Client Demo
============================================================

1. Connecting to database...
✅ Connected: Successfully logged in as admin

2. Querying existing data...
   Result: {"success": true, "rows_affected": 1, ...}

3. Inserting new user...
   Insert success: True

... (continues)

✅ Demo completed successfully!
```

---

## Part 5: Distributed Database Demo (30 minutes)

### Run the Full Distributed Demo

```bash
cd deed-rust
cargo run --example distributed_database_demo
```

**Expected output:**
```
╔════════════════════════════════════════════════════════════╗
║  Deed Distributed Database Demo                           ║
║  Biologically-Inspired Distributed Systems                ║
╚════════════════════════════════════════════════════════════╝

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
STEP 1: Building Small-World Network Topology
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Creating network with 5 nodes:

  Node 1 connections:
    Local connections: 4
    Long-range connections: 2
    Connected to: 2 3 4 5

  Network Statistics:
    Total nodes: 5
    Average path length: 1.40
    ✅ Small-world topology established!

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
STEP 2: Configuring Shard Assignment & Consistent Hashing
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Shard Configuration:
  Total shards: 64
  Replication factor: 3
  Virtual nodes per node: 150

Adding nodes to shard manager:
  ✓ Node 1 registered
  ✓ Node 2 registered
  ✓ Node 3 registered
  ✓ Node 4 registered
  ✓ Node 5 registered

Shard Distribution:
  Node 1: 38 shards (primary + replicas)
  Node 2: 39 shards (primary + replicas)
  Node 3: 37 shards (primary + replicas)
  Node 4: 40 shards (primary + replicas)
  Node 5: 38 shards (primary + replicas)

... (continues with all 7 steps)

🎉 Distributed Database Demo Complete!
```

---

## Part 6: Quick Reference

### Common Commands

```bash
# Build everything
cargo build --release

# Run tests
cargo test

# Run specific example
cargo run --example <example_name>

# Available examples:
cargo run --example my_first_deed
cargo run --example test_transactions
cargo run --example rest_api_server
cargo run --example distributed_database_demo
cargo run --example ecommerce_demo
cargo run --example python_client.py  # (Python)
```

### DQL Quick Reference

```dql
-- Insert
INSERT INTO Users VALUES ({name: "Alice", age: 30})

-- Query
FROM Users WHERE age > 25 SELECT name, age

-- Update
UPDATE Users SET age = 31 WHERE name = "Alice"

-- Delete
DELETE FROM Users WHERE age < 18

-- Aggregation
FROM Users SELECT city, COUNT(*) GROUP BY city

-- Graph traversal
FROM Users WHERE id = 1
TRAVERSE -[:FOLLOWS]-> Users
SELECT name
```

---

## Troubleshooting

### Issue: Cargo not found
```bash
# Solution: Add to PATH
echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> ~/.zshrc
source ~/.zshrc
```

### Issue: Build fails
```bash
# Solution: Update Rust
rustup update

# Clean and rebuild
cargo clean
cargo build --release
```

### Issue: Port already in use
```bash
# Solution: Kill process on port 8080
lsof -ti:8080 | xargs kill -9

# Or change port in rest_api_server.rs
# Line 213: Change 8080 to another port
```

---

## Next Steps

1. ✅ **You just created your first Deed database!**
2. 📖 Read [TUTORIAL_BEGINNER.md](TUTORIAL_BEGINNER.md) for in-depth guide
3. 🏢 Read [TUTORIAL_VETERAN.md](TUTORIAL_VETERAN.md) for advanced features
4. 🔍 Explore [COMPARISON_GUIDE.md](COMPARISON_GUIDE.md) vs PostgreSQL/Neo4j
5. 🚀 Check [ROADMAP.md](ROADMAP.md) for future features

---

## Summary - What You Learned

✅ **Installed Rust and built Deed**
✅ **Created your first database**
✅ **Executed CRUD operations with DQL**
✅ **Tested ACID transactions**
✅ **Used REST API from curl and Python**
✅ **Saw distributed features in action**

**You're now ready to build with Deed!** 🎉
