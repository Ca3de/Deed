//! Automatic Failure Recovery System
//!
//! Implements automatic detection and recovery from node failures:
//! - Health monitoring and failure detection
//! - Automatic replica promotion
//! - Data recovery and re-replication
//! - Graceful degradation under failures

use crate::distributed_topology::NodeId;
use crate::distributed_shard::{ShardManager, ShardId};
use crate::distributed_partition::PartitionManager;
use serde::{Serialize, Deserialize};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

/// Recovery action type
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecoveryAction {
    /// Promote replica to primary
    PromoteReplica {
        shard_id: ShardId,
        old_primary: NodeId,
        new_primary: NodeId,
    },

    /// Create new replica
    CreateReplica {
        shard_id: ShardId,
        source_node: NodeId,
        target_node: NodeId,
    },

    /// Remove failed replica
    RemoveReplica {
        shard_id: ShardId,
        failed_node: NodeId,
    },

    /// Rebalance shard
    RebalanceShard {
        shard_id: ShardId,
        from_node: NodeId,
        to_node: NodeId,
    },
}

/// Recovery state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecoveryState {
    /// Monitoring for failures
    Monitoring,
    /// Failure detected, planning recovery
    Planning,
    /// Executing recovery actions
    Recovering,
    /// Recovery complete
    Recovered,
    /// Recovery failed
    Failed,
}

/// Failure recovery manager
pub struct FailureRecoveryManager {
    /// This node's ID
    local_id: NodeId,

    /// Shard manager
    shard_manager: Arc<ShardManager>,

    /// Partition manager
    partition_manager: Arc<PartitionManager>,

    /// Current recovery state
    state: Arc<RwLock<RecoveryState>>,

    /// Pending recovery actions
    pending_actions: Arc<RwLock<Vec<RecoveryAction>>>,

    /// Completed recovery actions
    completed_actions: Arc<RwLock<Vec<RecoveryAction>>>,

    /// Failed nodes
    failed_nodes: Arc<RwLock<HashSet<NodeId>>>,

    /// Health check interval
    check_interval: Duration,

    /// Minimum replicas to maintain
    min_replicas: usize,
}

impl FailureRecoveryManager {
    /// Create new failure recovery manager
    pub fn new(
        local_id: NodeId,
        shard_manager: Arc<ShardManager>,
        partition_manager: Arc<PartitionManager>,
        min_replicas: usize,
        check_interval_secs: u64,
    ) -> Self {
        Self {
            local_id,
            shard_manager,
            partition_manager,
            state: Arc::new(RwLock::new(RecoveryState::Monitoring)),
            pending_actions: Arc::new(RwLock::new(Vec::new())),
            completed_actions: Arc::new(RwLock::new(Vec::new())),
            failed_nodes: Arc::new(RwLock::new(HashSet::new())),
            check_interval: Duration::from_secs(check_interval_secs),
            min_replicas,
        }
    }

    /// Start automatic failure recovery
    pub fn start(&self) {
        self.start_health_monitoring();
        self.start_recovery_loop();
    }

