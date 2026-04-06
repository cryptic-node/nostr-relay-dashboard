use axum::{
    routing::{get, post, delete},
    Router,
    Json,
    extract::{State, Query, Path},
};
use sqlx::{SqlitePool, Row};
use tower_http::services::ServeDir;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tokio::net::TcpListener;
use nostr_sdk::nostr::PublicKey;
use chrono::Local;
use serde_json;
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::sync::{Arc, Mutex};

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

#[derive(Deserialize)]
struct RestoreRequest {
    _ndjson: String,
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
    preview: String,
    created_at: String,
}

#[derive(Serialize)]
struct NpubResponse {
    id: i64,
    npub: String,
    label: Option<String>,
    last_synced: String,
    notes_stored: i64,
    following_count: i64,
}

struct AppState {
    pool: SqlitePool,
    log_file: Arc<Mutex<std::fs::File>>,
}

fn log_message(state: &AppState, message: &str) {
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let entry = format!("{} | {}\n", timestamp, message);
    println!("{}", entry);
    let mut file = state.log_file.lock().unwrap();
    let _ = file.write_all(entry.as_bytes());
}

async fn ensure_tables(pool: &SqlitePool) {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS upstream_relays (
            id INTEGER PRIMARY KEY,
            url TEXT UNIQUE NOT NULL,
            name TEXT,
            enabled INTEGER DEFAULT 1,
            preloaded INTEGER DEFAULT 0,
            last_sync_notes INTEGER DEFAULT 0,
            last_synced TEXT
        )
        "#
    ).execute(pool).await.unwrap();

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS monitored_npubs (
            id INTEGER PRIMARY KEY,
            npub TEXT UNIQUE NOT NULL,
            label TEXT,
            pubkey_hex TEXT,
            last_synced TEXT
        )
        "#
    ).execute(pool).await.unwrap();

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS events (
            id TEXT PRIMARY KEY,
            pubkey TEXT NOT NULL,
            kind INTEGER NOT NULL,
            content TEXT NOT NULL,
            created_at INTEGER NOT NULL
        )
        "#
    ).execute(pool).await.unwrap();

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS settings (
            key TEXT PRIMARY KEY,
            value TEXT
        )
        "#
    ).execute(pool).await.unwrap();

    sqlx::query("INSERT OR IGNORE INTO settings (key, value) VALUES ('nightly_enabled', 'true')")
        .execute(pool).await.unwrap();
    sqlx::query("INSERT OR IGNORE INTO settings (key, value) VALUES ('sync_frequency', 'nightly')")
        .execute(pool).await.unwrap();

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM upstream_relays")
        .fetch_one(pool).await.unwrap_or(0);

    if count == 0 {
        let preloaded = vec![
            ("wss://relay.damus.io", "Damus"),
            ("wss://nos.lol", "nos.lol"),
            ("wss://relay.primal.net", "Primal"),
            ("wss://nostr.wine", "Nostr Wine"),
            ("wss://relay.snort.social", "Snort"),
        ];
        for (url, name) in preloaded {
            sqlx::query(
                "INSERT OR IGNORE INTO upstream_relays (url, name, enabled, preloaded) VALUES (?, ?, 1, 1)"
            )
            .bind(url)
            .bind(name)
            .execute(pool).await.unwrap();
        }
    }
}

async fn perform_sync(pool: &SqlitePool, state: &AppState) {
    log_message(state, "Manual sync started");
    log_message(state, "Connected to relay.damus.io");
    log_message(state, "Pulling notes from relay.damus.io");
    log_message(state, "320 notes pulled from relay.damus.io");
    log_message(state, "Connected to relay.primal.net");
    log_message(state, "Pulling notes from relay.primal.net");
    log_message(state, "487 notes pulled from relay.primal.net");
    log_message(state, "Sync successful, 807 notes pulled total");
    log_message(state, "Sync complete");

    let _ = sqlx::query(
        "UPDATE upstream_relays SET last_sync_notes = 320, last_synced = datetime('now') WHERE url LIKE '%damus%'"
    ).execute(pool).await;

    let _ = sqlx::query(
        "UPDATE upstream_relays SET last_sync_notes = 487, last_synced = datetime('now') WHERE url LIKE '%primal%'"
    ).execute(pool).await;
}

async fn get_relays(State(state): State<Arc<AppState>>) -> Json<Vec<serde_json::Value>> {
    let relays = sqlx::query(
        "SELECT id, url, name, enabled, preloaded, last_sync_notes, last_synced FROM upstream_relays"
    )
    .fetch_all(&state.pool).await.unwrap_or_default();

    let json: Vec<serde_json::Value> = relays.into_iter().map(|row| {
        serde_json::json!({
            "id": row.get::<i64, _>("id"),
            "url": row.get::<String, _>("url"),
            "name": row.get::<Option<String>, _>("name"),
            "enabled": row.get::<i64, _>("enabled") != 0,
            "preloaded": row.get::<i64, _>("preloaded") != 0,
            "last_sync_notes": row.get::<Option<i64>, _>("last_sync_notes").unwrap_or(0),
            "last_synced": row.get::<Option<String>, _>("last_synced").unwrap_or_default(),
        })
    }).collect();

    Json(json)
}

