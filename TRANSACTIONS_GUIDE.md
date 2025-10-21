# Transaction Usage Guide - Deed Database

## Overview

Deed now supports full ACID-compliant transactions with multiple isolation levels, Write-Ahead Logging for durability, and Multi-Version Concurrency Control (MVCC) for high-performance concurrent access.

## Quick Start

### Basic Transaction

```dql
BEGIN TRANSACTION;

INSERT INTO Users VALUES ({name: "Alice", age: 30, email: "alice@example.com"});
INSERT INTO Users VALUES ({name: "Bob", age: 25, email: "bob@example.com"});

COMMIT;
```

All operations between BEGIN and COMMIT are atomic - they all succeed or all fail together.

### Rollback on Error

```dql
BEGIN TRANSACTION;

INSERT INTO Accounts VALUES ({id: 1, balance: 1000});
UPDATE Accounts SET balance = balance - 100 WHERE id = 1;

-- Oh no, something went wrong!
ROLLBACK;
```

ROLLBACK undoes all changes made since BEGIN.

## Transaction Commands

### BEGIN TRANSACTION

Start a new transaction with optional isolation level:

```dql
BEGIN TRANSACTION;
BEGIN TRANSACTION ISOLATION LEVEL READ COMMITTED;
BEGIN TRANSACTION ISOLATION LEVEL REPEATABLE READ;
BEGIN TRANSACTION ISOLATION LEVEL SERIALIZABLE;
```

**Isolation Levels:**
- `READ UNCOMMITTED` - Fastest but allows dirty reads
- `READ COMMITTED` - No dirty reads (common default)
- `REPEATABLE READ` - No dirty reads, no non-repeatable reads (Deed's default)
- `SERIALIZABLE` - Full isolation, prevents all anomalies

### COMMIT

Permanently save all changes made in the transaction:

```dql
COMMIT;
```

Only after COMMIT are changes visible to other transactions and durable (survive crashes).

### ROLLBACK

Undo all changes made in the transaction:

```dql
ROLLBACK;
```

Returns database to state before BEGIN.

## Auto-Commit

Single-statement mutations automatically commit:

```dql
-- This automatically commits
INSERT INTO Users VALUES ({name: "Charlie", age: 35});

-- Equivalent to:
BEGIN TRANSACTION;
INSERT INTO Users VALUES ({name: "Charlie", age: 35});
COMMIT;
```

**Auto-commit only applies to:**
- INSERT
- UPDATE
- DELETE
- CREATE (edges)

**SELECT queries don't use transactions** (they're read-only).

## Common Use Cases

### 1. Bank Transfer (Classic ACID Example)

```dql
BEGIN TRANSACTION ISOLATION LEVEL SERIALIZABLE;

-- Debit from account 1
UPDATE Accounts
SET balance = balance - 100
WHERE id = 1 AND balance >= 100;

-- Credit to account 2
UPDATE Accounts
SET balance = balance + 100
WHERE id = 2;

-- Record transaction
INSERT INTO Transactions VALUES ({
    from_account: 1,
    to_account: 2,
    amount: 100,
    timestamp: NOW()
});

COMMIT;
```

**Why SERIALIZABLE?** Prevents another transaction from reading inconsistent state (account 1 debited but account 2 not yet credited).

### 2. E-Commerce Order Processing

```dql
BEGIN TRANSACTION ISOLATION LEVEL REPEATABLE READ;

-- Create order
INSERT INTO Orders VALUES ({
    customer_id: 42,
    total: 299.99,
    status: "pending"
});

-- Reserve inventory
UPDATE Products
SET stock = stock - 1
WHERE id = 100 AND stock > 0;

-- If stock update failed (stock was 0), rollback
-- Otherwise commit

COMMIT;
```

### 3. User Registration with Related Data

