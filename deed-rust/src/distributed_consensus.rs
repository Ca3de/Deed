//! Raft Consensus Protocol Implementation
//!
//! Implements the Raft consensus algorithm for maintaining consistency across
//! distributed nodes in the Deed database cluster.
//!
//! Key features:
//! - Leader election with randomized timeouts
//! - Log replication across all nodes
//! - State machine for applying committed entries
//! - Term-based conflict resolution
//!
//! References:
//! - Raft Paper: "In Search of an Understandable Consensus Algorithm"
//! - https://raft.github.io/

use crate::distributed_topology::NodeId;
use crate::distributed_p2p::{P2PNetwork, MessageType as P2PMessageType, P2PMessage};
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock, Mutex};
use std::time::{Duration, Instant};

/// Raft term number (monotonically increasing)
pub type Term = u64;

/// Log entry index
pub type LogIndex = u64;

/// Raft node state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RaftState {
    Follower,
    Candidate,
    Leader,
}

/// Raft log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub term: Term,
    pub index: LogIndex,
    pub command: Vec<u8>, // Serialized command
}

/// Raft message types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RaftMessage {
    /// Request vote from other nodes
    RequestVote {
        term: Term,
        candidate_id: NodeId,
        last_log_index: LogIndex,
        last_log_term: Term,
    },

    /// Response to vote request
    VoteResponse {
        term: Term,
        vote_granted: bool,
    },

    /// Leader sends log entries (also used as heartbeat)
    AppendEntries {
        term: Term,
        leader_id: NodeId,
        prev_log_index: LogIndex,
        prev_log_term: Term,
        entries: Vec<LogEntry>,
        leader_commit: LogIndex,
    },

    /// Response to append entries
    AppendEntriesResponse {
        term: Term,
        success: bool,
        match_index: LogIndex,
    },
}

/// Raft configuration
#[derive(Debug, Clone)]
pub struct RaftConfig {
    /// Election timeout range (ms)
    /// Randomized between min and max to avoid split votes
    pub election_timeout_min_ms: u64,
    pub election_timeout_max_ms: u64,

    /// Heartbeat interval (ms)
    /// Leader sends heartbeats to maintain authority
    pub heartbeat_interval_ms: u64,

    /// How long to wait for responses before retrying
    pub rpc_timeout_ms: u64,
}

impl Default for RaftConfig {
    fn default() -> Self {
        Self {
            election_timeout_min_ms: 150,
            election_timeout_max_ms: 300,
            heartbeat_interval_ms: 50,
            rpc_timeout_ms: 100,
        }
    }
}

/// Raft consensus state machine
pub struct RaftNode {
    config: RaftConfig,

    /// This node's ID
    node_id: NodeId,

    /// Current state (Follower, Candidate, Leader)
    state: Arc<RwLock<RaftState>>,

    /// Current term
    current_term: Arc<RwLock<Term>>,

    /// Who we voted for in current term
    voted_for: Arc<RwLock<Option<NodeId>>>,

    /// Log entries
    log: Arc<RwLock<Vec<LogEntry>>>,

    /// Index of highest log entry known to be committed
    commit_index: Arc<RwLock<LogIndex>>,

    /// Index of highest log entry applied to state machine
    last_applied: Arc<RwLock<LogIndex>>,

    /// Leader-only: next index to send to each follower
    next_index: Arc<RwLock<HashMap<NodeId, LogIndex>>>,

    /// Leader-only: highest index replicated on each follower
    match_index: Arc<RwLock<HashMap<NodeId, LogIndex>>>,

    /// Current leader (if known)
    current_leader: Arc<RwLock<Option<NodeId>>>,

    /// List of all nodes in cluster
    cluster_nodes: Arc<RwLock<Vec<NodeId>>>,

    /// When we last heard from leader
    last_heartbeat: Arc<Mutex<Instant>>,

    /// P2P network for communication
    p2p_network: Arc<P2PNetwork>,
}

impl RaftNode {
    /// Create new Raft node
    pub fn new(
        node_id: NodeId,
        config: RaftConfig,
        p2p_network: Arc<P2PNetwork>,
    ) -> Self {
        Self {
            config,
            node_id,
            state: Arc::new(RwLock::new(RaftState::Follower)),
            current_term: Arc::new(RwLock::new(0)),
            voted_for: Arc::new(RwLock::new(None)),
            log: Arc::new(RwLock::new(Vec::new())),
            commit_index: Arc::new(RwLock::new(0)),
            last_applied: Arc::new(RwLock::new(0)),
            next_index: Arc::new(RwLock::new(HashMap::new())),
            match_index: Arc::new(RwLock::new(HashMap::new())),
            current_leader: Arc::new(RwLock::new(None)),
            cluster_nodes: Arc::new(RwLock::new(Vec::new())),
            last_heartbeat: Arc::new(Mutex::new(Instant::now())),
            p2p_network,
        }
    }

