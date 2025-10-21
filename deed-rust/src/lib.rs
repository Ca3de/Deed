//! Deed Database - Rust Core Engine
//!
//! High-performance storage and query execution engine for Deed database.
//! This is the "muscles" while Python provides the "brains" (biological algorithms).
//!
//! # Architecture
//!
//! - Storage Layer: RocksDB-based LSM tree
//! - Graph Layer: Optimized adjacency lists with pheromone weights
//! - DQL Layer: Unified query language combining relational + graph operations
//! - Schema Layer: Optional schema enforcement with constraints
//! - Execution Layer: Vectorized query processing with biological optimization
//! - Network Layer: Async I/O with Tokio
//! - Python FFI: PyO3 bindings for integration with Python optimizer

pub mod storage;
pub mod graph;
pub mod executor;
pub mod types;
pub mod ffi;
pub mod schema;

// Transaction modules
pub mod transaction;
pub mod mvcc;
pub mod wal;

// Index module
pub mod btree;

// Authentication module
pub mod auth;

// Connection pool module
pub mod connection_pool;

// Replication module
pub mod replication;

// Backup/restore module
pub mod backup;

// Admin dashboard module
pub mod admin_dashboard;

// Distributed database modules
pub mod distributed_topology;
pub mod distributed_p2p;
pub mod distributed_shard;
pub mod distributed_query;

// DQL (Deed Query Language) modules
pub mod dql_lexer;
pub mod dql_ast;
pub mod dql_parser;
pub mod dql_ir;
pub mod dql_optimizer;
pub mod dql_executor;

pub use storage::StorageEngine;
pub use graph::{Graph, Entity, Edge};
pub use types::{EntityId, EdgeId, PropertyValue};
pub use schema::{Schema, Field, FieldType, Constraint, SchemaValidator, ValidationError};

// Transaction exports
pub use transaction::{Transaction, TransactionId, TransactionState, IsolationLevel, TransactionManager, TransactionStats as TxnStats};
pub use mvcc::{EntityVersion, VersionedEntity, MVCCManager};
pub use wal::{WALEntry, WALManager, WALReader, WALWriter};

// Index exports
pub use btree::{BTreeIndex, IndexManager, IndexKey};

// Authentication exports
pub use auth::{AuthManager, User, Session, Role};

// Connection pool exports
pub use connection_pool::{ConnectionPool, PoolConfig, PoolStats, PooledConnectionHandle};

// Replication exports
pub use replication::{ReplicationManager, ReplicationEntry, ReplicationConfig, NodeRole, ReplicationSeq, SlaveState, ReplicationStats};

// Backup/restore exports
pub use backup::{BackupManager, BackupConfig, BackupMetadata, BackupType};

// Admin dashboard exports
pub use admin_dashboard::{AdminDashboard, DashboardStats, DatabaseStats, AuthStats, TransactionStats};

// Distributed database exports
pub use distributed_topology::{SmallWorldTopology, TopologyConfig, NodeInfo, NodeAddress, NodeId, Connection, ConnectionType, TopologyStatistics};
pub use distributed_p2p::{P2PNetwork, P2PMessage, P2PConfig, MessageType};
pub use distributed_shard::{ShardManager, ShardAssignment, ConsistentHash, ShardId};
pub use distributed_query::{DistributedQueryExecutor, DistributedQueryPlan};

// DQL exports
pub use dql_parser::Parser as DQLParser;
pub use dql_executor::{DQLExecutor, QueryResult};
pub use dql_optimizer::{AntColonyOptimizer, StigmergyCache};

// Re-export for Python
pub use ffi::*;