```dql
BEGIN TRANSACTION;

-- Create user
INSERT INTO Users VALUES ({
    name: "Alice",
    email: "alice@example.com",
    role: "customer"
});

-- Create user profile
INSERT INTO Profiles VALUES ({
    user_id: LAST_INSERT_ID,
    bio: "Software engineer",
    location: "San Francisco"
});

-- Create initial settings
INSERT INTO Settings VALUES ({
    user_id: LAST_INSERT_ID,
    theme: "dark",
    notifications: true
});

COMMIT;
```

### 4. Bulk Data Import

```dql
BEGIN TRANSACTION;

-- Import 1000 users
INSERT INTO Users VALUES ({name: "User1", age: 20});
INSERT INTO Users VALUES ({name: "User2", age: 21});
-- ... 998 more inserts ...

COMMIT;
```

**Benefits:**
- All 1000 users inserted atomically
- Much faster than 1000 individual auto-commits
- If any insert fails, all are rolled back

### 5. Graph Operations with Transactions

```dql
BEGIN TRANSACTION;

-- Create user nodes
INSERT INTO Users VALUES ({name: "Alice"});
INSERT INTO Users VALUES ({name: "Bob"});

-- Create friendship edge
CREATE (alice) -[:FRIEND_OF]-> (bob) {since: "2024"};

-- Update user stats
UPDATE Users SET friend_count = friend_count + 1 WHERE name = "Alice";
UPDATE Users SET friend_count = friend_count + 1 WHERE name = "Bob";

COMMIT;
```

## Isolation Levels Explained

### READ UNCOMMITTED

**Allows:** Dirty reads, non-repeatable reads, phantom reads
**Use when:** Performance is critical, approximate results are acceptable
**Example:** Real-time dashboards, analytics

```dql
BEGIN TRANSACTION ISOLATION LEVEL READ UNCOMMITTED;
-- Might see uncommitted changes from other transactions
SELECT COUNT(*) FROM Users;
COMMIT;
```

### READ COMMITTED

**Prevents:** Dirty reads
**Allows:** Non-repeatable reads, phantom reads
**Use when:** Default for most applications
**Example:** Web applications, APIs

```dql
BEGIN TRANSACTION ISOLATION LEVEL READ COMMITTED;
-- Only sees committed changes
SELECT * FROM Accounts WHERE id = 1;
-- Another transaction might commit changes here
SELECT * FROM Accounts WHERE id = 1;  -- Might see different data!
COMMIT;
```

### REPEATABLE READ (Default in Deed)

**Prevents:** Dirty reads, non-repeatable reads
**Allows:** Phantom reads
**Use when:** Need consistent reads within transaction
**Example:** Reports, batch processing

```dql
BEGIN TRANSACTION ISOLATION LEVEL REPEATABLE READ;
SELECT * FROM Accounts WHERE id = 1;  -- Reads version X
-- Another transaction updates and commits
SELECT * FROM Accounts WHERE id = 1;  -- Still reads version X (snapshot)
COMMIT;
```

**How it works:** Uses MVCC to provide a snapshot of database at transaction start.

### SERIALIZABLE

**Prevents:** All anomalies
**Use when:** Absolute correctness required
**Example:** Financial transactions, inventory management

```dql
BEGIN TRANSACTION ISOLATION LEVEL SERIALIZABLE;
-- Full isolation - as if transactions ran one at a time
SELECT COUNT(*) FROM Products WHERE category = "Electronics";
INSERT INTO Products VALUES ({name: "Laptop", category: "Electronics"});
COMMIT;
```

## MVCC (Multi-Version Concurrency Control)

Deed uses MVCC for high-performance concurrency:

### How MVCC Works

```
Time →

T1: BEGIN  ─────────────────> SELECT * FROM Users  ─────> COMMIT
                                  ↓
                              Sees snapshot
                              at T1 start

T2:        BEGIN ──> UPDATE Users ──> COMMIT
                        ↓
                    Creates new
                    version
```

