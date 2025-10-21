//! Prometheus Metrics Integration for Distributed Deed
//!
//! Exposes metrics about the distributed database cluster for monitoring
//! and alerting using Prometheus.
//!
//! Metric categories:
//! - Query latency and throughput
//! - Node health and availability
//! - Shard distribution and rebalancing
//! - Network partition events
//! - Recovery actions
//! - Consensus state

use prometheus::{
    Counter, CounterVec, Gauge, GaugeVec, Histogram, HistogramVec,
    Registry, Opts, HistogramOpts, opts,
};
use serde::{Serialize, Deserialize};
use std::sync::Arc;

/// Prometheus metrics collector for Deed
pub struct DeedMetrics {
    registry: Registry,

    // Query metrics
    query_total: CounterVec,
    query_duration: HistogramVec,
    query_errors: CounterVec,

    // Node metrics
    node_health: GaugeVec,
    node_partitioned: Gauge,
    node_has_quorum: Gauge,

    // Shard metrics
    shard_count: GaugeVec,
    shard_rebalance_total: Counter,
    shard_size_bytes: GaugeVec,

    // Network metrics
    network_messages_sent: CounterVec,
    network_messages_received: CounterVec,
    network_latency: HistogramVec,

    // Consensus metrics
    raft_term: Gauge,
    raft_leader: Gauge,
    raft_log_entries: Gauge,

    // Transaction metrics (2PC)
    transaction_total: CounterVec,
    transaction_duration: Histogram,

    // Recovery metrics
    recovery_actions_total: CounterVec,
    failed_nodes: Gauge,
}

impl DeedMetrics {
    /// Create new metrics collector
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let registry = Registry::new();

        // Query metrics
        let query_total = CounterVec::new(
            Opts::new("deed_queries_total", "Total number of queries executed"),
            &["type", "node_id"],
        )?;

        let query_duration = HistogramVec::new(
            HistogramOpts::new("deed_query_duration_seconds", "Query execution duration")
                .buckets(vec![0.001, 0.01, 0.1, 0.5, 1.0, 5.0]),
            &["type", "node_id"],
        )?;

        let query_errors = CounterVec::new(
            Opts::new("deed_query_errors_total", "Total number of query errors"),
            &["type", "error_type"],
        )?;

        // Node metrics
        let node_health = GaugeVec::new(
            Opts::new("deed_node_health", "Node health status (1=healthy, 0=unhealthy)"),
            &["node_id"],
        )?;

        let node_partitioned = Gauge::new(
            "deed_node_partitioned",
            "Whether this node is in a network partition (1=yes, 0=no)",
        )?;

        let node_has_quorum = Gauge::new(
            "deed_node_has_quorum",
            "Whether this node has quorum (1=yes, 0=no)",
        )?;

        // Shard metrics
        let shard_count = GaugeVec::new(
            Opts::new("deed_shards_count", "Number of shards per node"),
            &["node_id", "role"],
        )?;

        let shard_rebalance_total = Counter::new(
            "deed_shard_rebalance_total",
            "Total number of shard rebalancing operations",
        )?;

        let shard_size_bytes = GaugeVec::new(
            Opts::new("deed_shard_size_bytes", "Size of each shard in bytes"),
            &["shard_id", "node_id"],
        )?;

        // Network metrics
        let network_messages_sent = CounterVec::new(
            Opts::new("deed_network_messages_sent_total", "Total messages sent"),
            &["message_type", "target_node"],
        )?;

        let network_messages_received = CounterVec::new(
            Opts::new("deed_network_messages_received_total", "Total messages received"),
            &["message_type", "source_node"],
        )?;

        let network_latency = HistogramVec::new(
            HistogramOpts::new("deed_network_latency_seconds", "Network message latency")
                .buckets(vec![0.0001, 0.001, 0.01, 0.05, 0.1, 0.5]),
            &["target_node"],
        )?;

        // Consensus metrics
        let raft_term = Gauge::new(
            "deed_raft_term",
            "Current Raft term",
        )?;

        let raft_leader = Gauge::new(
            "deed_raft_leader",
            "Whether this node is the Raft leader (1=yes, 0=no)",
        )?;

        let raft_log_entries = Gauge::new(
            "deed_raft_log_entries",
            "Number of entries in Raft log",
        )?;

        // Transaction metrics
        let transaction_total = CounterVec::new(
            Opts::new("deed_transactions_total", "Total distributed transactions"),
            &["state"],
        )?;

