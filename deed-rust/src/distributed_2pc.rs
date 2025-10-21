//! Two-Phase Commit (2PC) Protocol for Distributed Transactions
//!
//! Implements 2PC to ensure atomic transactions across multiple shards/nodes.
//!
//! Protocol phases:
//! 1. PREPARE: Coordinator asks all participants to prepare transaction
//! 2. COMMIT/ABORT: Based on votes, coordinator tells all to commit or abort
//!
//! Properties:
//! - Atomicity: All participants commit or all abort
//! - Consistency: Transaction state is consistent across nodes
//! - Blocking: Coordinator failure can block participants

use crate::distributed_topology::NodeId;
use crate::distributed_shard::ShardId;
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

/// Transaction ID (globally unique)
pub type TransactionId = u128;

/// 2PC transaction state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TwoPhaseCommitState {
    /// Initial state
    Init,
    /// Preparing (waiting for votes)
    Preparing,
    /// All voted yes, ready to commit
    Prepared,
    /// Committing
    Committing,
    /// Committed successfully
    Committed,
    /// Aborting
    Aborting,
    /// Aborted
    Aborted,
}

/// Participant vote
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Vote {
    Yes,  // Ready to commit
    No,   // Cannot commit, must abort
}

/// 2PC message types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TwoPhaseCommitMessage {
    /// Coordinator -> Participants: Prepare to commit
    Prepare {
        txn_id: TransactionId,
        operations: Vec<u8>, // Serialized operations
    },

    /// Participants -> Coordinator: Vote response
    VoteResponse {
        txn_id: TransactionId,
        participant_id: NodeId,
        vote: Vote,
    },

    /// Coordinator -> Participants: Commit the transaction
    Commit {
        txn_id: TransactionId,
    },

    /// Coordinator -> Participants: Abort the transaction
    Abort {
        txn_id: TransactionId,
    },

    /// Participants -> Coordinator: Acknowledge commit/abort
    Ack {
        txn_id: TransactionId,
        participant_id: NodeId,
        success: bool,
    },
}

/// Distributed transaction
#[derive(Debug, Clone)]
pub struct DistributedTransaction {
    pub txn_id: TransactionId,
    pub coordinator: NodeId,
    pub participants: Vec<NodeId>,
    pub state: TwoPhaseCommitState,
    pub operations: Vec<u8>,
    pub created_at: Instant,
}

/// Participant state for a transaction
#[derive(Debug, Clone)]
struct ParticipantState {
    participant_id: NodeId,
    vote: Option<Vote>,
    acked: bool,
}

/// Coordinator for 2PC
pub struct TwoPhaseCommitCoordinator {
    /// This node's ID
    node_id: NodeId,

    /// Active transactions (as coordinator)
    transactions: Arc<RwLock<HashMap<TransactionId, DistributedTransaction>>>,

    /// Participant states for each transaction
    participant_states: Arc<RwLock<HashMap<TransactionId, Vec<ParticipantState>>>>,

    /// Transaction timeout
    timeout: Duration,
}

impl TwoPhaseCommitCoordinator {
    /// Create new 2PC coordinator
    pub fn new(node_id: NodeId, timeout_secs: u64) -> Self {
        Self {
            node_id,
            transactions: Arc::new(RwLock::new(HashMap::new())),
            participant_states: Arc::new(RwLock::new(HashMap::new())),
            timeout: Duration::from_secs(timeout_secs),
        }
    }

    /// Begin a new distributed transaction
    pub fn begin_transaction(
        &self,
        participants: Vec<NodeId>,
        operations: Vec<u8>,
    ) -> TransactionId {
        use std::sync::atomic::{AtomicU128, Ordering};
        static TXN_COUNTER: AtomicU128 = AtomicU128::new(1);

        let txn_id = TXN_COUNTER.fetch_add(1, Ordering::SeqCst);

        let txn = DistributedTransaction {
            txn_id,
            coordinator: self.node_id,
            participants: participants.clone(),
            state: TwoPhaseCommitState::Init,
            operations,
            created_at: Instant::now(),
        };

        // Initialize participant states
        let participant_states: Vec<ParticipantState> = participants
            .iter()
            .map(|&id| ParticipantState {
                participant_id: id,
                vote: None,
                acked: false,
            })
            .collect();

        self.transactions.write().unwrap().insert(txn_id, txn);
        self.participant_states.write().unwrap().insert(txn_id, participant_states);

        txn_id
    }

    /// Phase 1: Send PREPARE to all participants
    pub fn send_prepare(&self, txn_id: TransactionId) -> Result<(), String> {
        let mut transactions = self.transactions.write().unwrap();
        let txn = transactions.get_mut(&txn_id)
            .ok_or_else(|| "Transaction not found".to_string())?;

        txn.state = TwoPhaseCommitState::Preparing;

        // In production, would send via P2P network
        println!("Coordinator {} sending PREPARE for txn {}", self.node_id, txn_id);

        Ok(())
    }

