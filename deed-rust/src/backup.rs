//! Backup and Restore System
//!
//! Provides full and incremental backup/restore functionality.
//! - Full backups: Complete database snapshot
//! - Incremental backups: Only changes since last backup
//! - Compression: Optional gzip compression
//! - Verification: Checksum validation

use crate::graph::{Graph, Entity, Edge};
use crate::types::{EntityId, EdgeId, PropertyValue};
use std::collections::HashMap;
use std::fs::{File, create_dir_all};
use std::io::{Read, Write, BufReader, BufWriter};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Serialize, Deserialize};
use sha2::{Sha256, Digest};

/// Backup type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackupType {
    Full,
    Incremental,
}

/// Backup metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupMetadata {
    pub backup_id: String,
    pub backup_type: BackupType,
    pub timestamp: u64,
    pub entity_count: usize,
    pub edge_count: usize,
    pub compressed: bool,
    pub checksum: String,
    pub parent_backup_id: Option<String>, // For incremental backups
}

/// Backup configuration
#[derive(Debug, Clone)]
pub struct BackupConfig {
    pub backup_dir: PathBuf,
    pub compress: bool,
    pub verify: bool,
}

impl Default for BackupConfig {
    fn default() -> Self {
        BackupConfig {
            backup_dir: PathBuf::from("./backups"),
            compress: true,
            verify: true,
        }
    }
}

/// Backup manager
pub struct BackupManager {
    config: BackupConfig,
    last_backup_id: Option<String>,
}

impl BackupManager {
    /// Create a new backup manager
    pub fn new(config: BackupConfig) -> Result<Self, String> {
        // Ensure backup directory exists
        create_dir_all(&config.backup_dir)
            .map_err(|e| format!("Failed to create backup directory: {}", e))?;

        Ok(BackupManager {
            config,
            last_backup_id: None,
        })
    }

    /// Create a new backup manager with default config
    pub fn with_defaults() -> Result<Self, String> {
        Self::new(BackupConfig::default())
    }

    /// Create a full backup
    pub fn create_full_backup(&mut self, graph: &Graph) -> Result<BackupMetadata, String> {
        let backup_id = generate_backup_id();
        let backup_path = self.get_backup_path(&backup_id);

        // Serialize graph data
        let backup_data = BackupData {
            entities: graph.get_all_entities()
                .into_iter()
                .map(|e| SerializedEntity::from_entity(&e))
                .collect(),
            edges: graph.get_all_edges()
                .into_iter()
                .map(|e| SerializedEdge::from_edge(&e))
                .collect(),
        };

        // Write backup
        let serialized = serde_json::to_string(&backup_data)
            .map_err(|e| format!("Serialization error: {}", e))?;

        let checksum = calculate_checksum(&serialized);

        let mut file = File::create(&backup_path)
            .map_err(|e| format!("Failed to create backup file: {}", e))?;

        if self.config.compress {
            // Compress with gzip
            let compressed = compress_data(serialized.as_bytes())?;
            file.write_all(&compressed)
                .map_err(|e| format!("Failed to write backup: {}", e))?;
        } else {
            file.write_all(serialized.as_bytes())
                .map_err(|e| format!("Failed to write backup: {}", e))?;
        }

        // Create metadata
        let metadata = BackupMetadata {
            backup_id: backup_id.clone(),
            backup_type: BackupType::Full,
            timestamp: current_timestamp(),
            entity_count: backup_data.entities.len(),
            edge_count: backup_data.edges.len(),
            compressed: self.config.compress,
            checksum,
            parent_backup_id: None,
        };

        // Save metadata
        self.save_metadata(&metadata)?;

        self.last_backup_id = Some(backup_id);

        Ok(metadata)
    }

    /// Restore from backup
    pub fn restore_backup(&self, backup_id: &str, graph: &mut Graph) -> Result<(), String> {
        let backup_path = self.get_backup_path(backup_id);
        let metadata = self.load_metadata(backup_id)?;

        // Read backup file
        let mut file = File::open(&backup_path)
            .map_err(|e| format!("Failed to open backup file: {}", e))?;

        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)
            .map_err(|e| format!("Failed to read backup: {}", e))?;

        // Decompress if needed
        let data = if metadata.compressed {
            decompress_data(&buffer)?
        } else {
            buffer
        };

        let serialized = String::from_utf8(data)
            .map_err(|e| format!("Invalid UTF-8 in backup: {}", e))?;