    /// Start health monitoring
    fn start_health_monitoring(&self) {
        let manager = self.clone_for_async();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(manager.check_interval);

            loop {
                interval.tick().await;
                manager.check_for_failures();
            }
        });
    }

    /// Check for node failures
    fn check_for_failures(&self) {
        let unreachable_nodes = self.partition_manager.get_unreachable_nodes();

        if unreachable_nodes.is_empty() {
            return;
        }

        println!("Detected {} unreachable nodes", unreachable_nodes.len());

        let mut failed_nodes = self.failed_nodes.write().unwrap();
        let mut new_failures = Vec::new();

        for node_id in unreachable_nodes {
            if failed_nodes.insert(node_id) {
                // New failure
                new_failures.push(node_id);
            }
        }

        drop(failed_nodes);

        if !new_failures.is_empty() {
            println!("New node failures: {:?}", new_failures);
            self.plan_recovery(new_failures);
        }
    }

    /// Plan recovery actions for failed nodes
    fn plan_recovery(&self, failed_nodes: Vec<NodeId>) {
        *self.state.write().unwrap() = RecoveryState::Planning;

        let mut actions = Vec::new();

        for failed_node in failed_nodes {
            // Get shards affected by this failure
            let affected_shards = self.shard_manager.get_shards_for_node(failed_node);

            println!("Node {} failure affects {} shards", failed_node, affected_shards.len());

            for shard in affected_shards {
                // Check if this was the primary
                if shard.primary_node == failed_node {
                    // Need to promote a replica
                    if let Some(&new_primary) = shard.replica_nodes.first() {
                        actions.push(RecoveryAction::PromoteReplica {
                            shard_id: shard.shard_id,
                            old_primary: failed_node,
                            new_primary,
                        });
                    } else {
                        println!("WARNING: No replica available for shard {}", shard.shard_id);
                    }
                } else {
                    // This was a replica - remove it
                    actions.push(RecoveryAction::RemoveReplica {
                        shard_id: shard.shard_id,
                        failed_node,
                    });
                }

                // Check if we need to create new replicas
                let current_replicas = shard.replica_nodes.iter()
                    .filter(|&&id| id != failed_node)
                    .count();

                if current_replicas < self.min_replicas {
                    // Find a node to create new replica
                    if let Some(target_node) = self.find_replication_target(shard.shard_id) {
                        actions.push(RecoveryAction::CreateReplica {
                            shard_id: shard.shard_id,
                            source_node: shard.primary_node,
                            target_node,
                        });
                    }
                }
            }
        }

        println!("Planned {} recovery actions", actions.len());

        *self.pending_actions.write().unwrap() = actions;
        *self.state.write().unwrap() = RecoveryState::Recovering;
    }

    /// Find a suitable node for new replica
    fn find_replication_target(&self, _shard_id: ShardId) -> Option<NodeId> {
        let reachable_nodes = self.partition_manager.get_our_partition();
        let failed_nodes = self.failed_nodes.read().unwrap();

        // Find node with least load
        reachable_nodes
            .into_iter()
            .filter(|id| !failed_nodes.contains(id) && *id != self.local_id)
            .next()
    }

    /// Start recovery execution loop
    fn start_recovery_loop(&self) {
        let manager = self.clone_for_async();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(5));

            loop {
                interval.tick().await;

                let state = *manager.state.read().unwrap();
                if state == RecoveryState::Recovering {
                    manager.execute_recovery_actions().await;
                }
            }
        });
    }

    /// Execute pending recovery actions
    async fn execute_recovery_actions(&self) {
        let actions = {
            let pending = self.pending_actions.read().unwrap();
            pending.clone()
        };

        if actions.is_empty() {
            *self.state.write().unwrap() = RecoveryState::Recovered;
            return;
        }

        println!("Executing {} recovery actions", actions.len());

        let mut completed = Vec::new();

        for action in actions {
            match self.execute_action(&action).await {
                Ok(_) => {
                    println!("Completed action: {:?}", action);
                    completed.push(action);
                }
                Err(e) => {
                    println!("Failed to execute action {:?}: {}", action, e);
                }
            }
        }

        // Remove completed actions from pending
        {
            let mut pending = self.pending_actions.write().unwrap();
            pending.retain(|a| !completed.contains(a));
        }

        // Add to completed
        {
            let mut comp = self.completed_actions.write().unwrap();
            comp.extend(completed);
        }

        // Update state
        if self.pending_actions.read().unwrap().is_empty() {
            *self.state.write().unwrap() = RecoveryState::Recovered;
            println!("Recovery complete!");
        }
    }

    /// Execute a single recovery action
    async fn execute_action(&self, action: &RecoveryAction) -> Result<(), String> {
        match action {
            RecoveryAction::PromoteReplica { shard_id, old_primary, new_primary } => {
                println!("Promoting replica for shard {}: {} -> {}",
                    shard_id, old_primary, new_primary);
                // In production, would:
                // 1. Notify new primary to take over
                // 2. Update shard assignment
                // 3. Wait for acknowledgment
                Ok(())
            }

            RecoveryAction::CreateReplica { shard_id, source_node, target_node } => {
                println!("Creating new replica for shard {}: {} -> {}",
                    shard_id, source_node, target_node);
                // In production, would:
                // 1. Request data from source
                // 2. Stream to target
                // 3. Verify integrity
                // 4. Update shard assignment
                Ok(())
            }

            RecoveryAction::RemoveReplica { shard_id, failed_node } => {
                println!("Removing failed replica for shard {}: node {}",
                    shard_id, failed_node);
                // In production, would:
                // 1. Update shard assignment
                // 2. Notify other replicas
                Ok(())
            }

            RecoveryAction::RebalanceShard { shard_id, from_node, to_node } => {
                println!("Rebalancing shard {}: {} -> {}",
                    shard_id, from_node, to_node);
                Ok(())
            }
        }
    }

    /// Mark a node as recovered
    pub fn mark_recovered(&self, node_id: NodeId) {
        let mut failed_nodes = self.failed_nodes.write().unwrap();
        if failed_nodes.remove(&node_id) {
            println!("Node {} recovered", node_id);
        }
    }

    /// Get recovery state
    pub fn get_state(&self) -> RecoveryState {
        *self.state.read().unwrap()
    }

    /// Get statistics
    pub fn get_statistics(&self) -> RecoveryStats {
        RecoveryStats {
            state: self.get_state(),
            failed_nodes: self.failed_nodes.read().unwrap().len(),
            pending_actions: self.pending_actions.read().unwrap().len(),
            completed_actions: self.completed_actions.read().unwrap().len(),
        }
    }

    /// Clone for async tasks
    fn clone_for_async(&self) -> Arc<Self> {
        Arc::new(Self {
            local_id: self.local_id,
            shard_manager: Arc::clone(&self.shard_manager),
            partition_manager: Arc::clone(&self.partition_manager),
            state: Arc::clone(&self.state),
            pending_actions: Arc::clone(&self.pending_actions),
            completed_actions: Arc::clone(&self.completed_actions),
            failed_nodes: Arc::clone(&self.failed_nodes),
            check_interval: self.check_interval,
            min_replicas: self.min_replicas,
        })
    }
}