        let transaction_duration = Histogram::new(
            HistogramOpts::new("deed_transaction_duration_seconds", "Transaction duration")
                .buckets(vec![0.01, 0.1, 0.5, 1.0, 5.0, 10.0]),
        )?;

        // Recovery metrics
        let recovery_actions_total = CounterVec::new(
            Opts::new("deed_recovery_actions_total", "Total recovery actions"),
            &["action_type"],
        )?;

        let failed_nodes = Gauge::new(
            "deed_failed_nodes",
            "Number of failed nodes",
        )?;

        // Register all metrics
        registry.register(Box::new(query_total.clone()))?;
        registry.register(Box::new(query_duration.clone()))?;
        registry.register(Box::new(query_errors.clone()))?;
        registry.register(Box::new(node_health.clone()))?;
        registry.register(Box::new(node_partitioned.clone()))?;
        registry.register(Box::new(node_has_quorum.clone()))?;
        registry.register(Box::new(shard_count.clone()))?;
        registry.register(Box::new(shard_rebalance_total.clone()))?;
        registry.register(Box::new(shard_size_bytes.clone()))?;
        registry.register(Box::new(network_messages_sent.clone()))?;
        registry.register(Box::new(network_messages_received.clone()))?;
        registry.register(Box::new(network_latency.clone()))?;
        registry.register(Box::new(raft_term.clone()))?;
        registry.register(Box::new(raft_leader.clone()))?;
        registry.register(Box::new(raft_log_entries.clone()))?;
        registry.register(Box::new(transaction_total.clone()))?;
        registry.register(Box::new(transaction_duration.clone()))?;
        registry.register(Box::new(recovery_actions_total.clone()))?;
        registry.register(Box::new(failed_nodes.clone()))?;

        Ok(Self {
            registry,
            query_total,
            query_duration,
            query_errors,
            node_health,
            node_partitioned,
            node_has_quorum,
            shard_count,
            shard_rebalance_total,
            shard_size_bytes,
            network_messages_sent,
            network_messages_received,
            network_latency,
            raft_term,
            raft_leader,
            raft_log_entries,
            transaction_total,
            transaction_duration,
            recovery_actions_total,
            failed_nodes,
        })
    }

    /// Record a query
    pub fn record_query(&self, query_type: &str, node_id: &str, duration_secs: f64) {
        self.query_total
            .with_label_values(&[query_type, node_id])
            .inc();

        self.query_duration
            .with_label_values(&[query_type, node_id])
            .observe(duration_secs);
    }

    /// Record a query error
    pub fn record_query_error(&self, query_type: &str, error_type: &str) {
        self.query_errors
            .with_label_values(&[query_type, error_type])
            .inc();
    }

    /// Update node health
    pub fn set_node_health(&self, node_id: &str, is_healthy: bool) {
        self.node_health
            .with_label_values(&[node_id])
            .set(if is_healthy { 1.0 } else { 0.0 });
    }

    /// Update partition status
    pub fn set_partitioned(&self, is_partitioned: bool) {
        self.node_partitioned.set(if is_partitioned { 1.0 } else { 0.0 });
    }

    /// Update quorum status
    pub fn set_has_quorum(&self, has_quorum: bool) {
        self.node_has_quorum.set(if has_quorum { 1.0 } else { 0.0 });
    }

    /// Update shard count
    pub fn set_shard_count(&self, node_id: &str, role: &str, count: usize) {
        self.shard_count
            .with_label_values(&[node_id, role])
            .set(count as f64);
    }

    /// Record shard rebalance
    pub fn record_rebalance(&self) {
        self.shard_rebalance_total.inc();
    }

    /// Update shard size
    pub fn set_shard_size(&self, shard_id: &str, node_id: &str, size_bytes: u64) {
        self.shard_size_bytes
            .with_label_values(&[shard_id, node_id])
            .set(size_bytes as f64);
    }

    /// Record message sent
    pub fn record_message_sent(&self, message_type: &str, target_node: &str) {
        self.network_messages_sent
            .with_label_values(&[message_type, target_node])
            .inc();
    }

    /// Record message received
    pub fn record_message_received(&self, message_type: &str, source_node: &str) {
        self.network_messages_received
            .with_label_values(&[message_type, source_node])
            .inc();
    }

    /// Record network latency
    pub fn record_network_latency(&self, target_node: &str, latency_secs: f64) {
        self.network_latency
            .with_label_values(&[target_node])
            .observe(latency_secs);
    }

    /// Update Raft term
    pub fn set_raft_term(&self, term: u64) {
        self.raft_term.set(term as f64);
    }

    /// Update Raft leader status
    pub fn set_raft_leader(&self, is_leader: bool) {
        self.raft_leader.set(if is_leader { 1.0 } else { 0.0 });
    }

    /// Update Raft log entries count
    pub fn set_raft_log_entries(&self, count: usize) {
        self.raft_log_entries.set(count as f64);
    }

    /// Record transaction
    pub fn record_transaction(&self, state: &str, duration_secs: f64) {
        self.transaction_total
            .with_label_values(&[state])
            .inc();

        self.transaction_duration.observe(duration_secs);
    }

    /// Record recovery action
    pub fn record_recovery_action(&self, action_type: &str) {
        self.recovery_actions_total
            .with_label_values(&[action_type])
            .inc();
    }

    /// Update failed nodes count
    pub fn set_failed_nodes(&self, count: usize) {
        self.failed_nodes.set(count as f64);
    }

    /// Get registry for HTTP exposure
    pub fn registry(&self) -> &Registry {
        &self.registry
    }

    /// Gather metrics in Prometheus format
    pub fn gather(&self) -> String {
        use prometheus::Encoder;
        let encoder = prometheus::TextEncoder::new();

        let metric_families = self.registry.gather();
        let mut buffer = Vec::new();

        encoder.encode(&metric_families, &mut buffer).unwrap();
        String::from_utf8(buffer).unwrap()
    }
}

