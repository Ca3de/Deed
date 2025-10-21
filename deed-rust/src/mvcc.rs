//! MVCC (Multi-Version Concurrency Control)
//!
//! Provides snapshot isolation for concurrent transactions without locking.

use crate::graph::Entity;
use crate::transaction::{TransactionId, IsolationLevel};
use crate::types::EntityId;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};

/// Version number for entity
pub type VersionNumber = u64;

/// Versioned entity - stores multiple versions for MVCC
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityVersion {
    /// The entity data
    pub entity: Entity,
    /// Version number (monotonically increasing)
    pub version: VersionNumber,
    /// Transaction that created this version
    pub created_by_txn: TransactionId,
    /// Transaction that deleted this version (None if not deleted)
    pub deleted_by_txn: Option<TransactionId>,
    /// Timestamp when created
    pub created_at: u64,
}

impl EntityVersion {
    /// Create a new entity version
    pub fn new(
        entity: Entity,
        version: VersionNumber,
        created_by_txn: TransactionId,
        created_at: u64,
    ) -> Self {
        EntityVersion {
            entity,
            version,
            created_by_txn,
            deleted_by_txn: None,
            created_at,
        }
    }

    /// Mark this version as deleted by a transaction
    pub fn mark_deleted(&mut self, txn_id: TransactionId) {
        self.deleted_by_txn = Some(txn_id);
    }

    /// Check if this version is visible to a transaction
    pub fn is_visible(&self, txn_id: TransactionId, isolation_level: IsolationLevel) -> bool {
        match isolation_level {
            IsolationLevel::ReadUncommitted => {
                // See all versions, even uncommitted
                self.deleted_by_txn.is_none() || self.deleted_by_txn.unwrap() > txn_id
            }
            IsolationLevel::ReadCommitted | IsolationLevel::RepeatableRead | IsolationLevel::Serializable => {
                // Standard MVCC visibility rules:
                // 1. Version was created by a committed transaction before us
                // 2. Version is not deleted, OR deleted by a transaction after us
                self.created_by_txn < txn_id &&
                (self.deleted_by_txn.is_none() || self.deleted_by_txn.unwrap() > txn_id)
            }
        }
    }

    /// Check if this version is live (not deleted)
    pub fn is_live(&self) -> bool {
        self.deleted_by_txn.is_none()
    }
}

/// Multi-versioned entity storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionedEntity {
    /// Entity ID
    pub entity_id: EntityId,
    /// All versions of this entity
    pub versions: Vec<EntityVersion>,
    /// Latest version number
    #[serde(skip)]
    pub latest_version: AtomicU64,
}

impl VersionedEntity {
    /// Create a new versioned entity with initial version
    pub fn new(entity_id: EntityId, initial_entity: Entity, txn_id: TransactionId) -> Self {
        let version = EntityVersion::new(initial_entity, 1, txn_id, 0);

        VersionedEntity {
            entity_id,
            versions: vec![version],
            latest_version: AtomicU64::new(1),
        }
    }

    /// Add a new version
    pub fn add_version(&mut self, entity: Entity, txn_id: TransactionId, created_at: u64) -> VersionNumber {
        let version_num = self.latest_version.fetch_add(1, Ordering::SeqCst) + 1;
        let version = EntityVersion::new(entity, version_num, txn_id, created_at);

        self.versions.push(version);
        version_num
    }

    /// Mark entity as deleted by creating a deletion version
    pub fn delete(&mut self, txn_id: TransactionId) {
        // Find the latest version and mark it as deleted
        if let Some(latest) = self.versions.last_mut() {
            latest.mark_deleted(txn_id);
        }
    }

    /// Get the visible version for a transaction
    pub fn get_visible_version(
        &self,
        txn_id: TransactionId,
        isolation_level: IsolationLevel,
    ) -> Option<&EntityVersion> {
        // Find the most recent version visible to this transaction
        self.versions
            .iter()
            .rev() // Start from newest
            .find(|v| v.is_visible(txn_id, isolation_level))
    }

    /// Get the latest committed version
    pub fn get_latest_version(&self) -> Option<&EntityVersion> {
        self.versions.last()
    }

    /// Check if entity is live (not deleted)
    pub fn is_live(&self) -> bool {
        self.versions.last().map(|v| v.is_live()).unwrap_or(false)
    }

    /// Garbage collect old versions not visible to any active transaction
    pub fn garbage_collect(&mut self, min_active_txn: TransactionId) {
        // Keep versions that might be visible to active transactions
        // Keep at least one version for recovery

        if self.versions.len() <= 1 {
            return; // Always keep at least one version
        }

        let mut keep_count = 0;
        self.versions.retain(|v| {
            // Keep if:
            // 1. Created by active or recent transaction
            // 2. Might be visible to active transactions
            let should_keep = v.created_by_txn >= min_active_txn ||
                              v.deleted_by_txn.map_or(true, |d| d >= min_active_txn) ||
                              keep_count == 0; // Always keep at least one

            if should_keep {
                keep_count += 1;
            }
            should_keep
        });
    }

