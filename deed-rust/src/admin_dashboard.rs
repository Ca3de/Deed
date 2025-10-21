//! Admin Dashboard
//!
//! CLI-based administration dashboard for monitoring and managing Deed database.
//! Provides real-time statistics, metrics, and management capabilities.

use crate::graph::Graph;
use crate::auth::{AuthManager, Role};
use crate::connection_pool::{ConnectionPool, PoolStats};
use crate::replication::{ReplicationManager, ReplicationStats, NodeRole};
use crate::backup::{BackupManager, BackupMetadata};
use crate::transaction::TransactionManager;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

/// Dashboard statistics
#[derive(Debug, Clone)]
pub struct DashboardStats {
    /// Database statistics
    pub database: DatabaseStats,
    /// Connection pool statistics
    pub pool: Option<PoolStats>,
    /// Replication statistics
    pub replication: Option<ReplicationStats>,
    /// Authentication statistics
    pub auth: AuthStats,
    /// Transaction statistics
    pub transactions: TransactionStats,
    /// System uptime
    pub uptime_seconds: u64,
}

#[derive(Debug, Clone)]
pub struct DatabaseStats {
    pub entity_count: usize,
    pub edge_count: usize,
    pub total_properties: usize,
    pub collections: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct AuthStats {
    pub total_users: usize,
    pub active_sessions: usize,
    pub users_by_role: RoleBreakdown,
}

#[derive(Debug, Clone)]
pub struct RoleBreakdown {
    pub admin_count: usize,
    pub readwrite_count: usize,
    pub readonly_count: usize,
}

#[derive(Debug, Clone)]
pub struct TransactionStats {
    pub active_transactions: usize,
    pub committed_transactions: usize,
    pub rollbacked_transactions: usize,
}

/// Admin dashboard
pub struct AdminDashboard {
    start_time: u64,
}

impl AdminDashboard {
    pub fn new() -> Self {
        AdminDashboard {
            start_time: current_timestamp(),
        }
    }

    /// Get comprehensive dashboard statistics
    pub fn get_stats(
        &self,
        graph: &Graph,
        auth: &AuthManager,
        pool: Option<&ConnectionPool>,
        replication: Option<&ReplicationManager>,
        transaction_mgr: &TransactionManager,
    ) -> DashboardStats {
        DashboardStats {
            database: self.get_database_stats(graph),
            pool: pool.map(|p| p.stats()),
            replication: replication.map(|r| r.stats()),
            auth: self.get_auth_stats(auth),
            transactions: self.get_transaction_stats(transaction_mgr),
            uptime_seconds: current_timestamp() - self.start_time,
        }
    }

