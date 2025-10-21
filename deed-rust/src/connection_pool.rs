//! Connection Pool
//!
//! Manages a pool of database connections for concurrent client access.
//! Provides efficient connection reuse and limits concurrent connections.

use crate::dql_executor::DQLExecutor;
use crate::graph::Graph;
use crate::dql_optimizer::{AntColonyOptimizer, StigmergyCache};
use crate::transaction::TransactionManager;
use crate::wal::WALManager;
use std::sync::{Arc, Mutex, Condvar};
use std::time::{Duration, Instant};
use std::collections::VecDeque;

/// Configuration for connection pool
#[derive(Debug, Clone)]
pub struct PoolConfig {
    /// Minimum number of connections to maintain
    pub min_size: usize,
    /// Maximum number of connections allowed
    pub max_size: usize,
    /// Maximum time to wait for a connection (in seconds)
    pub connection_timeout: u64,
    /// Maximum idle time for a connection before being closed (in seconds)
    pub max_idle_time: u64,
    /// Enable connection health checks
    pub health_check_enabled: bool,
}

impl Default for PoolConfig {
    fn default() -> Self {
        PoolConfig {
            min_size: 2,
            max_size: 10,
            connection_timeout: 30,
            max_idle_time: 300, // 5 minutes
            health_check_enabled: true,
        }
    }
}

/// A connection wrapper that tracks usage
struct PooledConnection {
    executor: DQLExecutor,
    last_used: Instant,
    in_use: bool,
}

impl PooledConnection {
    fn new(executor: DQLExecutor) -> Self {
        PooledConnection {
            executor,
            last_used: Instant::now(),
            in_use: false,
        }
    }

    fn is_idle_too_long(&self, max_idle_time: Duration) -> bool {
        !self.in_use && self.last_used.elapsed() > max_idle_time
    }

    fn checkout(&mut self) -> &mut DQLExecutor {
        self.in_use = true;
        self.last_used = Instant::now();
        &mut self.executor
    }

    fn checkin(&mut self) {
        self.in_use = false;
        self.last_used = Instant::now();
    }
}

/// Connection pool for managing database connections
pub struct ConnectionPool {
    connections: Arc<Mutex<VecDeque<PooledConnection>>>,
    available: Arc<Condvar>,
    config: PoolConfig,
    current_size: Arc<Mutex<usize>>,

    // Shared database components
    graph: Arc<std::sync::RwLock<Graph>>,
    optimizer: Arc<std::sync::RwLock<AntColonyOptimizer>>,
    cache: Arc<std::sync::RwLock<StigmergyCache>>,
    transaction_manager: Arc<TransactionManager>,
    wal_manager: Option<Arc<WALManager>>,
}

impl ConnectionPool {
    /// Create a new connection pool
    pub fn new(
        graph: Arc<std::sync::RwLock<Graph>>,
        optimizer: Arc<std::sync::RwLock<AntColonyOptimizer>>,
        cache: Arc<std::sync::RwLock<StigmergyCache>>,
        transaction_manager: Arc<TransactionManager>,
        wal_manager: Option<Arc<WALManager>>,
        config: PoolConfig,
    ) -> Result<Self, String> {
        if config.min_size > config.max_size {
            return Err("min_size cannot be greater than max_size".to_string());
        }

        let pool = ConnectionPool {
            connections: Arc::new(Mutex::new(VecDeque::new())),
            available: Arc::new(Condvar::new()),
            config,
            current_size: Arc::new(Mutex::new(0)),
            graph,
            optimizer,
            cache,
            transaction_manager,
            wal_manager,
        };

        // Pre-create minimum connections
        for _ in 0..pool.config.min_size {
            pool.create_connection()?;
        }

        Ok(pool)
    }

    /// Create a new connection pool with default configuration
    pub fn with_defaults(
        graph: Arc<std::sync::RwLock<Graph>>,
        optimizer: Arc<std::sync::RwLock<AntColonyOptimizer>>,
        cache: Arc<std::sync::RwLock<StigmergyCache>>,
        transaction_manager: Arc<TransactionManager>,
        wal_manager: Option<Arc<WALManager>>,
    ) -> Result<Self, String> {
        Self::new(
            graph,
            optimizer,
            cache,
            transaction_manager,
            wal_manager,
            PoolConfig::default(),
        )
    }

