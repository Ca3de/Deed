# Transaction System Design for Deed Database

## Overview

This document outlines the design for ACID-compliant transactions in Deed, combining traditional database transaction semantics with biological optimization.

## Requirements

### ACID Properties

1. **Atomicity** - All operations in a transaction succeed or all fail
2. **Consistency** - Database moves from one valid state to another
3. **Isolation** - Concurrent transactions don't interfere with each other
4. **Durability** - Committed transactions survive system crashes

### Transaction Commands

```dql
BEGIN TRANSACTION;
-- ... operations ...
COMMIT;

BEGIN TRANSACTION;
-- ... operations ...
ROLLBACK;
```

## Architecture

### 1. Transaction State Management

```rust
pub struct Transaction {
    pub id: TransactionId,
    pub state: TransactionState,
    pub start_time: u64,
    pub operations: Vec<Operation>,
    pub isolation_level: IsolationLevel,
}

pub enum TransactionState {
    Active,      // Transaction is running
    Preparing,   // Pre-commit validation
    Committed,   // Successfully committed
    Aborted,     // Rolled back
}

pub type TransactionId = u64;
```

### 2. MVCC (Multi-Version Concurrency Control)

**Key Concept:** Instead of locking, keep multiple versions of each entity.

```rust
pub struct EntityVersion {
    pub entity_id: EntityId,
    pub data: Entity,
    pub created_by_txn: TransactionId,
    pub deleted_by_txn: Option<TransactionId>,
    pub version: u64,
}
```

**Visibility Rules:**
- Transaction T can see version V if:
  - `V.created_by_txn < T.id` AND
  - (`V.deleted_by_txn` is None OR `V.deleted_by_txn > T.id`)

**Benefits:**
- Readers don't block writers
- Writers don't block readers
- Snapshot isolation prevents phantom reads

### 3. Write-Ahead Log (WAL)

**Purpose:** Ensure durability - committed transactions survive crashes.

```rust
pub enum WALEntry {
    BeginTransaction { txn_id: TransactionId },
    InsertEntity { txn_id: TransactionId, entity_id: EntityId, data: Entity },
    UpdateEntity { txn_id: TransactionId, entity_id: EntityId, old: Entity, new: Entity },
    DeleteEntity { txn_id: TransactionId, entity_id: EntityId, data: Entity },
    CreateEdge { txn_id: TransactionId, edge_id: EdgeId, source: EntityId, target: EntityId },
    Commit { txn_id: TransactionId },
    Rollback { txn_id: TransactionId },
}
```

**WAL Protocol:**
1. Write operation to WAL (with fsync)
2. Apply operation to in-memory structures
3. Write COMMIT to WAL (with fsync)
4. Transaction is durable

**Recovery:**
- On startup, replay WAL to restore committed transactions
- Abort any transactions without COMMIT entry

### 4. Isolation Levels

```rust
pub enum IsolationLevel {
    ReadUncommitted,  // Dirty reads allowed
    ReadCommitted,    // No dirty reads
    RepeatableRead,   // No dirty reads, no non-repeatable reads
    Serializable,     // Full isolation (default)
}
```

**Implementation:**
- **Read Committed:** Read latest committed version
- **Repeatable Read:** Read snapshot at transaction start
- **Serializable:** Detect conflicts and abort if necessary

### 5. Concurrency Control

**Optimistic Concurrency Control:**
1. Read phase: Transaction reads data without locks
2. Validation phase: Check for conflicts before commit
3. Write phase: Apply changes if validation passes

**Conflict Detection:**
- Write-Write conflicts: Two transactions modify same entity
- Write-Read conflicts: Transaction reads data modified by another
- Serializability: Detect cycles in dependency graph

### 6. Lock-Free Design

**Biological Inspiration:** Ant stigmergy is inherently lock-free!

Instead of traditional locking:
- Use atomic operations (CAS - Compare-And-Swap)
- Version numbers for optimistic concurrency
- Retry on conflict with exponential backoff

```rust
// Example: Optimistic update
loop {
    let current_version = entity.version.load(Ordering::SeqCst);
    let new_entity = apply_update(entity);

    if entity.version.compare_exchange(
        current_version,
        current_version + 1,
        Ordering::SeqCst,
        Ordering::SeqCst
    ).is_ok() {
        // Success!
        break;
    }

    // Conflict detected - retry with biological backoff
    backoff.wait();
}
```

## Implementation Plan

### Phase 1: Core Transaction Infrastructure (Week 2)

**Files to Create:**
- `deed-rust/src/transaction.rs` - Transaction state management
- `deed-rust/src/mvcc.rs` - Multi-version concurrency control
- `deed-rust/src/wal.rs` - Write-ahead logging

**Changes:**
- Update `Graph` to support MVCC
- Add transaction context to all operations

### Phase 2: Transaction Commands (Week 3)

**Update DQL:**
- Add `BEGIN TRANSACTION` parsing
- Add `COMMIT` parsing
- Add `ROLLBACK` parsing
- Add `SET TRANSACTION ISOLATION LEVEL`

**Update Executor:**
- Execute operations within transaction context
- Implement commit protocol
- Implement rollback with undo log

### Phase 3: WAL and Recovery (Week 4)

**Implement:**
- WAL file format
- WAL writer with fsync
- WAL reader and recovery
- Checkpoint mechanism

### Phase 4: Testing and Optimization (Week 5)

**Tests:**
- ACID compliance tests
- Concurrent transaction tests
- Crash recovery tests
- Performance benchmarks

**Optimization:**
- Biological optimizer learns transaction patterns
- Adaptive isolation levels
- Intelligent conflict resolution

## Usage Examples

### Basic Transaction

