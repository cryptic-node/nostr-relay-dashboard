use axum::{routing::{get, post, delete}, Router, Json, extract::{State, Query, Path}};
use sqlx::{SqlitePool, Row};
use tower_http::services::ServeDir;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tracing_subscriber;
use std::collections::HashMap;
use nostr::PublicKey;

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

#[derive(Serialize)]
struct EventPreview {
    id: String,
    kind: u16,
    kind_name: String,
    content: String,
    created_at: String,
}

async fn get_events(Query(params): Query<HashMap<String, String>>, State(pool): State<SqlitePool>) -> Json<Vec<EventPreview>> {
    let npub_str = match params.get("npub") {
        Some(n) => n,
        None => return Json(vec![]),
    };

    let pubkey = match PublicKey::parse(npub_str) {
        Ok(pk) => pk,
        Err(_) => return Json(vec![]),
    };
    let pubkey_hex = pubkey.to_hex();

    let events = sqlx::query(
        "SELECT id, kind, content, created_at FROM events 
         WHERE pubkey = ? 
         ORDER BY created_at DESC LIMIT 10"
    )
    .bind(pubkey_hex)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    let previews: Vec<EventPreview> = events.into_iter().map(|row| {
        let kind = row.get::<i64, _>("kind") as u16;
        let kind_name = match kind {
            0 => "Profile", 1 => "Text", 3 => "Contacts",
            6 => "Repost", 7 => "Reaction", 9735 => "Zap",
            _ => "Event",
        }.to_string();

        EventPreview {
            id: row.get::<String, _>("id"),
            kind,
            kind_name,
            content: {
                let c = row.get::<String, _>("content");
                if c.len() > 120 { c.chars().take(120).collect::<String>() + "…" } else { c }
            },
            created_at: row.get::<Option<String>, _>("created_at").unwrap_or_default(),
        }
    }).collect();

    Json(previews)
}

async fn get_relays(State(pool): State<SqlitePool>) -> Json<Vec<serde_json::Value>> {
    let relays = sqlx::query("SELECT id, url, name, enabled, preloaded, created_at FROM upstream_relays")
        .fetch_all(&pool)
        .await
        .unwrap_or_default();

    let json_relays: Vec<serde_json::Value> = relays.into_iter().map(|row| {
        serde_json::json!({
            "id": row.get::<i64, _>("id"),
            "url": row.get::<String, _>("url"),
            "name": row.get::<Option<String>, _>("name"),
            "enabled": row.get::<i64, _>("enabled") != 0,
            "preloaded": row.get::<i64, _>("preloaded") != 0,
            "created_at": row.get::<Option<String>, _>("created_at").unwrap_or_default(),
        })
    }).collect();

    Json(json_relays)
}

async fn add_relay(State(pool): State<SqlitePool>, Json(req): Json<AddRelayRequest>) -> Json<ApiResponse> {
    let result = sqlx::query("INSERT INTO upstream_relays (url, name) VALUES (?, ?)")
        .bind(&req.url)
        .bind(&req.name)
        .execute(&pool)
        .await;

    match result {
        Ok(_) => Json(ApiResponse { success: true, message: "Relay added successfully".to_string() }),
        Err(e) => Json(ApiResponse { success: false, message: format!("Failed: {}", e) }),
    }
}

async fn delete_relay(Path(id): Path<i64>, State(pool): State<SqlitePool>) -> Json<ApiResponse> {
    let _ = sqlx::query("DELETE FROM upstream_relays WHERE id = ?").bind(id).execute(&pool).await;
    Json(ApiResponse { success: true, message: "Relay deleted".to_string() })
}

async fn get_npubs(State(pool): State<SqlitePool>) -> Json<Vec<