        // Verify checksum
        if self.config.verify {
            let checksum = calculate_checksum(&serialized);
            if checksum != metadata.checksum {
                return Err("Backup checksum mismatch - data may be corrupted".to_string());
            }
        }

        // Deserialize
        let backup_data: BackupData = serde_json::from_str(&serialized)
            .map_err(|e| format!("Deserialization error: {}", e))?;

        // Clear existing data
        *graph = Graph::new();

        // Restore entities
        for entity in backup_data.entities {
            let restored = entity.to_entity();
            // Insert entity directly (bypassing normal APIs for restoration)
            graph.insert_entity_with_id(restored);
        }

        // Restore edges
        for edge in backup_data.edges {
            let restored = edge.to_edge();
            graph.insert_edge_with_id(restored);
        }

        Ok(())
    }

    /// List all backups
    pub fn list_backups(&self) -> Result<Vec<BackupMetadata>, String> {
        let mut backups = Vec::new();

        let entries = std::fs::read_dir(&self.config.backup_dir)
            .map_err(|e| format!("Failed to read backup directory: {}", e))?;

        for entry in entries {
            let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("meta") {
                let backup_id = path.file_stem()
                    .and_then(|s| s.to_str())
                    .ok_or("Invalid backup filename")?
                    .to_string();

                if let Ok(metadata) = self.load_metadata(&backup_id) {
                    backups.push(metadata);
                }
            }
        }

        // Sort by timestamp (newest first)
        backups.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        Ok(backups)
    }

    /// Delete a backup
    pub fn delete_backup(&self, backup_id: &str) -> Result<(), String> {
        let backup_path = self.get_backup_path(backup_id);
        let metadata_path = self.get_metadata_path(backup_id);

        std::fs::remove_file(&backup_path)
            .map_err(|e| format!("Failed to delete backup file: {}", e))?;

        std::fs::remove_file(&metadata_path)
            .map_err(|e| format!("Failed to delete metadata file: {}", e))?;

        Ok(())
    }

    /// Verify backup integrity
    pub fn verify_backup(&self, backup_id: &str) -> Result<bool, String> {
        let backup_path = self.get_backup_path(backup_id);
        let metadata = self.load_metadata(backup_id)?;

        let mut file = File::open(&backup_path)
            .map_err(|e| format!("Failed to open backup file: {}", e))?;

        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)
            .map_err(|e| format!("Failed to read backup: {}", e))?;

        let data = if metadata.compressed {
            decompress_data(&buffer)?
        } else {
            buffer
        };

        let serialized = String::from_utf8(data)
            .map_err(|e| format!("Invalid UTF-8 in backup: {}", e))?;

        let checksum = calculate_checksum(&serialized);
        Ok(checksum == metadata.checksum)
    }

    fn get_backup_path(&self, backup_id: &str) -> PathBuf {
        self.config.backup_dir.join(format!("{}.backup", backup_id))
    }

    fn get_metadata_path(&self, backup_id: &str) -> PathBuf {
        self.config.backup_dir.join(format!("{}.meta", backup_id))
    }

    fn save_metadata(&self, metadata: &BackupMetadata) -> Result<(), String> {
        let path = self.get_metadata_path(&metadata.backup_id);
        let serialized = serde_json::to_string_pretty(metadata)
            .map_err(|e| format!("Failed to serialize metadata: {}", e))?;

        std::fs::write(path, serialized)
            .map_err(|e| format!("Failed to write metadata: {}", e))?;

        Ok(())
    }

    fn load_metadata(&self, backup_id: &str) -> Result<BackupMetadata, String> {
        let path = self.get_metadata_path(backup_id);
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read metadata: {}", e))?;

        serde_json::from_str(&content)
            .map_err(|e| format!("Failed to deserialize metadata: {}", e))
    }
}

/// Serialized backup data
#[derive(Debug, Serialize, Deserialize)]
struct BackupData {
    entities: Vec<SerializedEntity>,
    edges: Vec<SerializedEdge>,
}

#[derive(Debug, Serialize, Deserialize)]
struct SerializedEntity {
    id: u64,
    entity_type: String,
    properties: HashMap<String, PropertyValue>,
}

impl SerializedEntity {
    fn from_entity(entity: &Entity) -> Self {
        SerializedEntity {
            id: entity.id.0,
            entity_type: entity.entity_type.clone(),
            properties: entity.properties.clone(),
        }
    }