**Key Points:**
- Readers don't block writers
- Writers don't block readers
- Each transaction sees a consistent snapshot
- Old versions garbage collected when no longer needed

### Version Storage

Each entity can have multiple versions:

```
Entity ID 1:
  Version 1: {name: "Alice", age: 30}  [Created by Txn 1]
  Version 2: {name: "Alice", age: 31}  [Created by Txn 5]
  Version 3: {name: "Alice", age: 32}  [Created by Txn 9]
```

Transaction 7 sees Version 2 (created by Txn 5, before Txn 7 started).

## Write-Ahead Log (WAL)

WAL ensures durability - committed transactions survive crashes.

### WAL Guarantees

```dql
BEGIN TRANSACTION;
INSERT INTO Users VALUES ({name: "Alice"});
COMMIT;  -- ← WAL is fsynced here

-- CRASH! Power loss, system failure, etc.

-- After recovery:
SELECT * FROM Users WHERE name = "Alice";  -- ✓ Alice is there!
```

### Recovery Process

On startup after a crash:
1. Read WAL file
2. Replay all committed transactions
3. Abort any incomplete transactions
4. Database is restored to consistent state

### WAL Entry Example

```
Entry 1: BeginTransaction  {txn_id: 1, isolation: RepeatableRead}
Entry 2: InsertEntity      {txn_id: 1, entity_id: 100, ...}
Entry 3: UpdateEntity      {txn_id: 1, entity_id: 50, ...}
Entry 4: Commit            {txn_id: 1}  ← fsync happens here
```

## Error Handling

### Automatic Rollback on Error

```dql
BEGIN TRANSACTION;

INSERT INTO Users VALUES ({name: "Alice"});
INSERT INTO Users VALUES ({name: "Bob"});
INSERT INTO Users VALUES ({invalid syntax});  -- Error!

-- Transaction automatically rolled back
-- Neither Alice nor Bob are inserted
```

### Manual Rollback

```dql
BEGIN TRANSACTION;

INSERT INTO Orders VALUES ({customer_id: 1, total: 100});

-- Check inventory
SELECT stock FROM Products WHERE id = 5;

-- If stock is insufficient, rollback
ROLLBACK;
```

### Handling Conflicts

```dql
-- Transaction 1
BEGIN TRANSACTION ISOLATION LEVEL SERIALIZABLE;
UPDATE Accounts SET balance = balance - 100 WHERE id = 1;
-- ... takes time ...
COMMIT;  -- Might fail if Transaction 2 committed first!

-- Transaction 2 (concurrent)
BEGIN TRANSACTION ISOLATION LEVEL SERIALIZABLE;
UPDATE Accounts SET balance = balance + 50 WHERE id = 1;
COMMIT;  -- Might succeed and cause Transaction 1 to fail
```

**Conflict Resolution:**
- Deed uses optimistic concurrency control
- Transaction with conflict is aborted
- Application should retry

## Performance Tips

### 1. Keep Transactions Short

```dql
-- ❌ Bad: Long-running transaction
BEGIN TRANSACTION;
SELECT * FROM huge_table;  -- Scans millions of rows
-- ... process data in application ...
UPDATE summary SET count = 12345;
COMMIT;

-- ✓ Good: Short transaction
SELECT * FROM huge_table;  -- Outside transaction
-- ... process data in application ...
BEGIN TRANSACTION;
UPDATE summary SET count = 12345;
COMMIT;
```

### 2. Use Appropriate Isolation Level

```dql
-- ❌ Overkill for analytics
BEGIN TRANSACTION ISOLATION LEVEL SERIALIZABLE;
SELECT AVG(price) FROM Products;
COMMIT;

-- ✓ Better for analytics
BEGIN TRANSACTION ISOLATION LEVEL READ UNCOMMITTED;
SELECT AVG(price) FROM Products;
COMMIT;
```

### 3. Batch Inserts

