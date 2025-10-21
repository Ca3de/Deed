//! Authentication and Authorization
//!
//! Provides user authentication, password hashing, and role-based access control.

use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

/// User role for access control
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role {
    Admin,      // Full access
    ReadWrite,  // Can read and write data
    ReadOnly,   // Can only read data
}

/// User account
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub username: String,
    pub password_hash: String,
    pub role: Role,
    pub created_at: u64,
    pub last_login: Option<u64>,
}

impl User {
    /// Create a new user with hashed password
    pub fn new(username: String, password: &str, role: Role) -> Self {
        User {
            username,
            password_hash: hash_password(password),
            role,
            created_at: current_timestamp(),
            last_login: None,
        }
    }

    /// Verify password
    pub fn verify_password(&self, password: &str) -> bool {
        self.password_hash == hash_password(password)
    }

    /// Update last login timestamp
    pub fn update_last_login(&mut self) {
        self.last_login = Some(current_timestamp());
    }
}

/// Active session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub session_id: String,
    pub username: String,
    pub role: Role,
    pub created_at: u64,
    pub expires_at: u64,
}

impl Session {
    /// Create a new session
    pub fn new(username: String, role: Role, duration_secs: u64) -> Self {
        let session_id = generate_session_id(&username);
        let created_at = current_timestamp();
        let expires_at = created_at + duration_secs;

        Session {
            session_id,
            username,
            role,
            created_at,
            expires_at,
        }
    }

    /// Check if session is expired
    pub fn is_expired(&self) -> bool {
        current_timestamp() > self.expires_at
    }

    /// Check if session has permission for operation
    pub fn can_read(&self) -> bool {
        matches!(self.role, Role::Admin | Role::ReadWrite | Role::ReadOnly)
    }

    pub fn can_write(&self) -> bool {
        matches!(self.role, Role::Admin | Role::ReadWrite)
    }

    pub fn is_admin(&self) -> bool {
        matches!(self.role, Role::Admin)
    }
}

/// Authentication manager
pub struct AuthManager {
    users: Arc<RwLock<HashMap<String, User>>>,
    sessions: Arc<RwLock<HashMap<String, Session>>>,
    session_duration: u64, // seconds
}

impl AuthManager {
    /// Create a new auth manager
    pub fn new() -> Self {
        let mut manager = AuthManager {
            users: Arc::new(RwLock::new(HashMap::new())),
            sessions: Arc::new(RwLock::new(HashMap::new())),
            session_duration: 3600, // 1 hour default
        };

        // Create default admin user
        manager.create_user("admin".to_string(), "admin", Role::Admin).ok();

        manager
    }

    /// Create a new user
    pub fn create_user(&self, username: String, password: &str, role: Role) -> Result<(), String> {
        let mut users = self.users.write().unwrap();

        if users.contains_key(&username) {
            return Err(format!("User {} already exists", username));
        }

        let user = User::new(username.clone(), password, role);
        users.insert(username, user);

        Ok(())
    }

    /// Delete a user
    pub fn delete_user(&self, username: &str) -> Result<(), String> {
        let mut users = self.users.write().unwrap();

        if users.remove(username).is_none() {
            Err(format!("User {} not found", username))
        } else {
            Ok(())
        }
    }

    /// Change user password
    pub fn change_password(&self, username: &str, new_password: &str) -> Result<(), String> {
        let mut users = self.users.write().unwrap();

        if let Some(user) = users.get_mut(username) {
            user.password_hash = hash_password(new_password);
            Ok(())
        } else {
            Err(format!("User {} not found", username))
        }
    }

    /// Authenticate user and create session
    pub fn login(&self, username: &str, password: &str) -> Result<String, String> {
        let mut users = self.users.write().unwrap();

        if let Some(user) = users.get_mut(username) {
            if user.verify_password(password) {
                // Update last login
                user.update_last_login();

                // Create session
                let session = Session::new(
                    username.to_string(),
                    user.role.clone(),
                    self.session_duration,
                );
                let session_id = session.session_id.clone();

                let mut sessions = self.sessions.write().unwrap();
                sessions.insert(session_id.clone(), session);

                Ok(session_id)
            } else {
                Err("Invalid password".to_string())
            }
        } else {
            Err(format!("User {} not found", username))
        }
    }

    /// Logout (destroy session)
    pub fn logout(&self, session_id: &str) -> Result<(), String> {
        let mut sessions = self.sessions.write().unwrap();

        if sessions.remove(session_id).is_some() {
            Ok(())
        } else {
            Err("Invalid session".to_string())
        }
    }

    /// Validate session
    pub fn validate_session(&self, session_id: &str) -> Result<Session, String> {
        let mut sessions = self.sessions.write().unwrap();

        if let Some(session) = sessions.get(session_id) {
            if session.is_expired() {
                sessions.remove(session_id);
                Err("Session expired".to_string())
            } else {
                Ok(session.clone())
            }
        } else {
            Err("Invalid session".to_string())
        }
    }

    /// Check if session has read permission
    pub fn check_read_permission(&self, session_id: &str) -> Result<(), String> {
        let session = self.validate_session(session_id)?;
        if session.can_read() {
            Ok(())
        } else {
            Err("Permission denied: read access required".to_string())
        }
    }

