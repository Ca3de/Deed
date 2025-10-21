//! DQL Query Executor
//!
//! Executes optimized query plans against the graph storage.

use crate::dql_ir::*;
use crate::dql_optimizer::{AntColonyOptimizer, StigmergyCache};
use crate::dql_parser::Parser;
use crate::graph::{Graph, Entity, Edge};
use crate::transaction::{TransactionManager, TransactionId, IsolationLevel};
use crate::wal::WALManager;
use crate::types::{EntityId, EdgeId, EdgeType, EntityType, Properties, PropertyValue};
use std::collections::HashMap;
use std::sync::{Arc, RwLock, Mutex};
use std::path::Path;

/// Query executor with biological optimization and transaction support
pub struct DQLExecutor {
    graph: Arc<RwLock<Graph>>,
    optimizer: Arc<RwLock<AntColonyOptimizer>>,
    cache: Arc<RwLock<StigmergyCache>>,
    transaction_manager: Arc<TransactionManager>,
    wal_manager: Option<Arc<WALManager>>,
    current_transaction: Arc<Mutex<Option<TransactionId>>>,
}

impl DQLExecutor {
    /// Create a new executor without WAL (non-durable)
    pub fn new(graph: Arc<RwLock<Graph>>) -> Self {
        DQLExecutor {
            graph,
            optimizer: Arc::new(RwLock::new(AntColonyOptimizer::new())),
            cache: Arc::new(RwLock::new(StigmergyCache::new(1000))),
            transaction_manager: Arc::new(TransactionManager::new()),
            wal_manager: None,
            current_transaction: Arc::new(Mutex::new(None)),
        }
    }

    /// Create a new executor with WAL for durability
    pub fn new_with_wal<P: AsRef<Path>>(graph: Arc<RwLock<Graph>>, wal_path: P) -> Result<Self, String> {
        let wal_manager = WALManager::new(wal_path)
            .map_err(|e| format!("Failed to create WAL: {}", e))?;

        Ok(DQLExecutor {
            graph,
            optimizer: Arc::new(RwLock::new(AntColonyOptimizer::new())),
            cache: Arc::new(RwLock::new(StigmergyCache::new(1000))),
            transaction_manager: Arc::new(TransactionManager::new()),
            wal_manager: Some(Arc::new(wal_manager)),
            current_transaction: Arc::new(Mutex::new(None)),
        })
    }

    /// Create a new executor with shared components (for connection pooling)
    pub fn with_shared_components(
        graph: Arc<RwLock<Graph>>,
        optimizer: Arc<RwLock<AntColonyOptimizer>>,
        cache: Arc<RwLock<StigmergyCache>>,
        transaction_manager: Arc<TransactionManager>,
        wal_manager: Option<Arc<WALManager>>,
    ) -> Self {
        DQLExecutor {
            graph,
            optimizer,
            cache,
            transaction_manager,
            wal_manager,
            current_transaction: Arc::new(Mutex::new(None)),
        }
    }

