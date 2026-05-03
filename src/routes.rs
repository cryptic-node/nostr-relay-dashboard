use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

#[derive(Deserialize)]
pub struct AddNpubRequest {
    pub npub: String,
    pub label: Option<String>,
}

#[derive(Deserialize)]
pub struct AddRelayRequest {
    pub url: String,
    pub name: Option<String>,
}

#[derive(Serialize)]
pub struct ApiResponse {
    pub success: bool,
    pub message: String,
}

pub async fn get_relays(State(pool): State<SqlitePool>) -> Json<Vec<serde_json::Value>> {
    let relays: Vec<serde_json::Value> = sqlx::query_as(
        "SELECT id, url, name, enabled as 'enabled: bool', preloaded as 'preloaded: bool', created_at FROM upstream_relays ORDER BY preloaded DESC, name"
    )
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    Json(relays)
}

pub async fn add_relay(State(pool): State<SqlitePool>, Json(req): Json<AddRelayRequest>) -> Json<ApiResponse> {
    let result = sqlx::query(
        "INSERT OR IGNORE INTO upstream_relays (url, name) VALUES (?, ?)"
    )
    .bind(&req.url)
    .bind(&req.name)
    .execute(&pool)
    .await;

    match result {
        Ok(_) => Json(ApiResponse { success: true, message: "Relay added successfully!".to_string() }),
        Err(e) => Json(ApiResponse { success: false, message: format!("Failed: {}", e) }),
    }
}

pub async fn get_npubs(State(pool): State<SqlitePool>) -> Json<Vec<serde_json::Value>> {
    let npubs: Vec<serde_json::Value> = sqlx::query_as(
        "SELECT id, npub, label, last_synced, created_at FROM monitored_npubs ORDER BY created_at DESC"
    )
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    Json(npubs)
}

pub async fn add_npub(State(pool): State<SqlitePool>, Json(req): Json<AddNpubRequest>) -> Json<ApiResponse> {
    let result = sqlx::query(
        "INSERT OR IGNORE INTO monitored_npubs (npub, label) VALUES (?, ?)"
    )
    .bind(&req.npub)
    .bind(&req.label)
    .execute(&pool)
    .await;

    match result {
        Ok(_) => Json(ApiResponse { success: true, message: "Npub added successfully!".to_string() }),
        Err(e) => Json(ApiResponse { success: false, message: format!("Failed (duplicate npub?): {}", e) }),
    }
}