//! Graph structures optimized for hybrid relational + graph workloads
//!
//! Key optimizations:
//! - Lock-free concurrent access (DashMap)
//! - Cache-friendly memory layout
//! - Pheromone tracking for biological optimization
//! - Vectorized operations where possible

use crate::types::*;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

/// Entity (universal node)
///
/// Combines table row and graph vertex into single structure.
/// Optimized for cache locality and concurrent access.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    pub id: EntityId,
    pub entity_type: EntityType,
    pub properties: Properties,

    // Metadata for optimization
    pub access_count: u64,
    pub last_accessed: SystemTime,
    pub created_at: SystemTime,
}

impl Entity {
    pub fn new(id: EntityId, entity_type: EntityType, properties: Properties) -> Self {
        let now = SystemTime::now();
        Entity {
            id,
            entity_type,
            properties,
            access_count: 0,
            last_accessed: now,
            created_at: now,
        }
    }

    /// Mark entity as accessed (for pheromone tracking)
    pub fn mark_accessed(&mut self) {
        self.access_count += 1;
        self.last_accessed = SystemTime::now();
    }

    /// Get property value
    pub fn get_property(&self, key: &str) -> Option<&PropertyValue> {
        self.properties.get(key)
    }

    /// Set property value
    pub fn set_property(&mut self, key: String, value: PropertyValue) {
        self.properties.insert(key, value);
    }
}

/// Edge (directed relationship with pheromone)
///
/// Includes biological pheromone tracking for adaptive routing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub id: EdgeId,
    pub source: EntityId,
    pub target: EntityId,
    pub edge_type: EdgeType,
    pub properties: Properties,

    // Biological optimization
    pub pheromone: Pheromone,
    pub traversal_count: u64,
    pub avg_latency_ns: u64,

    // Metadata
    pub created_at: SystemTime,
    pub last_traversed: SystemTime,
}

impl Edge {
    pub fn new(
        id: EdgeId,
        source: EntityId,
        target: EntityId,
        edge_type: EdgeType,
        properties: Properties,
    ) -> Self {
        let now = SystemTime::now();
        Edge {
            id,
            source,
            target,
            edge_type,
            properties,
            pheromone: Pheromone::default(),
            traversal_count: 0,
            avg_latency_ns: 0,
            created_at: now,
            last_traversed: now,
        }
    }

    /// Mark edge as traversed during query
    pub fn mark_traversed(&mut self, latency_ns: u64) {
        self.traversal_count += 1;
        self.last_traversed = SystemTime::now();

        // Update exponential moving average of latency
        let alpha = 0.3;
        if self.avg_latency_ns == 0 {
            self.avg_latency_ns = latency_ns;
        } else {
            self.avg_latency_ns =
                ((alpha * latency_ns as f64) + ((1.0 - alpha) * self.avg_latency_ns as f64)) as u64;
        }

        // Reinforce pheromone inversely proportional to latency
        // Fast edges get stronger pheromone
        let reinforcement = 1.0 / (1.0 + (latency_ns as f32 / 1_000_000.0));
        self.pheromone.reinforce(reinforcement);
    }

    /// Get edge weight for routing (lower = better)
    pub fn weight(&self) -> f32 {
        // Base weight is inverse of pheromone
        let base = 1.0 / self.pheromone.strength();

        // Adjust by historical latency if available
        if self.avg_latency_ns > 0 {
            let latency_factor = (self.avg_latency_ns as f32).ln() / 10.0;
            base * (1.0 + latency_factor)
        } else {
            base
        }
    }
}

/// Adjacency list for fast graph traversal
///
/// Maps entity -> {edge_type -> [(target_entity, edge_id)]}
type AdjacencyList = DashMap<EntityId, DashMap<EdgeType, Vec<(EntityId, EdgeId)>>>;

/// In-memory graph structure
///
/// Uses concurrent data structures for lock-free access.
/// Production version would use RocksDB for persistence.
pub struct Graph {
    // Entity storage
    entities: DashMap<EntityId, Entity>,

    // Edge storage
    edges: DashMap<EdgeId, Edge>,

    // Adjacency lists for fast traversal
    outgoing: AdjacencyList,
    incoming: AdjacencyList,

    // Collections (table-like groupings)
    collections: DashMap<EntityType, Vec<EntityId>>,

    // ID generators
    next_entity_id: AtomicU64,
    next_edge_id: AtomicU64,
}