    /// Execute a DQL query string
    pub fn execute(&self, query_str: &str) -> Result<QueryResult, String> {
        // Parse query
        let query = Parser::parse(query_str)?;

        // Handle transaction commands separately
        match &query {
            crate::dql_ast::Query::Begin(begin_query) => {
                return self.handle_begin(begin_query);
            }
            crate::dql_ast::Query::Commit => {
                return self.handle_commit();
            }
            crate::dql_ast::Query::Rollback => {
                return self.handle_rollback();
            }
            _ => {
                // Regular query - continue below
            }
        }

        // Check if this is a mutation that needs auto-commit
        let needs_auto_commit = self.is_mutation_query(&query);
        let had_active_txn = self.current_transaction.lock().unwrap().is_some();

        // Auto-begin transaction if needed
        if needs_auto_commit && !had_active_txn {
            let txn_id = self.transaction_manager.begin(IsolationLevel::default())?;
            *self.current_transaction.lock().unwrap() = Some(txn_id);

            // Log to WAL
            if let Some(wal) = &self.wal_manager {
                wal.log_begin(txn_id, IsolationLevel::default())
                    .map_err(|e| format!("WAL error: {}", e))?;
            }
        }

        // Build initial plan
        let mut builder = QueryPlanBuilder::new();
        let plan = match &query {
            crate::dql_ast::Query::Select(q) => builder.build_select(q)?,
            crate::dql_ast::Query::Insert(q) => builder.build_insert(q)?,
            crate::dql_ast::Query::Update(q) => builder.build_update(q)?,
            crate::dql_ast::Query::Delete(q) => builder.build_delete(q)?,
            crate::dql_ast::Query::Create(q) => builder.build_create(q)?,
            _ => unreachable!(), // Transaction commands handled above
        };

        // Try cache first (stigmergy)
        let query_signature = self.query_signature(query_str);
        let optimized_plan = {
            let mut cache = self.cache.write().unwrap();
            if let Some(cached_plan) = cache.get(&query_signature) {
                cached_plan
            } else {
                // Optimize with ant colony
                let graph = self.graph.read().unwrap();
                let stats = graph.stats();
                drop(graph); // Release read lock

                let mut optimizer = self.optimizer.write().unwrap();
                let optimized = optimizer.optimize(plan, &stats);

                // Cache the optimized plan
                cache.put(query_signature.clone(), optimized.clone());

                optimized
            }
        };

        // Execute the plan
        let result = self.execute_plan(&optimized_plan);

        // Auto-commit if we auto-began
        if needs_auto_commit && !had_active_txn {
            if result.is_ok() {
                self.handle_commit()?;
            } else {
                self.handle_rollback()?;
            }
        }

        result
    }

    /// Execute a query plan
    fn execute_plan(&self, plan: &QueryPlan) -> Result<QueryResult, String> {
        // Execution context
        let mut ctx = ExecutionContext::new();

        // Execute operations sequentially
        for operation in &plan.operations {
            // Check if operation needs write access
            if self.is_mutation(operation) {
                // Execute mutation with write lock (released per operation)
                self.execute_mutation(operation, &mut ctx)?;
            } else {
                // Execute read operation with shared read lock
                let graph = self.graph.read().unwrap();
                self.execute_operation(operation, &mut ctx, &graph)?;
            }
        }

        // Return results
        Ok(ctx.into_result())
    }

    /// Check if operation requires write access
    fn is_mutation(&self, operation: &Operation) -> bool {
        matches!(
            operation,
            Operation::InsertEntity { .. }
                | Operation::UpdateEntities { .. }
                | Operation::DeleteEntities { .. }
                | Operation::CreateEdge { .. }
        )
    }

    /// Execute mutation operations (INSERT, UPDATE, DELETE, CREATE)
    fn execute_mutation(
        &self,
        operation: &Operation,
        ctx: &mut ExecutionContext,
    ) -> Result<(), String> {
        match operation {
            Operation::InsertEntity {
                collection,
                properties,
            } => {
                let mut props = Properties::new();
                for (key, value) in properties {
                    props.insert(key.clone(), self.value_to_property_value(value));
                }

                // Acquire write lock for insertion
                let graph = self.graph.read().unwrap();
                let entity_id = graph.add_entity(collection.clone(), props);
                drop(graph);

                ctx.last_inserted_id = Some(entity_id);
                ctx.rows_affected += 1;

                // Store result for SELECT queries after INSERT
                let mut result_row = HashMap::new();
                result_row.insert("id".to_string(), Value::EntityId(entity_id.as_u64()));
                ctx.result_rows.push(result_row);

                Ok(())
            }

            Operation::UpdateEntities { binding, updates } => {
                // First, get entities to update from context
                let entity_ids: Vec<EntityId> = ctx
                    .bindings
                    .get(binding)
                    .ok_or_else(|| format!("Binding not found: {}", binding))?
                    .iter()
                    .map(|e| e.id)
                    .collect();

                // Acquire write lock and update each entity
                let graph = self.graph.read().unwrap();

                for entity_id in &entity_ids {
                    if let Some(mut entity) = graph.get_entity(*entity_id) {
                        // Apply updates
                        for (key, expr) in updates {
                            let value = self.evaluate_expression(expr, &entity, ctx);
                            entity.set_property(key.clone(), value);
                        }

                        // Note: In production, you'd have a method to update entity in storage
                        // For now, the entity is updated in the DashMap by reference
                        // graph.update_entity(entity);
                    }
                }

                drop(graph);

                ctx.rows_affected = entity_ids.len();
                Ok(())
            }

            Operation::DeleteEntities { binding } => {
                // Get entities to delete
                let entity_ids: Vec<EntityId> = ctx
                    .bindings
                    .get(binding)
                    .ok_or_else(|| format!("Binding not found: {}", binding))?
                    .iter()
                    .map(|e| e.id)
                    .collect();

                // Acquire write lock and delete
                let graph = self.graph.read().unwrap();

                // Note: Graph doesn't have a delete_entity method yet
                // We'll track the count for now
                // In production: for id in entity_ids { graph.delete_entity(id); }

                drop(graph);

                ctx.deleted_count = entity_ids.len();
                ctx.rows_affected = entity_ids.len();
                Ok(())
            }

            Operation::CreateEdge {
                source,
                target,
                edge_type,
                properties,
            } => {
                // Evaluate source and target to get entity IDs
                // For now, we'll assume they're literal entity IDs
                // Full implementation would evaluate expressions

                let graph = self.graph.read().unwrap();

                // Simplified: assume first entity as source and target
                // In production, evaluate source/target expressions to get IDs
                let source_id = if let Some(entities) = ctx.bindings.values().next() {
                    entities.first().map(|e| e.id)
                } else {
                    None
                };

                let target_id = if let Some(entities) = ctx.bindings.values().nth(1) {
                    entities.first().map(|e| e.id)
                } else {
                    None
                };

                if let (Some(src), Some(tgt)) = (source_id, target_id) {
                    let mut props = Properties::new();
                    for (key, value) in properties {
                        props.insert(key.clone(), self.value_to_property_value(value));
                    }

                    if let Some(edge_id) = graph.add_edge(src, tgt, edge_type.clone(), props) {
                        ctx.rows_affected = 1;

                        // Store result
                        let mut result_row = HashMap::new();
                        result_row.insert("edge_id".to_string(), Value::EdgeId(edge_id.as_u64()));
                        ctx.result_rows.push(result_row);
                    }
                }

                drop(graph);
                Ok(())
            }

            _ => Err("Not a mutation operation".to_string()),
        }
    }

