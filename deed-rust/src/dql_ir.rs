//! DQL Intermediate Representation (IR)
//!
//! Lowered representation of DQL queries optimized for execution.
//! This is the output of the parser and input to the biological optimizer.

use crate::dql_ast::*;
use crate::types::{EntityId, EdgeId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// Re-export GraphStats from graph module to avoid duplication
pub use crate::graph::GraphStats;

/// Aggregate operation (for GROUP BY)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregateOp {
    pub function: AggregateFunc,
    pub argument: FilterExpr,
    pub alias: String,
}

/// Aggregate function type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AggregateFunc {
    Count,
    Sum,
    Avg,
    Min,
    Max,
}

impl From<&AggregateFunction> for AggregateFunc {
    fn from(func: &AggregateFunction) -> Self {
        match func {
            AggregateFunction::Count => AggregateFunc::Count,
            AggregateFunction::Sum => AggregateFunc::Sum,
            AggregateFunction::Avg => AggregateFunc::Avg,
            AggregateFunction::Min => AggregateFunc::Min,
            AggregateFunction::Max => AggregateFunc::Max,
        }
    }
}

/// Query execution plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryPlan {
    pub operations: Vec<Operation>,
    pub estimated_cost: f32,
    pub pheromone_strength: f32,
}

impl QueryPlan {
    pub fn new(operations: Vec<Operation>) -> Self {
        QueryPlan {
            operations,
            estimated_cost: 0.0,
            pheromone_strength: 1.0,
        }
    }

    /// Calculate estimated cost based on operations
    pub fn estimate_cost(&mut self, stats: &GraphStats) {
        let mut cost = 0.0;

        for op in &self.operations {
            cost += op.estimate_cost(stats);
        }

        self.estimated_cost = cost;
    }
}

/// Individual operation in execution plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Operation {
    /// Scan collection (table scan)
    Scan {
        collection: String,
        alias: String,
        filter: Option<FilterExpr>,
    },

    /// Index lookup (optimized scan)
    IndexLookup {
        collection: String,
        alias: String,
        index_name: String,
        key_values: Vec<Value>,
    },

    /// Graph traversal
    Traverse {
        source_binding: String,
        direction: TraverseDirection,
        edge_type: Option<String>,
        target_alias: String,
        min_hops: usize,
        max_hops: usize,
        filter: Option<FilterExpr>,
    },

    /// Filter results
    Filter {
        binding: String,
        condition: FilterExpr,
    },

    /// Project (select) fields
    Project {
        fields: Vec<ProjectField>,
    },

    /// Sort results
    Sort {
        fields: Vec<SortField>,
    },

    /// Limit results
    Limit {
        count: usize,
    },

    /// Skip results
    Skip {
        count: usize,
    },

    /// Join two result sets
    Join {
        left: String,
        right: String,
        condition: FilterExpr,
    },

    /// Insert entity
    InsertEntity {
        collection: String,
        properties: HashMap<String, Value>,
    },

    /// Update entities
    UpdateEntities {
        binding: String,
        updates: HashMap<String, FilterExpr>,
    },

    /// Delete entities
    DeleteEntities {
        binding: String,
    },

    /// Create edge
    CreateEdge {
        source: FilterExpr,
        target: FilterExpr,
        edge_type: String,
        properties: HashMap<String, Value>,
    },

    /// Group by aggregation
    GroupBy {
        group_fields: Vec<FilterExpr>,
        aggregates: Vec<AggregateOp>,
    },

    /// Filter aggregated results (HAVING)
    Having {
        condition: FilterExpr,
    },
}