    /// Add a node to the cluster
    pub fn add_cluster_node(&self, node_id: NodeId) {
        let mut nodes = self.cluster_nodes.write().unwrap();
        if !nodes.contains(&node_id) {
            nodes.push(node_id);
        }
    }

    /// Get current state
    pub fn get_state(&self) -> RaftState {
        *self.state.read().unwrap()
    }

    /// Get current term
    pub fn get_current_term(&self) -> Term {
        *self.current_term.read().unwrap()
    }

    /// Get current leader
    pub fn get_current_leader(&self) -> Option<NodeId> {
        *self.current_leader.read().unwrap()
    }

    /// Check if this node is the leader
    pub fn is_leader(&self) -> bool {
        self.get_state() == RaftState::Leader
    }

    /// Start the Raft consensus protocol
    pub fn start(&self) {
        self.start_election_timer();
        self.start_heartbeat_timer();
    }

    /// Start election timeout timer
    fn start_election_timer(&self) {
        let node = self.clone_for_async();

        tokio::spawn(async move {
            loop {
                // Random election timeout
                let timeout_ms = rand::random::<u64>() %
                    (node.config.election_timeout_max_ms - node.config.election_timeout_min_ms) +
                    node.config.election_timeout_min_ms;

                tokio::time::sleep(Duration::from_millis(timeout_ms)).await;

                // Check if we should start election
                let state = node.get_state();
                if state != RaftState::Leader {
                    let last_hb = node.last_heartbeat.lock().unwrap().elapsed();
                    if last_hb > Duration::from_millis(timeout_ms) {
                        node.start_election();
                    }
                }
            }
        });
    }