impl Graph {
    pub fn new() -> Self {
        Graph {
            entities: DashMap::new(),
            edges: DashMap::new(),
            outgoing: DashMap::new(),
            incoming: DashMap::new(),
            collections: DashMap::new(),
            next_entity_id: AtomicU64::new(1),
            next_edge_id: AtomicU64::new(1),
        }
    }

    /// Add a new entity
    pub fn add_entity(&self, entity_type: EntityType, properties: Properties) -> EntityId {
        let id = EntityId::new(self.next_entity_id.fetch_add(1, Ordering::SeqCst));
        let entity = Entity::new(id, entity_type.clone(), properties);

        self.entities.insert(id, entity);

        // Add to collection
        self.collections
            .entry(entity_type)
            .or_insert_with(Vec::new)
            .push(id);

        // Initialize adjacency lists
        self.outgoing.insert(id, DashMap::new());
        self.incoming.insert(id, DashMap::new());

        id
    }

    /// Get entity by ID
    pub fn get_entity(&self, id: EntityId) -> Option<Entity> {
        self.entities.get(&id).map(|e| {
            let mut entity = e.clone();
            entity.mark_accessed();
            entity
        })
    }

    /// Add a new edge
    pub fn add_edge(
        &self,
        source: EntityId,
        target: EntityId,
        edge_type: EdgeType,
        properties: Properties,
    ) -> Option<EdgeId> {
        // Check that source and target exist
        if !self.entities.contains_key(&source) || !self.entities.contains_key(&target) {
            return None;
        }

        let id = EdgeId::new(self.next_edge_id.fetch_add(1, Ordering::SeqCst));
        let edge = Edge::new(id, source, target, edge_type.clone(), properties);

        self.edges.insert(id, edge);

        // Update outgoing adjacency list
        let outgoing_entry = self.outgoing.get(&source).unwrap();
        outgoing_entry
            .entry(edge_type.clone())
            .or_insert_with(Vec::new)
            .push((target, id));

        // Update incoming adjacency list
        let incoming_entry = self.incoming.get(&target).unwrap();
        incoming_entry
            .entry(edge_type)
            .or_insert_with(Vec::new)
            .push((source, id));

        Some(id)
    }

    /// Get edge by ID
    pub fn get_edge(&self, id: EdgeId) -> Option<Edge> {
        self.edges.get(&id).map(|e| e.clone())
    }

    /// Get outgoing neighbors of an entity
    pub fn get_outgoing_neighbors(
        &self,
        entity_id: EntityId,
        edge_type: Option<&str>,
    ) -> Vec<(EntityId, EdgeId)> {
        let mut result = Vec::new();

        if let Some(outgoing) = self.outgoing.get(&entity_id) {
            if let Some(edge_type) = edge_type {
                if let Some(neighbors) = outgoing.get(edge_type) {
                    result.extend(neighbors.iter().cloned());
                }
            } else {
                // All edge types
                for neighbors in outgoing.iter() {
                    result.extend(neighbors.value().iter().cloned());
                }
            }
        }

        result
    }

    /// Get incoming neighbors of an entity
    pub fn get_incoming_neighbors(
        &self,
        entity_id: EntityId,
        edge_type: Option<&str>,
    ) -> Vec<(EntityId, EdgeId)> {
        let mut result = Vec::new();

        if let Some(incoming) = self.incoming.get(&entity_id) {
            if let Some(edge_type) = edge_type {
                if let Some(neighbors) = incoming.get(edge_type) {
                    result.extend(neighbors.iter().cloned());
                }
            } else {
                // All edge types
                for neighbors in incoming.iter() {
                    result.extend(neighbors.value().iter().cloned());
                }
            }
        }

        result
    }

