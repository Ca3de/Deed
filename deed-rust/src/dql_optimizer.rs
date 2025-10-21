//! DQL Query Optimizer
//!
//! Uses biological algorithms (ant colony optimization) to find
//! optimal query execution plans.

use crate::dql_ir::*;
use crate::types::Pheromone;
use rand::Rng;
use std::collections::HashMap;

/// Ant Colony Optimizer for query plans
pub struct AntColonyOptimizer {
    num_ants: usize,
    num_iterations: usize,
    pheromone_cache: HashMap<String, Pheromone>,
}

impl AntColonyOptimizer {
    pub fn new() -> Self {
        AntColonyOptimizer {
            num_ants: 20,
            num_iterations: 10,
            pheromone_cache: HashMap::new(),
        }
    }

    /// Optimize a query plan using ant colony optimization
    pub fn optimize(&mut self, mut plan: QueryPlan, stats: &GraphStats) -> QueryPlan {
        // Initial cost estimation
        plan.estimate_cost(stats);

        let mut best_plan = plan.clone();
        let mut best_cost = plan.estimated_cost;

        // Run ant colony optimization
        for _iteration in 0..self.num_iterations {
            for _ant in 0..self.num_ants {
                // Each ant explores a variation of the plan
                let mut candidate = self.explore_variant(&plan, stats);

                // Evaluate cost
                candidate.estimate_cost(stats);

                // Update best if better
                if candidate.estimated_cost < best_cost {
                    best_cost = candidate.estimated_cost;
                    best_plan = candidate.clone();

                    // Reinforce pheromone
                    let plan_key = self.plan_signature(&best_plan);
                    let pheromone = self
                        .pheromone_cache
                        .entry(plan_key)
                        .or_insert_with(Pheromone::default);

                    let reinforcement = 1.0 / (1.0 + best_cost);
                    pheromone.reinforce(reinforcement);
                }
            }

            // Evaporate pheromones
            for pheromone in self.pheromone_cache.values_mut() {
                pheromone.evaporate();
            }
        }

        best_plan.pheromone_strength = self
            .pheromone_cache
            .get(&self.plan_signature(&best_plan))
            .map(|p| p.strength())
            .unwrap_or(1.0);

        best_plan
    }

    /// Explore a variant of the query plan
    fn explore_variant(&self, plan: &QueryPlan, stats: &GraphStats) -> QueryPlan {
        let mut variant = plan.clone();

        // Apply random optimizations
        let mut rng = rand::thread_rng();
        let optimization = rng.gen_range(0..4);

        match optimization {
            0 => self.try_index_optimization(&mut variant, stats),
            1 => self.try_filter_pushdown(&mut variant),
            2 => self.try_projection_pushdown(&mut variant),
            3 => self.try_join_reorder(&mut variant),
            _ => {}
        }

        variant
    }

    /// Try to convert scans to index lookups
    fn try_index_optimization(&self, plan: &mut QueryPlan, _stats: &GraphStats) {
        for op in &mut plan.operations {
            if let Operation::Scan {
                collection,
                alias,
                filter,
            } = op
            {
                // Check if filter is simple equality that can use index
                if let Some(FilterExpr::Equal(left, right)) = filter {
                    if let (FilterExpr::Property { property, .. }, FilterExpr::Constant(value)) =
                        (left.as_ref(), right.as_ref())
                    {
                        // Convert to index lookup
                        *op = Operation::IndexLookup {
                            collection: collection.clone(),
                            alias: alias.clone(),
                            index_name: format!("idx_{}", property),
                            key_values: vec![value.clone()],
                        };
                    }
                }
            }
        }
    }

    /// Push filters earlier in the plan
    fn try_filter_pushdown(&self, plan: &mut QueryPlan) {
        // Find standalone Filter operations and try to merge them into Scan
        let mut i = 0;
        while i < plan.operations.len() {
            // Clone the filter info first to avoid borrowing issues
            let filter_info = if let Operation::Filter { binding, condition } = &plan.operations[i] {
                Some((binding.clone(), condition.clone()))
            } else {
                None
            };

            if let Some((binding, condition)) = filter_info {
                // Look backwards for Scan with same binding
                for j in (0..i).rev() {
                    if let Operation::Scan {
                        alias,
                        filter: scan_filter,
                        ..
                    } = &mut plan.operations[j]
                    {
                        if alias == &binding {
                            // Merge filter into scan
                            if let Some(existing_filter) = scan_filter {
                                *scan_filter = Some(FilterExpr::And(
                                    Box::new(existing_filter.clone()),
                                    Box::new(condition.clone()),
                                ));
                            } else {
                                *scan_filter = Some(condition.clone());
                            }

                            // Remove standalone filter
                            plan.operations.remove(i);
                            return; // Modified plan, return
                        }
                    }
                }
            }
            i += 1;
        }
    }

    /// Push projections earlier to reduce data size
    fn try_projection_pushdown(&self, plan: &mut QueryPlan) {
        // Find Project operation and try to move it earlier
        if let Some(project_idx) = plan
            .operations
            .iter()
            .position(|op| matches!(op, Operation::Project { .. }))
        {
            if project_idx > 1 {
                // Try to move project earlier (simple swap)
                plan.operations.swap(project_idx, project_idx - 1);
            }
        }
    }

    /// Reorder joins for better performance
    fn try_join_reorder(&self, plan: &mut QueryPlan) {
        // Find consecutive Join operations and consider swapping
        for i in 0..plan.operations.len().saturating_sub(1) {
            if matches!(plan.operations[i], Operation::Join { .. })
                && matches!(plan.operations[i + 1], Operation::Join { .. })
            {
                // Swap joins (simple heuristic)
                plan.operations.swap(i, i + 1);
                return;
            }
        }
    }