    /// Format dashboard as CLI output
    pub fn format_dashboard(&self, stats: &DashboardStats) -> String {
        let mut output = String::new();

        output.push_str("╔══════════════════════════════════════════════════════════════╗\n");
        output.push_str("║             DEED DATABASE ADMIN DASHBOARD                    ║\n");
        output.push_str("╚══════════════════════════════════════════════════════════════╝\n\n");

        // System Info
        output.push_str(&format!("⏱  Uptime: {} ({})\n\n",
            format_duration(stats.uptime_seconds),
            format_timestamp(current_timestamp())
        ));

        // Database Stats
        output.push_str("┌─ DATABASE ──────────────────────────────────────────────────┐\n");
        output.push_str(&format!("│ Entities:    {:>10}                                      │\n", stats.database.entity_count));
        output.push_str(&format!("│ Edges:       {:>10}                                      │\n", stats.database.edge_count));
        output.push_str(&format!("│ Properties:  {:>10}                                      │\n", stats.database.total_properties));
        output.push_str(&format!("│ Collections: {:>10}                                      │\n", stats.database.collections.len()));
        output.push_str("└─────────────────────────────────────────────────────────────┘\n\n");

        // Connection Pool
        if let Some(pool) = &stats.pool {
            output.push_str("┌─ CONNECTION POOL ───────────────────────────────────────────┐\n");
            output.push_str(&format!("│ Total:   {:>3} / {:>3} (min: {:>2}, max: {:>2})                     │\n",
                pool.active_connections + pool.idle_connections,
                pool.max_size,
                pool.min_size,
                pool.max_size
            ));
            output.push_str(&format!("│ Active:  {:>3}  {}                                │\n",
                pool.active_connections,
                format_bar(pool.active_connections, pool.max_size, 20)
            ));
            output.push_str(&format!("│ Idle:    {:>3}  {}                                │\n",
                pool.idle_connections,
                format_bar(pool.idle_connections, pool.max_size, 20)
            ));
            output.push_str(&format!("│ Utilization: {:>5.1}%                                         │\n", pool.utilization()));
            output.push_str("└─────────────────────────────────────────────────────────────┘\n\n");
        }

        // Replication
        if let Some(repl) = &stats.replication {
            output.push_str("┌─ REPLICATION ───────────────────────────────────────────────┐\n");
            output.push_str(&format!("│ Node:     {}                                          │\n",
                pad_right(&repl.node_id, 20)
            ));
            output.push_str(&format!("│ Role:     {:?}                                              │\n", repl.role));
            output.push_str(&format!("│ Sequence: {:>10}                                      │\n", repl.current_seq));
            output.push_str(&format!("│ Log Size: {:>10}                                      │\n", repl.log_size));

            if repl.role == NodeRole::Master {
                output.push_str(&format!("│ Slaves:   {:>10}                                      │\n", repl.slave_count));
                if repl.slave_count > 0 {
                    output.push_str(&format!("│ Max Lag:  {:>10} ms                                  │\n", repl.max_slave_lag_ms));
                }
            }
            output.push_str("└─────────────────────────────────────────────────────────────┘\n\n");
        }

        // Authentication
        output.push_str("┌─ AUTHENTICATION ────────────────────────────────────────────┐\n");
        output.push_str(&format!("│ Total Users:     {:>5}                                     │\n", stats.auth.total_users));
        output.push_str(&format!("│ Active Sessions: {:>5}                                     │\n", stats.auth.active_sessions));
        output.push_str("│                                                             │\n");
        output.push_str("│ Users by Role:                                              │\n");
        output.push_str(&format!("│   Admin:     {:>5}                                         │\n", stats.auth.users_by_role.admin_count));
        output.push_str(&format!("│   ReadWrite: {:>5}                                         │\n", stats.auth.users_by_role.readwrite_count));
        output.push_str(&format!("│   ReadOnly:  {:>5}                                         │\n", stats.auth.users_by_role.readonly_count));
        output.push_str("└─────────────────────────────────────────────────────────────┘\n\n");

        // Transactions
        output.push_str("┌─ TRANSACTIONS ──────────────────────────────────────────────┐\n");
        output.push_str(&format!("│ Active:      {:>10}                                      │\n", stats.transactions.active_transactions));
        output.push_str(&format!("│ Committed:   {:>10}                                      │\n", stats.transactions.committed_transactions));
        output.push_str(&format!("│ Rolled Back: {:>10}                                      │\n", stats.transactions.rollbacked_transactions));
        output.push_str("└─────────────────────────────────────────────────────────────┘\n\n");

        output
    }

    /// Format backup list
    pub fn format_backups(&self, backups: &[BackupMetadata]) -> String {
        let mut output = String::new();

        output.push_str("╔══════════════════════════════════════════════════════════════╗\n");
        output.push_str("║                         BACKUPS                              ║\n");
        output.push_str("╚══════════════════════════════════════════════════════════════╝\n\n");

        if backups.is_empty() {
            output.push_str("No backups found.\n");
            return output;
        }

        for (i, backup) in backups.iter().enumerate() {
            output.push_str(&format!("{}. {}\n", i + 1, backup.backup_id));
            output.push_str(&format!("   Type:      {:?}\n", backup.backup_type));
            output.push_str(&format!("   Time:      {}\n", format_timestamp(backup.timestamp)));
            output.push_str(&format!("   Entities:  {}\n", backup.entity_count));
            output.push_str(&format!("   Edges:     {}\n", backup.edge_count));
            output.push_str(&format!("   Compressed: {}\n", backup.compressed));
            output.push_str(&format!("   Checksum:  {}\n\n", &backup.checksum[..16]));
        }

        output
    }

