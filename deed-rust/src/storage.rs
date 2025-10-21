//! Persistent storage engine using RocksDB
//!
//! Provides ACID guarantees and efficient disk-based storage.
//! Uses LSM-tree for write-optimized workloads.

use crate::types::*;
use crate::graph::{Entity, Edge};
use rocksdb::{DB, Options, WriteBatch, IteratorMode};
use serde::{Serialize, Deserialize};
use std::path::Path;
use std::sync::Arc;

/// Column families for different data types
const CF_ENTITIES: &str = "entities";
const CF_EDGES: &str = "edges";
const CF_INDEXES: &str = "indexes";
const CF_METADATA: &str = "metadata";

/// Storage engine backed by RocksDB
///
/// Provides persistent storage with:
/// - ACID transactions
/// - Crash recovery
/// - Compression
/// - Point lookups in O(log N)
/// - Range scans
pub struct StorageEngine {
    db: Arc<DB>,
}

impl StorageEngine {
    /// Open or create a database at the given path
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);

        // Performance tuning
        opts.set_max_background_jobs(4);
        opts.set_bytes_per_sync(1024 * 1024); // 1MB
        opts.increase_parallelism(num_cpus::get() as i32);

        // Enable compression
        opts.set_compression_type(rocksdb::DBCompressionType::Lz4);

        // Column families for different data types
        let cfs = vec![CF_ENTITIES, CF_EDGES, CF_INDEXES, CF_METADATA];

        let db = DB::open_cf(&opts, path, cfs)
            .map_err(|e| format!("Failed to open database: {}", e))?;

        Ok(StorageEngine { db: Arc::new(db) })
    }

    /// Store an entity
    pub fn put_entity(&self, entity: &Entity) -> Result<(), String> {
        let cf = self.db.cf_handle(CF_ENTITIES)
            .ok_or("Entity column family not found")?;

        let key = entity_key(entity.id);
        let value = bincode::serialize(entity)
            .map_err(|e| format!("Serialization error: {}", e))?;

        self.db.put_cf(&cf, key, value)
            .map_err(|e| format!("Put error: {}", e))
    }

    /// Get an entity by ID
    pub fn get_entity(&self, id: EntityId) -> Result<Option<Entity>, String> {
        let cf = self.db.cf_handle(CF_ENTITIES)
            .ok_or("Entity column family not found")?;

        let key = entity_key(id);

        match self.db.get_cf(&cf, key) {
            Ok(Some(value)) => {
                let entity: Entity = bincode::deserialize(&value)
                    .map_err(|e| format!("Deserialization error: {}", e))?;
                Ok(Some(entity))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(format!("Get error: {}", e)),
        }
    }

    /// Delete an entity
    pub fn delete_entity(&self, id: EntityId) -> Result<(), String> {
        let cf = self.db.cf_handle(CF_ENTITIES)
            .ok_or("Entity column family not found")?;

        let key = entity_key(id);

        self.db.delete_cf(&cf, key)
            .map_err(|e| format!("Delete error: {}", e))
    }

    /// Store an edge
    pub fn put_edge(&self, edge: &Edge) -> Result<(), String> {
        let cf = self.db.cf_handle(CF_EDGES)
            .ok_or("Edge column family not found")?;

        let key = edge_key(edge.id);
        let value = bincode::serialize(edge)
            .map_err(|e| format!("Serialization error: {}", e))?;

        self.db.put_cf(&cf, key, value)
            .map_err(|e| format!("Put error: {}", e))
    }

    /// Get an edge by ID
    pub fn get_edge(&self, id: EdgeId) -> Result<Option<Edge>, String> {
        let cf = self.db.cf_handle(CF_EDGES)
            .ok_or("Edge column family not found")?;

        let key = edge_key(id);

        match self.db.get_cf(&cf, key) {
            Ok(Some(value)) => {
                let edge: Edge = bincode::deserialize(&value)
                    .map_err(|e| format!("Deserialization error: {}", e))?;
                Ok(Some(edge))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(format!("Get error: {}", e)),
        }
    }

    /// Batch write (for transactions)
    pub fn write_batch(&self, batch: WriteBatch) -> Result<(), String> {
        self.db.write(batch)
            .map_err(|e| format!("Batch write error: {}", e))
    }

    /// Scan all entities (range scan)
    pub fn scan_entities(&self) -> Result<Vec<Entity>, String> {
        let cf = self.db.cf_handle(CF_ENTITIES)
            .ok_or("Entity column family not found")?;

        let mut entities = Vec::new();

        let iter = self.db.iterator_cf(&cf, IteratorMode::Start);
        for item in iter {
            match item {
                Ok((_key, value)) => {
                    let entity: Entity = bincode::deserialize(&value)
                        .map_err(|e| format!("Deserialization error: {}", e))?;
                    entities.push(entity);
                }
                Err(e) => return Err(format!("Iterator error: {}", e)),
            }
        }

        Ok(entities)
    }

    /// Create a secondary index on a property
    ///
    /// Stores mapping: property_value -> [entity_ids]
    pub fn create_index(&self, collection: &str, property: &str) -> Result<(), String> {
        let cf_entities = self.db.cf_handle(CF_ENTITIES)
            .ok_or("Entity column family not found")?;
        let cf_indexes = self.db.cf_handle(CF_INDEXES)
            .ok_or("Index column family not found")?;

        // Scan all entities in collection
        let iter = self.db.iterator_cf(&cf_entities, IteratorMode::Start);

        let mut index_entries: std::collections::HashMap<Vec<u8>, Vec<EntityId>> =
            std::collections::HashMap::new();

        for item in iter {
            match item {
                Ok((_key, value)) => {
                    let entity: Entity = bincode::deserialize(&value)
                        .map_err(|e| format!("Deserialization error: {}", e))?;

                    // Filter by collection
                    if entity.entity_type != collection {
                        continue;
                    }

                    // Extract property value
                    if let Some(prop_value) = entity.get_property(property) {
                        let index_key = index_key(collection, property, prop_value);

                        index_entries
                            .entry(index_key)
                            .or_insert_with(Vec::new)
                            .push(entity.id);
                    }
                }
                Err(e) => return Err(format!("Iterator error: {}", e)),
            }
        }

        // Write index entries
        let mut batch = WriteBatch::default();
        for (key, entity_ids) in index_entries {
            let value = bincode::serialize(&entity_ids)
                .map_err(|e| format!("Serialization error: {}", e))?;
            batch.put_cf(&cf_indexes, key, value);
        }

        self.write_batch(batch)
    }

    /// Lookup entities by indexed property
    pub fn lookup_index(
        &self,
        collection: &str,
        property: &str,
        value: &PropertyValue,
    ) -> Result<Vec<EntityId>, String> {
        let cf = self.db.cf_handle(CF_INDEXES)
            .ok_or("Index column family not found")?;

        let key = index_key(collection, property, value);

        match self.db.get_cf(&cf, key) {
            Ok(Some(bytes)) => {
                let entity_ids: Vec<EntityId> = bincode::deserialize(&bytes)
                    .map_err(|e| format!("Deserialization error: {}", e))?;
                Ok(entity_ids)
            }
            Ok(None) => Ok(Vec::new()),
            Err(e) => Err(format!("Lookup error: {}", e)),
        }
    }

    /// Flush all writes to disk
    pub fn flush(&self) -> Result<(), String> {
        self.db.flush()
            .map_err(|e| format!("Flush error: {}", e))
    }
}

// Key encoding functions

fn entity_key(id: EntityId) -> Vec<u8> {
    format!("e:{}", id.as_u64()).into_bytes()
}

fn edge_key(id: EdgeId) -> Vec<u8> {
    format!("g:{}", id.as_u64()).into_bytes()
}

fn index_key(collection: &str, property: &str, value: &PropertyValue) -> Vec<u8> {
    // Encode as: i:{collection}:{property}:{value}
    let value_str = match value {
        PropertyValue::Int(v) => format!("int:{}", v),
        PropertyValue::Float(v) => format!("float:{}", v),
        PropertyValue::String(s) => format!("str:{}", s),
        PropertyValue::Bool(b) => format!("bool:{}", b),
        _ => String::from("null"),
    };

    format!("i:{}:{}:{}", collection, property, value_str).into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_storage_roundtrip() {
        let temp_dir = TempDir::new().unwrap();
        let storage = StorageEngine::open(temp_dir.path()).unwrap();

        let mut props = Properties::new();
        props.insert("name".to_string(), PropertyValue::String("Alice".to_string()));

        let entity = Entity::new(
            EntityId::new(1),
            "User".to_string(),
            props,
        );

        storage.put_entity(&entity).unwrap();

        let retrieved = storage.get_entity(EntityId::new(1)).unwrap().unwrap();
        assert_eq!(retrieved.id, entity.id);
        assert_eq!(retrieved.entity_type, entity.entity_type);
    }

    #[test]
    fn test_index_creation() {
        let temp_dir = TempDir::new().unwrap();
        let storage = StorageEngine::open(temp_dir.path()).unwrap();

        // Create some test entities
        for i in 1..=5 {
            let mut props = Properties::new();
            props.insert("age".to_string(), PropertyValue::Int(20 + i));

            let entity = Entity::new(
                EntityId::new(i as u64),
                "User".to_string(),
                props,
            );

            storage.put_entity(&entity).unwrap();
        }

        // Create index
        storage.create_index("User", "age").unwrap();

        // Lookup
        let results = storage.lookup_index(
            "User",
            "age",
            &PropertyValue::Int(23),
        ).unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0], EntityId::new(3));
    }
}