    /// Get total number of versions
    pub fn version_count(&self) -> usize {
        self.versions.len()
    }
}

/// MVCC Manager - coordinates versioning across all entities
pub struct MVCCManager {
    /// Minimum active transaction (for garbage collection)
    min_active_txn: AtomicU64,
}

impl MVCCManager {
    /// Create a new MVCC manager
    pub fn new() -> Self {
        MVCCManager {
            min_active_txn: AtomicU64::new(u64::MAX),
        }
    }

    /// Update minimum active transaction
    pub fn update_min_active_txn(&self, min_txn: TransactionId) {
        self.min_active_txn.store(min_txn, Ordering::SeqCst);
    }

    /// Get minimum active transaction
    pub fn get_min_active_txn(&self) -> TransactionId {
        self.min_active_txn.load(Ordering::SeqCst)
    }

    /// Check if a version should be garbage collected
    pub fn should_gc_version(&self, version: &EntityVersion) -> bool {
        let min_txn = self.get_min_active_txn();

        // Can GC if version is not visible to any active transaction
        version.created_by_txn < min_txn &&
        version.deleted_by_txn.map_or(false, |d| d < min_txn)
    }
}

impl Default for MVCCManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Properties;

    fn create_test_entity(id: u64, name: &str) -> Entity {
        let mut props = Properties::new();
        props.insert("name".to_string(), crate::types::PropertyValue::String(name.to_string()));

        Entity {
            id: EntityId::new(id),
            entity_type: "User".to_string(),
            properties: props,
        }
    }

    #[test]
    fn test_entity_version_visibility() {
        let entity = create_test_entity(1, "Alice");
        let version = EntityVersion::new(entity, 1, 10, 0);

        // Transaction 11 can see version created by txn 10
        assert!(version.is_visible(11, IsolationLevel::ReadCommitted));

        // Transaction 5 cannot see version created by txn 10
        assert!(!version.is_visible(5, IsolationLevel::ReadCommitted));
    }

    #[test]
    fn test_entity_version_deletion() {
        let entity = create_test_entity(1, "Alice");
        let mut version = EntityVersion::new(entity, 1, 10, 0);

        version.mark_deleted(20);

        // Transaction 15 can see (created at 10, deleted at 20)
        assert!(version.is_visible(15, IsolationLevel::ReadCommitted));

        // Transaction 25 cannot see (deleted at 20)
        assert!(!version.is_visible(25, IsolationLevel::ReadCommitted));
    }

    #[test]
    fn test_versioned_entity() {
        let entity = create_test_entity(1, "Alice");
        let mut versioned = VersionedEntity::new(EntityId::new(1), entity, 1);

        // Add a new version
        let entity2 = create_test_entity(1, "Alice Updated");
        versioned.add_version(entity2, 5, 100);

        assert_eq!(versioned.version_count(), 2);

        // Transaction 3 should see version 1
        let v3 = versioned.get_visible_version(3, IsolationLevel::ReadCommitted);
        assert!(v3.is_some());
        assert_eq!(v3.unwrap().version, 1);

        // Transaction 10 should see version 2
        let v10 = versioned.get_visible_version(10, IsolationLevel::ReadCommitted);
        assert!(v10.is_some());
        assert_eq!(v10.unwrap().version, 2);
    }

    #[test]
    fn test_garbage_collection() {
        let entity = create_test_entity(1, "Alice");
        let mut versioned = VersionedEntity::new(EntityId::new(1), entity.clone(), 1);

        // Add multiple versions
        for i in 2..10 {
            versioned.add_version(entity.clone(), i * 10, i * 100);
        }

        assert_eq!(versioned.version_count(), 9);

        // GC with min_active_txn = 50
        // Should keep versions created by txn >= 50
        versioned.garbage_collect(50);

        // Should have fewer versions now
        assert!(versioned.version_count() < 9);
        assert!(versioned.version_count() >= 1); // At least one kept
    }

    #[test]
    fn test_mvcc_manager() {
        let mgr = MVCCManager::new();

        mgr.update_min_active_txn(42);
        assert_eq!(mgr.get_min_active_txn(), 42);

        let entity = create_test_entity(1, "Alice");
        let version = EntityVersion::new(entity, 1, 10, 0);

        // Version created at 10 should be GC-able if min_active is 42
        assert!(mgr.should_gc_version(&version));
    }
}
