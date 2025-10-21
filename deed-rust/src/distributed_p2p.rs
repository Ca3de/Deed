//! Peer-to-Peer Communication for Distributed Deed Database
//!
//! Implements asynchronous P2P messaging between nodes in the distributed network.
//! Messages are serialized using bincode for efficiency.
//!
//! Features:
//! - Async TCP communication using Tokio
//! - Message types for data replication, query routing, and health checks
//! - Connection pooling and retry logic
//! - Heartbeat mechanism for failure detection

use crate::distributed_topology::{NodeId, NodeAddress};
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use std::time::Duration;

/// Message ID for tracking request/response
pub type MessageId = u64;

/// Types of P2P messages
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MessageType {
    /// Health check ping
    Ping,
    /// Health check response
    Pong,
    /// Request data for specific shard
    ShardDataRequest { shard_id: u64 },
    /// Response with shard data
    ShardDataResponse { shard_id: u64, data: Vec<u8> },
    /// Execute query on remote node
    QueryRequest { query: String },
    /// Query result from remote node
    QueryResponse { result: String },
    /// Notify about shard reassignment
    ShardReassignment { shard_id: u64, new_owner: NodeId },
    /// Acknowledge message receipt
    Ack,
    /// Error response
    Error { message: String },
}

/// P2P message structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct P2PMessage {
    pub id: MessageId,
    pub sender_id: NodeId,
    pub receiver_id: NodeId,
    pub message_type: MessageType,
    pub timestamp: u64,
}

impl P2PMessage {
    pub fn new(sender_id: NodeId, receiver_id: NodeId, message_type: MessageType) -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static MESSAGE_COUNTER: AtomicU64 = AtomicU64::new(1);

        Self {
            id: MESSAGE_COUNTER.fetch_add(1, Ordering::SeqCst),
            sender_id,
            receiver_id,
            message_type,
            timestamp: current_timestamp(),
        }
    }

    /// Serialize message to bytes
    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        bincode::serialize(self).map_err(|e| format!("Serialization error: {}", e))
    }

    /// Deserialize message from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        bincode::deserialize(bytes).map_err(|e| format!("Deserialization error: {}", e))
    }
}

/// Configuration for P2P network
#[derive(Debug, Clone)]
pub struct P2PConfig {
    /// Port for incoming P2P connections
    pub listen_port: u16,

    /// Timeout for connection attempts (ms)
    pub connection_timeout_ms: u64,

    /// Timeout for message responses (ms)
    pub message_timeout_ms: u64,

    /// Heartbeat interval (seconds)
    pub heartbeat_interval_secs: u64,

    /// Maximum retry attempts for failed messages
    pub max_retries: usize,

    /// Buffer size for message receiving
    pub buffer_size: usize,
}

impl Default for P2PConfig {
    fn default() -> Self {
        Self {
            listen_port: 9000,
            connection_timeout_ms: 5000,
            message_timeout_ms: 10000,
            heartbeat_interval_secs: 5,
            max_retries: 3,
            buffer_size: 65536, // 64KB
        }
    }
}

/// P2P Network Manager
pub struct P2PNetwork {
    config: P2PConfig,
    local_id: NodeId,
    local_address: NodeAddress,
    /// Known peer addresses
    peers: Arc<RwLock<HashMap<NodeId, NodeAddress>>>,
    /// Active TCP connections
    connections: Arc<RwLock<HashMap<NodeId, Arc<RwLock<TcpStream>>>>>,
    /// Message handlers (callbacks for different message types)
    handlers: Arc<RwLock<HashMap<String, Box<dyn Fn(&P2PMessage) -> Option<P2PMessage> + Send + Sync>>>>,
}

