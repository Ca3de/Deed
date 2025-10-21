//! Small-World Network Topology for Distributed Deed Database
//!
//! Implements a biologically-inspired small-world network topology where nodes
//! are connected in a way that provides:
//! - High clustering (local connectivity)
//! - Short path lengths (global reachability)
//! - Fault tolerance (multiple paths)
//!
//! This is inspired by social networks and neural networks where most connections
//! are local, but a few long-range connections create "shortcuts" across the network.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, RwLock};
use serde::{Serialize, Deserialize};

/// Unique identifier for a node in the distributed network
pub type NodeId = u64;

/// Network address for peer-to-peer communication
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeAddress {
    pub host: String,
    pub port: u16,
}

impl NodeAddress {
    pub fn new(host: String, port: u16) -> Self {
        Self { host, port }
    }

    pub fn to_string(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

/// Information about a node in the network
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    pub id: NodeId,
    pub address: NodeAddress,
    pub is_alive: bool,
    pub last_seen: u64, // Unix timestamp
    pub shard_count: usize, // How many shards this node manages
    pub capacity: usize, // Maximum shards this node can handle
}

impl NodeInfo {
    pub fn new(id: NodeId, address: NodeAddress, capacity: usize) -> Self {
        Self {
            id,
            address,
            is_alive: true,
            last_seen: current_timestamp(),
            shard_count: 0,
            capacity,
        }
    }
}

/// Connection type in the small-world topology
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionType {
    Local,     // Connection to nearby node (ring neighbor)
    LongRange, // Long-range shortcut connection
}

/// A connection between two nodes
#[derive(Debug, Clone)]
pub struct Connection {
    pub target_id: NodeId,
    pub connection_type: ConnectionType,
    pub latency_ms: u32, // Measured round-trip time
}

/// Configuration for small-world network topology
#[derive(Debug, Clone)]
pub struct TopologyConfig {
    /// Number of local connections (k-nearest neighbors on ring)
    /// Typical value: 4-8
    pub local_connections: usize,

    /// Number of long-range connections (random shortcuts)
    /// Typical value: 2-4
    pub longrange_connections: usize,

    /// Probability of rewiring a local connection to random node
    /// Small-world networks typically use 0.01-0.1
    pub rewiring_probability: f64,

    /// Maximum latency (ms) before marking node as slow
    pub max_latency_ms: u32,

