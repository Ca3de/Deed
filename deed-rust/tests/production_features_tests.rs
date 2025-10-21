//! Integration tests for production features
//!
//! Tests B-tree indexes, authentication, and connection pooling

use deed_rust::*;
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::Duration;

// ============================================================================
// B-TREE INDEX TESTS
// ============================================================================

#[test]
fn test_create_index() {
    let graph = Arc::new(RwLock::new(Graph::new()));
    let executor = DQLExecutor::new(graph);

    // Create index
    let result = executor.execute("CREATE INDEX idx_user_age ON Users(age)");
    assert!(result.is_ok());
}

#[test]
fn test_create_unique_index() {
    let graph = Arc::new(RwLock::new(Graph::new()));
    let executor = DQLExecutor::new(graph);

    // Create unique index
    let result = executor.execute("CREATE UNIQUE INDEX idx_user_email ON Users(email)");
    assert!(result.is_ok());
}

#[test]
fn test_drop_index() {
    let graph = Arc::new(RwLock::new(Graph::new()));
    let executor = DQLExecutor::new(graph);

    // Create then drop index
    executor.execute("CREATE INDEX idx_user_age ON Users(age)").unwrap();
    let result = executor.execute("DROP INDEX idx_user_age");
    assert!(result.is_ok());
}

#[test]
fn test_unique_index_constraint() {
    let index_manager = IndexManager::new();

    // Create unique index
    index_manager.create_index(
        "idx_email".to_string(),
        "Users".to_string(),
        "email".to_string(),
        true,
    ).unwrap();

    // Insert first value
    let result1 = index_manager.insert_into_index(
        "idx_email",
        &PropertyValue::String("alice@example.com".to_string()),
        EntityId::new(1),
    );
    assert!(result1.is_ok());

    // Insert duplicate value - should fail
    let result2 = index_manager.insert_into_index(
        "idx_email",
        &PropertyValue::String("alice@example.com".to_string()),
        EntityId::new(2),
    );
    assert!(result2.is_err());
    assert!(result2.unwrap_err().contains("already exists"));
}

#[test]
fn test_index_lookup() {
    let index_manager = IndexManager::new();

    // Create index and insert values
    index_manager.create_index(
        "idx_age".to_string(),
        "Users".to_string(),
        "age".to_string(),
        false,
    ).unwrap();

    index_manager.insert_into_index(
        "idx_age",
        &PropertyValue::Integer(25),
        EntityId::new(1),
    ).unwrap();

    index_manager.insert_into_index(
        "idx_age",
        &PropertyValue::Integer(25),
        EntityId::new(2),
    ).unwrap();

    // Lookup
    let results = index_manager.lookup_in_index(
        "idx_age",
        &PropertyValue::Integer(25),
    ).unwrap();

    assert_eq!(results.len(), 2);
}

#[test]
fn test_index_range_scan() {
    let index_manager = IndexManager::new();

    // Create index
    index_manager.create_index(
        "idx_price".to_string(),
        "Products".to_string(),
        "price".to_string(),
        false,
    ).unwrap();

    // Insert values
    for (id, price) in [(1, 50), (2, 100), (3, 150), (4, 200)] {
        index_manager.insert_into_index(
            "idx_price",
            &PropertyValue::Integer(price),
            EntityId::new(id),
        ).unwrap();
    }

    // Range scan: price >= 100 and price <= 150
    let results = index_manager.range_scan_in_index(
        "idx_price",
        &PropertyValue::Integer(100),
        &PropertyValue::Integer(150),
    ).unwrap();

    assert_eq!(results.len(), 2); // Should find IDs 2 and 3
}

// ============================================================================
// AUTHENTICATION TESTS
// ============================================================================

#[test]
fn test_auth_create_user() {
    let auth = AuthManager::new();

    let result = auth.create_user("alice".to_string(), "password123", Role::ReadWrite);
    assert!(result.is_ok());

    // Try to create duplicate user
    let result2 = auth.create_user("alice".to_string(), "password456", Role::Admin);
    assert!(result2.is_err());
    assert!(result2.unwrap_err().contains("already exists"));
}

#[test]
fn test_auth_login_success() {
    let auth = AuthManager::new();

    auth.create_user("bob".to_string(), "secret", Role::Admin).unwrap();

    let result = auth.login("bob", "secret");
    assert!(result.is_ok());

    let session_id = result.unwrap();
    assert!(!session_id.is_empty());
}

#[test]
fn test_auth_login_failure() {
    let auth = AuthManager::new();

    auth.create_user("charlie".to_string(), "correct_password", Role::Admin).unwrap();

    // Wrong password
    let result = auth.login("charlie", "wrong_password");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Invalid password"));

    // Nonexistent user
    let result2 = auth.login("nonexistent", "password");
    assert!(result2.is_err());
    assert!(result2.unwrap_err().contains("not found"));
}

