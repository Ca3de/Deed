//! REST API Server for Deed Database
//!
//! A production-ready HTTP server that makes Deed accessible from ANY programming language.
//!
//! Run with: cargo run --example rest_api_server
//! Then connect from Python, Java, Node.js, curl, etc.
//!
//! Endpoints:
//!   POST /api/login   - Authenticate and get session token
//!   POST /api/query   - Execute DQL queries
//!   POST /api/logout  - Invalidate session
//!
//! Example usage:
//!   curl -X POST http://localhost:8080/api/login \
//!     -H "Content-Type: application/json" \
//!     -d '{"username": "admin", "password": "admin123"}'

use deed_rust::*;
use std::sync::{Arc, RwLock};
use axum::{
    Router,
    routing::post,
    extract::State,
    Json,
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tower_http::cors::CorsLayer;

// ============================================================================
// Request/Response Types
// ============================================================================

#[derive(Debug, Deserialize)]
struct LoginRequest {
    username: String,
    password: String,
}

#[derive(Debug, Serialize)]
struct LoginResponse {
    success: bool,
    session_id: Option<String>,
    message: String,
}

#[derive(Debug, Deserialize)]
struct QueryRequest {
    session_id: String,
    query: String,
}

#[derive(Debug, Serialize)]
struct QueryResponse {
    success: bool,
    rows_affected: Option<usize>,
    message: String,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LogoutRequest {
    session_id: String,
}

#[derive(Debug, Serialize)]
struct LogoutResponse {
    success: bool,
    message: String,
}

// ============================================================================
// Application State
// ============================================================================

#[derive(Clone)]
struct AppState {
    executor: Arc<DQLExecutor>,
    auth: Arc<AuthManager>,
}

// ============================================================================
// API Handlers
// ============================================================================

async fn login_handler(
    State(state): State<AppState>,
    Json(payload): Json<LoginRequest>,
) -> (StatusCode, Json<LoginResponse>) {
    match state.auth.login(&payload.username, &payload.password) {
        Ok(session_id) => {
            (StatusCode::OK, Json(LoginResponse {
                success: true,
                session_id: Some(session_id.clone()),
                message: format!("Successfully logged in as {}", payload.username),
            }))
        }
        Err(e) => {
            (StatusCode::UNAUTHORIZED, Json(LoginResponse {
                success: false,
                session_id: None,
                message: format!("Login failed: {}", e),
            }))
        }
    }
}

async fn query_handler(
    State(state): State<AppState>,
    Json(payload): Json<QueryRequest>,
) -> (StatusCode, Json<QueryResponse>) {
    // Validate session
    if !state.auth.validate_session(&payload.session_id) {
        return (StatusCode::UNAUTHORIZED, Json(QueryResponse {
            success: false,
            rows_affected: None,
            message: "Invalid or expired session".to_string(),
            error: Some("Authentication required".to_string()),
        }));
    }

    // Execute query
    match state.executor.execute(&payload.query) {
        Ok(result) => {
            (StatusCode::OK, Json(QueryResponse {
                success: true,
                rows_affected: Some(result.rows_affected),
                message: format!("Query executed successfully. {} rows affected.", result.rows_affected),
                error: None,
            }))
        }
        Err(e) => {
            (StatusCode::BAD_REQUEST, Json(QueryResponse {
                success: false,
                rows_affected: None,
                message: "Query execution failed".to_string(),
                error: Some(e),
            }))
        }
    }
}

async fn logout_handler(
    State(state): State<AppState>,
    Json(payload): Json<LogoutRequest>,
) -> (StatusCode, Json<LogoutResponse>) {
    match state.auth.logout(&payload.session_id) {
        Ok(_) => {
            (StatusCode::OK, Json(LogoutResponse {
                success: true,
                message: "Successfully logged out".to_string(),
            }))
        }
        Err(e) => {
            (StatusCode::BAD_REQUEST, Json(LogoutResponse {
                success: false,
                message: format!("Logout failed: {}", e),
            }))
        }
    }
}

// ============================================================================
// Main Server
// ============================================================================

#[tokio::main]
async fn main() {
    println!("🚀 Deed REST API Server v0.2.0");
    println!("================================\n");

    // Initialize database
    println!("📊 Initializing database...");
    let graph = Arc::new(RwLock::new(Graph::new()));
    let executor = Arc::new(DQLExecutor::new(graph.clone()));
    let auth = Arc::new(AuthManager::new());

    // Create demo users
    println!("👥 Creating demo users...");
    auth.create_user("admin".to_string(), "admin123", Role::Admin)
        .expect("Failed to create admin user");
    auth.create_user("user".to_string(), "user123", Role::ReadWrite)
        .expect("Failed to create regular user");

    println!("   ✓ admin / admin123 (Admin)");
    println!("   ✓ user / user123 (ReadWrite)\n");

    // Insert sample data
    println!("📝 Loading sample data...");
    executor.execute(r#"INSERT INTO Users VALUES ({name: "Alice", age: 30, city: "New York"})"#).unwrap();
    executor.execute(r#"INSERT INTO Users VALUES ({name: "Bob", age: 25, city: "San Francisco"})"#).unwrap();
    executor.execute(r#"INSERT INTO Products VALUES ({name: "Laptop", price: 999, stock: 10})"#).unwrap();
    executor.execute(r#"INSERT INTO Products VALUES ({name: "Mouse", price: 29, stock: 50})"#).unwrap();
    println!("   ✓ 2 users inserted");
    println!("   ✓ 2 products inserted\n");

    // Setup application state
    let app_state = AppState {
        executor,
        auth,
    };

    // Build router
    let app = Router::new()
        .route("/api/login", post(login_handler))
        .route("/api/query", post(query_handler))
        .route("/api/logout", post(logout_handler))
        .layer(CorsLayer::permissive())
        .with_state(app_state);

    // Start server
    let addr = SocketAddr::from(([127, 0, 0, 1], 8080));

    println!("🌐 Server running at http://localhost:8080");
    println!("\n📡 API Endpoints:");
    println!("   POST http://localhost:8080/api/login");
    println!("   POST http://localhost:8080/api/query");
    println!("   POST http://localhost:8080/api/logout\n");

    print_usage_examples();

    println!("\n⏳ Server starting...\n");

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

// ============================================================================
// Usage Examples
// ============================================================================

fn print_usage_examples() {
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("📖 QUICK START EXAMPLES");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    println!("1️⃣  Login (get session token):\n");
    println!(r#"curl -X POST http://localhost:8080/api/login \
  -H "Content-Type: application/json" \
  -d '{{"username": "admin", "password": "admin123"}}'"#);
    println!("\n   Response:");
    println!(r#"   {{"success": true, "session_id": "abc123...", "message": "Successfully logged in"}}"#);

    println!("\n\n2️⃣  Execute query (use session_id from login):\n");
    println!(r#"curl -X POST http://localhost:8080/api/query \
  -H "Content-Type: application/json" \
  -d '{{
    "session_id": "YOUR_SESSION_ID_HERE",
    "query": "FROM Users WHERE age > 25 SELECT name, city"
  }}'"#);
    println!("\n   Response:");
    println!(r#"   {{"success": true, "rows_affected": 1, "message": "Query executed successfully"}}"#);

    println!("\n\n3️⃣  Insert data:\n");
    println!(r#"curl -X POST http://localhost:8080/api/query \
  -H "Content-Type: application/json" \
  -d '{{
    "session_id": "YOUR_SESSION_ID_HERE",
    "query": "INSERT INTO Users VALUES ({{name: \"Carol\", age: 28}})"
  }}'"#);

    println!("\n\n4️⃣  Aggregation query:\n");
    println!(r#"curl -X POST http://localhost:8080/api/query \
  -H "Content-Type: application/json" \
  -d '{{
    "session_id": "YOUR_SESSION_ID_HERE",
    "query": "FROM Users SELECT city, COUNT(*) GROUP BY city"
  }}'"#);

    println!("\n\n5️⃣  Logout:\n");
    println!(r#"curl -X POST http://localhost:8080/api/logout \
  -H "Content-Type: application/json" \
  -d '{{"session_id": "YOUR_SESSION_ID_HERE"}}'"#);

    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("💡 CLIENT EXAMPLES");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    println!("🐍 Python: See examples/python_client.py");
    println!("📦 Node.js: See examples/nodejs_client.js");
    println!("☕ Java: See examples/JavaClient.java");
}