    /// Health check interval (seconds)
    pub health_check_interval_secs: u64,
}

impl Default for TopologyConfig {
    fn default() -> Self {
        Self {
            local_connections: 6,        // Connect to 3 neighbors on each side
            longrange_connections: 3,     // 3 random shortcuts
            rewiring_probability: 0.05,   // 5% chance of rewiring
            max_latency_ms: 100,          // 100ms max latency
            health_check_interval_secs: 10,
        }
    }
}

/// Small-World Network Topology Manager
///
/// Maintains the network structure and routing information for distributed nodes.
pub struct SmallWorldTopology {
    config: TopologyConfig,
    /// This node's ID
    local_id: NodeId,
    /// All known nodes in the network
    nodes: Arc<RwLock<HashMap<NodeId, NodeInfo>>>,
    /// Outgoing connections from this node
    connections: Arc<RwLock<HashMap<NodeId, Connection>>>,
}

impl SmallWorldTopology {
    /// Create a new small-world topology for this node
    pub fn new(local_id: NodeId, config: TopologyConfig) -> Self {
        Self {
            config,
            local_id,
            nodes: Arc::new(RwLock::new(HashMap::new())),
            connections: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get this node's ID
    pub fn local_id(&self) -> NodeId {
        self.local_id
    }

    /// Register a new node in the network
    pub fn add_node(&self, node: NodeInfo) {
        let mut nodes = self.nodes.write().unwrap();
        nodes.insert(node.id, node);
    }

    /// Remove a node from the network
    pub fn remove_node(&self, node_id: NodeId) {
        let mut nodes = self.nodes.write().unwrap();
        nodes.remove(&node_id);

        let mut connections = self.connections.write().unwrap();
        connections.remove(&node_id);
    }

    /// Get all nodes in the network
    pub fn get_all_nodes(&self) -> Vec<NodeInfo> {
        let nodes = self.nodes.read().unwrap();
        nodes.values().cloned().collect()
    }

    /// Get a specific node's info
    pub fn get_node(&self, node_id: NodeId) -> Option<NodeInfo> {
        let nodes = self.nodes.read().unwrap();
        nodes.get(&node_id).cloned()
    }

    /// Build small-world topology connections
    ///
    /// Creates a ring topology with local connections to k-nearest neighbors,
    /// then adds random long-range connections for shortcuts.
    pub fn build_topology(&self) {
        let nodes = self.nodes.read().unwrap();
        let node_ids: Vec<NodeId> = nodes.keys().copied().collect();
        drop(nodes);

        if node_ids.len() <= 1 {
            return; // No connections needed for single node
        }

        let mut new_connections = HashMap::new();

        // Step 1: Create ring topology with k local connections
        let my_index = node_ids.iter().position(|&id| id == self.local_id);
        if let Some(idx) = my_index {
            let n = node_ids.len();

            // Connect to k/2 neighbors on each side
            for offset in 1..=(self.config.local_connections / 2) {
                // Right neighbor
                let right_idx = (idx + offset) % n;
                new_connections.insert(
                    node_ids[right_idx],
                    Connection {
                        target_id: node_ids[right_idx],
                        connection_type: ConnectionType::Local,
                        latency_ms: 0, // Will be measured
                    },
                );

                // Left neighbor
                let left_idx = (idx + n - offset) % n;
                new_connections.insert(
                    node_ids[left_idx],
                    Connection {
                        target_id: node_ids[left_idx],
                        connection_type: ConnectionType::Local,
                        latency_ms: 0,
                    },
                );
            }

            // Step 2: Add long-range connections (random shortcuts)
            use rand::Rng;
            let mut rng = rand::thread_rng();

            for _ in 0..self.config.longrange_connections {
                // Choose random node that's not local neighbor
                let mut attempts = 0;
                while attempts < 10 {
                    let random_idx = rng.gen_range(0..n);
                    let random_id = node_ids[random_idx];

                    // Skip if it's self or already connected
                    if random_id == self.local_id || new_connections.contains_key(&random_id) {
                        attempts += 1;
                        continue;
                    }

                    new_connections.insert(
                        random_id,
                        Connection {
                            target_id: random_id,
                            connection_type: ConnectionType::LongRange,
                            latency_ms: 0,
                        },
                    );
                    break;
                }
            }
        }

        // Update connections
        let mut connections = self.connections.write().unwrap();
        *connections = new_connections;
    }

    /// Get all connections from this node
    pub fn get_connections(&self) -> Vec<Connection> {
        let connections = self.connections.read().unwrap();
        connections.values().cloned().collect()
    }

    /// Update latency for a connection
    pub fn update_latency(&self, node_id: NodeId, latency_ms: u32) {
        let mut connections = self.connections.write().unwrap();
        if let Some(conn) = connections.get_mut(&node_id) {
            conn.latency_ms = latency_ms;
        }
    }

    /// Find shortest path to target node using BFS
    ///
    /// Returns list of node IDs representing the path, or None if unreachable.
    pub fn find_route(&self, target_id: NodeId) -> Option<Vec<NodeId>> {
        if target_id == self.local_id {
            return Some(vec![self.local_id]);
        }

        let connections = self.connections.read().unwrap();
        if connections.contains_key(&target_id) {
            // Direct connection
            return Some(vec![self.local_id, target_id]);
        }

        // BFS to find shortest path
        let mut queue = VecDeque::new();
        let mut visited = HashSet::new();
        let mut parent: HashMap<NodeId, NodeId> = HashMap::new();

        queue.push_back(self.local_id);
        visited.insert(self.local_id);

        while let Some(current) = queue.pop_front() {
            if current == target_id {
                // Reconstruct path
                let mut path = vec![target_id];
                let mut node = target_id;
                while let Some(&prev) = parent.get(&node) {
                    path.push(prev);
                    node = prev;
                }
                path.reverse();
                return Some(path);
            }

            // Get neighbors of current node
            if let Some(neighbors) = self.get_neighbors(current) {
                for neighbor in neighbors {
                    if !visited.contains(&neighbor) {
                        visited.insert(neighbor);
                        parent.insert(neighbor, current);
                        queue.push_back(neighbor);
                    }
                }
            }
        }

        None // Target unreachable
    }

    /// Get neighbors of a node (for routing purposes)
    fn get_neighbors(&self, node_id: NodeId) -> Option<Vec<NodeId>> {
        if node_id == self.local_id {
            let connections = self.connections.read().unwrap();
            Some(connections.keys().copied().collect())
        } else {
            // For other nodes, we'd need to query them or maintain global routing table
            // For now, return None (will implement in distributed query execution)
            None
        }
    }

    /// Calculate average path length (network diameter metric)
    pub fn calculate_avg_path_length(&self) -> f64 {
        let nodes = self.nodes.read().unwrap();
        let node_ids: Vec<NodeId> = nodes.keys().copied().collect();
        drop(nodes);

        if node_ids.len() <= 1 {
            return 0.0;
        }

        let mut total_distance = 0;
        let mut path_count = 0;

        for &target in &node_ids {
            if target == self.local_id {
                continue;
            }

            if let Some(path) = self.find_route(target) {
                total_distance += path.len() - 1; // Path length = nodes - 1
                path_count += 1;
            }
        }

        if path_count == 0 {
            0.0
        } else {
            total_distance as f64 / path_count as f64
        }
    }

    /// Calculate clustering coefficient (measure of local connectivity)
    pub fn calculate_clustering_coefficient(&self) -> f64 {
        let connections = self.connections.read().unwrap();
        let neighbors: Vec<NodeId> = connections.keys().copied().collect();
        drop(connections);

        if neighbors.len() < 2 {
            return 0.0;
        }

        // Count triangles: how many of my neighbors are connected to each other
        let mut triangle_count = 0;
        let max_triangles = neighbors.len() * (neighbors.len() - 1) / 2;

        // For simplicity, we'll estimate based on local information
        // In a full implementation, we'd query each neighbor's connections

        // This is a simplified calculation
        triangle_count as f64 / max_triangles as f64
    }

    /// Get network statistics
    pub fn get_statistics(&self) -> TopologyStatistics {
        let nodes = self.nodes.read().unwrap();
        let connections = self.connections.read().unwrap();

        let local_count = connections
            .values()
            .filter(|c| c.connection_type == ConnectionType::Local)
            .count();

        let longrange_count = connections
            .values()
            .filter(|c| c.connection_type == ConnectionType::LongRange)
            .count();

        let avg_latency = if connections.is_empty() {
            0.0
        } else {
            let total: u32 = connections.values().map(|c| c.latency_ms).sum();
            total as f64 / connections.len() as f64
        };

        TopologyStatistics {
            total_nodes: nodes.len(),
            total_connections: connections.len(),
            local_connections: local_count,
            longrange_connections: longrange_count,
            avg_latency_ms: avg_latency,
            avg_path_length: self.calculate_avg_path_length(),
            clustering_coefficient: self.calculate_clustering_coefficient(),
        }
    }
}

/// Network topology statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopologyStatistics {
    pub total_nodes: usize,
    pub total_connections: usize,
    pub local_connections: usize,
    pub longrange_connections: usize,
    pub avg_latency_ms: f64,
    pub avg_path_length: f64,
    pub clustering_coefficient: f64,
}

/// Helper function to get current Unix timestamp
fn current_timestamp() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_address() {
        let addr = NodeAddress::new("192.168.1.10".to_string(), 8080);
        assert_eq!(addr.to_string(), "192.168.1.10:8080");
    }

