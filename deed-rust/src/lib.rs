//! Deed Database - Rust Core Engine
//!
//! High-performance storage and query execution engine for Deed database.
//! This is the "muscles" while Python provides the "brains" (biological algorithms).
//!
//! # Architecture
//!
//! - Storage Layer: RocksDB-based LSM tree
//! - Graph Layer: Optimized adjacency lists with pheromone weights
//! - Execution Layer: Vectorized query processing
//! - Network Layer: Async I/O with Tokio
//! - Python FFI: PyO3 bindings for integration with Python optimizer

pub mod storage;
pub mod graph;
pub mod executor;
pub mod types;
pub mod ffi;

pub use storage::StorageEngine;
pub use graph::{Graph, Entity, Edge};
pub use types::{EntityId, EdgeId, PropertyValue};

// Re-export for Python
pub use ffi::*;
