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
pub use transaction::{Transaction, TransactionId, TransactionState, IsolationLevel, TransactionManager};
pub use mvcc::{EntityVersion, VersionedEntity, MVCCManager};
pub use wal::{WALEntry, WALManager, WALReader, WALWriter};

// DQL exports
pub use dql_parser::Parser as DQLParser;
pub use dql_executor::{DQLExecutor, QueryResult};
pub use dql_optimizer::{AntColonyOptimizer, StigmergyCache};

// Re-export for Python
pub use ffi::*;