    #[test]
    fn test_add_remove_node() {
        let topology = SmallWorldTopology::new(1, TopologyConfig::default());

        let node2 = NodeInfo::new(2, NodeAddress::new("localhost".to_string(), 8081), 100);
        topology.add_node(node2.clone());

        assert_eq!(topology.get_all_nodes().len(), 1);
        assert_eq!(topology.get_node(2).unwrap().id, 2);

        topology.remove_node(2);
        assert_eq!(topology.get_all_nodes().len(), 0);
    }

    #[test]
    fn test_build_topology_single_node() {
        let topology = SmallWorldTopology::new(1, TopologyConfig::default());
        topology.build_topology();

        assert_eq!(topology.get_connections().len(), 0);
    }

    #[test]
    fn test_build_topology_multiple_nodes() {
        let config = TopologyConfig {
            local_connections: 4,
            longrange_connections: 2,
            rewiring_probability: 0.05,
            max_latency_ms: 100,
            health_check_interval_secs: 10,
        };

        let topology = SmallWorldTopology::new(1, config);

        // Add 10 nodes
        topology.add_node(NodeInfo::new(1, NodeAddress::new("localhost".to_string(), 8081), 100));
        for i in 2..=10 {
            topology.add_node(NodeInfo::new(i, NodeAddress::new("localhost".to_string(), 8080 + i as u16), 100));
        }

        topology.build_topology();

        let connections = topology.get_connections();

        // Should have local_connections + longrange_connections
        // (4 local + 2 long-range = 6 connections)
        assert!(connections.len() <= 6);
        assert!(connections.len() >= 4); // At least local connections

        // Verify we have both types
        let local_count = connections.iter().filter(|c| c.connection_type == ConnectionType::Local).count();
        let longrange_count = connections.iter().filter(|c| c.connection_type == ConnectionType::LongRange).count();

        assert!(local_count > 0);
        assert!(longrange_count > 0);
    }

