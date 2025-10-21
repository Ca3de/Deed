//! Shard Assignment and Consistent Hashing for Distributed Deed Database
//!
//! Implements consistent hashing for data partitioning across distributed nodes.
//! When nodes are added or removed, only a minimal amount of data needs to be moved.
//!
//! Features:
//! - Consistent hashing with virtual nodes (vnodes)
//! - Automatic shard assignment based on key hash
//! - Shard rebalancing when nodes join/leave
//! - Replication factor support (multiple copies of each shard)

use crate::distributed_topology::NodeId;
use std::collections::{HashMap, BTreeMap};
use std::sync::{Arc, RwLock};
use serde::{Serialize, Deserialize};
use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;

/// Shard identifier
pub type ShardId = u64;

/// Hash ring position (0 to 2^64-1)
type HashPosition = u64;

/// Configuration for shard management
#[derive(Debug, Clone)]
pub struct ShardConfig {
    /// Number of virtual nodes per physical node
    /// Higher value = better load distribution
    /// Typical range: 100-500
    pub virtual_nodes_per_node: usize,

    /// Replication factor (how many copies of each shard)
    /// Typical value: 3 (for fault tolerance)
    pub replication_factor: usize,

    /// Total number of shards in the system
    /// Should be >> number of nodes for better distribution
    /// Typical: 1024 or 4096
    pub total_shards: usize,
}

impl Default for ShardConfig {
    fn default() -> Self {
        Self {
            virtual_nodes_per_node: 150,
            replication_factor: 3,
            total_shards: 1024,
        }
    }
}

/// Consistent hash ring for shard assignment
pub struct ConsistentHash {
    config: ShardConfig,
    /// Hash ring: position -> (node_id, vnode_index)
    ring: Arc<RwLock<BTreeMap<HashPosition, (NodeId, usize)>>>,
    /// Reverse mapping: node_id -> list of ring positions
    node_positions: Arc<RwLock<HashMap<NodeId, Vec<HashPosition>>>>,
}

impl ConsistentHash {
    /// Create new consistent hash ring
    pub fn new(config: ShardConfig) -> Self {
        Self {
            config,
            ring: Arc::new(RwLock::new(BTreeMap::new())),
            node_positions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Add a node to the hash ring
    pub fn add_node(&self, node_id: NodeId) {
        let mut ring = self.ring.write().unwrap();
        let mut node_positions = self.node_positions.write().unwrap();

        let mut positions = Vec::new();

        // Add virtual nodes to the ring
        for vnode_idx in 0..self.config.virtual_nodes_per_node {
            let position = self.hash_vnode(node_id, vnode_idx);
            ring.insert(position, (node_id, vnode_idx));
            positions.push(position);
        }

        node_positions.insert(node_id, positions);
    }

    /// Remove a node from the hash ring
    pub fn remove_node(&self, node_id: NodeId) -> Vec<HashPosition> {
        let mut ring = self.ring.write().unwrap();
        let mut node_positions = self.node_positions.write().unwrap();

        let positions = node_positions.remove(&node_id).unwrap_or_default();

        for position in &positions {
            ring.remove(position);
        }

        positions
    }

    /// Get the node responsible for a given key
    pub fn get_node(&self, key: &str) -> Option<NodeId> {
        let ring = self.ring.read().unwrap();

        if ring.is_empty() {
            return None;
        }

        let hash = self.hash_key(key);

        // Find first node with position >= hash (clockwise search)
        let node = ring.range(hash..)
            .next()
            .or_else(|| ring.iter().next()) // Wrap around to first node
            .map(|(_, (node_id, _))| *node_id);

        node
    }

    /// Get multiple nodes for replication
    pub fn get_replica_nodes(&self, key: &str) -> Vec<NodeId> {
        let ring = self.ring.read().unwrap();

        if ring.is_empty() {
            return Vec::new();
        }

        let hash = self.hash_key(key);
        let mut nodes = Vec::new();
        let mut seen_nodes = std::collections::HashSet::new();

        // Start from hash position and go clockwise
        let mut iter = ring.range(hash..).chain(ring.iter());

        for (_, (node_id, _)) in iter {
            if !seen_nodes.contains(node_id) {
                nodes.push(*node_id);
                seen_nodes.insert(*node_id);

                if nodes.len() >= self.config.replication_factor {
                    break;
                }
            }
        }

        nodes
    }

    /// Hash a key to ring position
    fn hash_key(&self, key: &str) -> HashPosition {
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        hasher.finish()
    }

    /// Hash a virtual node to ring position
    fn hash_vnode(&self, node_id: NodeId, vnode_idx: usize) -> HashPosition {
        let mut hasher = DefaultHasher::new();
        node_id.hash(&mut hasher);
        vnode_idx.hash(&mut hasher);
        hasher.finish()
    }

    /// Get distribution statistics
    pub fn get_statistics(&self) -> HashRingStats {
        let ring = self.ring.read().unwrap();
        let node_positions = self.node_positions.read().unwrap();

        let total_nodes = node_positions.len();
        let total_vnodes = ring.len();

        HashRingStats {
            total_nodes,
            total_vnodes,
            vnodes_per_node: if total_nodes > 0 {
                total_vnodes / total_nodes
            } else {
                0
            },
        }
    }
}

/// Statistics for the consistent hash ring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HashRingStats {
    pub total_nodes: usize,
    pub total_vnodes: usize,
    pub vnodes_per_node: usize,
}

/// Shard assignment tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShardAssignment {
    pub shard_id: ShardId,
    pub primary_node: NodeId,
    pub replica_nodes: Vec<NodeId>,
    pub key_range_start: HashPosition,
    pub key_range_end: HashPosition,
    pub entity_count: usize,
}

