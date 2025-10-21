# Schema & Transaction Design

## Overview

This document designs two critical database features for Deed:
1. **Optional Schema System** - Type safety and validation
2. **Transaction System** - ACID guarantees

---

## Part 1: Schema System

### Philosophy: Optional Schemas

Deed supports **both** schema-less and schema-enforced collections:

```dql
-- Schema-less (flexible, like MongoDB)
INSERT INTO FlexibleData VALUES ({anything: 'goes', here: 42})

-- Schema-enforced (validated, like PostgreSQL)
CREATE SCHEMA Users {
    name: String NOT NULL,
    age: Integer CHECK (age >= 0 AND age <= 150),
    email: String UNIQUE,
    created_at: Timestamp DEFAULT NOW(),
}

INSERT INTO Users VALUES ({name: 'Alice', age: 28, email: 'alice@example.com'})
-- ✓ Validated against schema

INSERT INTO Users VALUES ({name: 'Bob', age: -5})
-- ✗ Error: age CHECK constraint failed
```

### Schema Definition Syntax

```dql
CREATE SCHEMA <collection_name> {
    <property>: <type> [constraints...],
    <property>: <type> [constraints...],
    ...
}
```

#### Supported Types

```
String      - UTF-8 text
Integer     - i64 signed integer
Float       - f64 floating point
Boolean     - true/false
Timestamp   - ISO 8601 datetime
Bytes       - Binary data
Json        - Nested JSON object
Array<T>    - Array of type T
```

#### Supported Constraints

```
NOT NULL              - Field is required
UNIQUE                - Value must be unique in collection
DEFAULT <value>       - Default value if not provided
CHECK (<condition>)   - Custom validation
PRIMARY KEY           - Auto-indexed, NOT NULL, UNIQUE
INDEX                 - Create index on field
```

### Schema Examples

**User Schema:**
```dql
CREATE SCHEMA Users {
    id: Integer PRIMARY KEY,
    name: String NOT NULL,
    email: String UNIQUE NOT NULL,
    age: Integer CHECK (age >= 18 AND age <= 120),
    is_active: Boolean DEFAULT true,
    created_at: Timestamp DEFAULT NOW(),
    role: String CHECK (role IN ('admin', 'user', 'guest')),
}
```

**Product Schema:**
```dql
CREATE SCHEMA Products {
    id: Integer PRIMARY KEY,
    name: String NOT NULL,
    price: Float CHECK (price > 0),
    stock: Integer CHECK (stock >= 0) DEFAULT 0,
    tags: Array<String>,
    metadata: Json,
}
```

**Hybrid - Some properties optional:**
```dql
CREATE SCHEMA FlexibleUsers {
    id: Integer PRIMARY KEY,
    name: String NOT NULL,
    -- All other properties allowed but not validated
} ALLOW_EXTRA_PROPERTIES
```

### Schema Operations

**Create Schema:**
```dql
CREATE SCHEMA Users { ... }
```

**Modify Schema (Add field):**
```dql
ALTER SCHEMA Users ADD phone: String
```

**Modify Schema (Drop field):**
```dql
ALTER SCHEMA Users DROP phone
```

**Drop Schema (make collection schema-less):**
```dql
DROP SCHEMA Users
```

**Describe Schema:**
```dql
DESCRIBE Users
-- Returns schema definition
```

### Validation Behavior

**On INSERT:**
```dql
INSERT INTO Users VALUES ({name: 'Alice', age: 28, email: 'alice@example.com'})

Validation:
1. Check all NOT NULL fields present
2. Apply DEFAULT values for missing fields
3. Validate types match schema
4. Check UNIQUE constraints (index lookup)
5. Evaluate CHECK constraints
6. Insert if all pass, error otherwise
```

**On UPDATE:**
```dql
UPDATE Users SET age = -5 WHERE name = 'Alice'

Validation:
1. Validate new value type
2. Check CHECK constraints
3. Check UNIQUE constraints (if updating unique field)
4. Update if pass, error otherwise
```

**On SELECT:**
No validation (read-only)

### Schema Storage