#[test]
fn test_auth_session_validation() {
    let auth = AuthManager::new();

    auth.create_user("dave".to_string(), "pass", Role::ReadWrite).unwrap();
    let session_id = auth.login("dave", "pass").unwrap();

    // Validate valid session
    let result = auth.validate_session(&session_id);
    assert!(result.is_ok());

    let session = result.unwrap();
    assert_eq!(session.username, "dave");
    assert_eq!(session.role, Role::ReadWrite);

    // Validate invalid session
    let result2 = auth.validate_session("invalid_session_id");
    assert!(result2.is_err());
}

#[test]
fn test_auth_permissions() {
    let auth = AuthManager::new();

    // Create users with different roles
    auth.create_user("admin_user".to_string(), "pass", Role::Admin).unwrap();
    auth.create_user("writer".to_string(), "pass", Role::ReadWrite).unwrap();
    auth.create_user("reader".to_string(), "pass", Role::ReadOnly).unwrap();

    let admin_session = auth.login("admin_user", "pass").unwrap();
    let writer_session = auth.login("writer", "pass").unwrap();
    let reader_session = auth.login("reader", "pass").unwrap();

    // Admin permissions
    assert!(auth.check_read_permission(&admin_session).is_ok());
    assert!(auth.check_write_permission(&admin_session).is_ok());
    assert!(auth.check_admin_permission(&admin_session).is_ok());

    // ReadWrite permissions
    assert!(auth.check_read_permission(&writer_session).is_ok());
    assert!(auth.check_write_permission(&writer_session).is_ok());
    assert!(auth.check_admin_permission(&writer_session).is_err());

    // ReadOnly permissions
    assert!(auth.check_read_permission(&reader_session).is_ok());
    assert!(auth.check_write_permission(&reader_session).is_err());
    assert!(auth.check_admin_permission(&reader_session).is_err());
}

#[test]
fn test_auth_logout() {
    let auth = AuthManager::new();

    auth.create_user("eve".to_string(), "pass", Role::Admin).unwrap();
    let session_id = auth.login("eve", "pass").unwrap();

    // Session is valid
    assert!(auth.validate_session(&session_id).is_ok());

    // Logout
    auth.logout(&session_id).unwrap();

    // Session is now invalid
    assert!(auth.validate_session(&session_id).is_err());
}

#[test]
fn test_auth_change_password() {
    let auth = AuthManager::new();

    auth.create_user("frank".to_string(), "old_password", Role::Admin).unwrap();

    // Old password works
    assert!(auth.login("frank", "old_password").is_ok());

    // Change password
    auth.change_password("frank", "new_password").unwrap();

    // Old password doesn't work
    assert!(auth.login("frank", "old_password").is_err());

    // New password works
    assert!(auth.login("frank", "new_password").is_ok());
}

#[test]
fn test_auth_delete_user() {
    let auth = AuthManager::new();

    auth.create_user("grace".to_string(), "pass", Role::Admin).unwrap();

    // Can login
    assert!(auth.login("grace", "pass").is_ok());

    // Delete user
    auth.delete_user("grace").unwrap();

    // Can't login anymore
    assert!(auth.login("grace", "pass").is_err());
}

// ============================================================================
// CONNECTION POOL TESTS
// ============================================================================

fn create_test_pool() -> ConnectionPool {
    let graph = Arc::new(RwLock::new(Graph::new()));
    let optimizer = Arc::new(RwLock::new(AntColonyOptimizer::new()));
    let cache = Arc::new(RwLock::new(StigmergyCache::new()));
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

    // Pool should have min_size connections
    assert!(pool.size() >= 2);
    assert_eq!(pool.active_connections(), 0);
}

#[test]
fn test_pool_get_connection() {
    let pool = create_test_pool();

    let result = pool.get_connection();
    assert!(result.is_ok());

    assert_eq!(pool.active_connections(), 1);
}

#[test]
fn test_pool_connection_return() {
    let pool = create_test_pool();

    {
        let _conn = pool.get_connection().unwrap();
        assert_eq!(pool.active_connections(), 1);
    }

    // Connection should be returned
    assert_eq!(pool.active_connections(), 0);
}

#[test]
fn test_pool_multiple_connections() {
    let pool = create_test_pool();

    let _conn1 = pool.get_connection().unwrap();
    let _conn2 = pool.get_connection().unwrap();
    let _conn3 = pool.get_connection().unwrap();

    assert_eq!(pool.active_connections(), 3);
}

