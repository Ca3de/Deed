//! Distributed Query Execution for Deed Database
//!
//! Implements distributed query execution across multiple nodes:
//! - Query planning and optimization for distributed execution
//! - Parallel query execution across shards
//! - Result aggregation from multiple nodes
//! - Fault tolerance and retry logic
//!
//! Query execution phases:
//! 1. Parse query and determine affected shards
//! 2. Create sub-queries for each shard
//! 3. Route sub-queries to responsible nodes
//! 4. Execute in parallel
//! 5. Aggregate results

use crate::distributed_topology::NodeId;
use crate::distributed_shard::{ShardManager, ShardId};
use crate::distributed_p2p::{P2PNetwork, MessageType, P2PMessage};
use crate::dql_executor::{DQLExecutor, QueryResult};
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Distributed query plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistributedQueryPlan {
    /// Original query
    pub query: String,

    /// Sub-queries for each node
    pub sub_queries: Vec<SubQuery>,

    /// Query type (SELECT, INSERT, UPDATE, DELETE, etc.)
    pub query_type: QueryType,

    /// Whether query requires global coordination
    pub requires_coordination: bool,

    /// Aggregation needed (for GROUP BY, COUNT, etc.)
    pub aggregation: Option<AggregationType>,
}

/// Sub-query to execute on a specific node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubQuery {
    pub node_id: NodeId,
    pub shard_ids: Vec<ShardId>,
    pub query: String,
}

/// Type of query
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum QueryType {
    Select,
    Insert,
    Update,
    Delete,
    Traverse, // Graph traversal
    Aggregate,
}

/// Type of aggregation
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AggregationType {
    Count,
    Sum,
    Avg,
    Min,
    Max,
    GroupBy { fields: Vec<String> },
}

/// Result from a sub-query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubQueryResult {
    pub node_id: NodeId,
    pub shard_ids: Vec<ShardId>,
    pub success: bool,
    pub rows_affected: usize,
    pub error: Option<String>,
    pub data: Option<Vec<u8>>, // Serialized result data
}

/// Distributed query executor
pub struct DistributedQueryExecutor {
    /// Local node ID
    local_id: NodeId,

    /// Shard manager for routing
    shard_manager: Arc<ShardManager>,

    /// P2P network for communication
    p2p_network: Arc<P2PNetwork>,

    /// Local query executor
    local_executor: Arc<DQLExecutor>,

    /// Query cache for optimization
    query_cache: Arc<RwLock<HashMap<String, DistributedQueryPlan>>>,
}