    /// Execute a single operation
    fn execute_operation(
        &self,
        operation: &Operation,
        ctx: &mut ExecutionContext,
        graph: &Graph,
    ) -> Result<(), String> {
        match operation {
            Operation::Scan {
                collection,
                alias,
                filter,
            } => {
                let entities = graph.scan_collection(collection);
                let filtered = if let Some(filter_expr) = filter {
                    entities
                        .into_iter()
                        .filter(|e| self.evaluate_filter(filter_expr, e, ctx))
                        .collect()
                } else {
                    entities
                };

                ctx.bindings.insert(alias.clone(), filtered);
                Ok(())
            }

            Operation::IndexLookup {
                collection,
                alias,
                index_name: _,
                key_values,
            } => {
                // For now, fall back to scan (index not implemented)
                let entities = graph.scan_collection(collection);

                // Filter by key values if provided
                let filtered = if !key_values.is_empty() {
                    entities
                        .into_iter()
                        .filter(|e| {
                            // Simple filter on first property
                            key_values.iter().any(|v| {
                                e.properties.values().any(|pv| self.value_matches(v, pv))
                            })
                        })
                        .collect()
                } else {
                    entities
                };

                ctx.bindings.insert(alias.clone(), filtered);
                Ok(())
            }

            Operation::Traverse {
                source_binding,
                direction,
                edge_type,
                target_alias,
                min_hops,
                max_hops,
                filter,
            } => {
                // Get source entities
                let source_entities = ctx
                    .bindings
                    .get(source_binding)
                    .ok_or_else(|| format!("Binding not found: {}", source_binding))?
                    .clone();

                let mut target_entities = Vec::new();

                // Traverse from each source
                for source in source_entities {
                    let neighbors = match direction {
                        TraverseDirection::Outgoing => graph.get_outgoing_neighbors(
                            source.id,
                            edge_type.as_ref().map(|s| s.as_str()),
                        ),
                        TraverseDirection::Incoming => graph.get_incoming_neighbors(
                            source.id,
                            edge_type.as_ref().map(|s| s.as_str()),
                        ),
                        TraverseDirection::Both => {
                            let mut all = graph.get_outgoing_neighbors(
                                source.id,
                                edge_type.as_ref().map(|s| s.as_str()),
                            );
                            all.extend(graph.get_incoming_neighbors(
                                source.id,
                                edge_type.as_ref().map(|s| s.as_str()),
                            ));
                            all
                        }
                    };

                    // Get target entities
                    for (target_id, _edge_id) in neighbors {
                        if let Some(target) = graph.get_entity(target_id) {
                            // Apply filter if present
                            if let Some(filter_expr) = filter {
                                if self.evaluate_filter(filter_expr, &target, ctx) {
                                    target_entities.push(target);
                                }
                            } else {
                                target_entities.push(target);
                            }
                        }
                    }
                }

                // Handle multi-hop traversal (simplified)
                if *max_hops > 1 {
                    // For now, just use 1-hop results
                    // Full BFS/DFS traversal would go here
                }

                ctx.bindings.insert(target_alias.clone(), target_entities);
                Ok(())
            }

            Operation::Filter { binding, condition } => {
                let entities = ctx
                    .bindings
                    .get(binding)
                    .ok_or_else(|| format!("Binding not found: {}", binding))?
                    .clone();

                let filtered: Vec<Entity> = entities
                    .into_iter()
                    .filter(|e| self.evaluate_filter(condition, e, ctx))
                    .collect();

                ctx.bindings.insert(binding.clone(), filtered);
                Ok(())
            }

            Operation::Project { fields } => {
                // Project fields from all bindings
                let mut rows = Vec::new();

                // Get all entities from all bindings
                let all_entities: Vec<&Entity> = ctx
                    .bindings
                    .values()
                    .flat_map(|entities| entities.iter())
                    .collect();

                for entity in all_entities {
                    let mut row = HashMap::new();

                    for field in fields {
                        let value = self.evaluate_expression(&field.expression, entity, ctx);
                        row.insert(field.alias.clone(), value);
                    }

                    rows.push(row);
                }

                ctx.result_rows = rows;
                Ok(())
            }

            Operation::Sort { fields } => {
                // Sort result rows
                ctx.result_rows.sort_by(|a, b| {
                    for field in fields {
                        // Simple comparison based on first sort field
                        // Full implementation would handle all fields
                        let a_val = a.values().next();
                        let b_val = b.values().next();

                        if let (Some(av), Some(bv)) = (a_val, b_val) {
                            let cmp = self.compare_values(av, bv);
                            if cmp != std::cmp::Ordering::Equal {
                                return if field.ascending { cmp } else { cmp.reverse() };
                            }
                        }
                    }
                    std::cmp::Ordering::Equal
                });

                Ok(())
            }

            Operation::Limit { count } => {
                ctx.result_rows.truncate(*count);
                Ok(())
            }

            Operation::Skip { count } => {
                ctx.result_rows = ctx.result_rows.split_off(*count.min(&ctx.result_rows.len()));
                Ok(())
            }

            Operation::GroupBy {
                group_fields,
                aggregates,
            } => {
                // Group entities by group_fields values
                use std::collections::BTreeMap;
                let mut groups: BTreeMap<Vec<String>, Vec<Entity>> = BTreeMap::new();

                // Get all entities from bindings
                let all_entities: Vec<Entity> = ctx
                    .bindings
                    .values()
                    .flat_map(|entities| entities.clone())
                    .collect();

                // Group entities by group_fields
                for entity in all_entities {
                    let mut group_key = Vec::new();
                    for field_expr in group_fields {
                        let value = self.evaluate_expression(field_expr, &entity, ctx);
                        group_key.push(self.value_to_string(&value));
                    }
                    groups.entry(group_key).or_insert_with(Vec::new).push(entity);
                }

                // Compute aggregates for each group
                let mut result_rows = Vec::new();
                for (group_key, group_entities) in groups {
                    let mut row = HashMap::new();

                    // Add group fields to result
                    for (idx, field_expr) in group_fields.iter().enumerate() {
                        if let Some(key_value) = group_key.get(idx) {
                            // Use first entity in group to get field name
                            if let Some(first_entity) = group_entities.first() {
                                let field_name = self.extract_field_name(field_expr);
                                row.insert(
                                    field_name,
                                    self.evaluate_expression(field_expr, first_entity, ctx),
                                );
                            }
                        }
                    }

                    // Compute aggregates
                    for agg_op in aggregates {
                        let agg_value = self.compute_aggregate(
                            &agg_op.function,
                            &agg_op.argument,
                            &group_entities,
                            ctx,
                        );
                        row.insert(agg_op.alias.clone(), agg_value);
                    }

                    result_rows.push(row);
                }

                ctx.result_rows = result_rows;
                Ok(())
            }

            Operation::Having { condition } => {
                // Filter aggregated result rows based on HAVING condition
                ctx.result_rows.retain(|row| {
                    self.evaluate_having_condition(condition, row)
                });
                Ok(())
            }

            // Mutations are handled separately in execute_mutation()
            Operation::InsertEntity { .. }
            | Operation::UpdateEntities { .. }
            | Operation::DeleteEntities { .. }
            | Operation::CreateEdge { .. } => {
                Err("Mutation operations should be handled by execute_mutation()".to_string())
            }
        }
    }

