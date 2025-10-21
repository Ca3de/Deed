//! Query execution engine
//!
//! Executes optimized query plans with vectorized processing.

use crate::graph::{Graph, Entity};
use crate::types::*;
use crate::transaction::IsolationLevel;
use std::sync::Arc;

/// Query executor
pub struct Executor {
    graph: Arc<Graph>,
}

impl Executor {
    pub fn new(graph: Arc<Graph>) -> Self {
        Executor { graph }
    }

    /// Execute a simple filter query (WHERE clause)
    ///
    /// This is a simplified version. Production would have:
    /// - Vectorized filtering (SIMD)
    /// - Predicate pushdown
    /// - Column pruning
    pub fn execute_filter(
        &self,
        collection: &str,
        predicate: Box<dyn Fn(&Entity) -> bool + Send + Sync>,
    ) -> Vec<Entity> {
        self.graph
            .scan_collection(collection)
            .into_iter()
            .filter(|e| predicate(e))
            .collect()
    }

    /// Execute graph traversal
    pub fn execute_traversal(
        &self,
        start_id: EntityId,
        edge_type: Option<&str>,
        max_depth: usize,
    ) -> Vec<Entity> {
        let mut visited = std::collections::HashSet::new();
        let mut queue = vec![(start_id, 0)];
        let mut results = Vec::new();

        while let Some((current_id, depth)) = queue.pop() {
            if depth > max_depth || visited.contains(&current_id) {
                continue;
            }

            visited.insert(current_id);

            if depth > 0 {
                if let Some(entity) = self.graph.get_entity(current_id) {
                    results.push(entity);
                }
            }

            if depth < max_depth {
                let neighbors = self.graph.get_outgoing_neighbors(current_id, edge_type);
                for (neighbor_id, _edge_id) in neighbors {
                    if !visited.contains(&neighbor_id) {
                        queue.push((neighbor_id, depth + 1));
                    }
                }
            }
        }

        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_execution() {
        let graph = Arc::new(Graph::new());

        // Add test data
        for i in 1..=5 {
            let mut props = Properties::new();
            props.insert("age".to_string(), PropertyValue::Int(20 + i));

            graph.add_entity("User".to_string(), props);
        }

        let executor = Executor::new(graph);

        let results = executor.execute_filter(
            "User",
            Box::new(|e| {
                e.get_property("age")
                    .and_then(|v| v.as_i64())
                    .map(|age| age > 23)
                    .unwrap_or(false)
            }),
        );

        assert_eq!(results.len(), 2); // ages 24 and 25
    }
}