    /// Create a new connection and add it to the pool
    fn create_connection(&self) -> Result<(), String> {
        let mut size = self.current_size.lock().unwrap();

        if *size >= self.config.max_size {
            return Err("Connection pool is at maximum capacity".to_string());
        }

        let executor = DQLExecutor::with_shared_components(
            self.graph.clone(),
            self.optimizer.clone(),
            self.cache.clone(),
            self.transaction_manager.clone(),
            self.wal_manager.clone(),
        );

        let pooled_conn = PooledConnection::new(executor);

        self.connections.lock().unwrap().push_back(pooled_conn);
        *size += 1;

        Ok(())
    }

    /// Get a connection from the pool
    pub fn get_connection(&self) -> Result<PooledConnectionHandle, String> {
        let timeout = Duration::from_secs(self.config.connection_timeout);
        let start = Instant::now();

        loop {
            let mut connections = self.connections.lock().unwrap();

            // Find an available connection
            if let Some(conn_idx) = connections.iter().position(|c| !c.in_use) {
                let conn = &mut connections[conn_idx];

                // Health check if enabled
                if self.config.health_check_enabled {
                    // For now, just check if it's not idle too long
                    let max_idle = Duration::from_secs(self.config.max_idle_time);
                    if conn.is_idle_too_long(max_idle) {
                        // Remove this connection and create a new one
                        connections.remove(conn_idx);
                        drop(connections);

                        let mut size = self.current_size.lock().unwrap();
                        *size -= 1;
                        drop(size);

                        self.create_connection()?;
                        continue;
                    }
                }

                conn.checkout();

                return Ok(PooledConnectionHandle {
                    pool: self.connections.clone(),
                    available: self.available.clone(),
                    index: conn_idx,
                });
            }

            // No available connections - try to create a new one
            drop(connections);

            let current_size = *self.current_size.lock().unwrap();
            if current_size < self.config.max_size {
                if self.create_connection().is_ok() {
                    continue;
                }
            }

            // Wait for a connection to become available
            if start.elapsed() >= timeout {
                return Err("Connection timeout: no connections available".to_string());
            }

            let (guard, timeout_result) = self.available
                .wait_timeout(
                    self.connections.lock().unwrap(),
                    timeout - start.elapsed()
                )
                .unwrap();

            if timeout_result.timed_out() {
                return Err("Connection timeout: no connections available".to_string());
            }
        }
    }

    /// Get the current number of connections in the pool
    pub fn size(&self) -> usize {
        *self.current_size.lock().unwrap()
    }

    /// Get the number of active (in-use) connections
    pub fn active_connections(&self) -> usize {
        self.connections
            .lock()
            .unwrap()
            .iter()
            .filter(|c| c.in_use)
            .count()
    }

    /// Get the number of idle connections
    pub fn idle_connections(&self) -> usize {
        self.connections
            .lock()
            .unwrap()
            .iter()
            .filter(|c| !c.in_use)
            .count()
    }

    /// Clean up idle connections that have exceeded max idle time
    pub fn cleanup_idle_connections(&self) {
        let max_idle = Duration::from_secs(self.config.max_idle_time);
        let mut connections = self.connections.lock().unwrap();
        let min_size = self.config.min_size;

        // Keep removing idle connections until we hit min_size or no more idle
        while connections.len() > min_size {
            if let Some(idx) = connections.iter().position(|c| c.is_idle_too_long(max_idle)) {
                connections.remove(idx);
                let mut size = self.current_size.lock().unwrap();
                *size -= 1;
            } else {
                break;
            }
        }
    }

    /// Get pool statistics
    pub fn stats(&self) -> PoolStats {
        PoolStats {
            total_connections: self.size(),
            active_connections: self.active_connections(),
            idle_connections: self.idle_connections(),
            max_size: self.config.max_size,
            min_size: self.config.min_size,
        }
    }
}

/// Handle to a pooled connection that automatically returns it to the pool on drop
pub struct PooledConnectionHandle {
    pool: Arc<Mutex<VecDeque<PooledConnection>>>,
    available: Arc<Condvar>,
    index: usize,
}

impl PooledConnectionHandle {
    /// Execute a query using this connection
    ///
    /// Note: Direct executor access is not provided due to lifetime constraints.
    /// Use this method to execute queries instead.
    pub fn execute(&mut self, query: &str) -> Result<crate::dql_executor::QueryResult, String> {
        let mut connections = self.pool.lock().unwrap();

        if let Some(conn) = connections.get_mut(self.index) {
            conn.executor.execute(query)
        } else {
            Err("Invalid connection handle".to_string())
        }
    }
}