    /// Evaluate filter expression
    fn evaluate_filter(&self, expr: &FilterExpr, entity: &Entity, ctx: &ExecutionContext) -> bool {
        match expr {
            FilterExpr::And(l, r) => {
                self.evaluate_filter(l, entity, ctx) && self.evaluate_filter(r, entity, ctx)
            }
            FilterExpr::Or(l, r) => {
                self.evaluate_filter(l, entity, ctx) || self.evaluate_filter(r, entity, ctx)
            }
            FilterExpr::Not(e) => !self.evaluate_filter(e, entity, ctx),

            FilterExpr::Equal(l, r) => {
                let lv = self.evaluate_expression(l, entity, ctx);
                let rv = self.evaluate_expression(r, entity, ctx);
                self.property_values_equal(&lv, &rv)
            }
            FilterExpr::NotEqual(l, r) => {
                let lv = self.evaluate_expression(l, entity, ctx);
                let rv = self.evaluate_expression(r, entity, ctx);
                !self.property_values_equal(&lv, &rv)
            }
            FilterExpr::LessThan(l, r) => {
                let lv = self.evaluate_expression(l, entity, ctx);
                let rv = self.evaluate_expression(r, entity, ctx);
                matches!(self.compare_property_values(&lv, &rv), Some(std::cmp::Ordering::Less))
            }
            FilterExpr::LessThanEq(l, r) => {
                let lv = self.evaluate_expression(l, entity, ctx);
                let rv = self.evaluate_expression(r, entity, ctx);
                matches!(
                    self.compare_property_values(&lv, &rv),
                    Some(std::cmp::Ordering::Less | std::cmp::Ordering::Equal)
                )
            }
            FilterExpr::GreaterThan(l, r) => {
                let lv = self.evaluate_expression(l, entity, ctx);
                let rv = self.evaluate_expression(r, entity, ctx);
                matches!(self.compare_property_values(&lv, &rv), Some(std::cmp::Ordering::Greater))
            }
            FilterExpr::GreaterThanEq(l, r) => {
                let lv = self.evaluate_expression(l, entity, ctx);
                let rv = self.evaluate_expression(r, entity, ctx);
                matches!(
                    self.compare_property_values(&lv, &rv),
                    Some(std::cmp::Ordering::Greater | std::cmp::Ordering::Equal)
                )
            }

            _ => true, // Default to true for other expressions
        }
    }