**Schema Metadata Collection:**
```rust
// Internal collection: _schemas
{
    collection_name: "Users",
    schema: {
        fields: [
            {name: "id", type: "Integer", constraints: ["PRIMARY KEY"]},
            {name: "name", type: "String", constraints: ["NOT NULL"]},
            {name: "email", type: "String", constraints: ["UNIQUE", "NOT NULL"]},
            {name: "age", type: "Integer", check: "age >= 18 AND age <= 120"},
        ],
        allow_extra: false,
    }
}
```

### Implementation Plan

```rust
// deed-rust/src/schema.rs

pub struct Schema {
    pub collection: String,
    pub fields: Vec<Field>,
    pub allow_extra_properties: bool,
}

pub struct Field {
    pub name: String,
    pub field_type: FieldType,
    pub constraints: Vec<Constraint>,
}

pub enum FieldType {
    String,
    Integer,
    Float,
    Boolean,
    Timestamp,
    Bytes,
    Json,
    Array(Box<FieldType>),
}

pub enum Constraint {
    NotNull,
    Unique,
    Default(PropertyValue),
    Check(String),  // Expression to evaluate
    PrimaryKey,
    Index,
}

pub struct SchemaValidator {
    schemas: HashMap<String, Schema>,
}

impl SchemaValidator {
    pub fn validate_insert(&self, collection: &str, properties: &Properties) -> Result<(), ValidationError>;
    pub fn validate_update(&self, collection: &str, updates: &HashMap<String, PropertyValue>) -> Result<(), ValidationError>;
    pub fn apply_defaults(&self, collection: &str, properties: &mut Properties);
}
```

---

## Part 2: Transaction System

### Philosophy: ACID Transactions

Deed supports **full ACID transactions** for multi-operation consistency:

```dql
BEGIN TRANSACTION;

-- Transfer money between accounts
UPDATE Accounts SET balance = balance - 100 WHERE id = 1;
UPDATE Accounts SET balance = balance + 100 WHERE id = 2;

-- Both succeed or both fail
COMMIT;
```

### Transaction Syntax

**Begin Transaction:**
```dql
BEGIN TRANSACTION;
-- or
BEGIN;
```

**Commit Transaction:**
```dql
COMMIT;
```

**Rollback Transaction:**
```dql
ROLLBACK;
```

**Savepoints (advanced):**
```dql
BEGIN;
INSERT INTO Users VALUES ({name: 'Alice'});
SAVEPOINT sp1;
INSERT INTO Users VALUES ({name: 'Bob'});
ROLLBACK TO sp1;  -- Bob insert undone
COMMIT;           -- Alice insert committed
```

### ACID Properties

**Atomicity**: All operations succeed or all fail
```dql
BEGIN;
UPDATE Accounts SET balance = balance - 100 WHERE id = 1;  -- Succeeds
UPDATE Accounts SET balance = balance + 100 WHERE id = 999; -- Fails (not found)
ROLLBACK; -- First update also rolled back
```

**Consistency**: Database moves from valid state to valid state
```dql
BEGIN;
-- Schema validation enforced
-- CHECK constraints enforced
-- UNIQUE constraints enforced
COMMIT; -- Only if all constraints satisfied
```

**Isolation**: Concurrent transactions don't interfere
```dql
Transaction 1:        Transaction 2:
BEGIN;                BEGIN;
SELECT balance        SELECT balance
  FROM Accounts         FROM Accounts
  WHERE id = 1;         WHERE id = 1;
-- Sees: 100          -- Sees: 100 (isolated)
UPDATE Accounts       UPDATE Accounts
  SET balance = 50      SET balance = 75
  WHERE id = 1;         WHERE id = 1;
COMMIT;               COMMIT;
-- Final: 75 (last commit wins, or error based on isolation level)
```

**Durability**: Committed changes persist even after crash
```dql
BEGIN;
INSERT INTO Users VALUES ({name: 'Alice'});
COMMIT;
-- Even if database crashes now, Alice is saved
```

### Isolation Levels

```dql
-- Set isolation level
SET TRANSACTION ISOLATION LEVEL READ COMMITTED;

Levels:
- READ UNCOMMITTED (dirty reads allowed)
- READ COMMITTED (default - no dirty reads)
- REPEATABLE READ (no non-repeatable reads)
- SERIALIZABLE (full isolation)
```