    /// Handle vote response from participant
    pub fn handle_vote(&self, txn_id: TransactionId, participant_id: NodeId, vote: Vote) -> Result<(), String> {
        let mut states = self.participant_states.write().unwrap();
        let participants = states.get_mut(&txn_id)
            .ok_or_else(|| "Transaction not found".to_string())?;

        // Record vote
        if let Some(state) = participants.iter_mut().find(|p| p.participant_id == participant_id) {
            state.vote = Some(vote);
        }

        // Check if all votes received
        let all_voted = participants.iter().all(|p| p.vote.is_some());

        if all_voted {
            let all_yes = participants.iter().all(|p| p.vote == Some(Vote::Yes));

            if all_yes {
                // All voted YES - proceed to commit
                println!("All participants voted YES for txn {}", txn_id);
                self.send_commit(txn_id)?;
            } else {
                // At least one NO - abort
                println!("At least one NO vote for txn {}, aborting", txn_id);
                self.send_abort(txn_id)?;
            }
        }

        Ok(())
    }

    /// Phase 2: Send COMMIT to all participants
    pub fn send_commit(&self, txn_id: TransactionId) -> Result<(), String> {
        let mut transactions = self.transactions.write().unwrap();
        let txn = transactions.get_mut(&txn_id)
            .ok_or_else(|| "Transaction not found".to_string())?;

        txn.state = TwoPhaseCommitState::Committing;

        // In production, would send via P2P network
        println!("Coordinator {} sending COMMIT for txn {}", self.node_id, txn_id);

        Ok(())
    }

    /// Phase 2: Send ABORT to all participants
    pub fn send_abort(&self, txn_id: TransactionId) -> Result<(), String> {
        let mut transactions = self.transactions.write().unwrap();
        let txn = transactions.get_mut(&txn_id)
            .ok_or_else(|| "Transaction not found".to_string())?;

        txn.state = TwoPhaseCommitState::Aborting;

        // In production, would send via P2P network
        println!("Coordinator {} sending ABORT for txn {}", self.node_id, txn_id);

        Ok(())
    }

    /// Handle acknowledgment from participant
    pub fn handle_ack(&self, txn_id: TransactionId, participant_id: NodeId, success: bool) -> Result<(), String> {
        let mut states = self.participant_states.write().unwrap();
        let participants = states.get_mut(&txn_id)
            .ok_or_else(|| "Transaction not found".to_string())?;

        // Record ack
        if let Some(state) = participants.iter_mut().find(|p| p.participant_id == participant_id) {
            state.acked = true;
        }

        // Check if all acked
        let all_acked = participants.iter().all(|p| p.acked);

        if all_acked {
            let mut transactions = self.transactions.write().unwrap();
            if let Some(txn) = transactions.get_mut(&txn_id) {
                txn.state = if success {
                    TwoPhaseCommitState::Committed
                } else {
                    TwoPhaseCommitState::Aborted
                };

                println!("Transaction {} completed: {:?}", txn_id, txn.state);
            }
        }

        Ok(())
    }

    /// Check for timed out transactions
    pub fn check_timeouts(&self) -> Vec<TransactionId> {
        let mut timed_out = Vec::new();
        let transactions = self.transactions.read().unwrap();

        for (txn_id, txn) in transactions.iter() {
            if txn.created_at.elapsed() > self.timeout {
                if txn.state != TwoPhaseCommitState::Committed &&
                   txn.state != TwoPhaseCommitState::Aborted {
                    timed_out.push(*txn_id);
                }
            }
        }

        timed_out
    }

    /// Get transaction state
    pub fn get_transaction(&self, txn_id: TransactionId) -> Option<DistributedTransaction> {
        self.transactions.read().unwrap().get(&txn_id).cloned()
    }

    /// Get statistics
    pub fn get_statistics(&self) -> TwoPhaseCommitStats {
        let transactions = self.transactions.read().unwrap();

        let total = transactions.len();
        let committed = transactions.values().filter(|t| t.state == TwoPhaseCommitState::Committed).count();
        let aborted = transactions.values().filter(|t| t.state == TwoPhaseCommitState::Aborted).count();
        let in_progress = transactions.values().filter(|t|
            t.state != TwoPhaseCommitState::Committed &&
            t.state != TwoPhaseCommitState::Aborted
        ).count();

        TwoPhaseCommitStats {
            total_transactions: total,
            committed_transactions: committed,
            aborted_transactions: aborted,
            in_progress_transactions: in_progress,
        }
    }
}

/// Participant in 2PC
pub struct TwoPhaseCommitParticipant {
    /// This node's ID
    node_id: NodeId,

    /// Transactions we're participating in
    transactions: Arc<RwLock<HashMap<TransactionId, DistributedTransaction>>>,
}