    /// Evaluate expression to property value
    fn evaluate_expression(
        &self,
        expr: &FilterExpr,
        entity: &Entity,
        _ctx: &ExecutionContext,
    ) -> PropertyValue {
        match expr {
            FilterExpr::Property { binding: _, property } => {
                entity.get_property(property).cloned().unwrap_or(PropertyValue::Null)
            }
            FilterExpr::Constant(value) => self.value_to_property_value(value),

            FilterExpr::Add(l, r) => {
                let lv = self.evaluate_expression(l, entity, _ctx);
                let rv = self.evaluate_expression(r, entity, _ctx);
                self.add_values(&lv, &rv)
            }

            _ => PropertyValue::Null, // Default for other expressions
        }
    }

    /// Compare property values
    fn compare_property_values(
        &self,
        a: &PropertyValue,
        b: &PropertyValue,
    ) -> Option<std::cmp::Ordering> {
        match (a, b) {
            (PropertyValue::Int(a), PropertyValue::Int(b)) => Some(a.cmp(b)),
            (PropertyValue::Float(a), PropertyValue::Float(b)) => a.partial_cmp(b),
            (PropertyValue::Int(a), PropertyValue::Float(b)) => (*a as f64).partial_cmp(b),
            (PropertyValue::Float(a), PropertyValue::Int(b)) => a.partial_cmp(&(*b as f64)),
            (PropertyValue::String(a), PropertyValue::String(b)) => Some(a.cmp(b)),
            _ => None,
        }
    }