    fn get_database_stats(&self, graph: &Graph) -> DatabaseStats {
        let entities = graph.get_all_entities();
        let edges = graph.get_all_edges();

        let total_properties: usize = entities.iter()
            .map(|e| e.properties.len())
            .sum();

        let mut collections = std::collections::HashSet::new();
        for entity in &entities {
            collections.insert(entity.entity_type.clone());
        }

        DatabaseStats {
            entity_count: entities.len(),
            edge_count: edges.len(),
            total_properties,
            collections: collections.into_iter().collect(),
        }
    }

    fn get_auth_stats(&self, auth: &AuthManager) -> AuthStats {
        let users = auth.list_users();
        let sessions = auth.list_active_sessions();

        let mut admin_count = 0;
        let mut readwrite_count = 0;
        let mut readonly_count = 0;

        for user in &users {
            match user.role {
                Role::Admin => admin_count += 1,
                Role::ReadWrite => readwrite_count += 1,
                Role::ReadOnly => readonly_count += 1,
            }
        }

        AuthStats {
            total_users: users.len(),
            active_sessions: sessions.len(),
            users_by_role: RoleBreakdown {
                admin_count,
                readwrite_count,
                readonly_count,
            },
        }
    }

    fn get_transaction_stats(&self, transaction_mgr: &TransactionManager) -> TransactionStats {
        let stats = transaction_mgr.stats();

        TransactionStats {
            active_transactions: stats.active_count,
            committed_transactions: stats.committed_count,
            rollbacked_transactions: stats.rollbacked_count,
        }
    }
}

// Helper functions for formatting

fn format_duration(seconds: u64) -> String {
    let days = seconds / 86400;
    let hours = (seconds % 86400) / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;

    if days > 0 {
        format!("{}d {}h {}m {}s", days, hours, minutes, secs)
    } else if hours > 0 {
        format!("{}h {}m {}s", hours, minutes, secs)
    } else if minutes > 0 {
        format!("{}m {}s", minutes, secs)
    } else {
        format!("{}s", secs)
    }
}

fn format_timestamp(timestamp: u64) -> String {
    use chrono::{DateTime, Utc, NaiveDateTime};

    let dt = DateTime::<Utc>::from_utc(
        NaiveDateTime::from_timestamp(timestamp as i64, 0),
        Utc
    );

    dt.format("%Y-%m-%d %H:%M:%S UTC").to_string()
}

fn format_bar(value: usize, max: usize, width: usize) -> String {
    let filled = if max > 0 {
        (value * width) / max
    } else {
        0
    };

    let mut bar = String::new();
    bar.push('[');
    for i in 0..width {
        if i < filled {
            bar.push('█');
        } else {
            bar.push('░');
        }
    }
    bar.push(']');
    bar
}

fn pad_right(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{}{}", s, " ".repeat(width - s.len()))
    }
}

fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dashboard_creation() {
        let dashboard = AdminDashboard::new();
        assert!(dashboard.start_time > 0);
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(30), "30s");
        assert_eq!(format_duration(90), "1m 30s");
        assert_eq!(format_duration(3665), "1h 1m 5s");
        assert_eq!(format_duration(90000), "1d 1h 0m 0s");
    }

    #[test]
    fn test_format_bar() {
        assert_eq!(format_bar(0, 10, 10), "[░░░░░░░░░░]");
        assert_eq!(format_bar(5, 10, 10), "[█████░░░░░]");
        assert_eq!(format_bar(10, 10, 10), "[██████████]");
    }

    #[test]
    fn test_pad_right() {
        assert_eq!(pad_right("test", 10), "test      ");
        assert_eq!(pad_right("testing123", 5), "testi");
    }

    #[test]
    fn test_database_stats() {
        let dashboard = AdminDashboard::new();
        let graph = Graph::new();

        let stats = dashboard.get_database_stats(&graph);
        assert_eq!(stats.entity_count, 0);
        assert_eq!(stats.edge_count, 0);
    }
}