impl TwoPhaseCommitParticipant {
    /// Create new 2PC participant
    pub fn new(node_id: NodeId) -> Self {
        Self {
            node_id,
            transactions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Handle PREPARE message
    pub fn handle_prepare(&self, txn_id: TransactionId, operations: Vec<u8>) -> Vote {
        println!("Participant {} received PREPARE for txn {}", self.node_id, txn_id);

        // Try to prepare (lock resources, validate, etc.)
        let can_commit = self.try_prepare(txn_id, operations);

        if can_commit {
            Vote::Yes
        } else {
            Vote::No
        }
    }

    /// Try to prepare transaction
    fn try_prepare(&self, txn_id: TransactionId, operations: Vec<u8>) -> bool {
        // Simplified: In production would:
        // 1. Validate operations
        // 2. Lock required resources
        // 3. Write to temporary storage (WAL)
        // 4. Check constraints

        let txn = DistributedTransaction {
            txn_id,
            coordinator: 0, // Unknown
            participants: vec![],
            state: TwoPhaseCommitState::Prepared,
            operations,
            created_at: Instant::now(),
        };

        self.transactions.write().unwrap().insert(txn_id, txn);

        true // Simplified: always succeed
    }

    /// Handle COMMIT message
    pub fn handle_commit(&self, txn_id: TransactionId) -> bool {
        println!("Participant {} received COMMIT for txn {}", self.node_id, txn_id);

        let mut transactions = self.transactions.write().unwrap();
        if let Some(txn) = transactions.get_mut(&txn_id) {
            txn.state = TwoPhaseCommitState::Committed;

            // Apply changes permanently
            self.do_commit(txn_id);

            true
        } else {
            false
        }
    }

    /// Actually commit the transaction
    fn do_commit(&self, txn_id: TransactionId) {
        // Simplified: In production would:
        // 1. Apply changes from WAL to database
        // 2. Release locks
        // 3. Remove from WAL
        println!("Participant {} committed txn {}", self.node_id, txn_id);
    }

    /// Handle ABORT message
    pub fn handle_abort(&self, txn_id: TransactionId) -> bool {
        println!("Participant {} received ABORT for txn {}", self.node_id, txn_id);

        let mut transactions = self.transactions.write().unwrap();
        if let Some(txn) = transactions.get_mut(&txn_id) {
            txn.state = TwoPhaseCommitState::Aborted;

            // Rollback changes
            self.do_abort(txn_id);

            true
        } else {
            false
        }
    }

    /// Actually abort the transaction
    fn do_abort(&self, txn_id: TransactionId) {
        // Simplified: In production would:
        // 1. Discard changes from WAL
        // 2. Release locks
        // 3. Clean up resources
        println!("Participant {} aborted txn {}", self.node_id, txn_id);
    }
}

/// 2PC statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TwoPhaseCommitStats {
    pub total_transactions: usize,
    pub committed_transactions: usize,
    pub aborted_transactions: usize,
    pub in_progress_transactions: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_begin_transaction() {
        let coordinator = TwoPhaseCommitCoordinator::new(1, 30);
        let txn_id = coordinator.begin_transaction(vec![2, 3, 4], vec![1, 2, 3]);

        let txn = coordinator.get_transaction(txn_id).unwrap();
        assert_eq!(txn.participants.len(), 3);
        assert_eq!(txn.state, TwoPhaseCommitState::Init);
    }

    #[test]
    fn test_all_votes_yes() {
        let coordinator = TwoPhaseCommitCoordinator::new(1, 30);
        let txn_id = coordinator.begin_transaction(vec![2, 3], vec![1, 2, 3]);

        coordinator.send_prepare(txn_id).unwrap();

        // Both vote yes
        coordinator.handle_vote(txn_id, 2, Vote::Yes).unwrap();
        coordinator.handle_vote(txn_id, 3, Vote::Yes).unwrap();

        let txn = coordinator.get_transaction(txn_id).unwrap();
        assert_eq!(txn.state, TwoPhaseCommitState::Committing);
    }

    #[test]
    fn test_one_vote_no() {
        let coordinator = TwoPhaseCommitCoordinator::new(1, 30);
        let txn_id = coordinator.begin_transaction(vec![2, 3], vec![1, 2, 3]);

        coordinator.send_prepare(txn_id).unwrap();

        // One yes, one no
        coordinator.handle_vote(txn_id, 2, Vote::Yes).unwrap();
        coordinator.handle_vote(txn_id, 3, Vote::No).unwrap();

        let txn = coordinator.get_transaction(txn_id).unwrap();
        assert_eq!(txn.state, TwoPhaseCommitState::Aborting);
    }

    #[test]
    fn test_participant_prepare() {
        let participant = TwoPhaseCommitParticipant::new(2);
        let vote = participant.handle_prepare(100, vec![1, 2, 3]);

        assert_eq!(vote, Vote::Yes);
    }

    #[test]
    fn test_participant_commit() {
        let participant = TwoPhaseCommitParticipant::new(2);
        participant.handle_prepare(100, vec![1, 2, 3]);

        let success = participant.handle_commit(100);
        assert!(success);
    }

    #[test]
    fn test_stats() {
        let coordinator = TwoPhaseCommitCoordinator::new(1, 30);

        coordinator.begin_transaction(vec![2, 3], vec![1, 2, 3]);
        coordinator.begin_transaction(vec![2, 3], vec![4, 5, 6]);

        let stats = coordinator.get_statistics();
        assert_eq!(stats.total_transactions, 2);
        assert_eq!(stats.in_progress_transactions, 2);
    }
}