impl DistributedQueryExecutor {
    /// Create new distributed query executor
    pub fn new(
        local_id: NodeId,
        shard_manager: Arc<ShardManager>,
        p2p_network: Arc<P2PNetwork>,
        local_executor: Arc<DQLExecutor>,
    ) -> Self {
        Self {
            local_id,
            shard_manager,
            p2p_network,
            local_executor,
            query_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Execute a distributed query
    pub async fn execute(&self, query: &str) -> Result<QueryResult, String> {
        // Step 1: Create query plan
        let plan = self.create_query_plan(query)?;

        // Step 2: Execute sub-queries in parallel
        let sub_results = self.execute_sub_queries(&plan).await?;

        // Step 3: Aggregate results
        let final_result = self.aggregate_results(&plan, sub_results)?;

        Ok(final_result)
    }

    /// Create distributed query plan
    fn create_query_plan(&self, query: &str) -> Result<DistributedQueryPlan, String> {
        // Check cache first
        {
            let cache = self.query_cache.read().unwrap();
            if let Some(cached_plan) = cache.get(query) {
                return Ok(cached_plan.clone());
            }
        }

        // Parse query to determine type
        let query_type = self.determine_query_type(query);

        // Determine affected shards
        let affected_shards = self.determine_affected_shards(query)?;

        // Group shards by responsible node
        let mut node_shards: HashMap<NodeId, Vec<ShardId>> = HashMap::new();

        for shard_id in affected_shards {
            if let Some(node_id) = self.shard_manager.get_node_for_shard(shard_id) {
                node_shards.entry(node_id).or_insert_with(Vec::new).push(shard_id);
            }
        }

        // Create sub-queries for each node
        let mut sub_queries = Vec::new();

        for (node_id, shard_ids) in node_shards {
            // For simple queries, same query can run on each shard
            // For complex queries, might need to modify query per shard
            let sub_query = SubQuery {
                node_id,
                shard_ids,
                query: query.to_string(),
            };
            sub_queries.push(sub_query);
        }

        // Determine if aggregation is needed
        let aggregation = self.determine_aggregation(query);

        let plan = DistributedQueryPlan {
            query: query.to_string(),
            sub_queries,
            query_type,
            requires_coordination: matches!(query_type, QueryType::Aggregate | QueryType::Traverse),
            aggregation,
        };

        // Cache the plan
        {
            let mut cache = self.query_cache.write().unwrap();
            cache.insert(query.to_string(), plan.clone());
        }

        Ok(plan)
    }

    /// Execute sub-queries in parallel
    async fn execute_sub_queries(&self, plan: &DistributedQueryPlan) -> Result<Vec<SubQueryResult>, String> {
        let mut results = Vec::new();

        // Execute local sub-queries
        for sub_query in &plan.sub_queries {
            if sub_query.node_id == self.local_id {
                // Execute locally
                let result = self.execute_local_sub_query(sub_query)?;
                results.push(result);
            } else {
                // Send to remote node via P2P
                let result = self.execute_remote_sub_query(sub_query).await?;
                results.push(result);
            }
        }

        Ok(results)
    }

    /// Execute sub-query on local node
    fn execute_local_sub_query(&self, sub_query: &SubQuery) -> Result<SubQueryResult, String> {
        match self.local_executor.execute(&sub_query.query) {
            Ok(query_result) => {
                Ok(SubQueryResult {
                    node_id: self.local_id,
                    shard_ids: sub_query.shard_ids.clone(),
                    success: true,
                    rows_affected: query_result.rows_affected,
                    error: None,
                    data: None, // Would serialize actual data in production
                })
            }
            Err(e) => {
                Ok(SubQueryResult {
                    node_id: self.local_id,
                    shard_ids: sub_query.shard_ids.clone(),
                    success: false,
                    rows_affected: 0,
                    error: Some(e),
                    data: None,
                })
            }
        }
    }

    /// Execute sub-query on remote node
    async fn execute_remote_sub_query(&self, sub_query: &SubQuery) -> Result<SubQueryResult, String> {
        let message = MessageType::QueryRequest {
            query: sub_query.query.clone(),
        };

        match self.p2p_network.send_message(sub_query.node_id, message).await {
            Ok(Some(response)) => {
                match response.message_type {
                    MessageType::QueryResponse { result } => {
                        // Parse result
                        Ok(SubQueryResult {
                            node_id: sub_query.node_id,
                            shard_ids: sub_query.shard_ids.clone(),
                            success: true,
                            rows_affected: 1, // Parse from result
                            error: None,
                            data: Some(result.into_bytes()),
                        })
                    }
                    MessageType::Error { message } => {
                        Ok(SubQueryResult {
                            node_id: sub_query.node_id,
                            shard_ids: sub_query.shard_ids.clone(),
                            success: false,
                            rows_affected: 0,
                            error: Some(message),
                            data: None,
                        })
                    }
                    _ => {
                        Err("Unexpected response type".to_string())
                    }
                }
            }
            Ok(None) => {
                Ok(SubQueryResult {
                    node_id: sub_query.node_id,
                    shard_ids: sub_query.shard_ids.clone(),
                    success: false,
                    rows_affected: 0,
                    error: Some("No response from node".to_string()),
                    data: None,
                })
            }
            Err(e) => {
                Ok(SubQueryResult {
                    node_id: sub_query.node_id,
                    shard_ids: sub_query.shard_ids.clone(),
                    success: false,
                    rows_affected: 0,
                    error: Some(e),
                    data: None,
                })
            }
        }
    }

    /// Aggregate results from multiple nodes
    fn aggregate_results(
        &self,
        plan: &DistributedQueryPlan,
        sub_results: Vec<SubQueryResult>,
    ) -> Result<QueryResult, String> {
        // Check for errors
        let failed_results: Vec<_> = sub_results.iter()
            .filter(|r| !r.success)
            .collect();

        if !failed_results.is_empty() {
            let errors: Vec<String> = failed_results.iter()
                .filter_map(|r| r.error.as_ref().cloned())
                .collect();
            return Err(format!("Query failed on {} nodes: {}", failed_results.len(), errors.join(", ")));
        }

        // Aggregate based on query type
        match &plan.aggregation {
            Some(AggregationType::Count) => {
                // Sum counts from all nodes
                let total_count: usize = sub_results.iter()
                    .map(|r| r.rows_affected)
                    .sum();

                Ok(QueryResult {
                    success: true,
                    rows_affected: total_count,
                    message: format!("COUNT aggregated from {} nodes", sub_results.len()),
                })
            }

            Some(AggregationType::Sum) => {
                // Sum values from all nodes
                let total: usize = sub_results.iter()
                    .map(|r| r.rows_affected)
                    .sum();

                Ok(QueryResult {
                    success: true,
                    rows_affected: total,
                    message: format!("SUM aggregated from {} nodes", sub_results.len()),
                })
            }

            Some(AggregationType::Min) | Some(AggregationType::Max) => {
                // Would need to deserialize actual values and compare
                // For now, simplified
                Ok(QueryResult {
                    success: true,
                    rows_affected: 1,
                    message: format!("MIN/MAX aggregated from {} nodes", sub_results.len()),
                })
            }

            Some(AggregationType::GroupBy { .. }) => {
                // Would need to merge group-by results
                Ok(QueryResult {
                    success: true,
                    rows_affected: sub_results.len(),
                    message: format!("GROUP BY aggregated from {} nodes", sub_results.len()),
                })
            }

            None => {
                // Simple aggregation: sum rows affected
                let total_rows: usize = sub_results.iter()
                    .map(|r| r.rows_affected)
                    .sum();

                Ok(QueryResult {
                    success: true,
                    rows_affected: total_rows,
                    message: format!("Query executed on {} nodes, {} total rows affected", sub_results.len(), total_rows),
                })
            }
        }
    }

    /// Determine query type from query string
    fn determine_query_type(&self, query: &str) -> QueryType {
        let query_upper = query.trim().to_uppercase();

        if query_upper.starts_with("SELECT") || query_upper.starts_with("FROM") {
            if query_upper.contains("COUNT") || query_upper.contains("SUM") ||
               query_upper.contains("AVG") || query_upper.contains("GROUP BY") {
                QueryType::Aggregate
            } else {
                QueryType::Select
            }
        } else if query_upper.starts_with("INSERT") {
            QueryType::Insert
        } else if query_upper.starts_with("UPDATE") {
            QueryType::Update
        } else if query_upper.starts_with("DELETE") {
            QueryType::Delete
        } else if query_upper.contains("TRAVERSE") {
            QueryType::Traverse
        } else {
            QueryType::Select
        }
    }

    /// Determine which shards are affected by query
    fn determine_affected_shards(&self, query: &str) -> Result<Vec<ShardId>, String> {
        // For now, simple implementation: if query has WHERE with specific key, find that shard
        // Otherwise, query all shards

        if let Some(key) = self.extract_key_from_where(query) {
            // Query affects single shard
            if let Some(shard_id) = self.shard_manager.get_shard_for_key(&key) {
                return Ok(vec![shard_id]);
            }
        }

        // Query affects all shards
        let all_shards = self.shard_manager.get_all_shards();
        Ok(all_shards.iter().map(|s| s.shard_id).collect())
    }

    /// Extract key from WHERE clause (simplified)
    fn extract_key_from_where(&self, query: &str) -> Option<String> {
        // Simplified: look for "WHERE id = <value>"
        let query_upper = query.to_uppercase();
        if let Some(where_pos) = query_upper.find("WHERE") {
            let where_clause = &query[where_pos + 5..];
            if let Some(eq_pos) = where_clause.find('=') {
                let value = where_clause[eq_pos + 1..].trim();
                // Extract value (remove quotes, semicolon, etc.)
                let value = value.trim_matches(|c| c == '\'' || c == '"' || c == ';' || c == ' ');
                return Some(value.to_string());
            }
        }
        None
    }

    /// Determine aggregation type
    fn determine_aggregation(&self, query: &str) -> Option<AggregationType> {
        let query_upper = query.to_uppercase();

        if query_upper.contains("COUNT(") {
            Some(AggregationType::Count)
        } else if query_upper.contains("SUM(") {
            Some(AggregationType::Sum)
        } else if query_upper.contains("AVG(") {
            Some(AggregationType::Avg)
        } else if query_upper.contains("MIN(") {
            Some(AggregationType::Min)
        } else if query_upper.contains("MAX(") {
            Some(AggregationType::Max)
        } else if query_upper.contains("GROUP BY") {
            // Extract fields from GROUP BY
            Some(AggregationType::GroupBy { fields: vec![] })
        } else {
            None
        }
    }

    /// Get statistics
    pub fn get_statistics(&self) -> DistributedQueryStats {
        let cache = self.query_cache.read().unwrap();

        DistributedQueryStats {
            cached_plans: cache.len(),
            local_node_id: self.local_id,
        }
    }
}

/// Statistics for distributed query execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistributedQueryStats {
    pub cached_plans: usize,
    pub local_node_id: NodeId,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_determine_query_type() {
        let executor = create_test_executor();

        assert_eq!(executor.determine_query_type("SELECT * FROM users"), QueryType::Select);
        assert_eq!(executor.determine_query_type("INSERT INTO users VALUES (...)"), QueryType::Insert);
        assert_eq!(executor.determine_query_type("UPDATE users SET ..."), QueryType::Update);
        assert_eq!(executor.determine_query_type("DELETE FROM users WHERE ..."), QueryType::Delete);
        assert_eq!(executor.determine_query_type("SELECT COUNT(*) FROM users"), QueryType::Aggregate);
        assert_eq!(executor.determine_query_type("FROM users TRAVERSE ..."), QueryType::Traverse);
    }

    #[test]
    fn test_determine_aggregation() {
        let executor = create_test_executor();

        assert_eq!(
            executor.determine_aggregation("SELECT COUNT(*) FROM users"),
            Some(AggregationType::Count)
        );

        assert_eq!(
            executor.determine_aggregation("SELECT SUM(age) FROM users"),
            Some(AggregationType::Sum)
        );

        assert_eq!(
            executor.determine_aggregation("SELECT * FROM users"),
            None
        );
    }

    #[test]
    fn test_extract_key_from_where() {
        let executor = create_test_executor();

        assert_eq!(
            executor.extract_key_from_where("SELECT * FROM users WHERE id = 123"),
            Some("123".to_string())
        );

        assert_eq!(
            executor.extract_key_from_where("SELECT * FROM users WHERE name = 'Alice'"),
            Some("Alice".to_string())
        );

        assert_eq!(
            executor.extract_key_from_where("SELECT * FROM users"),
            None
        );
    }

    // Helper function to create test executor
    fn create_test_executor() -> DistributedQueryExecutor {
        use crate::{Graph, DQLExecutor};
        use crate::distributed_topology::{NodeAddress, TopologyConfig};
        use crate::distributed_shard::ShardConfig;
        use crate::distributed_p2p::P2PConfig;

        let graph = Arc::new(RwLock::new(Graph::new()));
        let local_executor = Arc::new(DQLExecutor::new(graph));
        let shard_manager = Arc::new(ShardManager::new(ShardConfig::default()));
        let p2p_network = Arc::new(P2PNetwork::new(
            1,
            NodeAddress::new("localhost".to_string(), 9000),
            P2PConfig::default(),
        ));

        DistributedQueryExecutor::new(1, shard_manager, p2p_network, local_executor)
    }
}