/// Shard Manager - Manages shard assignment and rebalancing
pub struct ShardManager {
    config: ShardConfig,
    consistent_hash: ConsistentHash,
    /// Shard assignments
    shards: Arc<RwLock<HashMap<ShardId, ShardAssignment>>>,
}

impl ShardManager {
    /// Create new shard manager
    pub fn new(config: ShardConfig) -> Self {
        let consistent_hash = ConsistentHash::new(config.clone());

        Self {
            config,
            consistent_hash,
            shards: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Add a node to the cluster
    pub fn add_node(&self, node_id: NodeId) {
        self.consistent_hash.add_node(node_id);
        self.rebuild_shard_assignments();
    }

    /// Remove a node from the cluster
    pub fn remove_node(&self, node_id: NodeId) -> Vec<ShardId> {
        let affected_positions = self.consistent_hash.remove_node(node_id);
        self.rebuild_shard_assignments();

        // Return list of shards that need to be migrated
        let shards = self.shards.read().unwrap();
        shards
            .iter()
            .filter(|(_, assignment)| {
                assignment.primary_node == node_id
                    || assignment.replica_nodes.contains(&node_id)
            })
            .map(|(&shard_id, _)| shard_id)
            .collect()
    }

    /// Determine which shard a key belongs to
    pub fn get_shard_for_key(&self, key: &str) -> Option<ShardId> {
        // Hash key to determine shard
        let hash = self.hash_key(key);
        let shard_id = (hash % self.config.total_shards as u64) as ShardId;
        Some(shard_id)
    }

    /// Get the node responsible for a shard
    pub fn get_node_for_shard(&self, shard_id: ShardId) -> Option<NodeId> {
        let shards = self.shards.read().unwrap();
        shards.get(&shard_id).map(|assignment| assignment.primary_node)
    }

    /// Get all replica nodes for a shard
    pub fn get_replicas_for_shard(&self, shard_id: ShardId) -> Vec<NodeId> {
        let shards = self.shards.read().unwrap();
        shards
            .get(&shard_id)
            .map(|assignment| assignment.replica_nodes.clone())
            .unwrap_or_default()
    }

    /// Get the primary node for a key (convenience method)
    pub fn get_node_for_key(&self, key: &str) -> Option<NodeId> {
        self.consistent_hash.get_node(key)
    }

    /// Get all replica nodes for a key (convenience method)
    pub fn get_replicas_for_key(&self, key: &str) -> Vec<NodeId> {
        self.consistent_hash.get_replica_nodes(key)
    }

    /// Rebuild shard assignments after topology change
    fn rebuild_shard_assignments(&self) {
        let mut shards = self.shards.write().unwrap();
        shards.clear();

        // Divide hash space into shards
        let shard_size = u64::MAX / self.config.total_shards as u64;

        for shard_id in 0..self.config.total_shards {
            let key_range_start = shard_id as u64 * shard_size;
            let key_range_end = if shard_id == self.config.total_shards - 1 {
                u64::MAX
            } else {
                (shard_id as u64 + 1) * shard_size - 1
            };

            // Use midpoint of range to determine responsible node
            let midpoint_key = format!("shard_{}", shard_id);
            let replica_nodes = self.consistent_hash.get_replica_nodes(&midpoint_key);

            if let Some(&primary_node) = replica_nodes.first() {
                let assignment = ShardAssignment {
                    shard_id: shard_id as ShardId,
                    primary_node,
                    replica_nodes: replica_nodes[1..].to_vec(),
                    key_range_start,
                    key_range_end,
                    entity_count: 0,
                };

                shards.insert(shard_id as ShardId, assignment);
            }
        }
    }

    /// Get all shard assignments
    pub fn get_all_shards(&self) -> Vec<ShardAssignment> {
        let shards = self.shards.read().unwrap();
        shards.values().cloned().collect()
    }

    /// Get shards managed by a specific node
    pub fn get_shards_for_node(&self, node_id: NodeId) -> Vec<ShardAssignment> {
        let shards = self.shards.read().unwrap();
        shards
            .values()
            .filter(|assignment| {
                assignment.primary_node == node_id
                    || assignment.replica_nodes.contains(&node_id)
            })
            .cloned()
            .collect()
    }

    /// Calculate rebalancing operations needed
    pub fn calculate_rebalancing(&self, target_node_id: NodeId) -> Vec<RebalanceOperation> {
        let shards = self.shards.read().unwrap();
        let mut operations = Vec::new();

        for (shard_id, assignment) in shards.iter() {
            // If this node should own the shard but doesn't
            let replica_nodes = self.consistent_hash.get_replica_nodes(&format!("shard_{}", shard_id));

            if replica_nodes.contains(&target_node_id) && assignment.primary_node != target_node_id {
                operations.push(RebalanceOperation {
                    shard_id: *shard_id,
                    from_node: assignment.primary_node,
                    to_node: target_node_id,
                    operation_type: RebalanceType::Copy,
                });
            }
        }

        operations
    }

    /// Hash a key
    fn hash_key(&self, key: &str) -> u64 {
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        hasher.finish()
    }

    /// Get shard manager statistics
    pub fn get_statistics(&self) -> ShardManagerStats {
        let shards = self.shards.read().unwrap();
        let hash_stats = self.consistent_hash.get_statistics();

        let mut node_shard_counts: HashMap<NodeId, usize> = HashMap::new();

        for assignment in shards.values() {
            *node_shard_counts.entry(assignment.primary_node).or_insert(0) += 1;
            for &replica in &assignment.replica_nodes {
                *node_shard_counts.entry(replica).or_insert(0) += 1;
            }
        }

        let avg_shards_per_node = if node_shard_counts.is_empty() {
            0.0
        } else {
            node_shard_counts.values().sum::<usize>() as f64 / node_shard_counts.len() as f64
        };

        ShardManagerStats {
            total_shards: shards.len(),
            total_nodes: hash_stats.total_nodes,
            avg_shards_per_node,
            replication_factor: self.config.replication_factor,
        }
    }
}

/// Type of rebalancing operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RebalanceType {
    /// Copy shard to new node
    Copy,
    /// Move shard to new node (and delete from old)
    Move,
    /// Delete shard from node
    Delete,
}

/// Rebalancing operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RebalanceOperation {
    pub shard_id: ShardId,
    pub from_node: NodeId,
    pub to_node: NodeId,
    pub operation_type: RebalanceType,
}

