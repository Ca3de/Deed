//! B-tree Index Implementation
//!
//! Provides fast O(log n) lookups for indexed fields.

use crate::types::{EntityId, PropertyValue};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

/// B-tree index for fast lookups
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BTreeIndex {
    /// Index name
    pub name: String,
    /// Collection name
    pub collection: String,
    /// Indexed field
    pub field: String,
    /// B-tree mapping field value to entity IDs
    pub tree: BTreeMap<IndexKey, Vec<EntityId>>,
    /// Whether index is unique
    pub unique: bool,
}

/// Index key - wrapper around PropertyValue for BTreeMap
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum IndexKey {
    Null,
    Bool(bool),
    Int(i64),
    Float(OrderedFloat),
    String(String),
}

/// Wrapper for f64 to make it Ord (required for BTreeMap keys)
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct OrderedFloat(f64);

impl PartialEq for OrderedFloat {
    fn eq(&self, other: &Self) -> bool {
        self.0.to_bits() == other.0.to_bits()
    }
}

impl Eq for OrderedFloat {}

impl PartialOrd for OrderedFloat {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for OrderedFloat {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.partial_cmp(&other.0).unwrap_or(std::cmp::Ordering::Equal)
    }
}

impl From<&PropertyValue> for IndexKey {
    fn from(value: &PropertyValue) -> Self {
        match value {
            PropertyValue::Null => IndexKey::Null,
            PropertyValue::Bool(b) => IndexKey::Bool(*b),
            PropertyValue::Int(i) => IndexKey::Int(*i),
            PropertyValue::Float(f) => IndexKey::Float(OrderedFloat(*f)),
            PropertyValue::String(s) => IndexKey::String(s.clone()),
        }
    }
}

impl BTreeIndex {
    /// Create a new empty index
    pub fn new(name: String, collection: String, field: String, unique: bool) -> Self {
        BTreeIndex {
            name,
            collection,
            field,
            tree: BTreeMap::new(),
            unique,
        }
    }

    /// Insert a value into the index
    pub fn insert(&mut self, key: &PropertyValue, entity_id: EntityId) -> Result<(), String> {
        let index_key = IndexKey::from(key);

        if self.unique {
            // Check if key already exists
            if self.tree.contains_key(&index_key) {
                return Err(format!(
                    "Unique constraint violated: duplicate key {:?}",
                    index_key
                ));
            }
            self.tree.insert(index_key, vec![entity_id]);
        } else {
            // Non-unique index - append to list
            self.tree
                .entry(index_key)
                .or_insert_with(Vec::new)
                .push(entity_id);
        }

        Ok(())
    }

    /// Remove a value from the index
    pub fn remove(&mut self, key: &PropertyValue, entity_id: EntityId) {
        let index_key = IndexKey::from(key);

        if let Some(ids) = self.tree.get_mut(&index_key) {
            ids.retain(|id| *id != entity_id);
            if ids.is_empty() {
                self.tree.remove(&index_key);
            }
        }
    }

    /// Lookup entities by exact key match
    pub fn lookup(&self, key: &PropertyValue) -> Vec<EntityId> {
        let index_key = IndexKey::from(key);
        self.tree.get(&index_key).cloned().unwrap_or_default()
    }

    /// Range scan: find all keys in range [start, end]
    pub fn range_scan(&self, start: &PropertyValue, end: &PropertyValue) -> Vec<EntityId> {
        let start_key = IndexKey::from(start);
        let end_key = IndexKey::from(end);

        let mut result = Vec::new();
        for (_, ids) in self.tree.range(start_key..=end_key) {
            result.extend(ids);
        }
        result
    }

    /// Find keys greater than value
    pub fn greater_than(&self, value: &PropertyValue) -> Vec<EntityId> {
        let key = IndexKey::from(value);

        let mut result = Vec::new();
        for (k, ids) in self.tree.iter() {
            if k > &key {
                result.extend(ids);
            }
        }
        result
    }

    /// Find keys less than value
    pub fn less_than(&self, value: &PropertyValue) -> Vec<EntityId> {
        let key = IndexKey::from(value);

        let mut result = Vec::new();
        for (k, ids) in self.tree.iter() {
            if k < &key {
                result.extend(ids);
            }
        }
        result
    }

    /// Get index size (number of unique keys)
    pub fn size(&self) -> usize {
        self.tree.len()
    }

    /// Get total number of indexed entities
    pub fn total_entities(&self) -> usize {
        self.tree.values().map(|v| v.len()).sum()
    }
}

/// Index manager - manages all indexes for a database
#[derive(Debug, Clone)]
pub struct IndexManager {
    indexes: Arc<RwLock<Vec<BTreeIndex>>>,
}