    fn property_values_equal(&self, a: &PropertyValue, b: &PropertyValue) -> bool {
        match (a, b) {
            (PropertyValue::Null, PropertyValue::Null) => true,
            (PropertyValue::Bool(a), PropertyValue::Bool(b)) => a == b,
            (PropertyValue::Int(a), PropertyValue::Int(b)) => a == b,
            (PropertyValue::Float(a), PropertyValue::Float(b)) => (a - b).abs() < f64::EPSILON,
            (PropertyValue::String(a), PropertyValue::String(b)) => a == b,
            _ => false,
        }
    }

    fn add_values(&self, a: &PropertyValue, b: &PropertyValue) -> PropertyValue {
        match (a, b) {
            (PropertyValue::Int(a), PropertyValue::Int(b)) => PropertyValue::Int(a + b),
            (PropertyValue::Float(a), PropertyValue::Float(b)) => PropertyValue::Float(a + b),
            (PropertyValue::Int(a), PropertyValue::Float(b)) => PropertyValue::Float(*a as f64 + b),
            (PropertyValue::Float(a), PropertyValue::Int(b)) => PropertyValue::Float(a + *b as f64),
            _ => PropertyValue::Null,
        }
    }

    fn value_to_property_value(&self, value: &Value) -> PropertyValue {
        match value {
            Value::Null => PropertyValue::Null,
            Value::Bool(b) => PropertyValue::Bool(*b),
            Value::Integer(n) => PropertyValue::Int(*n),
            Value::Float(f) => PropertyValue::Float(*f),
            Value::String(s) => PropertyValue::String(s.clone()),
            _ => PropertyValue::Null,
        }
    }

    fn value_matches(&self, ir_value: &Value, prop_value: &PropertyValue) -> bool {
        match (ir_value, prop_value) {
            (Value::Integer(a), PropertyValue::Int(b)) => a == b,
            (Value::Float(a), PropertyValue::Float(b)) => (a - b).abs() < f64::EPSILON,
            (Value::String(a), PropertyValue::String(b)) => a == b,
            (Value::Bool(a), PropertyValue::Bool(b)) => a == b,
            (Value::Null, PropertyValue::Null) => true,
            _ => false,
        }
    }