/// Shard manager statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShardManagerStats {
    pub total_shards: usize,
    pub total_nodes: usize,
    pub avg_shards_per_node: f64,
    pub replication_factor: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_consistent_hash_add_node() {
        let config = ShardConfig::default();
        let hash = ConsistentHash::new(config);

        hash.add_node(1);
        hash.add_node(2);
        hash.add_node(3);

        let stats = hash.get_statistics();
        assert_eq!(stats.total_nodes, 3);
        assert_eq!(stats.total_vnodes, 3 * 150); // 3 nodes * 150 vnodes each
    }

    #[test]
    fn test_consistent_hash_get_node() {
        let config = ShardConfig::default();
        let hash = ConsistentHash::new(config);

        hash.add_node(1);
        hash.add_node(2);
        hash.add_node(3);

        let node = hash.get_node("test_key");
        assert!(node.is_some());
        assert!(node.unwrap() >= 1 && node.unwrap() <= 3);
    }

    #[test]
    fn test_consistent_hash_replicas() {
        let config = ShardConfig {
            replication_factor: 3,
            ..Default::default()
        };
        let hash = ConsistentHash::new(config);

        hash.add_node(1);
        hash.add_node(2);
        hash.add_node(3);
        hash.add_node(4);

        let replicas = hash.get_replica_nodes("test_key");
        assert_eq!(replicas.len(), 3);

        // All replicas should be different nodes
        let unique: std::collections::HashSet<_> = replicas.iter().collect();
        assert_eq!(unique.len(), 3);
    }

    #[test]
    fn test_consistent_hash_remove_node() {
        let config = ShardConfig::default();
        let hash = ConsistentHash::new(config);

        hash.add_node(1);
        hash.add_node(2);
        hash.add_node(3);

        hash.remove_node(2);

        let stats = hash.get_statistics();
        assert_eq!(stats.total_nodes, 2);
    }

    #[test]
    fn test_shard_manager_add_node() {
        let config = ShardConfig {
            total_shards: 16,
            replication_factor: 3,
            ..Default::default()
        };
        let manager = ShardManager::new(config);

        manager.add_node(1);
        manager.add_node(2);
        manager.add_node(3);

        let stats = manager.get_statistics();
        assert_eq!(stats.total_shards, 16);
        assert_eq!(stats.total_nodes, 3);
    }

    #[test]
    fn test_shard_manager_get_shard_for_key() {
        let config = ShardConfig {
            total_shards: 16,
            ..Default::default()
        };
        let manager = ShardManager::new(config);

        manager.add_node(1);
        manager.add_node(2);

        let shard_id = manager.get_shard_for_key("test_key");
        assert!(shard_id.is_some());
        assert!(shard_id.unwrap() < 16);
    }

    #[test]
    fn test_shard_manager_get_node_for_key() {
        let config = ShardConfig::default();
        let manager = ShardManager::new(config);

        manager.add_node(1);
        manager.add_node(2);
        manager.add_node(3);

        let node = manager.get_node_for_key("test_key");
        assert!(node.is_some());
    }

    #[test]
    fn test_shard_manager_rebalancing() {
        let config = ShardConfig {
            total_shards: 16,
            ..Default::default()
        };
        let manager = ShardManager::new(config);

        manager.add_node(1);
        manager.add_node(2);
        manager.add_node(3);

        // Add a new node and check rebalancing operations
        manager.add_node(4);

        let operations = manager.calculate_rebalancing(4);
        // Should have some operations to move shards to the new node
        assert!(operations.len() > 0);
    }
}