impl IndexManager {
    /// Create a new index manager
    pub fn new() -> Self {
        IndexManager {
            indexes: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Create a new index
    pub fn create_index(
        &self,
        name: String,
        collection: String,
        field: String,
        unique: bool,
    ) -> Result<(), String> {
        let mut indexes = self.indexes.write().unwrap();

        // Check if index already exists
        if indexes.iter().any(|idx| idx.name == name) {
            return Err(format!("Index {} already exists", name));
        }

        let index = BTreeIndex::new(name, collection, field, unique);
        indexes.push(index);

        Ok(())
    }

    /// Drop an index
    pub fn drop_index(&self, name: &str) -> Result<(), String> {
        let mut indexes = self.indexes.write().unwrap();

        let initial_len = indexes.len();
        indexes.retain(|idx| idx.name != name);

        if indexes.len() == initial_len {
            Err(format!("Index {} not found", name))
        } else {
            Ok(())
        }
    }

    /// Get index by name
    pub fn get_index(&self, name: &str) -> Option<BTreeIndex> {
        let indexes = self.indexes.read().unwrap();
        indexes.iter().find(|idx| idx.name == name).cloned()
    }

    /// Find index for collection and field
    pub fn find_index(&self, collection: &str, field: &str) -> Option<BTreeIndex> {
        let indexes = self.indexes.read().unwrap();
        indexes
            .iter()
            .find(|idx| idx.collection == collection && idx.field == field)
            .cloned()
    }

    /// Insert into all relevant indexes
    pub fn insert_into_indexes(
        &self,
        collection: &str,
        entity_id: EntityId,
        properties: &std::collections::HashMap<String, PropertyValue>,
    ) -> Result<(), String> {
        let mut indexes = self.indexes.write().unwrap();

        for index in indexes.iter_mut() {
            if index.collection == collection {
                if let Some(value) = properties.get(&index.field) {
                    index.insert(value, entity_id)?;
                }
            }
        }

        Ok(())
    }

    /// Remove from all relevant indexes
    pub fn remove_from_indexes(
        &self,
        collection: &str,
        entity_id: EntityId,
        properties: &std::collections::HashMap<String, PropertyValue>,
    ) {
        let mut indexes = self.indexes.write().unwrap();

        for index in indexes.iter_mut() {
            if index.collection == collection {
                if let Some(value) = properties.get(&index.field) {
                    index.remove(value, entity_id);
                }
            }
        }
    }

    /// List all indexes
    pub fn list_indexes(&self) -> Vec<String> {
        let indexes = self.indexes.read().unwrap();
        indexes.iter().map(|idx| idx.name.clone()).collect()
    }

    /// Get index statistics
    pub fn index_stats(&self, name: &str) -> Option<IndexStats> {
        let indexes = self.indexes.read().unwrap();
        indexes.iter().find(|idx| idx.name == name).map(|idx| IndexStats {
            name: idx.name.clone(),
            collection: idx.collection.clone(),
            field: idx.field.clone(),
            unique: idx.unique,
            size: idx.size(),
            total_entities: idx.total_entities(),
        })
    }
}

impl Default for IndexManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Index statistics
#[derive(Debug, Clone)]
pub struct IndexStats {
    pub name: String,
    pub collection: String,
    pub field: String,
    pub unique: bool,
    pub size: usize,
    pub total_entities: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::PropertyValue;

    #[test]
    fn test_btree_index_insert_lookup() {
        let mut index = BTreeIndex::new(
            "idx_age".to_string(),
            "Users".to_string(),
            "age".to_string(),
            false,
        );

        let entity1 = EntityId::new(1);
        let entity2 = EntityId::new(2);
        let entity3 = EntityId::new(3);

        index.insert(&PropertyValue::Int(30), entity1).unwrap();
        index.insert(&PropertyValue::Int(25), entity2).unwrap();
        index.insert(&PropertyValue::Int(30), entity3).unwrap();

        // Lookup exact match
        let result = index.lookup(&PropertyValue::Int(30));
        assert_eq!(result.len(), 2);
        assert!(result.contains(&entity1));
        assert!(result.contains(&entity3));

        let result = index.lookup(&PropertyValue::Int(25));
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], entity2);
    }

    #[test]
    fn test_unique_index() {
        let mut index = BTreeIndex::new(
            "idx_email".to_string(),
            "Users".to_string(),
            "email".to_string(),
            true,
        );

        let entity1 = EntityId::new(1);
        let entity2 = EntityId::new(2);

        // First insert succeeds
        index
            .insert(&PropertyValue::String("alice@example.com".to_string()), entity1)
            .unwrap();

        // Duplicate insert fails
        let result = index.insert(
            &PropertyValue::String("alice@example.com".to_string()),
            entity2,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_range_scan() {
        let mut index = BTreeIndex::new(
            "idx_age".to_string(),
            "Users".to_string(),
            "age".to_string(),
            false,
        );

        index.insert(&PropertyValue::Int(20), EntityId::new(1)).unwrap();
        index.insert(&PropertyValue::Int(25), EntityId::new(2)).unwrap();
        index.insert(&PropertyValue::Int(30), EntityId::new(3)).unwrap();
        index.insert(&PropertyValue::Int(35), EntityId::new(4)).unwrap();

        // Range scan [25, 32]
        let result = index.range_scan(&PropertyValue::Int(25), &PropertyValue::Int(32));
        assert_eq!(result.len(), 2); // Ages 25 and 30
    }

    #[test]
    fn test_index_manager() {
        let manager = IndexManager::new();

        // Create index
        manager
            .create_index(
                "idx_age".to_string(),
                "Users".to_string(),
                "age".to_string(),
                false,
            )
            .unwrap();

        // Find index
        let index = manager.find_index("Users", "age");
        assert!(index.is_some());

        // Drop index
        manager.drop_index("idx_age").unwrap();

        // Index should be gone
        let index = manager.find_index("Users", "age");
        assert!(index.is_none());
    }

    #[test]
    fn test_index_remove() {
        let mut index = BTreeIndex::new(
            "idx_age".to_string(),
            "Users".to_string(),
            "age".to_string(),
            false,
        );

        let entity1 = EntityId::new(1);
        index.insert(&PropertyValue::Int(30), entity1).unwrap();

        assert_eq!(index.lookup(&PropertyValue::Int(30)).len(), 1);

        index.remove(&PropertyValue::Int(30), entity1);

        assert_eq!(index.lookup(&PropertyValue::Int(30)).len(), 0);
    }
}