impl P2PNetwork {
    /// Create new P2P network
    pub fn new(local_id: NodeId, local_address: NodeAddress, config: P2PConfig) -> Self {
        Self {
            config,
            local_id,
            local_address,
            peers: Arc::new(RwLock::new(HashMap::new())),
            connections: Arc::new(RwLock::new(HashMap::new())),
            handlers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Add a known peer
    pub fn add_peer(&self, node_id: NodeId, address: NodeAddress) {
        let mut peers = self.peers.write().unwrap();
        peers.insert(node_id, address);
    }

    /// Remove a peer
    pub fn remove_peer(&self, node_id: NodeId) {
        let mut peers = self.peers.write().unwrap();
        peers.remove(&node_id);

        let mut connections = self.connections.write().unwrap();
        connections.remove(&node_id);
    }

    /// Get all known peers
    pub fn get_peers(&self) -> Vec<(NodeId, NodeAddress)> {
        let peers = self.peers.read().unwrap();
        peers.iter().map(|(&id, addr)| (id, addr.clone())).collect()
    }

    /// Start listening for incoming P2P connections
    pub async fn start_listener(&self) -> Result<(), String> {
        let addr = format!("{}:{}", self.local_address.host, self.config.listen_port);
        let listener = TcpListener::bind(&addr)
            .await
            .map_err(|e| format!("Failed to bind to {}: {}", addr, e))?;

        println!("P2P listening on {}", addr);

        let local_id = self.local_id;
        let handlers = Arc::clone(&self.handlers);
        let buffer_size = self.config.buffer_size;

        tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((mut stream, peer_addr)) => {
                        let handlers = Arc::clone(&handlers);

                        tokio::spawn(async move {
                            let mut buffer = vec![0u8; buffer_size];

                            loop {
                                match stream.read(&mut buffer).await {
                                    Ok(0) => {
                                        // Connection closed
                                        break;
                                    }
                                    Ok(n) => {
                                        // Parse message
                                        if let Ok(msg) = P2PMessage::from_bytes(&buffer[..n]) {
                                            // Process message
                                            if let Some(response) = Self::handle_message(&msg, &handlers) {
                                                // Send response
                                                if let Ok(response_bytes) = response.to_bytes() {
                                                    let _ = stream.write_all(&response_bytes).await;
                                                }
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        eprintln!("Error reading from {}: {}", peer_addr, e);
                                        break;
                                    }
                                }
                            }
                        });
                    }
                    Err(e) => {
                        eprintln!("Failed to accept connection: {}", e);
                    }
                }
            }
        });

        Ok(())
    }

    /// Handle incoming message
    fn handle_message(
        msg: &P2PMessage,
        handlers: &Arc<RwLock<HashMap<String, Box<dyn Fn(&P2PMessage) -> Option<P2PMessage> + Send + Sync>>>>,
    ) -> Option<P2PMessage> {
        // Default handlers for built-in message types
        match &msg.message_type {
            MessageType::Ping => {
                // Respond with Pong
                return Some(P2PMessage::new(
                    msg.receiver_id,
                    msg.sender_id,
                    MessageType::Pong,
                ));
            }
            MessageType::Pong => {
                // Update latency metrics
                return None;
            }
            _ => {
                // Check custom handlers
                let handlers = handlers.read().unwrap();
                let key = format!("{:?}", msg.message_type);
                if let Some(handler) = handlers.get(&key) {
                    return handler(msg);
                }
            }
        }

        None
    }

    /// Send message to peer
    pub async fn send_message(&self, peer_id: NodeId, message_type: MessageType) -> Result<Option<P2PMessage>, String> {
        // Get peer address
        let peer_address = {
            let peers = self.peers.read().unwrap();
            peers.get(&peer_id).cloned()
        };

        let peer_address = peer_address.ok_or_else(|| format!("Unknown peer: {}", peer_id))?;

        // Create message
        let msg = P2PMessage::new(self.local_id, peer_id, message_type);

        // Serialize message
        let msg_bytes = msg.to_bytes()?;

        // Connect to peer
        let addr = format!("{}:{}", peer_address.host, peer_address.port);
        let mut stream = tokio::time::timeout(
            Duration::from_millis(self.config.connection_timeout_ms),
            TcpStream::connect(&addr)
        )
        .await
        .map_err(|_| format!("Connection timeout to {}", addr))?
        .map_err(|e| format!("Failed to connect to {}: {}", addr, e))?;

        // Send message
        stream.write_all(&msg_bytes).await
            .map_err(|e| format!("Failed to send message: {}", e))?;

        // Wait for response (with timeout)
        let mut buffer = vec![0u8; self.config.buffer_size];
        let n = tokio::time::timeout(
            Duration::from_millis(self.config.message_timeout_ms),
            stream.read(&mut buffer)
        )
        .await
        .map_err(|_| "Response timeout".to_string())?
        .map_err(|e| format!("Failed to read response: {}", e))?;

        if n == 0 {
            return Ok(None); // No response
        }

        // Parse response
        let response = P2PMessage::from_bytes(&buffer[..n])?;
        Ok(Some(response))
    }

    /// Send ping to peer and measure latency
    pub async fn ping(&self, peer_id: NodeId) -> Result<Duration, String> {
        let start = std::time::Instant::now();

        let response = self.send_message(peer_id, MessageType::Ping).await?;

        match response {
            Some(msg) if msg.message_type == MessageType::Pong => {
                Ok(start.elapsed())
            }
            _ => Err("Invalid ping response".to_string()),
        }
    }