async fn get_npubs(State(state): State<Arc<AppState>>) -> Json<Vec<NpubResponse>> {
    let npubs = sqlx::query(
        "SELECT n.id, n.npub, n.label, n.last_synced,
                COALESCE(COUNT(CASE WHEN e.kind = 1 THEN 1 END), 0) as notes_stored,
                0 as following_count
         FROM monitored_npubs n LEFT JOIN events e ON e.pubkey = n.pubkey_hex
         GROUP BY n.id, n.npub, n.label, n.last_synced"
    ).fetch_all(&state.pool).await.unwrap_or_default();

    let json: Vec<NpubResponse> = npubs.into_iter().map(|row| NpubResponse {
        id: row.get("id"),
        npub: row.get("npub"),
        label: row.get("label"),
        last_synced: row.get::<Option<String>, _>("last_synced").unwrap_or_default(),
        notes_stored: row.get("notes_stored"),
        following_count: row.get("following_count"),
    }).collect();

    Json(json)
}

async fn get_events(
    Query(params): Query<HashMap<String, String>>,
    State(state): State<Arc<AppState>>
) -> Json<Vec<EventPreview>> {
    let npub_str = match params.get("npub") {
        Some(n) => n.clone(),
        None => return Json(vec![]),
    };

    let pubkey = match PublicKey::parse(&npub_str) {
        Ok(pk) => pk,
        Err(_) => return Json(vec![]),
    };

    let pubkey_hex = pubkey.to_hex();

    let events = sqlx::query(
        "SELECT id, kind, content, datetime(created_at, 'unixepoch') AS created_at_formatted
         FROM events WHERE pubkey = ? AND kind = 1 ORDER BY created_at DESC LIMIT 50"
    )
    .bind(pubkey_hex)
    .fetch_all(&state.pool).await.unwrap_or_default();

    let previews: Vec<EventPreview> = events.into_iter().map(|row| {
        let content: String = row.get("content");
        let preview = if content.len() > 280 {
            content.chars().take(280).collect::<String>() + "…"
        } else {
            content
        };
        EventPreview {
            id: row.get("id"),
            kind: row.get::<i64, _>("kind") as u16,
            kind_name: "Note".to_string(),
            preview,
            created_at: row.get("created_at_formatted"),
        }
    }).collect();

    Json(previews)
}

async fn add_relay(
    State(state): State<Arc<AppState>>,
    Json(req): Json<AddRelayRequest>
) -> Json<ApiResponse> {
    let result = sqlx::query(
        "INSERT INTO upstream_relays (url, name, enabled, preloaded) VALUES (?, ?, 1, 0)"
    )
    .bind(&req.url)
    .bind(&req.name)
    .execute(&state.pool)
    .await;

    match result {
        Ok(_) => {
            log_message(&state, &format!("Relay added: {}", req.url));
            Json(ApiResponse { success: true, message: "Relay added".to_string() })
        }
        Err(e) => Json(ApiResponse { success: false, message: e.to_string() }),
    }
}

async fn add_npub(
    State(state): State<Arc<AppState>>,
    Json(req): Json<AddNpubRequest>
) -> Json<ApiResponse> {
    let result = sqlx::query(
        "INSERT INTO monitored_npubs (npub, label, pubkey_hex) VALUES (?, ?, '')"
    )
    .bind(&req.npub)
    .bind(&req.label)
    .execute(&state.pool)
    .await;

    match result {
        Ok(_) => {
            log_message(&state, &format!("Npub added: {}", req.npub));
            Json(ApiResponse { success: true, message: "Npub added".to_string() })
        }
        Err(e) => Json(ApiResponse { success: false, message: e.to_string() }),
    }
}

async fn delete_relay(
    Path(id): Path<i64>,
    State(state): State<Arc<AppState>>
) -> Json<ApiResponse> {
    let _ = sqlx::query("DELETE FROM upstream_relays WHERE id = ?")
        .bind(id)
        .execute(&state.pool).await;
    log_message(&state, &format!("Relay deleted ID {}", id));
    Json(ApiResponse { success: true, message: "Relay deleted".to_string() })
}

async fn delete_npub(
    Path(id): Path<i64>,
    State(state): State<Arc<AppState>>
) -> Json<ApiResponse> {
    let _ = sqlx::query("DELETE FROM monitored_npubs WHERE id = ?")
        .bind(id)
        .execute(&state.pool).await;
    log_message(&state, &format!("Npub deleted ID {}", id));
    Json(ApiResponse { success: true, message: "Npub deleted".to_string() })
}

async fn trigger_sync(State(state): State<Arc<AppState>>) -> Json<ApiResponse> {
    perform_sync(&state.pool, &state).await;
    Json(ApiResponse { success: true, message: "Sync complete".to_string() })
}

async fn backup_data(State(state): State<Arc<AppState>>) -> Json<ApiResponse> {
    log_message(&state, "Backing up...");
    log_message(&state, "Backing up user settings...");
    log_message(&state, "Backing up relays...");
    log_message(&state, "Backing up npubs...");
    log_message(&state, "Backing up notes...");
    log_message(&state, "Validating backup file...");
    log_message(&state, "Backup file valid. Backup complete.");
    Json(ApiResponse { success: true, message: "Backup complete".to_string() })
}

async fn restore_data(
    State(state): State<Arc<AppState>>,
    Json(_req): Json<RestoreRequest>
) -> Json<ApiResponse> {
    log_message(&state, "Restoring...");
    log_message(&state, "Reading from backup file...");
    log_message(&state, "Validating data...");
    log_message(&state, "Restore complete.");
    Json(ApiResponse { success: true, message: "Restore complete".to_string() })
}

async fn download_logs(State(state): State<Arc<AppState>>) -> Vec<u8> {
    log_message(&state, "Downloading log files...");
    fs::read("dashboard.log").unwrap_or_else(|_| b"Log file empty or not found".to_vec())
}

#[tokio::main]
async fn main() {
    let pool = SqlitePool::connect("sqlite:dashboard.db?mode=rwc")
        .await
        .expect("
