use axum::{routing::{get, post}, Router, Json, extract::State};
use sqlx::SqlitePool;
use tower_http::services::ServeDir;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tracing_subscriber;

mod sync;

#[derive(Deserialize)]
struct AddNpubRequest {
    npub: String,
    label: Option<String>,
}

#[derive(Deserialize)]
struct AddRelayRequest {
    url: String,
    name: Option<String>,
}

#[derive(Serialize)]
struct ApiResponse {
    success: bool,
    message: String,
}

async fn get_relays(State(pool): State<SqlitePool>) -> Json<Vec<serde_json::Value>> {
    let relays: Vec<serde_json::Value> = sqlx::query_as(
        "SELECT id, url, name, enabled, preloaded, created_at FROM upstream_relays"
    )
    .fetch_all(&pool)
    .await
    .unwrap_or_default();
    Json(relays)
}

async fn add_relay(State(pool): State<SqlitePool>, Json(req): Json<AddRelayRequest>) -> Json<ApiResponse> {
    let result = sqlx::query(
        "INSERT INTO upstream_relays (url, name) VALUES (?, ?)"
    )
    .bind(&req.url)
    .bind(&req.name)
    .execute(&pool)
    .await;

    match result {
        Ok(_) => Json(ApiResponse { success: true, message: "Relay added successfully".to_string() }),
        Err(e) => Json(ApiResponse { success: false, message: format!("Failed to add relay: {}", e) }),
    }
}

async fn get_npubs(State(pool): State<SqlitePool>) -> Json<Vec<serde_json::Value>> {
    let npubs: Vec<serde_json::Value> = sqlx::query_as(
        "SELECT id, npub, label, last_synced, created_at FROM monitored_npubs"
    )
    .fetch_all(&pool)
    .await
    .unwrap_or_default();
    Json(npubs)
}

async fn add_npub(State(pool): State<SqlitePool>, Json(req): Json<AddNpubRequest>) -> Json<ApiResponse> {
    let result = sqlx::query(
        "INSERT INTO monitored_npubs (npub, label) VALUES (?, ?)"
    )
    .bind(&req.npub)
    .bind(&req.label)
    .execute(&pool)
    .await;

    match result {
        Ok(_) => Json(ApiResponse { success: true, message: "Npub added successfully".to_string() }),
        Err(e) => Json(ApiResponse { success: false, message: format!("Failed to add npub: {}", e) }),
    }
}

async fn trigger_sync(State(pool): State<SqlitePool>) -> Json<ApiResponse> {
    match sync::sync_npubs(pool.clone()).await {
        Ok(msg) => Json(ApiResponse { success: true, message: msg }),
        Err(e) => Json(ApiResponse { success: false, message: e }),
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    // Connect to SQLite (creates db if it doesn't exist)
    let pool = SqlitePool::connect("sqlite:nostr_relay.db?mode=rwc")
        .await
        .expect("Failed to connect to SQLite");

    // Run migrations (assumes you have the migration files in ./migrations/)
    sqlx::migrate!()
        .run(&pool)
        .await
        .expect("Failed to run database migrations");

    println!("Database connected and migrations applied.");

    let app = Router::new()
        // API routes
        .route("/api/relays", get(get_relays).post(add_relay))
        .route("/api/npubs", get(get_npubs).post(add_npub))
        .route("/api/sync", post(trigger_sync))
        
        // Serve the frontend dashboard
        .nest_service("/", ServeDir::new("public"))
        
        .with_state(pool);

    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    println!("🚀 Nostr Relay Dashboard running on http://0.0.0.0:8080");
    println!("Open it in your browser and try adding an npub + clicking Sync Now");

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}