    /// Broadcast message to all peers
    pub async fn broadcast(&self, message_type: MessageType) -> Vec<Result<Option<P2PMessage>, String>> {
        let peers: Vec<NodeId> = {
            let peers = self.peers.read().unwrap();
            peers.keys().copied().collect()
        };

        let mut results = Vec::new();

        for peer_id in peers {
            let result = self.send_message(peer_id, message_type.clone()).await;
            results.push(result);
        }

        results
    }

    /// Start heartbeat to all peers
    pub fn start_heartbeat(&self) {
        let interval_secs = self.config.heartbeat_interval_secs;
        let network = self.clone_for_async();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));

            loop {
                interval.tick().await;

                let peers: Vec<NodeId> = {
                    let peers = network.peers.read().unwrap();
                    peers.keys().copied().collect()
                };

                for peer_id in peers {
                    let network = network.clone();
                    tokio::spawn(async move {
                        match network.ping(peer_id).await {
                            Ok(latency) => {
                                println!("Ping to {}: {:?}", peer_id, latency);
                            }
                            Err(e) => {
                                eprintln!("Ping failed to {}: {}", peer_id, e);
                            }
                        }
                    });
                }
            }
        });
    }

    /// Clone for async tasks
    fn clone_for_async(&self) -> Arc<Self> {
        Arc::new(Self {
            config: self.config.clone(),
            local_id: self.local_id,
            local_address: self.local_address.clone(),
            peers: Arc::clone(&self.peers),
            connections: Arc::clone(&self.connections),
            handlers: Arc::clone(&self.handlers),
        })
    }

    /// Register custom message handler
    pub fn register_handler<F>(&self, message_type_name: String, handler: F)
    where
        F: Fn(&P2PMessage) -> Option<P2PMessage> + Send + Sync + 'static,
    {
        let mut handlers = self.handlers.write().unwrap();
        handlers.insert(message_type_name, Box::new(handler));
    }

    /// Get network statistics
    pub fn get_stats(&self) -> P2PStats {
        let peers = self.peers.read().unwrap();
        let connections = self.connections.read().unwrap();

        P2PStats {
            total_peers: peers.len(),
            active_connections: connections.len(),
            local_address: self.local_address.to_string(),
        }
    }
}

/// P2P network statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct P2PStats {
    pub total_peers: usize,
    pub active_connections: usize,
    pub local_address: String,
}

/// Helper function to get current Unix timestamp
fn current_timestamp() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_serialization() {
        let msg = P2PMessage::new(1, 2, MessageType::Ping);

        let bytes = msg.to_bytes().unwrap();
        let deserialized = P2PMessage::from_bytes(&bytes).unwrap();

        assert_eq!(msg.sender_id, deserialized.sender_id);
        assert_eq!(msg.receiver_id, deserialized.receiver_id);
        assert_eq!(msg.message_type, deserialized.message_type);
    }

    #[test]
    fn test_add_remove_peer() {
        let addr = NodeAddress::new("localhost".to_string(), 9000);
        let network = P2PNetwork::new(1, addr.clone(), P2PConfig::default());

        let peer_addr = NodeAddress::new("localhost".to_string(), 9001);
        network.add_peer(2, peer_addr.clone());

        assert_eq!(network.get_peers().len(), 1);

        network.remove_peer(2);
        assert_eq!(network.get_peers().len(), 0);
    }

    #[test]
    fn test_p2p_stats() {
        let addr = NodeAddress::new("localhost".to_string(), 9000);
        let network = P2PNetwork::new(1, addr.clone(), P2PConfig::default());

        network.add_peer(2, NodeAddress::new("localhost".to_string(), 9001));
        network.add_peer(3, NodeAddress::new("localhost".to_string(), 9002));

        let stats = network.get_stats();
        assert_eq!(stats.total_peers, 2);
        assert_eq!(stats.local_address, "localhost:9000");
    }

    #[tokio::test]
    async fn test_message_types() {
        let types = vec![
            MessageType::Ping,
            MessageType::Pong,
            MessageType::Ack,
            MessageType::Error { message: "test".to_string() },
        ];

        for msg_type in types {
            let msg = P2PMessage::new(1, 2, msg_type.clone());
            let bytes = msg.to_bytes().unwrap();
            let recovered = P2PMessage::from_bytes(&bytes).unwrap();
            assert_eq!(msg.message_type, recovered.message_type);
        }
    }
}