    /// Start heartbeat timer (for leader)
    fn start_heartbeat_timer(&self) {
        let node = self.clone_for_async();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(
                Duration::from_millis(node.config.heartbeat_interval_ms)
            );

            loop {
                interval.tick().await;

                if node.is_leader() {
                    node.send_heartbeats().await;
                }
            }
        });
    }

    /// Start an election
    fn start_election(&self) {
        println!("Node {} starting election", self.node_id);

        // Transition to candidate
        *self.state.write().unwrap() = RaftState::Candidate;

        // Increment term
        let mut term = self.current_term.write().unwrap();
        *term += 1;
        let new_term = *term;
        drop(term);

        // Vote for self
        *self.voted_for.write().unwrap() = Some(self.node_id);
        let mut votes_received = 1;

        // Get last log info
        let log = self.log.read().unwrap();
        let last_log_index = log.len() as LogIndex;
        let last_log_term = log.last().map(|e| e.term).unwrap_or(0);
        drop(log);

        // Request votes from all other nodes
        let nodes = self.cluster_nodes.read().unwrap().clone();
        let total_nodes = nodes.len();

        for node_id in nodes {
            if node_id == self.node_id {
                continue;
            }

            let node = self.clone_for_async();
            tokio::spawn(async move {
                // Send vote request
                // In real implementation, would use P2P network
                // For now, simplified
            });
        }

        // Check if we won (majority)
        let quorum = (total_nodes / 2) + 1;
        if votes_received >= quorum {
            self.become_leader();
        }
    }

    /// Become the leader
    fn become_leader(&self) {
        println!("Node {} became leader for term {}", self.node_id, self.get_current_term());

        *self.state.write().unwrap() = RaftState::Leader;
        *self.current_leader.write().unwrap() = Some(self.node_id);

        // Initialize next_index and match_index
        let log_len = self.log.read().unwrap().len() as LogIndex;
        let nodes = self.cluster_nodes.read().unwrap().clone();

        let mut next_index = self.next_index.write().unwrap();
        let mut match_index = self.match_index.write().unwrap();

        for node_id in nodes {
            if node_id != self.node_id {
                next_index.insert(node_id, log_len + 1);
                match_index.insert(node_id, 0);
            }
        }
    }

    /// Send heartbeats to all followers
    async fn send_heartbeats(&self) {
        let nodes = self.cluster_nodes.read().unwrap().clone();
        let current_term = self.get_current_term();

        for node_id in nodes {
            if node_id == self.node_id {
                continue;
            }

            // Send empty AppendEntries (heartbeat)
            let msg = RaftMessage::AppendEntries {
                term: current_term,
                leader_id: self.node_id,
                prev_log_index: 0,
                prev_log_term: 0,
                entries: vec![],
                leader_commit: *self.commit_index.read().unwrap(),
            };

            // In production, would send via P2P network
            // For now, simplified
        }
    }

    /// Append new entry to log (leader only)
    pub fn append_entry(&self, command: Vec<u8>) -> Result<LogIndex, String> {
        if !self.is_leader() {
            return Err("Not the leader".to_string());
        }

        let mut log = self.log.write().unwrap();
        let index = log.len() as LogIndex + 1;
        let term = self.get_current_term();

        let entry = LogEntry {
            term,
            index,
            command,
        };

        log.push(entry);

        Ok(index)
    }

    /// Handle incoming Raft message
    pub fn handle_message(&self, sender: NodeId, message: RaftMessage) -> Option<RaftMessage> {
        match message {
            RaftMessage::RequestVote { term, candidate_id, last_log_index, last_log_term } => {
                self.handle_request_vote(term, candidate_id, last_log_index, last_log_term)
            }

            RaftMessage::VoteResponse { term, vote_granted } => {
                self.handle_vote_response(sender, term, vote_granted);
                None
            }

            RaftMessage::AppendEntries { term, leader_id, prev_log_index, prev_log_term, entries, leader_commit } => {
                self.handle_append_entries(term, leader_id, prev_log_index, prev_log_term, entries, leader_commit)
            }

            RaftMessage::AppendEntriesResponse { term, success, match_index } => {
                self.handle_append_entries_response(sender, term, success, match_index);
                None
            }
        }
    }

    /// Handle RequestVote RPC
    fn handle_request_vote(&self, term: Term, candidate_id: NodeId, last_log_index: LogIndex, last_log_term: Term) -> Option<RaftMessage> {
        let mut current_term = self.current_term.write().unwrap();

        // Update term if we're behind
        if term > *current_term {
            *current_term = term;
            *self.voted_for.write().unwrap() = None;
            *self.state.write().unwrap() = RaftState::Follower;
        }

        let term_value = *current_term;
        drop(current_term);

        // Check if we can grant vote
        let voted_for = self.voted_for.read().unwrap();
        let can_vote = term == term_value &&
            (voted_for.is_none() || *voted_for == Some(candidate_id));
        drop(voted_for);

        if can_vote {
            // Check if candidate's log is at least as up-to-date
            let log = self.log.read().unwrap();
            let our_last_term = log.last().map(|e| e.term).unwrap_or(0);
            let our_last_index = log.len() as LogIndex;
            drop(log);

            let log_ok = (last_log_term > our_last_term) ||
                (last_log_term == our_last_term && last_log_index >= our_last_index);

            if log_ok {
                *self.voted_for.write().unwrap() = Some(candidate_id);

                return Some(RaftMessage::VoteResponse {
                    term: term_value,
                    vote_granted: true,
                });
            }
        }

        Some(RaftMessage::VoteResponse {
            term: term_value,
            vote_granted: false,
        })
    }

    /// Handle VoteResponse
    fn handle_vote_response(&self, _sender: NodeId, term: Term, vote_granted: bool) {
        if self.get_state() != RaftState::Candidate {
            return;
        }

        if term > self.get_current_term() {
            *self.current_term.write().unwrap() = term;
            *self.state.write().unwrap() = RaftState::Follower;
        }

        if vote_granted {
            // Count votes and check for majority
            // Simplified for now
        }
    }

    /// Handle AppendEntries RPC
    fn handle_append_entries(&self, term: Term, leader_id: NodeId, _prev_log_index: LogIndex, _prev_log_term: Term, entries: Vec<LogEntry>, leader_commit: LogIndex) -> Option<RaftMessage> {
        let mut current_term = self.current_term.write().unwrap();

        // Update term if behind
        if term > *current_term {
            *current_term = term;
            *self.voted_for.write().unwrap() = None;
            *self.state.write().unwrap() = RaftState::Follower;
        }

        let term_value = *current_term;
        drop(current_term);

        // Reject if term is old
        if term < term_value {
            return Some(RaftMessage::AppendEntriesResponse {
                term: term_value,
                success: false,
                match_index: 0,
            });
        }

        // Valid heartbeat from leader
        *self.current_leader.write().unwrap() = Some(leader_id);
        *self.last_heartbeat.lock().unwrap() = Instant::now();

        // Append entries
        if !entries.is_empty() {
            let mut log = self.log.write().unwrap();
            log.extend(entries);
        }

        // Update commit index
        if leader_commit > *self.commit_index.read().unwrap() {
            let log_len = self.log.read().unwrap().len() as LogIndex;
            let new_commit = std::cmp::min(leader_commit, log_len);
            *self.commit_index.write().unwrap() = new_commit;
        }

        Some(RaftMessage::AppendEntriesResponse {
            term: term_value,
            success: true,
            match_index: self.log.read().unwrap().len() as LogIndex,
        })
    }

    /// Handle AppendEntriesResponse
    fn handle_append_entries_response(&self, sender: NodeId, term: Term, success: bool, match_index: LogIndex) {
        if !self.is_leader() {
            return;
        }

        if term > self.get_current_term() {
            *self.current_term.write().unwrap() = term;
            *self.state.write().unwrap() = RaftState::Follower;
            return;
        }

        if success {
            // Update match_index and next_index
            let mut match_idx = self.match_index.write().unwrap();
            match_idx.insert(sender, match_index);

            let mut next_idx = self.next_index.write().unwrap();
            next_idx.insert(sender, match_index + 1);

            // Check if we can advance commit_index
            self.advance_commit_index();
        }
    }

    /// Advance commit index if majority replicated
    fn advance_commit_index(&self) {
        let match_index = self.match_index.read().unwrap();
        let nodes = self.cluster_nodes.read().unwrap();
        let quorum = (nodes.len() / 2) + 1;

        // Find highest index replicated on majority
        let log_len = self.log.read().unwrap().len() as LogIndex;

        for n in ((*self.commit_index.read().unwrap() + 1)..=log_len).rev() {
            let mut count = 1; // Count self

            for index in match_index.values() {
                if *index >= n {
                    count += 1;
                }
            }

            if count >= quorum {
                *self.commit_index.write().unwrap() = n;
                break;
            }
        }
    }

    /// Get statistics
    pub fn get_statistics(&self) -> RaftStats {
        RaftStats {
            node_id: self.node_id,
            state: self.get_state(),
            current_term: self.get_current_term(),
            current_leader: self.get_current_leader(),
            log_length: self.log.read().unwrap().len(),
            commit_index: *self.commit_index.read().unwrap(),
            last_applied: *self.last_applied.read().unwrap(),
        }
    }

    /// Clone for async tasks
    fn clone_for_async(&self) -> Arc<Self> {
        Arc::new(Self {
            config: self.config.clone(),
            node_id: self.node_id,
            state: Arc::clone(&self.state),
            current_term: Arc::clone(&self.current_term),
            voted_for: Arc::clone(&self.voted_for),
            log: Arc::clone(&self.log),
            commit_index: Arc::clone(&self.commit_index),
            last_applied: Arc::clone(&self.last_applied),
            next_index: Arc::clone(&self.next_index),
            match_index: Arc::clone(&self.match_index),
            current_leader: Arc::clone(&self.current_leader),
            cluster_nodes: Arc::clone(&self.cluster_nodes),
            last_heartbeat: Arc::clone(&self.last_heartbeat),
            p2p_network: Arc::clone(&self.p2p_network),
        })
    }
}