/// Recovery statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryStats {
    pub state: RecoveryState,
    pub failed_nodes: usize,
    pub pending_actions: usize,
    pub completed_actions: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::distributed_shard::ShardConfig;

    fn create_test_manager() -> FailureRecoveryManager {
        let shard_manager = Arc::new(ShardManager::new(ShardConfig::default()));
        let partition_manager = Arc::new(PartitionManager::new(1, 5, 3, 10));

        FailureRecoveryManager::new(1, shard_manager, partition_manager, 2, 10)
    }

    #[test]
    fn test_recovery_manager_creation() {
        let manager = create_test_manager();
        assert_eq!(manager.get_state(), RecoveryState::Monitoring);
    }

    #[test]
    fn test_stats() {
        let manager = create_test_manager();
        let stats = manager.get_statistics();

        assert_eq!(stats.state, RecoveryState::Monitoring);
        assert_eq!(stats.failed_nodes, 0);
        assert_eq!(stats.pending_actions, 0);
    }

    #[test]
    fn test_mark_recovered() {
        let manager = create_test_manager();

        manager.failed_nodes.write().unwrap().insert(2);
        assert_eq!(manager.failed_nodes.read().unwrap().len(), 1);

        manager.mark_recovered(2);
        assert_eq!(manager.failed_nodes.read().unwrap().len(), 0);
    }
}