### Implementation Plan

```rust
// deed-rust/src/transaction.rs

pub struct TransactionManager {
    active_transactions: DashMap<TransactionId, Transaction>,
    next_txn_id: AtomicU64,
}

pub struct Transaction {
    pub id: TransactionId,
    pub isolation_level: IsolationLevel,
    pub started_at: Timestamp,
    pub operations: Vec<Operation>,
    pub read_set: HashSet<EntityId>,   // For conflict detection
    pub write_set: HashSet<EntityId>,  // For conflict detection
    pub savepoints: Vec<Savepoint>,
}

pub enum IsolationLevel {
    ReadUncommitted,
    ReadCommitted,
    RepeatableRead,
    Serializable,
}

impl TransactionManager {
    pub fn begin(&self) -> TransactionId;
    pub fn commit(&self, txn_id: TransactionId) -> Result<(), TransactionError>;
    pub fn rollback(&self, txn_id: TransactionId) -> Result<(), TransactionError>;
    pub fn savepoint(&self, txn_id: TransactionId, name: String) -> Result<(), TransactionError>;
    pub fn rollback_to_savepoint(&self, txn_id: TransactionId, name: String) -> Result<(), TransactionError>;
}
```

### MVCC (Multi-Version Concurrency Control)

**Instead of locking, use versioning:**

```rust
pub struct Entity {
    pub id: EntityId,
    pub versions: Vec<EntityVersion>,  // Multiple versions
}

pub struct EntityVersion {
    pub version_id: u64,
    pub created_by_txn: TransactionId,
    pub deleted_by_txn: Option<TransactionId>,
    pub properties: Properties,
}

// Read: Find version visible to current transaction
impl Entity {
    pub fn read(&self, txn: &Transaction) -> Option<&EntityVersion> {
        self.versions.iter().find(|v|
            v.created_by_txn <= txn.id &&
            (v.deleted_by_txn.is_none() || v.deleted_by_txn.unwrap() > txn.id)
        )
    }
}
```

### Transaction Log (Write-Ahead Log)

```rust
pub struct WriteAheadLog {
    log_file: File,
    entries: Vec<LogEntry>,
}

pub enum LogEntry {
    BeginTransaction(TransactionId),
    Operation {
        txn_id: TransactionId,
        operation: Operation,
    },
    Commit(TransactionId),
    Rollback(TransactionId),
    Checkpoint,
}

// Recovery after crash
impl WriteAheadLog {
    pub fn recover(&self) -> Result<(), RecoveryError> {
        // Replay log
        // Redo committed transactions
        // Undo uncommitted transactions
    }
}
```

### DQL Transaction Examples

**Bank Transfer:**
```dql
BEGIN;

-- Debit from Alice
UPDATE Accounts
SET balance = balance - 100
WHERE account_holder = 'Alice';

-- Credit to Bob
UPDATE Accounts
SET balance = balance + 100
WHERE account_holder = 'Bob';

-- Record transaction
INSERT INTO Transactions VALUES ({
    from: 'Alice',
    to: 'Bob',
    amount: 100,
    timestamp: NOW()
});

COMMIT;
```

**Inventory Management:**
```dql
BEGIN;

-- Reduce stock
UPDATE Products
SET stock = stock - 5
WHERE id = 101;

-- Create order
INSERT INTO Orders VALUES ({
    product_id: 101,
    quantity: 5,
    customer_id: 42
});

-- Check stock didn't go negative
SELECT stock FROM Products WHERE id = 101;
-- If negative, ROLLBACK; else COMMIT;

COMMIT;
```

**Error Handling:**
```dql
BEGIN;

INSERT INTO Users VALUES ({name: 'Alice', email: 'alice@example.com'});
-- Succeeds

INSERT INTO Users VALUES ({name: 'Bob', email: 'alice@example.com'});
-- Fails: UNIQUE constraint on email

ROLLBACK;
-- Alice insert also undone
```

---

## Integration with DQL Executor

### Schema Integration