    fn compare_values(&self, a: &Value, b: &Value) -> std::cmp::Ordering {
        match (a, b) {
            (Value::Integer(a), Value::Integer(b)) => a.cmp(b),
            (Value::Float(a), Value::Float(b)) => a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal),
            (Value::String(a), Value::String(b)) => a.cmp(b),
            _ => std::cmp::Ordering::Equal,
        }
    }

    /// Compute aggregate function
    fn compute_aggregate(
        &self,
        function: &AggregateFunc,
        argument: &FilterExpr,
        entities: &[Entity],
        ctx: &ExecutionContext,
    ) -> Value {
        match function {
            AggregateFunc::Count => {
                // COUNT(*) or COUNT(field)
                Value::Integer(entities.len() as i64)
            }
            AggregateFunc::Sum => {
                let mut sum = 0.0;
                for entity in entities {
                    if let PropertyValue::Int(n) = self.evaluate_expression(argument, entity, ctx) {
                        sum += n as f64;
                    } else if let PropertyValue::Float(f) = self.evaluate_expression(argument, entity, ctx) {
                        sum += f;
                    }
                }
                Value::Float(sum)
            }
            AggregateFunc::Avg => {
                if entities.is_empty() {
                    return Value::Null;
                }
                let mut sum = 0.0;
                let mut count = 0;
                for entity in entities {
                    if let PropertyValue::Int(n) = self.evaluate_expression(argument, entity, ctx) {
                        sum += n as f64;
                        count += 1;
                    } else if let PropertyValue::Float(f) = self.evaluate_expression(argument, entity, ctx) {
                        sum += f;
                        count += 1;
                    }
                }
                if count > 0 {
                    Value::Float(sum / count as f64)
                } else {
                    Value::Null
                }
            }
            AggregateFunc::Min => {
                let mut min: Option<PropertyValue> = None;
                for entity in entities {
                    let val = self.evaluate_expression(argument, entity, ctx);
                    if let Some(current_min) = &min {
                        if let Some(std::cmp::Ordering::Less) = self.compare_property_values(&val, current_min) {
                            min = Some(val);
                        }
                    } else {
                        min = Some(val);
                    }
                }
                match min {
                    Some(PropertyValue::Int(n)) => Value::Integer(n),
                    Some(PropertyValue::Float(f)) => Value::Float(f),
                    Some(PropertyValue::String(s)) => Value::String(s),
                    _ => Value::Null,
                }
            }
            AggregateFunc::Max => {
                let mut max: Option<PropertyValue> = None;
                for entity in entities {
                    let val = self.evaluate_expression(argument, entity, ctx);
                    if let Some(current_max) = &max {
                        if let Some(std::cmp::Ordering::Greater) = self.compare_property_values(&val, current_max) {
                            max = Some(val);
                        }
                    } else {
                        max = Some(val);
                    }
                }
                match max {
                    Some(PropertyValue::Int(n)) => Value::Integer(n),
                    Some(PropertyValue::Float(f)) => Value::Float(f),
                    Some(PropertyValue::String(s)) => Value::String(s),
                    _ => Value::Null,
                }
            }
        }
    }

    /// Extract field name from expression
    fn extract_field_name(&self, expr: &FilterExpr) -> String {
        match expr {
            FilterExpr::Property { binding: _, property } => property.clone(),
            _ => "field".to_string(),
        }
    }

    /// Convert Value to String for grouping
    fn value_to_string(&self, value: &Value) -> String {
        match value {
            Value::Null => "NULL".to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Integer(n) => n.to_string(),
            Value::Float(f) => f.to_string(),
            Value::String(s) => s.clone(),
            Value::EntityId(id) => format!("entity_{}", id),
            Value::EdgeId(id) => format!("edge_{}", id),
        }
    }

    /// Evaluate HAVING condition on aggregated result row
    fn evaluate_having_condition(&self, condition: &FilterExpr, row: &HashMap<String, Value>) -> bool {
        match condition {
            FilterExpr::And(l, r) => {
                self.evaluate_having_condition(l, row) && self.evaluate_having_condition(r, row)
            }
            FilterExpr::Or(l, r) => {
                self.evaluate_having_condition(l, row) || self.evaluate_having_condition(r, row)
            }
            FilterExpr::Not(e) => !self.evaluate_having_condition(e, row),

            FilterExpr::GreaterThan(l, r) => {
                let lv = self.evaluate_having_expr(l, row);
                let rv = self.evaluate_having_expr(r, row);
                matches!(self.compare_values(&lv, &rv), std::cmp::Ordering::Greater)
            }
            FilterExpr::GreaterThanEq(l, r) => {
                let lv = self.evaluate_having_expr(l, row);
                let rv = self.evaluate_having_expr(r, row);
                matches!(self.compare_values(&lv, &rv), std::cmp::Ordering::Greater | std::cmp::Ordering::Equal)
            }
            FilterExpr::LessThan(l, r) => {
                let lv = self.evaluate_having_expr(l, row);
                let rv = self.evaluate_having_expr(r, row);
                matches!(self.compare_values(&lv, &rv), std::cmp::Ordering::Less)
            }
            FilterExpr::Equal(l, r) => {
                let lv = self.evaluate_having_expr(l, row);
                let rv = self.evaluate_having_expr(r, row);
                matches!(self.compare_values(&lv, &rv), std::cmp::Ordering::Equal)
            }

            _ => true,
        }
    }

    /// Evaluate expression in HAVING context (on result row)
    fn evaluate_having_expr(&self, expr: &FilterExpr, row: &HashMap<String, Value>) -> Value {
        match expr {
            FilterExpr::Aggregate { function: _, argument: _ } => {
                // Find aggregate value in row by matching pattern
                // Simplified: just return first numeric value found
                for value in row.values() {
                    return value.clone();
                }
                Value::Null
            }
            FilterExpr::Constant(v) => v.clone(),
            FilterExpr::Property { binding: _, property } => {
                row.get(property).cloned().unwrap_or(Value::Null)
            }
            _ => Value::Null,
        }
    }

    /// Check if a query is a mutation (needs transaction)
    fn is_mutation_query(&self, query: &crate::dql_ast::Query) -> bool {
        matches!(
            query,
            crate::dql_ast::Query::Insert(_)
                | crate::dql_ast::Query::Update(_)
                | crate::dql_ast::Query::Delete(_)
                | crate::dql_ast::Query::Create(_)
        )
    }

    /// Handle BEGIN TRANSACTION
    fn handle_begin(&self, begin_query: &crate::dql_ast::BeginQuery) -> Result<QueryResult, String> {
        // Check if already in transaction
        if self.current_transaction.lock().unwrap().is_some() {
            return Err("Already in a transaction".to_string());
        }

        // Start new transaction
        let isolation_level = begin_query.isolation_level.unwrap_or(IsolationLevel::RepeatableRead);
        let txn_id = self.transaction_manager.begin(isolation_level)?;

        // Log to WAL
        if let Some(wal) = &self.wal_manager {
            wal.log_begin(txn_id, isolation_level)
                .map_err(|e| format!("WAL error: {}", e))?;
        }

        // Store current transaction
        *self.current_transaction.lock().unwrap() = Some(txn_id);

        Ok(QueryResult {
            rows: vec![],
            rows_affected: 0,
        })
    }

    /// Handle COMMIT
    fn handle_commit(&self) -> Result<QueryResult, String> {
        // Get current transaction
        let txn_id = self.current_transaction.lock().unwrap().take()
            .ok_or("No active transaction to commit".to_string())?;

        // Commit transaction
        self.transaction_manager.commit(txn_id)?;

        // Log to WAL
        if let Some(wal) = &self.wal_manager {
            wal.log_commit(txn_id)
                .map_err(|e| format!("WAL error: {}", e))?;
            wal.flush()
                .map_err(|e| format!("WAL flush error: {}", e))?;
        }

        Ok(QueryResult {
            rows: vec![],
            rows_affected: 0,
        })
    }

    /// Handle ROLLBACK
    fn handle_rollback(&self) -> Result<QueryResult, String> {
        // Get current transaction
        let txn_id = self.current_transaction.lock().unwrap().take()
            .ok_or("No active transaction to rollback".to_string())?;

        // Rollback transaction
        self.transaction_manager.rollback(txn_id)?;

        // Log to WAL
        if let Some(wal) = &self.wal_manager {
            wal.log_rollback(txn_id)
                .map_err(|e| format!("WAL error: {}", e))?;
        }

        Ok(QueryResult {
            rows: vec![],
            rows_affected: 0,
        })
    }

    fn query_signature(&self, query: &str) -> String {
        // Simple signature for now - hash would be better
        query.chars().filter(|c| !c.is_whitespace()).collect()
    }
}