    /// Scan all entities in a collection (table scan)
    pub fn scan_collection(&self, entity_type: &str) -> Vec<Entity> {
        if let Some(entity_ids) = self.collections.get(entity_type) {
            entity_ids
                .iter()
                .filter_map(|id| self.get_entity(*id))
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Evaporate pheromones on all edges (called periodically)
    pub fn evaporate_pheromones(&self) {
        for mut edge in self.edges.iter_mut() {
            edge.pheromone.evaporate();
        }
    }

    /// Get statistics
    pub fn stats(&self) -> GraphStats {
        GraphStats {
            entity_count: self.entities.len(),
            edge_count: self.edges.len(),
            collection_count: self.collections.len(),
            avg_pheromone: self.average_pheromone(),
        }
    }

    /// Get all entities (for backup)
    pub fn get_all_entities(&self) -> Vec<Entity> {
        self.entities.iter().map(|e| e.value().clone()).collect()
    }

    /// Get all edges (for backup)
    pub fn get_all_edges(&self) -> Vec<Edge> {
        self.edges.iter().map(|e| e.value().clone()).collect()
    }

    /// Insert entity with specific ID (for restore)
    pub fn insert_entity_with_id(&self, entity: Entity) {
        let id = entity.id;
        let entity_type = entity.entity_type.clone();

        // Insert into entities map
        self.entities.insert(id, entity);

        // Add to collections
        let mut collections = self.collections.entry(entity_type.clone())
            .or_insert_with(Vec::new);
        if !collections.contains(&id) {
            collections.push(id);
        }

        // Update next ID if necessary
        if id.0 >= self.next_entity_id.load(std::sync::atomic::Ordering::SeqCst) {
            self.next_entity_id.store(id.0 + 1, std::sync::atomic::Ordering::SeqCst);
        }
    }

    /// Insert edge with specific ID (for restore)
    pub fn insert_edge_with_id(&self, edge: Edge) {
        let id = edge.id;
        let from = edge.from;
        let to = edge.to;
        let edge_type = edge.edge_type.clone();

        // Insert into edges map
        self.edges.insert(id, edge);

        // Add to outgoing neighbors
        self.outgoing.entry(from)
            .or_insert_with(Vec::new)
            .push((to, id, edge_type.clone()));

        // Add to incoming neighbors
        self.incoming.entry(to)
            .or_insert_with(Vec::new)
            .push((from, id, edge_type));

        // Update next ID if necessary
        if id.0 >= self.next_edge_id.load(std::sync::atomic::Ordering::SeqCst) {
            self.next_edge_id.store(id.0 + 1, std::sync::atomic::Ordering::SeqCst);
        }
    }

    /// Create entity with properties (alias for add_entity)
    pub fn create_entity(&self, entity_type: String, properties: Properties) -> EntityId {
        self.add_entity(entity_type, properties)
    }

    fn average_pheromone(&self) -> f32 {
        if self.edges.is_empty() {
            return 0.0;
        }

        let sum: f32 = self.edges.iter().map(|e| e.pheromone.strength()).sum();
        sum / self.edges.len() as f32
    }
}

impl Default for Graph {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphStats {
    pub entity_count: usize,
    pub edge_count: usize,
    pub collection_count: usize,
    pub avg_pheromone: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entity_creation() {
        let graph = Graph::new();

        let mut props = Properties::new();
        props.insert("name".to_string(), PropertyValue::String("Alice".to_string()));
        props.insert("age".to_string(), PropertyValue::Int(28));

        let id = graph.add_entity("User".to_string(), props);

        let entity = graph.get_entity(id).unwrap();
        assert_eq!(entity.entity_type, "User");
        assert_eq!(entity.get_property("name").unwrap().as_str().unwrap(), "Alice");
        assert_eq!(entity.get_property("age").unwrap().as_i64().unwrap(), 28);
    }

    #[test]
    fn test_edge_creation() {
        let graph = Graph::new();

        let alice = graph.add_entity("User".to_string(), Properties::new());
        let bob = graph.add_entity("User".to_string(), Properties::new());

        let edge_id = graph.add_edge(
            alice,
            bob,
            "FOLLOWS".to_string(),
            Properties::new(),
        ).unwrap();

        let edge = graph.get_edge(edge_id).unwrap();
        assert_eq!(edge.source, alice);
        assert_eq!(edge.target, bob);
        assert_eq!(edge.edge_type, "FOLLOWS");
    }

    #[test]
    fn test_graph_traversal() {
        let graph = Graph::new();

        let alice = graph.add_entity("User".to_string(), Properties::new());
        let bob = graph.add_entity("User".to_string(), Properties::new());
        let carol = graph.add_entity("User".to_string(), Properties::new());

        graph.add_edge(alice, bob, "FOLLOWS".to_string(), Properties::new());
        graph.add_edge(alice, carol, "FOLLOWS".to_string(), Properties::new());

        let neighbors = graph.get_outgoing_neighbors(alice, Some("FOLLOWS"));
        assert_eq!(neighbors.len(), 2);
    }

    #[test]
    fn test_pheromone_reinforcement() {
        let mut edge = Edge::new(
            EdgeId::new(1),
            EntityId::new(1),
            EntityId::new(2),
            "TEST".to_string(),
            Properties::new(),
        );

        let initial = edge.pheromone.strength();
        edge.mark_traversed(1_000_000); // 1ms latency
        assert!(edge.pheromone.strength() > initial);
        assert_eq!(edge.traversal_count, 1);
    }
}