```rust
pub struct DQLExecutor {
    graph: Arc<RwLock<Graph>>,
    schema_validator: Arc<RwLock<SchemaValidator>>,
    transaction_manager: Arc<RwLock<TransactionManager>>,
    // ... existing fields
}

impl DQLExecutor {
    pub fn execute(&self, query: &str) -> Result<QueryResult, String> {
        // Get current transaction (if any)
        let txn = self.get_current_transaction();

        // Parse query
        let query = Parser::parse(query)?;

        // Validate against schema (if exists)
        if let Query::Insert(insert) = &query {
            self.schema_validator.read().unwrap()
                .validate_insert(&insert.collection, &insert.properties)?;
        }

        // Execute within transaction context
        if let Some(txn_id) = txn {
            self.execute_in_transaction(query, txn_id)
        } else {
            // Auto-commit single operation
            let txn_id = self.transaction_manager.write().unwrap().begin();
            let result = self.execute_in_transaction(query, txn_id)?;
            self.transaction_manager.write().unwrap().commit(txn_id)?;
            Ok(result)
        }
    }
}
```

### Transaction Context

```rust
// Thread-local transaction context
thread_local! {
    static CURRENT_TRANSACTION: RefCell<Option<TransactionId>> = RefCell::new(None);
}

pub fn begin_transaction() -> TransactionId {
    let txn_id = TRANSACTION_MANAGER.begin();
    CURRENT_TRANSACTION.with(|t| *t.borrow_mut() = Some(txn_id));
    txn_id
}

pub fn commit_transaction() -> Result<(), TransactionError> {
    CURRENT_TRANSACTION.with(|t| {
        if let Some(txn_id) = *t.borrow() {
            TRANSACTION_MANAGER.commit(txn_id)?;
            *t.borrow_mut() = None;
            Ok(())
        } else {
            Err(TransactionError::NoActiveTransaction)
        }
    })
}
```

---

## Comparison with Other Databases

### Schema System

| Database | Schema Model | Validation |
|----------|--------------|------------|
| PostgreSQL | Required, strict | Compile-time + runtime |
| MongoDB | Optional (3.6+) | Runtime only |
| Neo4j | Optional labels/types | Limited |
| **Deed** | **Optional, strict** | **Runtime + constraints** |

### Transaction System

| Database | ACID | MVCC | Isolation Levels |
|----------|------|------|------------------|
| PostgreSQL | Full | Yes | All 4 levels |
| MongoDB | Full (4.0+) | Yes | Read Committed, Snapshot |
| Neo4j | Full | Yes | Read Committed, Serializable |
| **Deed** | **Full** | **Yes** | **All 4 levels** |

---

## Implementation Roadmap

### Phase 1: Schema System (Week 1)
- [ ] Schema AST and parser
- [ ] Schema storage (_schemas collection)
- [ ] SchemaValidator implementation
- [ ] Type validation
- [ ] Constraint validation (NOT NULL, UNIQUE, CHECK)
- [ ] DEFAULT value application
- [ ] Integration with INSERT/UPDATE

### Phase 2: Basic Transactions (Week 2)
- [ ] Transaction AST (BEGIN, COMMIT, ROLLBACK)
- [ ] TransactionManager implementation
- [ ] Transaction context (thread-local)
- [ ] Operation buffering
- [ ] Basic commit/rollback

### Phase 3: MVCC (Week 3)
- [ ] Multi-version entity storage
- [ ] Version visibility rules
- [ ] Garbage collection of old versions
- [ ] READ COMMITTED isolation

### Phase 4: Advanced Transactions (Week 4)
- [ ] Savepoints
- [ ] Isolation levels
- [ ] Conflict detection
- [ ] Write-Ahead Log (WAL)
- [ ] Crash recovery

### Phase 5: Testing & Optimization (Week 5)
- [ ] Schema tests
- [ ] Transaction tests
- [ ] Concurrency tests
- [ ] Performance benchmarks
- [ ] Documentation

---

## Summary

**Schemas**: Optional, best-of-both-worlds approach
- Schema-less for flexibility
- Schema-enforced for safety
- Full constraint support

**Transactions**: Full ACID with MVCC
- BEGIN/COMMIT/ROLLBACK
- Savepoints
- All isolation levels
- Crash recovery

This makes Deed a truly production-ready database!