    #[test]
    fn test_find_route_direct() {
        let topology = SmallWorldTopology::new(1, TopologyConfig::default());

        topology.add_node(NodeInfo::new(1, NodeAddress::new("localhost".to_string(), 8081), 100));
        topology.add_node(NodeInfo::new(2, NodeAddress::new("localhost".to_string(), 8082), 100));

        topology.build_topology();

        // Should find direct route from 1 to 2
        let route = topology.find_route(2);
        assert!(route.is_some());
        let path = route.unwrap();
        assert_eq!(path, vec![1, 2]);
    }

    #[test]
    fn test_update_latency() {
        let topology = SmallWorldTopology::new(1, TopologyConfig::default());

        topology.add_node(NodeInfo::new(1, NodeAddress::new("localhost".to_string(), 8081), 100));
        topology.add_node(NodeInfo::new(2, NodeAddress::new("localhost".to_string(), 8082), 100));

        topology.build_topology();
        topology.update_latency(2, 50);

        let connections = topology.get_connections();
        let conn_to_2 = connections.iter().find(|c| c.target_id == 2);
        assert!(conn_to_2.is_some());
        assert_eq!(conn_to_2.unwrap().latency_ms, 50);
    }

    #[test]
    fn test_statistics() {
        let topology = SmallWorldTopology::new(1, TopologyConfig::default());

        for i in 1..=5 {
            topology.add_node(NodeInfo::new(i, NodeAddress::new("localhost".to_string(), 8080 + i as u16), 100));
        }

        topology.build_topology();

        let stats = topology.get_statistics();
        assert_eq!(stats.total_nodes, 5);
        assert!(stats.total_connections > 0);
        assert!(stats.local_connections > 0);
    }
}