```dql
BEGIN TRANSACTION;

INSERT INTO Users VALUES ({name: "Alice", age: 30});
INSERT INTO Users VALUES ({name: "Bob", age: 25});

COMMIT;
```

### Transfer with Rollback

```dql
BEGIN TRANSACTION;

UPDATE Accounts
SET balance = balance - 100
WHERE id = 1;

UPDATE Accounts
SET balance = balance + 100
WHERE id = 2;

-- If validation fails, rollback
ROLLBACK;
```

### Concurrent Transactions

```dql
-- Transaction 1
BEGIN TRANSACTION;
SELECT balance FROM Accounts WHERE id = 1; -- Reads 1000
UPDATE Accounts SET balance = balance - 100 WHERE id = 1;
-- T1 waits...
COMMIT;

-- Transaction 2 (concurrent)
BEGIN TRANSACTION;
SELECT balance FROM Accounts WHERE id = 1; -- Also reads 1000 (snapshot)
UPDATE Accounts SET balance = balance - 50 WHERE id = 1;
COMMIT; -- May fail if T1 committed first (conflict detection)
```

### Isolation Level

```dql
BEGIN TRANSACTION ISOLATION LEVEL SERIALIZABLE;

SELECT * FROM Products WHERE category = 'Electronics';
-- No other transaction can insert/delete Electronics products
-- until this transaction commits (prevents phantom reads)

COMMIT;
```

## MVCC Implementation Details

### Entity Storage with Versions

```rust
pub struct VersionedEntity {
    pub entity_id: EntityId,
    pub versions: Vec<EntityVersion>,
    pub latest_version: AtomicU64,
}

impl VersionedEntity {
    // Get version visible to transaction
    pub fn get_version(&self, txn_id: TransactionId) -> Option<&EntityVersion> {
        self.versions.iter()
            .filter(|v| self.is_visible(v, txn_id))
            .max_by_key(|v| v.version)
    }

    fn is_visible(&self, version: &EntityVersion, txn_id: TransactionId) -> bool {
        version.created_by_txn <= txn_id &&
        (version.deleted_by_txn.is_none() || version.deleted_by_txn.unwrap() > txn_id)
    }
}
```

### Garbage Collection

Old versions need cleanup:

```rust
pub struct GarbageCollector {
    min_active_txn: AtomicU64,
}

impl GarbageCollector {
    pub fn collect(&mut self, entity: &mut VersionedEntity) {
        let min_txn = self.min_active_txn.load(Ordering::SeqCst);

        // Remove versions not visible to any active transaction
        entity.versions.retain(|v| {
            v.created_by_txn >= min_txn ||
            v.deleted_by_txn.map_or(true, |d| d >= min_txn)
        });
    }
}
```

## WAL File Format

```
+------------------+
| WAL Header       |
| - Magic Number   |
| - Version        |
| - Checksum       |
+------------------+
| Entry 1          |
| - Length         |
| - Type           |
| - Transaction ID |
| - Data           |
| - Checksum       |
+------------------+
| Entry 2          |
| ...              |
+------------------+
```

## Performance Characteristics

### Without Transactions (Current)
- **Write:** O(1) - Direct write to DashMap
- **Read:** O(1) - Direct read from DashMap

### With MVCC Transactions
- **Write:** O(1) + WAL write - Append new version
- **Read:** O(log V) - Binary search through versions (V = version count)
- **Commit:** O(W + fsync) - W = write set size
- **Rollback:** O(1) - Just mark transaction as aborted

### Space Overhead
- Each entity has ~2-5 versions on average
- WAL size grows linearly with transactions
- Periodic checkpointing compacts WAL

## Biological Optimization for Transactions

### Ant-Colony Transaction Scheduler

```rust
impl AntColonyTransactionScheduler {
    // Ants explore different transaction orderings
    pub fn optimize_schedule(&mut self, pending_txns: &[Transaction]) -> Vec<TransactionId> {
        // Pheromones indicate successful orderings
        // Low conflict orderings get stronger pheromones
        // High conflict orderings get weaker pheromones
    }
}
```

### Stigmergy-Based Conflict Resolution

```rust
impl StigmergyConflictResolver {
    // Learn which conflicts can be safely resolved
    pub fn resolve(&mut self, conflict: &Conflict) -> Resolution {
        // If similar conflicts resolved successfully before,
        // use stigmergy cache to resolve quickly
        // Otherwise, use conservative approach
    }
}
```

## Migration Path

### Stage 1: Optional Transactions (Week 2-3)
- Transactions are opt-in
- Non-transactional queries work as before
- Allows gradual adoption

### Stage 2: Default Transactions (Week 4)
- All writes are transactional by default
- Auto-commit for single statements
- Explicit transactions for multi-statement

### Stage 3: Full ACID (Week 5)
- WAL enabled by default
- Crash recovery on startup
- Full production readiness

## Testing Strategy

### Unit Tests
- Transaction state transitions
- MVCC visibility rules
- WAL write/read
- Conflict detection

### Integration Tests
- Multi-transaction scenarios
- Concurrent reads/writes
- Rollback correctness
- Recovery from crash

### Chaos Tests
- Random transaction ordering
- Random crashes during commit
- High concurrency stress tests
- Disk full scenarios

## Success Metrics

- ✅ All ACID tests pass
- ✅ Concurrent transactions (100+) without deadlocks
- ✅ Recovery from crash in <1 second
- ✅ Throughput: >10,000 transactions/second
- ✅ Latency: <10ms for simple transactions

---

**Next Steps:** Implement Phase 1 - Core Transaction Infrastructure

**Timeline:**
- Week 2: Transaction state + MVCC
- Week 3: BEGIN/COMMIT/ROLLBACK commands
- Week 4: WAL and recovery
- Week 5: Testing and optimization
