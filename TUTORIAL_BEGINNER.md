# Deed Database - Beginner's Tutorial

**Learn Deed from scratch in 30 minutes**

---

## What is Deed?

Deed is a **hybrid database** that combines:
- **Relational** features (like MySQL, PostgreSQL) - tables, rows, queries
- **Graph** features (like Neo4j) - relationships, traversals
- **Biological optimization** - learns and adapts like natural systems

**Why hybrid?** You can store structured data (users, products) AND complex relationships (friendships, purchases) in one database without JOINs!

---

## Installation (2 minutes)

### Prerequisites

You need Rust installed. If you don't have it:

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Restart terminal, then verify
rustc --version
```

### Get Deed

```bash
# Clone repository
git clone https://github.com/yourusername/Deed.git
cd Deed/deed-rust

# Test installation
cargo run --example ecommerce_demo
```

**You should see a demo running!** ✅

---

## Your First Database (5 minutes)

### Step 1: Create Your First Test File

Create `deed-rust/examples/my_first_database.rs`:

```rust
use deed_rust::*;
use std::sync::{Arc, RwLock};

fn main() {
    println!("🎓 My First Deed Database\n");

    // 1. CREATE DATABASE (in Deed, we create a graph)
    let graph = Arc::new(RwLock::new(Graph::new()));
    let executor = DQLExecutor::new(graph.clone());

    println!("✅ Database created!\n");

    // 2. CREATE TABLE (in Deed, we just insert - tables are implicit)
    println!("📝 Inserting data...");

    executor.execute(r#"
        INSERT INTO Users VALUES ({
            name: "Alice",
            age: 30,
            city: "New York"
        })
    "#).unwrap();

    executor.execute(r#"
        INSERT INTO Users VALUES ({
            name: "Bob",
            age: 25,
            city: "San Francisco"
        })
    "#).unwrap();

    println!("✅ Inserted 2 users\n");

    // 3. SELECT (Query data)
    println!("🔍 Querying data...");

    let result = executor.execute(
        "FROM Users SELECT name, age, city"
    ).unwrap();

    println!("✅ Found {} users\n", result.rows_affected);

    // 4. WHERE clause (Filter)
    println!("🔍 Filtering data...");

    let result = executor.execute(
        "FROM Users WHERE age > 25 SELECT name, city"
    ).unwrap();

    println!("✅ Found {} users over 25\n", result.rows_affected);

    // 5. Count (Aggregation)
    println!("📊 Counting users...");

    executor.execute(
        "FROM Users SELECT COUNT(*)"
    ).unwrap();

    println!("✅ Count complete\n");

    println!("🎉 Tutorial complete!");
}
```

### Step 2: Run It

```bash
cargo run --example my_first_database
```

**Expected output:**
```
🎓 My First Deed Database

✅ Database created!

📝 Inserting data...
✅ Inserted 2 users

🔍 Querying data...
✅ Found 2 users

🔍 Filtering data...
✅ Found 1 users over 25

📊 Counting users...
✅ Count complete

🎉 Tutorial complete!
```

---

## CRUD Operations (10 minutes)

### Create (INSERT)

```rust
// Single insert
executor.execute(r#"
    INSERT INTO Products VALUES ({
        name: "Laptop",
        price: 999,
        category: "Electronics"
    })
"#).unwrap();

// Multiple inserts in a transaction
executor.execute("BEGIN TRANSACTION").unwrap();

executor.execute(r#"INSERT INTO Products VALUES ({name: "Mouse", price: 29})"#).unwrap();
executor.execute(r#"INSERT INTO Products VALUES ({name: "Keyboard", price: 79})"#).unwrap();

executor.execute("COMMIT").unwrap();
```

### Read (SELECT)

```rust
// Select all
executor.execute("FROM Products SELECT name, price").unwrap();

// With WHERE
executor.execute("FROM Products WHERE price < 100 SELECT name").unwrap();

// With ORDER BY (coming soon)
// executor.execute("FROM Products SELECT name, price ORDER BY price DESC").unwrap();

// With LIMIT (coming soon)
// executor.execute("FROM Products SELECT name LIMIT 10").unwrap();
```

### Update

```rust
executor.execute(r#"
    UPDATE Products
    SET price = 899
    WHERE name = "Laptop"
"#).unwrap();
```

### Delete

```rust
// Delete specific rows
executor.execute("DELETE FROM Products WHERE price < 30").unwrap();

// Delete all (careful!)
executor.execute("DELETE FROM Products").unwrap();
```

---

## Queries: SQL vs Deed (5 minutes)

### Basic Queries

| SQL | Deed DQL |
|-----|----------|
| `SELECT * FROM Users` | `FROM Users SELECT name, age, city` |
| `SELECT name FROM Users WHERE age > 25` | `FROM Users WHERE age > 25 SELECT name` |
| `SELECT COUNT(*) FROM Users` | `FROM Users SELECT COUNT(*)` |
| `SELECT AVG(age) FROM Users` | `FROM Users SELECT AVG(age)` |

### Aggregations

| SQL | Deed DQL |
|-----|----------|
| `SELECT COUNT(*), AVG(price) FROM Products` | `FROM Products SELECT COUNT(*), AVG(price)` |
| `SELECT category, COUNT(*) FROM Products GROUP BY category` | `FROM Products SELECT category, COUNT(*) GROUP BY category` |
| `SELECT category, SUM(price) FROM Products GROUP BY category HAVING SUM(price) > 1000` | `FROM Products SELECT category, SUM(price) GROUP BY category HAVING SUM(price) > 1000` |

### Try It:

```rust
// Count users by city
executor.execute(r#"
    FROM Users
    SELECT city, COUNT(*)
    GROUP BY city
"#).unwrap();

// Average price by category
executor.execute(r#"
    FROM Products
    SELECT category, AVG(price)
    GROUP BY category
"#).unwrap();

// Categories with expensive products
executor.execute(r#"
    FROM Products
    SELECT category, AVG(price) as avg_price
    GROUP BY category
    HAVING avg_price > 500
"#).unwrap();
```

---

## Graph Features: Why Deed is Different (10 minutes)

### The Problem with SQL JOINs

**SQL approach** (complex, slow):
```sql
SELECT u.name, p.name
FROM Users u
JOIN Orders o ON u.id = o.user_id
JOIN OrderItems oi ON o.id = oi.order_id
JOIN Products p ON oi.product_id = p.id
WHERE u.name = 'Alice';
```

**Deed approach** (simple, fast):
```rust
executor.execute(r#"
    FROM Users WHERE name = 'Alice'
    TRAVERSE -[:PURCHASED]-> Products
    SELECT name
"#).unwrap();
```

### Graph Example: Social Network

```rust
// Create users
executor.execute(r#"INSERT INTO Users VALUES ({name: "Alice", city: "NYC"})"#).unwrap();
executor.execute(r#"INSERT INTO Users VALUES ({name: "Bob", city: "SF"})"#).unwrap();
executor.execute(r#"INSERT INTO Users VALUES ({name: "Carol", city: "LA"})"#).unwrap();

// Create relationships (friendships)
executor.execute("CREATE (1)-[:FOLLOWS]->(2)").unwrap();  // Alice follows Bob
executor.execute("CREATE (1)-[:FOLLOWS]->(3)").unwrap();  // Alice follows Carol
executor.execute("CREATE (2)-[:FOLLOWS]->(3)").unwrap();  // Bob follows Carol

// Find who Alice follows
executor.execute(r#"
    FROM Users WHERE name = 'Alice'
    TRAVERSE -[:FOLLOWS]-> Users
    SELECT name
"#).unwrap();
// Result: Bob, Carol

// Find Alice's followers
executor.execute(r#"
    FROM Users WHERE name = 'Alice'
    TRAVERSE <-[:FOLLOWS]- Users
    SELECT name
"#).unwrap();
// Result: (none in this example)

// Find friends of friends (2 hops)
executor.execute(r#"
    FROM Users WHERE name = 'Alice'
    TRAVERSE -[:FOLLOWS]-> Users
    TRAVERSE -[:FOLLOWS]-> Users
    SELECT name
"#).unwrap();
// Result: Carol (through Bob)
```

### Graph vs Relational

| Task | Relational (SQL) | Graph (Deed) |
|------|------------------|--------------|
| Store user data | ✅ Easy (tables) | ✅ Easy (collections) |
| Find direct friends | ⚠️ JOIN needed | ✅ One TRAVERSE |
| Friends of friends | ❌ Multiple JOINs, slow | ✅ Easy, fast |
| Recommendation engine | ❌ Very complex | ✅ Natural fit |
| Analytics on data | ✅ Excellent (GROUP BY) | ✅ Excellent (same syntax) |

---

## Complete Example: E-Commerce (20 minutes)

Create `deed-rust/examples/learn_ecommerce.rs`:

```rust
use deed_rust::*;
use std::sync::{Arc, RwLock};

fn main() {
    println!("🛒 E-Commerce Tutorial\n");

    let graph = Arc::new(RwLock::new(Graph::new()));
    let executor = DQLExecutor::new(graph.clone());

    // Step 1: Create users
    println!("👥 Creating users...");
    executor.execute(r#"INSERT INTO Users VALUES ({name: "Alice", email: "alice@example.com"})"#).unwrap();
    executor.execute(r#"INSERT INTO Users VALUES ({name: "Bob", email: "bob@example.com"})"#).unwrap();
    println!("✅ Created 2 users\n");

    // Step 2: Create products
    println!("📦 Creating products...");
    executor.execute(r#"INSERT INTO Products VALUES ({name: "Laptop", price: 999, stock: 10})"#).unwrap();
    executor.execute(r#"INSERT INTO Products VALUES ({name: "Mouse", price: 29, stock: 50})"#).unwrap();
    executor.execute(r#"INSERT INTO Products VALUES ({name: "Keyboard", price: 79, stock: 30})"#).unwrap();
    println!("✅ Created 3 products\n");

    // Step 3: Create orders
    println!("🛍️  Creating orders...");
    executor.execute(r#"INSERT INTO Orders VALUES ({user_id: 1, total: 1028, status: "shipped"})"#).unwrap();
    executor.execute(r#"INSERT INTO Orders VALUES ({user_id: 2, total: 108, status: "pending"})"#).unwrap();
    println!("✅ Created 2 orders\n");

    // Step 4: Query - Find cheap products
    println!("💰 Finding products under $100...");
    executor.execute("FROM Products WHERE price < 100 SELECT name, price").unwrap();
    println!();

    // Step 5: Query - Count orders by status
    println!("📊 Counting orders by status...");
    executor.execute("FROM Orders SELECT status, COUNT(*) GROUP BY status").unwrap();
    println!();

    // Step 6: Query - Calculate total revenue
    println!("💵 Calculating total revenue...");
    executor.execute("FROM Orders SELECT SUM(total)").unwrap();
    println!();

    // Step 7: Create purchase relationships
    println!("🔗 Creating purchase relationships...");
    executor.execute("CREATE (1)-[:PURCHASED]->(1)").unwrap();  // Alice bought Laptop
    executor.execute("CREATE (1)-[:PURCHASED]->(2)").unwrap();  // Alice bought Mouse
    executor.execute("CREATE (2)-[:PURCHASED]->(2)").unwrap();  // Bob bought Mouse
    executor.execute("CREATE (2)-[:PURCHASED]->(3)").unwrap();  // Bob bought Keyboard
    println!("✅ Created purchase relationships\n");

    // Step 8: Graph query - What did Alice buy?
    println!("🔍 What did Alice buy?");
    executor.execute(r#"
        FROM Users WHERE name = 'Alice'
        TRAVERSE -[:PURCHASED]-> Products
        SELECT name, price
    "#).unwrap();
    println!();

    // Step 9: Graph query - Who bought the Mouse?
    println!("🔍 Who bought the Mouse?");
    executor.execute(r#"
        FROM Products WHERE name = 'Mouse'
        TRAVERSE <-[:PURCHASED]- Users
        SELECT name, email
    "#).unwrap();
    println!();

    println!("🎉 E-Commerce tutorial complete!");
    println!("\n💡 Key takeaways:");
    println!("   • Deed stores data like SQL (Users, Products, Orders)");
    println!("   • Deed queries like SQL (WHERE, GROUP BY, aggregations)");
    println!("   • Deed traverses like Neo4j (TRAVERSE relationships)");
    println!("   • No JOINs needed - relationships are first-class!");
}
```

Run it:
```bash
cargo run --example learn_ecommerce
```

---

## Next Steps

### 1. Learn Advanced Features

```bash
# Run the full-featured demo
cargo run --example ecommerce_demo
```

This shows:
- Authentication
- Connection pooling
- Transactions
- Indexes
- Backups
- Admin dashboard

### 2. Read the Guides

- `RUST_QUICKSTART.md` - Quick testing guide
- `PRODUCTION_FEATURES_GUIDE.md` - All production features
- `HIGH_AVAILABILITY_GUIDE.md` - Replication, backups

### 3. Try Your Own Ideas

Create your own examples:
- Blog system (posts, comments, tags)
- Recommendation engine (users, products, likes)
- Knowledge graph (concepts, relationships)

---

## Common Questions

**Q: Do I need to create tables?**
No! Just INSERT INTO a collection name and it's created automatically.

**Q: How do I see what's in the database?**
```rust
let result = executor.execute("FROM Users SELECT name").unwrap();
```

**Q: Can I use SQL?**
DQL is very similar to SQL! Most SQL knowledge transfers directly.

**Q: What about JOINs?**
Use TRAVERSE instead - it's simpler and faster for relationships.

**Q: Is it production-ready?**
Yes! It has transactions, indexes, authentication, replication, backups.

**Q: How is it different from PostgreSQL?**
PostgreSQL = relational only. Deed = relational + graph + biological optimization.

**Q: How is it different from Neo4j?**
Neo4j = graph only. Deed = graph + relational + SQL-like queries.

---

## Quick Reference

### DQL Cheat Sheet

```rust
// INSERT
INSERT INTO Users VALUES ({name: "Alice", age: 30})

// SELECT
FROM Users SELECT name, age
FROM Users WHERE age > 25 SELECT name

// AGGREGATIONS
FROM Users SELECT COUNT(*)
FROM Users SELECT AVG(age)
FROM Orders SELECT category, SUM(price) GROUP BY category

// UPDATE
UPDATE Users SET age = 31 WHERE name = "Alice"

// DELETE
DELETE FROM Users WHERE age < 18

// GRAPH TRAVERSAL
FROM Users WHERE name = 'Alice' TRAVERSE -[:FOLLOWS]-> Users SELECT name

// TRANSACTIONS
BEGIN TRANSACTION
INSERT INTO Users VALUES ({name: "Bob"})
COMMIT

// INDEXES
CREATE INDEX idx_age ON Users(age)
DROP INDEX idx_age
```

---

**Congratulations! You now know how to use Deed!** 🎉

Try building something and see how it compares to traditional databases.