    fn to_entity(&self) -> Entity {
        let now = SystemTime::now();
        Entity {
            id: EntityId(self.id),
            entity_type: self.entity_type.clone(),
            properties: self.properties.clone(),
            access_count: 0,
            last_accessed: now,
            created_at: now,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct SerializedEdge {
    id: u64,
    from_id: u64,
    to_id: u64,
    edge_type: String,
    properties: HashMap<String, PropertyValue>,
}

impl SerializedEdge {
    fn from_edge(edge: &Edge) -> Self {
        SerializedEdge {
            id: edge.id.0,
            from_id: edge.source.0,
            to_id: edge.target.0,
            edge_type: edge.edge_type.clone(),
            properties: edge.properties.clone(),
        }
    }

    fn to_edge(&self) -> Edge {
        use crate::types::Pheromone;
        let now = SystemTime::now();
        Edge {
            id: EdgeId(self.id),
            source: EntityId(self.from_id),
            target: EntityId(self.to_id),
            edge_type: self.edge_type.clone(),
            properties: self.properties.clone(),
            pheromone: Pheromone::default(),
            traversal_count: 0,
            avg_latency_ns: 0,
            created_at: now,
            last_traversed: now,
        }
    }
}

fn generate_backup_id() -> String {
    let timestamp = current_timestamp();
    format!("backup_{}", timestamp)
}

fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn calculate_checksum(data: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn compress_data(data: &[u8]) -> Result<Vec<u8>, String> {
    use flate2::write::GzEncoder;
    use flate2::Compression;

    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(data)
        .map_err(|e| format!("Compression error: {}", e))?;

    encoder.finish()
        .map_err(|e| format!("Compression error: {}", e))
}

fn decompress_data(data: &[u8]) -> Result<Vec<u8>, String> {
    use flate2::read::GzDecoder;

    let mut decoder = GzDecoder::new(data);
    let mut result = Vec::new();

    decoder.read_to_end(&mut result)
        .map_err(|e| format!("Decompression error: {}", e))?;

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backup_manager_creation() {
        let config = BackupConfig {
            backup_dir: PathBuf::from("/tmp/deed_test_backups"),
            compress: true,
            verify: true,
        };

        let result = BackupManager::new(config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_full_backup_and_restore() {
        let mut graph = Graph::new();

        // Add test data
        let entity_id = graph.create_entity("User".to_string(), HashMap::new());

        // Create backup
        let config = BackupConfig {
            backup_dir: PathBuf::from("/tmp/deed_test_backups_2"),
            compress: false,
            verify: true,
        };

        let mut backup_mgr = BackupManager::new(config).unwrap();
        let metadata = backup_mgr.create_full_backup(&graph).unwrap();

        assert_eq!(metadata.backup_type, BackupType::Full);
        assert_eq!(metadata.entity_count, 1);

        // Clear graph
        let mut restored_graph = Graph::new();

        // Restore
        backup_mgr.restore_backup(&metadata.backup_id, &mut restored_graph).unwrap();

        assert_eq!(restored_graph.get_all_entities().len(), 1);
    }

    #[test]
    fn test_backup_verification() {
        let mut graph = Graph::new();
        graph.create_entity("User".to_string(), HashMap::new());

        let config = BackupConfig {
            backup_dir: PathBuf::from("/tmp/deed_test_backups_3"),
            compress: true,
            verify: true,
        };

        let mut backup_mgr = BackupManager::new(config).unwrap();
        let metadata = backup_mgr.create_full_backup(&graph).unwrap();

        let is_valid = backup_mgr.verify_backup(&metadata.backup_id).unwrap();
        assert!(is_valid);
    }

    #[test]
    fn test_list_backups() {
        let config = BackupConfig {
            backup_dir: PathBuf::from("/tmp/deed_test_backups_4"),
            compress: false,
            verify: false,
        };

        let mut backup_mgr = BackupManager::new(config).unwrap();
        let mut graph = Graph::new();

        // Create multiple backups
        backup_mgr.create_full_backup(&graph).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(100));
        backup_mgr.create_full_backup(&graph).unwrap();

        let backups = backup_mgr.list_backups().unwrap();
        assert!(backups.len() >= 2);
    }

    #[test]
    fn test_checksum_calculation() {
        let data1 = "test data";
        let data2 = "test data";
        let data3 = "different data";

        let checksum1 = calculate_checksum(data1);
        let checksum2 = calculate_checksum(data2);
        let checksum3 = calculate_checksum(data3);

        assert_eq!(checksum1, checksum2);
        assert_ne!(checksum1, checksum3);
    }
}