/// HTTP server for Prometheus metrics endpoint
pub struct MetricsServer {
    metrics: Arc<DeedMetrics>,
    port: u16,
}

impl MetricsServer {
    /// Create new metrics server
    pub fn new(metrics: Arc<DeedMetrics>, port: u16) -> Self {
        Self { metrics, port }
    }

    /// Start HTTP server for /metrics endpoint
    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        use axum::{Router, routing::get};
        use std::net::SocketAddr;

        let metrics = Arc::clone(&self.metrics);

        let app = Router::new().route("/metrics", get(move || {
            let m = Arc::clone(&metrics);
            async move { m.gather() }
        }));

        let addr = SocketAddr::from(([0, 0, 0, 0], self.port));
        println!("Prometheus metrics server listening on http://{}/metrics", addr);

        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, app).await?;

        Ok(())
    }
}

/// Metrics snapshot for JSON export
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSnapshot {
    pub queries_total: u64,
    pub queries_per_second: f64,
    pub avg_query_latency_ms: f64,
    pub nodes_healthy: usize,
    pub nodes_total: usize,
    pub shards_total: usize,
    pub is_partitioned: bool,
    pub has_quorum: bool,
    pub raft_term: u64,
    pub is_leader: bool,
    pub transactions_total: u64,
    pub failed_nodes: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_creation() {
        let metrics = DeedMetrics::new().unwrap();
        let output = metrics.gather();
        assert!(output.contains("deed_"));
    }

    #[test]
    fn test_record_query() {
        let metrics = DeedMetrics::new().unwrap();
        metrics.record_query("SELECT", "node1", 0.05);

        let output = metrics.gather();
        assert!(output.contains("deed_queries_total"));
    }

    #[test]
    fn test_node_health() {
        let metrics = DeedMetrics::new().unwrap();
        metrics.set_node_health("node1", true);
        metrics.set_node_health("node2", false);

        let output = metrics.gather();
        assert!(output.contains("deed_node_health"));
    }

    #[test]
    fn test_shard_metrics() {
        let metrics = DeedMetrics::new().unwrap();
        metrics.set_shard_count("node1", "primary", 10);
        metrics.set_shard_count("node1", "replica", 15);
        metrics.record_rebalance();

        let output = metrics.gather();
        assert!(output.contains("deed_shards_count"));
        assert!(output.contains("deed_shard_rebalance_total"));
    }

    #[test]
    fn test_raft_metrics() {
        let metrics = DeedMetrics::new().unwrap();
        metrics.set_raft_term(5);
        metrics.set_raft_leader(true);
        metrics.set_raft_log_entries(100);

        let output = metrics.gather();
        assert!(output.contains("deed_raft_term"));
        assert!(output.contains("deed_raft_leader"));
    }
}