```dql
-- ❌ Slow: 1000 transactions
INSERT INTO Users VALUES ({name: "User1"});  -- Auto-commit
INSERT INTO Users VALUES ({name: "User2"});  -- Auto-commit
-- ... 998 more ...

-- ✓ Fast: 1 transaction
BEGIN TRANSACTION;
INSERT INTO Users VALUES ({name: "User1"});
INSERT INTO Users VALUES ({name: "User2"});
-- ... 998 more ...
COMMIT;
```

### 4. Read-Only Queries Don't Need Transactions

```dql
-- ❌ Unnecessary
BEGIN TRANSACTION;
SELECT * FROM Users;
COMMIT;

-- ✓ Better
SELECT * FROM Users;
```

## Advanced Patterns

### Savepoints (Future Feature)

```dql
-- Not yet implemented, but planned
BEGIN TRANSACTION;
INSERT INTO Users VALUES ({name: "Alice"});
SAVEPOINT sp1;
INSERT INTO Users VALUES ({name: "Bob"});
ROLLBACK TO sp1;  -- Bob not inserted, Alice still is
COMMIT;  -- Only Alice committed
```

### Distributed Transactions (Future Feature)

```dql
-- Not yet implemented, but planned
BEGIN DISTRIBUTED TRANSACTION;
-- Operations across multiple Deed instances
COMMIT;
```

## Troubleshooting

### "Already in a transaction"

```dql
BEGIN TRANSACTION;
BEGIN TRANSACTION;  -- Error!
```

**Solution:** COMMIT or ROLLBACK the current transaction first.

### "No active transaction to commit"

```dql
COMMIT;  -- Error! No BEGIN
```

**Solution:** Use BEGIN TRANSACTION before COMMIT.

### Transaction Deadlock

```
T1: UPDATE A...  ──────────> UPDATE B...  ← Waiting for T2
                                ↑
T2: UPDATE B...  ──────────> UPDATE A...  ← Waiting for T1
```

**Detection:** Deed will detect and abort one transaction
**Solution:** Retry the aborted transaction

## Comparison with Other Databases

### PostgreSQL

```sql
-- PostgreSQL
BEGIN;
INSERT INTO users VALUES ('Alice', 30);
COMMIT;
```

```dql
-- Deed (same semantics!)
BEGIN TRANSACTION;
INSERT INTO Users VALUES ({name: "Alice", age: 30});
COMMIT;
```

### MongoDB

```javascript
// MongoDB
session.startTransaction();
db.users.insertOne({name: "Alice", age: 30});
session.commitTransaction();
```

```dql
-- Deed
BEGIN TRANSACTION;
INSERT INTO Users VALUES ({name: "Alice", age: 30});
COMMIT;
```

## Monitoring and Debugging

### Check Active Transactions

```dql
-- Future feature
SELECT * FROM system.transactions WHERE state = 'active';
```

### WAL Stats

```dql
-- Future feature
SELECT * FROM system.wal_stats;
```

## Best Practices Summary

✅ **DO:**
- Use transactions for all related writes
- Keep transactions short
- Choose appropriate isolation level
- Handle rollback gracefully
- Batch inserts when possible

❌ **DON'T:**
- Leave transactions open indefinitely
- Use SERIALIZABLE for read-only queries
- Ignore transaction errors
- Mix transactional and non-transactional code

## What's Next

**Currently Supported:**
- ✅ BEGIN/COMMIT/ROLLBACK
- ✅ 4 isolation levels
- ✅ Auto-commit
- ✅ MVCC snapshot isolation
- ✅ WAL durability
- ✅ Crash recovery

**Coming Soon:**
- Savepoints
- Distributed transactions
- Transaction monitoring
- Performance metrics
- Deadlock detection

---

**Generated:** 2025-10-21
**Deed Version:** 0.1.0 (Pre-production)
**Transaction System:** v1.0