impl Operation {
    /// Estimate cost of operation (for optimization)
    pub fn estimate_cost(&self, stats: &GraphStats) -> f32 {
        match self {
            Operation::Scan { .. } => {
                // Table scan is expensive
                stats.entity_count as f32
            }
            Operation::IndexLookup { .. } => {
                // Index lookup is cheap (log N)
                (stats.entity_count as f32).log2()
            }
            Operation::Traverse {
                min_hops, max_hops, ..
            } => {
                // Traversal cost grows exponentially with hops
                let avg_degree = if stats.entity_count > 0 {
                    stats.edge_count as f32 / stats.entity_count as f32
                } else {
                    2.0
                };
                let avg_hops = (min_hops + max_hops) as f32 / 2.0;
                avg_degree.powf(avg_hops)
            }
            Operation::Filter { .. } => {
                // Filter is linear in input size
                stats.entity_count as f32 * 0.5
            }
            Operation::Project { .. } => {
                // Projection is cheap
                stats.entity_count as f32 * 0.1
            }
            Operation::Sort { .. } => {
                // Sort is N log N
                let n = stats.entity_count as f32;
                n * n.log2()
            }
            Operation::Limit { .. } | Operation::Skip { .. } => {
                // Limit/skip are cheap
                1.0
            }
            Operation::Join { .. } => {
                // Join is expensive (N * M)
                (stats.entity_count as f32).powi(2)
            }
            Operation::InsertEntity { .. } => 10.0,
            Operation::UpdateEntities { .. } => 20.0,
            Operation::DeleteEntities { .. } => 15.0,
            Operation::CreateEdge { .. } => 12.0,
            Operation::GroupBy { .. } => {
                // Group by requires sorting/hashing - N log N
                let n = stats.entity_count as f32;
                n * n.log2()
            }
            Operation::Having { .. } => {
                // Having is a simple filter on aggregated results
                stats.entity_count as f32 * 0.1
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TraverseDirection {
    Outgoing,
    Incoming,
    Both,
}

impl From<Direction> for TraverseDirection {
    fn from(dir: Direction) -> Self {
        match dir {
            Direction::Outgoing => TraverseDirection::Outgoing,
            Direction::Incoming => TraverseDirection::Incoming,
            Direction::Both => TraverseDirection::Both,
        }
    }
}

/// Filter expression (simplified from AST Expression)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FilterExpr {
    // Logical
    And(Box<FilterExpr>, Box<FilterExpr>),
    Or(Box<FilterExpr>, Box<FilterExpr>),
    Not(Box<FilterExpr>),

    // Comparisons
    Equal(Box<FilterExpr>, Box<FilterExpr>),
    NotEqual(Box<FilterExpr>, Box<FilterExpr>),
    LessThan(Box<FilterExpr>, Box<FilterExpr>),
    LessThanEq(Box<FilterExpr>, Box<FilterExpr>),
    GreaterThan(Box<FilterExpr>, Box<FilterExpr>),
    GreaterThanEq(Box<FilterExpr>, Box<FilterExpr>),

    // Arithmetic
    Add(Box<FilterExpr>, Box<FilterExpr>),
    Subtract(Box<FilterExpr>, Box<FilterExpr>),
    Multiply(Box<FilterExpr>, Box<FilterExpr>),
    Divide(Box<FilterExpr>, Box<FilterExpr>),

    // Aggregates
    Aggregate {
        function: AggregateFunc,
        argument: Box<FilterExpr>,
    },

    // Values
    Property {
        binding: String,
        property: String,
    },
    Constant(Value),
}

impl FilterExpr {
    /// Convert AST Expression to IR FilterExpr
    pub fn from_ast(expr: &Expression, default_binding: &str) -> Self {
        match expr {
            Expression::And(l, r) => FilterExpr::And(
                Box::new(Self::from_ast(l, default_binding)),
                Box::new(Self::from_ast(r, default_binding)),
            ),
            Expression::Or(l, r) => FilterExpr::Or(
                Box::new(Self::from_ast(l, default_binding)),
                Box::new(Self::from_ast(r, default_binding)),
            ),
            Expression::Not(e) => {
                FilterExpr::Not(Box::new(Self::from_ast(e, default_binding)))
            }
            Expression::Equal(l, r) => FilterExpr::Equal(
                Box::new(Self::from_ast(l, default_binding)),
                Box::new(Self::from_ast(r, default_binding)),
            ),
            Expression::NotEqual(l, r) => FilterExpr::NotEqual(
                Box::new(Self::from_ast(l, default_binding)),
                Box::new(Self::from_ast(r, default_binding)),
            ),
            Expression::LessThan(l, r) => FilterExpr::LessThan(
                Box::new(Self::from_ast(l, default_binding)),
                Box::new(Self::from_ast(r, default_binding)),
            ),
            Expression::LessThanEq(l, r) => FilterExpr::LessThanEq(
                Box::new(Self::from_ast(l, default_binding)),
                Box::new(Self::from_ast(r, default_binding)),
            ),
            Expression::GreaterThan(l, r) => FilterExpr::GreaterThan(
                Box::new(Self::from_ast(l, default_binding)),
                Box::new(Self::from_ast(r, default_binding)),
            ),
            Expression::GreaterThanEq(l, r) => FilterExpr::GreaterThanEq(
                Box::new(Self::from_ast(l, default_binding)),
                Box::new(Self::from_ast(r, default_binding)),
            ),
            Expression::Add(l, r) => FilterExpr::Add(
                Box::new(Self::from_ast(l, default_binding)),
                Box::new(Self::from_ast(r, default_binding)),
            ),
            Expression::Subtract(l, r) => FilterExpr::Subtract(
                Box::new(Self::from_ast(l, default_binding)),
                Box::new(Self::from_ast(r, default_binding)),
            ),
            Expression::Multiply(l, r) => FilterExpr::Multiply(
                Box::new(Self::from_ast(l, default_binding)),
                Box::new(Self::from_ast(r, default_binding)),
            ),
            Expression::Divide(l, r) => FilterExpr::Divide(
                Box::new(Self::from_ast(l, default_binding)),
                Box::new(Self::from_ast(r, default_binding)),
            ),
            Expression::Property(prop_ref) => FilterExpr::Property {
                binding: prop_ref
                    .entity
                    .clone()
                    .unwrap_or_else(|| default_binding.to_string()),
                property: prop_ref.property.clone(),
            },
            Expression::Literal(lit) => FilterExpr::Constant(Value::from_literal(lit)),
            Expression::Aggregate(func, arg) => FilterExpr::Aggregate {
                function: func.into(),
                argument: Box::new(Self::from_ast(arg, default_binding)),
            },
        }
    }
}

/// Projection field
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectField {
    pub expression: FilterExpr,
    pub alias: String,
}

/// Sort field
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SortField {
    pub expression: FilterExpr,
    pub ascending: bool,
}

/// Runtime value
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Value {
    Null,
    Bool(bool),
    Integer(i64),
    Float(f64),
    String(String),
    EntityId(u64),
    EdgeId(u64),
}

impl Value {
    pub fn from_literal(lit: &Literal) -> Self {
        match lit {
            Literal::Null => Value::Null,
            Literal::Bool(b) => Value::Bool(*b),
            Literal::Integer(n) => Value::Integer(*n),
            Literal::Float(f) => Value::Float(*f),
            Literal::String(s) => Value::String(s.clone()),
        }
    }
}

/// Query plan builder - converts AST to IR
pub struct QueryPlanBuilder {
    next_binding_id: usize,
}

impl QueryPlanBuilder {
    pub fn new() -> Self {
        QueryPlanBuilder {
            next_binding_id: 0,
        }
    }

    /// Build execution plan from SELECT query
    pub fn build_select(&mut self, query: &SelectQuery) -> Result<QueryPlan, String> {
        let mut operations = Vec::new();

        // Step 1: FROM clause - scan or index lookup
        let from_binding = query
            .from
            .alias
            .clone()
            .unwrap_or_else(|| query.from.collection.clone());

        // Check if WHERE can use index (simple optimization)
        if let Some(where_clause) = &query.where_clause {
            // For now, always use scan (ant colony will optimize later)
            operations.push(Operation::Scan {
                collection: query.from.collection.clone(),
                alias: from_binding.clone(),
                filter: Some(FilterExpr::from_ast(&where_clause.condition, &from_binding)),
            });
        } else {
            operations.push(Operation::Scan {
                collection: query.from.collection.clone(),
                alias: from_binding.clone(),
                filter: None,
            });
        }

        // Step 2: TRAVERSE clause (if present)
        if let Some(traverse) = &query.traverse {
            for pattern in &traverse.patterns {
                let target_binding = pattern
                    .target_alias
                    .clone()
                    .unwrap_or_else(|| self.next_binding());

                operations.push(Operation::Traverse {
                    source_binding: from_binding.clone(),
                    direction: pattern.direction.clone().into(),
                    edge_type: pattern.edge_type.clone(),
                    target_alias: target_binding.clone(),
                    min_hops: pattern.min_hops,
                    max_hops: pattern.max_hops,
                    filter: None, // WHERE filter applied separately
                });
            }
        }

        // Step 3: GROUP BY (if present)
        if let Some(group_by) = &query.group_by {
            // Extract aggregate functions from SELECT fields
            let mut aggregates = Vec::new();
            for (idx, field) in query.select.fields.iter().enumerate() {
                if let Expression::Aggregate(func, arg) = &field.expression {
                    let alias = field
                        .alias
                        .clone()
                        .unwrap_or_else(|| format!("agg_{}", idx));

                    aggregates.push(AggregateOp {
                        function: func.into(),
                        argument: FilterExpr::from_ast(arg, &from_binding),
                        alias,
                    });
                }
            }

            let group_fields: Vec<FilterExpr> = group_by
                .fields
                .iter()
                .map(|f| FilterExpr::from_ast(f, &from_binding))
                .collect();

            operations.push(Operation::GroupBy {
                group_fields,
                aggregates,
            });
        }

        // Step 4: HAVING (if present, must come after GROUP BY)
        if let Some(having) = &query.having {
            operations.push(Operation::Having {
                condition: FilterExpr::from_ast(&having.condition, &from_binding),
            });
        }

        // Step 5: PROJECT (SELECT fields)
        let mut project_fields = Vec::new();
        for (idx, field) in query.select.fields.iter().enumerate() {
            let alias = field
                .alias
                .clone()
                .unwrap_or_else(|| format!("col_{}", idx));

            project_fields.push(ProjectField {
                expression: FilterExpr::from_ast(&field.expression, &from_binding),
                alias,
            });
        }

        operations.push(Operation::Project {
            fields: project_fields,
        });

        // Step 6: ORDER BY
        if let Some(order_by) = &query.order_by {
            let sort_fields: Vec<SortField> = order_by
                .fields
                .iter()
                .map(|f| SortField {
                    expression: FilterExpr::from_ast(&f.expression, &from_binding),
                    ascending: f.ascending,
                })
                .collect();

            operations.push(Operation::Sort {
                fields: sort_fields,
            });
        }

        // Step 7: LIMIT/OFFSET
        if let Some(offset) = query.offset {
            operations.push(Operation::Skip { count: offset });
        }

        if let Some(limit) = query.limit {
            operations.push(Operation::Limit { count: limit });
        }

        Ok(QueryPlan::new(operations))
    }

    /// Build execution plan from INSERT query
    pub fn build_insert(&mut self, query: &InsertQuery) -> Result<QueryPlan, String> {
        let mut properties = HashMap::new();

        for (key, value) in &query.properties {
            properties.insert(key.clone(), Value::from_literal(value));
        }

        let operations = vec![Operation::InsertEntity {
            collection: query.collection.clone(),
            properties,
        }];

        Ok(QueryPlan::new(operations))
    }

    /// Build execution plan from UPDATE query
    pub fn build_update(&mut self, query: &UpdateQuery) -> Result<QueryPlan, String> {
        let mut operations = Vec::new();

        let binding = query.collection.clone();

        // Scan with filter
        operations.push(Operation::Scan {
            collection: query.collection.clone(),
            alias: binding.clone(),
            filter: query
                .where_clause
                .as_ref()
                .map(|w| FilterExpr::from_ast(&w.condition, &binding)),
        });

        // Update
        let mut updates = HashMap::new();
        for (key, expr) in &query.set {
            updates.insert(key.clone(), FilterExpr::from_ast(expr, &binding));
        }

        operations.push(Operation::UpdateEntities { binding, updates });

        Ok(QueryPlan::new(operations))
    }

    /// Build execution plan from DELETE query
    pub fn build_delete(&mut self, query: &DeleteQuery) -> Result<QueryPlan, String> {
        let mut operations = Vec::new();

        let binding = query.collection.clone();

        // Scan with filter
        operations.push(Operation::Scan {
            collection: query.collection.clone(),
            alias: binding.clone(),
            filter: query
                .where_clause
                .as_ref()
                .map(|w| FilterExpr::from_ast(&w.condition, &binding)),
        });

        // Delete
        operations.push(Operation::DeleteEntities { binding });

        Ok(QueryPlan::new(operations))
    }

    /// Build execution plan from CREATE query
    pub fn build_create(&mut self, query: &CreateQuery) -> Result<QueryPlan, String> {
        let mut properties = HashMap::new();

        for (key, value) in &query.properties {
            properties.insert(key.clone(), Value::from_literal(value));
        }

        let operations = vec![Operation::CreateEdge {
            source: FilterExpr::from_ast(&query.source, "_default"),
            target: FilterExpr::from_ast(&query.target, "_default"),
            edge_type: query.edge_type.clone(),
            properties,
        }];

        Ok(QueryPlan::new(operations))
    }

    fn next_binding(&mut self) -> String {
        let id = self.next_binding_id;
        self.next_binding_id += 1;
        format!("_binding_{}", id)
    }
}

impl Default for QueryPlanBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_simple_select() {
        let query = SelectQuery {
            from: FromClause {
                collection: "Users".to_string(),
                alias: None,
            },
            traverse: None,
            where_clause: Some(WhereClause {
                condition: Expression::Equal(
                    Box::new(Expression::property(None, "age")),
                    Box::new(Expression::integer(25)),
                ),
            }),
            select: SelectClause {
                fields: vec![SelectField {
                    expression: Expression::property(None, "name"),
                    alias: None,
                }],
            },
            group_by: None,
            having: None,
            order_by: None,
            limit: None,
            offset: None,
        };

        let mut builder = QueryPlanBuilder::new();
        let plan = builder.build_select(&query).unwrap();

        assert_eq!(plan.operations.len(), 2); // Scan + Project
    }

    #[test]
    fn test_build_hybrid_query() {
        let query = SelectQuery {
            from: FromClause {
                collection: "Users".to_string(),
                alias: Some("u".to_string()),
            },
            traverse: Some(TraverseClause {
                patterns: vec![TraversePattern {
                    direction: Direction::Outgoing,
                    edge_type: Some("PURCHASED".to_string()),
                    target_alias: Some("p".to_string()),
                    min_hops: 1,
                    max_hops: 1,
                }],
            }),
            where_clause: None,
            select: SelectClause {
                fields: vec![
                    SelectField {
                        expression: Expression::property(Some("u"), "name"),
                        alias: None,
                    },
                    SelectField {
                        expression: Expression::property(Some("p"), "name"),
                        alias: Some("product_name".to_string()),
                    },
                ],
            },
            group_by: None,
            having: None,
            order_by: None,
            limit: Some(10),
            offset: None,
        };

        let mut builder = QueryPlanBuilder::new();
        let plan = builder.build_select(&query).unwrap();

        // Scan + Traverse + Project + Limit
        assert!(plan.operations.len() >= 3);
    }
}