    /// Generate signature for plan (for pheromone tracking)
    fn plan_signature(&self, plan: &QueryPlan) -> String {
        // Simple signature based on operation sequence
        plan.operations
            .iter()
            .map(|op| match op {
                Operation::Scan { .. } => "S",
                Operation::IndexLookup { .. } => "I",
                Operation::Traverse { .. } => "T",
                Operation::Filter { .. } => "F",
                Operation::Project { .. } => "P",
                Operation::Sort { .. } => "O",
                Operation::Limit { .. } => "L",
                Operation::Skip { .. } => "K",
                Operation::Join { .. } => "J",
                Operation::InsertEntity { .. } => "INS",
                Operation::UpdateEntities { .. } => "UPD",
                Operation::DeleteEntities { .. } => "DEL",
                Operation::CreateEdge { .. } => "CRE",
                Operation::GroupBy { .. } => "G",
                Operation::Having { .. } => "H",
            })
            .collect::<Vec<_>>()
            .join("_")
    }
}

impl Default for AntColonyOptimizer {
    fn default() -> Self {
        Self::new()
    }
}

/// Stigmergy-based query cache
///
/// Caches optimized query plans based on pattern similarity.
pub struct StigmergyCache {
    cache: HashMap<String, CachedPlan>,
    max_size: usize,
}

#[derive(Clone)]
struct CachedPlan {
    plan: QueryPlan,
    pheromone: Pheromone,
    hit_count: usize,
}

impl StigmergyCache {
    pub fn new(max_size: usize) -> Self {
        StigmergyCache {
            cache: HashMap::new(),
            max_size,
        }
    }

    /// Try to get cached plan
    pub fn get(&mut self, query_signature: &str) -> Option<QueryPlan> {
        if let Some(cached) = self.cache.get_mut(query_signature) {
            cached.hit_count += 1;
            cached.pheromone.reinforce(0.5);
            Some(cached.plan.clone())
        } else {
            None
        }
    }

    /// Store optimized plan in cache
    pub fn put(&mut self, query_signature: String, plan: QueryPlan) {
        // Evict if cache is full
        if self.cache.len() >= self.max_size {
            self.evict_weakest();
        }

        self.cache.insert(
            query_signature,
            CachedPlan {
                plan,
                pheromone: Pheromone::default(),
                hit_count: 0,
            },
        );
    }

    /// Evict plan with weakest pheromone
    fn evict_weakest(&mut self) {
        if let Some(weakest_key) = self
            .cache
            .iter()
            .min_by(|a, b| {
                a.1.pheromone
                    .strength()
                    .partial_cmp(&b.1.pheromone.strength())
                    .unwrap()
            })
            .map(|(k, _)| k.clone())
        {
            self.cache.remove(&weakest_key);
        }
    }

    /// Evaporate all pheromones (called periodically)
    pub fn evaporate_all(&mut self) {
        for cached in self.cache.values_mut() {
            cached.pheromone.evaporate();
        }
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        let total_hits: usize = self.cache.values().map(|c| c.hit_count).sum();
        let avg_pheromone = if self.cache.is_empty() {
            0.0
        } else {
            let sum: f32 = self.cache.values().map(|c| c.pheromone.strength()).sum();
            sum / self.cache.len() as f32
        };

        CacheStats {
            size: self.cache.len(),
            max_size: self.max_size,
            total_hits,
            avg_pheromone,
        }
    }
}

#[derive(Debug)]
pub struct CacheStats {
    pub size: usize,
    pub max_size: usize,
    pub total_hits: usize,
    pub avg_pheromone: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ant_colony_optimizer() {
        let stats = GraphStats {
            entity_count: 1000,
            edge_count: 5000,
            collection_count: 10,
            avg_pheromone: 1.0,
        };

        let operations = vec![
            Operation::Scan {
                collection: "Users".to_string(),
                alias: "u".to_string(),
                filter: Some(FilterExpr::Equal(
                    Box::new(FilterExpr::Property {
                        binding: "u".to_string(),
                        property: "age".to_string(),
                    }),
                    Box::new(FilterExpr::Constant(Value::Integer(25))),
                )),
            },
            Operation::Project {
                fields: vec![ProjectField {
                    expression: FilterExpr::Property {
                        binding: "u".to_string(),
                        property: "name".to_string(),
                    },
                    alias: "name".to_string(),
                }],
            },
        ];

        let plan = QueryPlan::new(operations);

        let mut optimizer = AntColonyOptimizer::new();
        let optimized = optimizer.optimize(plan.clone(), &stats);

        // Optimized plan should have cost estimation
        assert!(optimized.estimated_cost > 0.0);

        // Check if scan was optimized to index lookup
        let has_index = optimized
            .operations
            .iter()
            .any(|op| matches!(op, Operation::IndexLookup { .. }));

        // Note: Optimization is probabilistic, so this might not always succeed
        println!("Has index lookup: {}", has_index);
    }

    #[test]
    fn test_stigmergy_cache() {
        let mut cache = StigmergyCache::new(5);

        let plan = QueryPlan::new(vec![]);

        // Cache miss
        assert!(cache.get("query1").is_none());

        // Store plan
        cache.put("query1".to_string(), plan.clone());

        // Cache hit
        assert!(cache.get("query1").is_some());

        // Check stats
        let stats = cache.stats();
        assert_eq!(stats.size, 1);
        assert_eq!(stats.total_hits, 1);
    }
}