#[test]
fn test_pool_concurrent_access() {
    let pool = Arc::new(create_test_pool());
    let mut handles = vec![];

    // Spawn 5 threads
    for _ in 0..5 {
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
fn test_pool_max_size_enforcement() {
    let graph = Arc::new(RwLock::new(Graph::new()));
    let optimizer = Arc::new(RwLock::new(AntColonyOptimizer::new()));
    let cache = Arc::new(RwLock::new(StigmergyCache::new()));
    let transaction_manager = Arc::new(TransactionManager::new());

    let config = PoolConfig {
        min_size: 1,
        max_size: 2,
        connection_timeout: 1, // 1 second timeout
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

    // Should timeout trying to get third connection
    let result = pool.get_connection();
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("timeout"));
}

#[test]
fn test_pool_statistics() {
    let pool = create_test_pool();

    let stats = pool.stats();
    assert!(stats.total_connections >= 2);
    assert_eq!(stats.min_size, 2);
    assert_eq!(stats.max_size, 10);

    let _conn = pool.get_connection().unwrap();

    let stats = pool.stats();
    assert_eq!(stats.active_connections, 1);
    assert!(stats.utilization() > 0.0);
}

// ============================================================================
// INTEGRATION TESTS (All Features Together)
// ============================================================================

#[test]
fn test_integrated_workflow() {
    // Setup all components
    let graph = Arc::new(RwLock::new(Graph::new()));
    let optimizer = Arc::new(RwLock::new(AntColonyOptimizer::new()));
    let cache = Arc::new(RwLock::new(StigmergyCache::new()));
    let transaction_manager = Arc::new(TransactionManager::new());

    let pool = Arc::new(ConnectionPool::with_defaults(
        graph,
        optimizer,
        cache,
        transaction_manager,
        None,
    ).unwrap());

    let auth = Arc::new(AuthManager::new());

    // Create application user
    auth.create_user("app_user".to_string(), "app_pass", Role::ReadWrite).unwrap();

    // Login
    let session_id = auth.login("app_user", "app_pass").unwrap();

    // Verify permissions
    auth.check_write_permission(&session_id).unwrap();

    // Get connection from pool
    let mut conn = pool.get_connection().unwrap();
    let executor = conn.executor().unwrap();

    // Create index
    executor.execute("CREATE INDEX idx_user_age ON Users(age)").unwrap();

    // Insert data (uses auto-commit transaction)
    executor.execute("INSERT INTO Users VALUES ({name: 'Alice', age: 30})").unwrap();

    // Query data (should use index)
    let result = executor.execute("FROM Users WHERE age = 30 SELECT name");
    assert!(result.is_ok());

    // Cleanup
    auth.logout(&session_id).unwrap();
}

#[test]
fn test_concurrent_authenticated_queries() {
    // Setup
    let graph = Arc::new(RwLock::new(Graph::new()));
    let optimizer = Arc::new(RwLock::new(AntColonyOptimizer::new()));
    let cache = Arc::new(RwLock::new(StigmergyCache::new()));
    let transaction_manager = Arc::new(TransactionManager::new());

    let pool = Arc::new(ConnectionPool::with_defaults(
        graph,
        optimizer,
        cache,
        transaction_manager,
        None,
    ).unwrap());

    let auth = Arc::new(AuthManager::new());

    // Create multiple users
    for i in 0..3 {
        auth.create_user(
            format!("user{}", i),
            "password",
            Role::ReadWrite,
        ).unwrap();
    }

    let mut handles = vec![];

    // Spawn threads for concurrent queries
    for i in 0..3 {
        let pool_clone = pool.clone();
        let auth_clone = auth.clone();
        let username = format!("user{}", i);

        let handle = thread::spawn(move || {
            // Login
            let session_id = auth_clone.login(&username, "password").unwrap();

            // Check permission
            auth_clone.check_write_permission(&session_id).unwrap();

            // Get connection
            let mut conn = pool_clone.get_connection().unwrap();
            let executor = conn.executor().unwrap();

            // Execute query
            executor.execute(&format!(
                "INSERT INTO Users VALUES ({{name: '{}', age: {}}})",
                username, 20 + i
            )).unwrap();

            // Logout
            auth_clone.logout(&session_id).unwrap();
        });

        handles.push(handle);
    }

    // Wait for all threads
    for handle in handles {
        handle.join().unwrap();
    }

    // Verify pool is clean
    assert_eq!(pool.active_connections(), 0);
}

#[test]
fn test_permission_denied_scenario() {
    // Setup
    let graph = Arc::new(RwLock::new(Graph::new()));
    let executor = DQLExecutor::new(graph);
    let auth = AuthManager::new();

    // Create read-only user
    auth.create_user("readonly".to_string(), "pass", Role::ReadOnly).unwrap();
    let session_id = auth.login("readonly", "pass").unwrap();

    // Verify read permission
    assert!(auth.check_read_permission(&session_id).is_ok());

    // Verify write permission denied
    let result = auth.check_write_permission(&session_id);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("write access required"));
}

#[test]
fn test_index_with_transactions() {
    let graph = Arc::new(RwLock::new(Graph::new()));
    let executor = DQLExecutor::new(graph);

    // Create index
    executor.execute("CREATE UNIQUE INDEX idx_email ON Users(email)").unwrap();

    // Start transaction
    executor.execute("BEGIN TRANSACTION").unwrap();

    // Insert data
    executor.execute("INSERT INTO Users VALUES ({name: 'Alice', email: 'alice@example.com'})").unwrap();
    executor.execute("INSERT INTO Users VALUES ({name: 'Bob', email: 'bob@example.com'})").unwrap();

    // Commit
    executor.execute("COMMIT").unwrap();

    // Data should be persisted
    // (In a real test, we'd query to verify)
}