    /// Check if session has write permission
    pub fn check_write_permission(&self, session_id: &str) -> Result<(), String> {
        let session = self.validate_session(session_id)?;
        if session.can_write() {
            Ok(())
        } else {
            Err("Permission denied: write access required".to_string())
        }
    }

    /// Check if session is admin
    pub fn check_admin_permission(&self, session_id: &str) -> Result<(), String> {
        let session = self.validate_session(session_id)?;
        if session.is_admin() {
            Ok(())
        } else {
            Err("Permission denied: admin access required".to_string())
        }
    }

    /// List all users (admin only)
    pub fn list_users(&self) -> Vec<User> {
        let users = self.users.read().unwrap();
        users.values().cloned().collect()
    }

    /// List active sessions
    pub fn list_active_sessions(&self) -> Vec<Session> {
        let sessions = self.sessions.read().unwrap();
        let now = current_timestamp();

        sessions.values()
            .filter(|s| s.expires_at > now)
            .cloned()
            .collect()
    }

    /// Cleanup expired sessions
    pub fn cleanup_expired_sessions(&self) {
        let mut sessions = self.sessions.write().unwrap();
        let current_time = current_timestamp();

        sessions.retain(|_, session| session.expires_at > current_time);
    }
}

impl Default for AuthManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Hash password using SHA-256
fn hash_password(password: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(password.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Generate a session ID
fn generate_session_id(username: &str) -> String {
    let timestamp = current_timestamp();
    let mut hasher = Sha256::new();
    hasher.update(username.as_bytes());
    hasher.update(timestamp.to_string().as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Get current timestamp in seconds
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
    fn test_password_hashing() {
        let password = "secret123";
        let hash1 = hash_password(password);
        let hash2 = hash_password(password);

        assert_eq!(hash1, hash2); // Same password = same hash
        assert_ne!(hash1, hash_password("different"));
    }

    #[test]
    fn test_user_creation() {
        let user = User::new("alice".to_string(), "password123", Role::ReadWrite);

        assert_eq!(user.username, "alice");
        assert!(user.verify_password("password123"));
        assert!(!user.verify_password("wrong"));
        assert_eq!(user.role, Role::ReadWrite);
    }

    #[test]
    fn test_auth_manager_login() {
        let manager = AuthManager::new();

        manager.create_user("alice".to_string(), "secret", Role::ReadWrite).unwrap();

        // Successful login
        let session_id = manager.login("alice", "secret").unwrap();
        assert!(!session_id.is_empty());

        // Failed login - wrong password
        assert!(manager.login("alice", "wrong").is_err());

        // Failed login - user doesn't exist
        assert!(manager.login("bob", "secret").is_err());
    }

    #[test]
    fn test_session_validation() {
        let manager = AuthManager::new();

        manager.create_user("alice".to_string(), "secret", Role::Admin).unwrap();
        let session_id = manager.login("alice", "secret").unwrap();

        // Valid session
        let session = manager.validate_session(&session_id).unwrap();
        assert_eq!(session.username, "alice");
        assert_eq!(session.role, Role::Admin);

        // Invalid session
        assert!(manager.validate_session("invalid_id").is_err());
    }

    #[test]
    fn test_permissions() {
        let manager = AuthManager::new();

        manager.create_user("admin_user".to_string(), "pass", Role::Admin).unwrap();
        manager.create_user("writer".to_string(), "pass", Role::ReadWrite).unwrap();
        manager.create_user("reader".to_string(), "pass", Role::ReadOnly).unwrap();

        let admin_session = manager.login("admin_user", "pass").unwrap();
        let writer_session = manager.login("writer", "pass").unwrap();
        let reader_session = manager.login("reader", "pass").unwrap();

        // Admin can do everything
        assert!(manager.check_read_permission(&admin_session).is_ok());
        assert!(manager.check_write_permission(&admin_session).is_ok());
        assert!(manager.check_admin_permission(&admin_session).is_ok());

        // Writer can read and write
        assert!(manager.check_read_permission(&writer_session).is_ok());
        assert!(manager.check_write_permission(&writer_session).is_ok());
        assert!(manager.check_admin_permission(&writer_session).is_err());

        // Reader can only read
        assert!(manager.check_read_permission(&reader_session).is_ok());
        assert!(manager.check_write_permission(&reader_session).is_err());
        assert!(manager.check_admin_permission(&reader_session).is_err());
    }

    #[test]
    fn test_logout() {
        let manager = AuthManager::new();

        manager.create_user("alice".to_string(), "secret", Role::Admin).unwrap();
        let session_id = manager.login("alice", "secret").unwrap();

        // Session is valid
        assert!(manager.validate_session(&session_id).is_ok());

        // Logout
        manager.logout(&session_id).unwrap();

        // Session is now invalid
        assert!(manager.validate_session(&session_id).is_err());
    }

    #[test]
    fn test_change_password() {
        let manager = AuthManager::new();

        manager.create_user("alice".to_string(), "old_pass", Role::Admin).unwrap();

        // Old password works
        assert!(manager.login("alice", "old_pass").is_ok());

        // Change password
        manager.change_password("alice", "new_pass").unwrap();

        // Old password doesn't work
        assert!(manager.login("alice", "old_pass").is_err());

        // New password works
        assert!(manager.login("alice", "new_pass").is_ok());
    }
}