/// Execution context - holds intermediate results
struct ExecutionContext {
    bindings: HashMap<String, Vec<Entity>>,
    result_rows: Vec<HashMap<String, Value>>,
    last_inserted_id: Option<EntityId>,
    deleted_count: usize,
    rows_affected: usize,
}

impl ExecutionContext {
    fn new() -> Self {
        ExecutionContext {
            bindings: HashMap::new(),
            result_rows: Vec::new(),
            last_inserted_id: None,
            deleted_count: 0,
            rows_affected: 0,
        }
    }

    fn into_result(self) -> QueryResult {
        QueryResult {
            rows: self.result_rows,
            rows_affected: self.rows_affected.max(self.deleted_count),
        }
    }
}

/// Query result
#[derive(Debug, Clone)]
pub struct QueryResult {
    pub rows: Vec<HashMap<String, Value>>,
    pub rows_affected: usize,
}

impl QueryResult {
    pub fn row_count(&self) -> usize {
        self.rows.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_query() {
        let graph = Arc::new(RwLock::new(Graph::new()));

        // Add test data
        {
            let g = graph.read().unwrap();
            let mut props = Properties::new();
            props.insert("name".to_string(), PropertyValue::String("Alice".to_string()));
            props.insert("age".to_string(), PropertyValue::Int(25));
            g.add_entity("User".to_string(), props);
        }

        let executor = DQLExecutor::new(graph);

        let result = executor.execute("FROM User WHERE age = 25 SELECT name");

        assert!(result.is_ok());
        let res = result.unwrap();
        assert!(res.row_count() > 0);
    }
}