impl Drop for PooledConnectionHandle {
    fn drop(&mut self) {
        // Return connection to pool
        if let Ok(mut connections) = self.pool.lock() {
            if let Some(conn) = connections.get_mut(self.index) {
                conn.checkin();
            }
        }

        // Notify waiting threads
        self.available.notify_one();
    }
}

/// Pool statistics
#[derive(Debug, Clone)]
pub struct PoolStats {
    pub total_connections: usize,
    pub active_connections: usize,
    pub idle_connections: usize,
    pub max_size: usize,
    pub min_size: usize,
}

impl PoolStats {
    pub fn utilization(&self) -> f64 {
        if self.max_size == 0 {
            0.0
        } else {
            (self.active_connections as f64 / self.max_size as f64) * 100.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::Graph;
    use std::thread;

    fn create_test_pool() -> ConnectionPool {
        let graph = Arc::new(std::sync::RwLock::new(Graph::new()));
        let optimizer = Arc::new(std::sync::RwLock::new(AntColonyOptimizer::new()));
        let cache = Arc::new(std::sync::RwLock::new(StigmergyCache::new()));
        let transaction_manager = Arc::new(TransactionManager::new());

        ConnectionPool::with_defaults(
            graph,
            optimizer,
            cache,
            transaction_manager,
            None,
        ).unwrap()
    }

    #[test]
    fn test_pool_creation() {
        let pool = create_test_pool();

        // Should have min_size connections
        assert_eq!(pool.size(), 2);
        assert_eq!(pool.idle_connections(), 2);
        assert_eq!(pool.active_connections(), 0);
    }

    #[test]
    fn test_get_connection() {
        let pool = create_test_pool();

        let mut handle = pool.get_connection().unwrap();

        assert_eq!(pool.active_connections(), 1);
        assert_eq!(pool.idle_connections(), 1);

        // Should be able to get executor
        assert!(handle.executor().is_ok());
    }

    #[test]
    fn test_connection_return() {
        let pool = create_test_pool();

        {
            let _handle = pool.get_connection().unwrap();
            assert_eq!(pool.active_connections(), 1);
        }

        // Connection should be returned
        assert_eq!(pool.active_connections(), 0);
        assert_eq!(pool.idle_connections(), 2);
    }

    #[test]
    fn test_concurrent_connections() {
        let pool = Arc::new(create_test_pool());
        let mut handles = vec![];

        // Spawn 5 threads, each getting a connection
        for i in 0..5 {
            let pool_clone = pool.clone();
            let handle = thread::spawn(move || {
                let mut conn = pool_clone.get_connection().unwrap();
                thread::sleep(Duration::from_millis(10));
                conn.executor().unwrap();
            });
            handles.push(handle);
        }

        // Wait for all threads
        for handle in handles {
            handle.join().unwrap();
        }

        // All connections should be returned
        assert_eq!(pool.active_connections(), 0);
    }

    #[test]
    fn test_pool_max_size() {
        let graph = Arc::new(std::sync::RwLock::new(Graph::new()));
        let optimizer = Arc::new(std::sync::RwLock::new(AntColonyOptimizer::new()));
        let cache = Arc::new(std::sync::RwLock::new(StigmergyCache::new()));
        let transaction_manager = Arc::new(TransactionManager::new());

        let config = PoolConfig {
            min_size: 1,
            max_size: 2,
            connection_timeout: 1,
            max_idle_time: 300,
            health_check_enabled: false,
        };

        let pool = ConnectionPool::new(
            graph,
            optimizer,
            cache,
            transaction_manager,
            None,
            config,
        ).unwrap();

        let _conn1 = pool.get_connection().unwrap();
        let _conn2 = pool.get_connection().unwrap();

        // Should not be able to get a third connection (with short timeout)
        let result = pool.get_connection();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("timeout"));
    }

    #[test]
    fn test_pool_stats() {
        let pool = create_test_pool();

        let stats = pool.stats();
        assert_eq!(stats.total_connections, 2);
        assert_eq!(stats.min_size, 2);
        assert_eq!(stats.max_size, 10);

        let _conn = pool.get_connection().unwrap();
        let stats = pool.stats();
        assert_eq!(stats.active_connections, 1);
        assert!(stats.utilization() > 0.0);
    }
}