/// Raft statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RaftStats {
    pub node_id: NodeId,
    pub state: RaftState,
    pub current_term: Term,
    pub current_leader: Option<NodeId>,
    pub log_length: usize,
    pub commit_index: LogIndex,
    pub last_applied: LogIndex,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::distributed_topology::NodeAddress;
    use crate::distributed_p2p::P2PConfig;

    fn create_test_node(node_id: NodeId) -> RaftNode {
        let p2p = Arc::new(P2PNetwork::new(
            node_id,
            NodeAddress::new("localhost".to_string(), 9000 + node_id as u16),
            P2PConfig::default(),
        ));

        RaftNode::new(node_id, RaftConfig::default(), p2p)
    }

    #[test]
    fn test_initial_state() {
        let node = create_test_node(1);
        assert_eq!(node.get_state(), RaftState::Follower);
        assert_eq!(node.get_current_term(), 0);
        assert_eq!(node.get_current_leader(), None);
    }

    #[test]
    fn test_add_cluster_node() {
        let node = create_test_node(1);
        node.add_cluster_node(2);
        node.add_cluster_node(3);

        let nodes = node.cluster_nodes.read().unwrap();
        assert_eq!(nodes.len(), 2);
    }

    #[test]
    fn test_append_entry_not_leader() {
        let node = create_test_node(1);
        let result = node.append_entry(vec![1, 2, 3]);
        assert!(result.is_err());
    }

    #[test]
    fn test_become_leader() {
        let node = create_test_node(1);
        node.add_cluster_node(2);
        node.add_cluster_node(3);

        node.become_leader();

        assert_eq!(node.get_state(), RaftState::Leader);
        assert_eq!(node.get_current_leader(), Some(1));
        assert!(node.is_leader());
    }

    #[test]
    fn test_raft_stats() {
        let node = create_test_node(1);
        let stats = node.get_statistics();

        assert_eq!(stats.node_id, 1);
        assert_eq!(stats.state, RaftState::Follower);
        assert_eq!(stats.current_term, 0);
        assert_eq!(stats.log_length, 0);
    }
}
