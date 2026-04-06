use axum::{routing::{get, post, delete}, Router, Json, extract::{State, Query, Path}};
use sqlx::{SqlitePool, Row};
use tower_http::services::ServeDir;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tracing_subscriber;
use nostr_sdk::nostr::PublicKey;
use chrono::{Local, Timelike};
use serde_json;
use std::collections::HashMap;

mod sync;

#[derive(Deserialize)]
struct AddNpubRequest { npub: String, label: Option<String> }

#[derive(Deserialize)]
struct AddRelayRequest { url: String, name: Option<String> }

#[derive(Deserialize)]
struct RestoreRequest { ndjson: String }

#[derive(Deserialize)]
struct SetSettingRequest { nightly_enabled: bool, sync_frequency: Option<String> }

#[derive(Serialize)]
struct ApiResponse { success: bool, message: String }

#[derive(Serialize)]
struct SettingsResponse { nightly_enabled: bool, sync_frequency: String }

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

// ==================== DATABASE SETUP + PRELOADED RELAYS ====================
async fn ensure_tables(pool: &SqlitePool) {
    // Add missing columns if needed
    let has_notes: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM pragma_table_info('upstream_relays') WHERE name = 'last_sync_notes'")
        .fetch_one(pool).await.unwrap_or(0);
    if has_notes == 0 {
        let _ = sqlx::query("ALTER TABLE upstream_relays ADD COLUMN last_sync_notes INTEGER DEFAULT 0").execute(pool).await;
    }
    let has_synced: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM pragma_table_info('upstream_relays') WHERE name = 'last_synced'")
        .fetch_one(pool).await.unwrap_or(0);
    if has_synced == 0 {
        let _ = sqlx::query("ALTER TABLE upstream_relays ADD COLUMN last_synced TEXT").execute(pool).await;
    }

    // Create settings table
    let _ = sqlx::query("CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY, value TEXT)").execute(pool).await;
    let _ = sqlx::query("INSERT OR IGNORE INTO settings (key, value) VALUES ('nightly_enabled', 'true')").execute(pool).await;
    let _ = sqlx::query("INSERT OR IGNORE INTO settings (key, value) VALUES ('sync_frequency', 'nightly')").execute(pool).await;

    // === SEED PRELOADED RELAYS (this was missing) ===
    let relay_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM upstream_relays").fetch_one(pool).await.unwrap_or(0);
    if relay_count == 0 {
        let preloaded = vec![
            ("wss://relay.damus.io", "Damus"),
            ("wss://nos.lol", "nos.lol"),
            ("wss://relay.primal.net", "Primal"),
            ("wss://nostr.wine", "Nostr Wine"),
            ("wss://relay.snort.social", "Snort"),
        ];
        for (url, name) in preloaded {
            let _ = sqlx::query("INSERT OR IGNORE INTO upstream_relays (url, name, enabled, preloaded) VALUES (?, ?, 1, 1)")
                .bind(url).bind(name).execute(pool).await;
        }
        println!("✅ Preloaded 5 default relays");
    }
}

// ==================== EVENTS ====================
async fn get_events(Query(params): Query<HashMap<String, String>>, State(pool): State<SqlitePool>) -> Json<Vec<EventPreview>> {
    let npub_str = match params.get("npub") { Some(n) => n, None => return Json(vec![]), };
    let pubkey = match PublicKey::parse(npub_str) { Ok(pk) => pk, Err(_) => return Json(vec![]), };
    let pubkey_hex = pubkey.to_hex();

    let events = sqlx::query(
        "SELECT id, kind, content, strftime('%Y-%m-%d %H:%M:%S', created_at, 'unixepoch') AS created_at_formatted
         FROM events WHERE pubkey = ? AND kind = 1 ORDER BY created_at DESC LIMIT 800"
    )
    .bind(pubkey_hex)
    .fetch_all(&pool).await.unwrap_or_default();

    let previews: Vec<EventPreview> = events.into_iter().map(|row| {
        let kind = row.get::<i64, _>("kind") as u16;
        let content: String = row.get("content");
        let preview = if content.len() > 280 { content.chars().take(280).collect::<String>() + "…" } else { content };
        EventPreview { id: row.get("id"), kind, kind_name: "Note".to_string(), preview, created_at: row.get("created_at_formatted") }
    }).collect();
    Json(previews)
}

// ==================== RELAYS & NPUBS ====================
async fn get_relays(State(pool): State<SqlitePool>) -> Json<Vec<serde_json::Value>> {
    let relays = sqlx::query("SELECT id, url, name, enabled, preloaded, created_at, last_sync_notes, last_synced FROM upstream_relays")
        .fetch_all(&pool).await.unwrap_or_default();
    let json_relays: Vec<serde_json::Value> = relays.into_iter().map(|row| {
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
    Json(json_relays)
}

async fn get_npubs(State(pool): State<SqlitePool>) -> Json<Vec<NpubResponse>> {
    let npubs = sqlx::query(
        "SELECT n.id, n.npub, n.label, n.last_synced,
                COALESCE(COUNT(CASE WHEN e.kind = 1 THEN 1 END), 0) as notes_stored,
                COALESCE((SELECT COUNT(DISTINCT p_tag) FROM events WHERE pubkey = n.pubkey_hex AND kind = 3 ORDER BY created_at DESC LIMIT 1), 0) as following_count
         FROM monitored_npubs n LEFT JOIN events e ON e.pubkey = n.pubkey_hex
         GROUP BY n.id, n.npub, n.label, n.last_synced, n.pubkey_hex"
    ).fetch_all(&pool).await.unwrap_or_default();

    let json_npubs: Vec<NpubResponse> = npubs.into_iter().map(|row| NpubResponse {
        id: row.get("id"),
        npub: row.get("npub"),
        label: row.get("label"),
        last_synced: row.get::<Option<String>, _>("last_synced").unwrap_or_default(),
        notes_stored: row.get("notes_stored"),
        following_count: row.get("following_count"),
    }).collect();
    Json(json_npubs)
}

// ==================== ADD / DELETE ====================
async fn add_relay(State(pool): State<SqlitePool>, Json(req): Json<AddRelayRequest>) -> Json<ApiResponse> {
    let result = sqlx::query("INSERT INTO upstream_relays (url, name, enabled, preloaded) VALUES (?, ?, 1, 0)")
        .bind(&req.url).bind(&req.name).execute(&pool).await;
    match result {
        Ok(_) => Json(ApiResponse { success: true, message: "Relay added".to_string() }),
        Err(e) => Json(ApiResponse { success: false, message: format!("Failed: {}", e) }),
    }
}

async fn delete_relay(Path(id): Path<i64>, State(pool): State<SqlitePool>) -> Json<ApiResponse> {
    let _ = sqlx::query("DELETE FROM upstream_relays WHERE id = ?").bind(id).execute(&pool).await;
    Json(ApiResponse { success: true, message: "Relay deleted".to_string() })
}

async fn add_npub(State(pool): State<SqlitePool>, Json(req): Json<AddNpubRequest>) -> Json<ApiResponse> {
    let pubkey_hex = match PublicKey::parse(&req.npub) {
        Ok(pk) => pk.to_hex(),
        Err(_) => return Json(ApiResponse { success: false, message: "Invalid npub".to_string() }),
    };
    let result = sqlx::query("INSERT OR IGNORE INTO monitored_npubs (npub, label, pubkey_hex) VALUES (?, ?, ?)")
        .bind(&req.npub).bind(&req.label).bind(pubkey_hex).execute(&pool).await;
    match result {
        Ok(_) => Json(ApiResponse { success: true, message: "Npub added".to_string() }),
        Err(e) => Json(ApiResponse { success: false, message: format!("Failed: {}", e) }),
    }
}

// ==================== RESTORE (NDJSON) ====================
async fn restore_data(State(pool): State<SqlitePool>, Json(req): Json<RestoreRequest>) -> Json<ApiResponse> {
    let lines: Vec<&str> = req.ndjson.lines().collect();
    let mut imported = 0;
    for line in lines {
        if line.trim().is_empty() { continue; }
        if let Ok(event) = serde_json::from_str::<serde_json::Value>(line) {
            let _ = sqlx::query(
                "INSERT OR IGNORE INTO events (id, pubkey, kind, content, tags, created_at, sig) 
                 VALUES (?, ?, ?, ?, ?, ?, ?)"
            )
            .bind(event["id"].as_str().unwrap_or(""))
            .bind(event["pubkey"].as_str().unwrap_or(""))
            .bind(event["kind"].as_u64().unwrap_or(0) as i64)
            .bind(event["content"].as_str().unwrap_or(""))
            .bind(event["tags"].to_string())
            .bind(event["created_at"].as_u64().unwrap_or(0) as i64)
            .bind(event["sig"].as_str().unwrap_or(""))
            .execute(&pool).await;
            imported += 1;
        }
    }
    Json(ApiResponse { success: true, message: format!("Restored {} events", imported) })
}

// ==================== BACKUP ====================
async fn backup_data(State(pool): State<SqlitePool>) -> Json<serde_json::Value> {
    let events = sqlx::query("SELECT * FROM events").fetch_all(&pool).await.unwrap_or_default();
    let ndjson: Vec<String> = events.into_iter().map(|row| {
        serde_json::json!({
            "id": row.get::<String, _>("id"),
            "pubkey": row.get::<String, _>("pubkey"),
            "kind": row.get::<i64, _>("kind"),
            "content": row.get::<String, _>("content"),
            "tags": serde_json::from_str::<serde_json::Value>(&row.get::<String, _>("tags")).unwrap_or(serde_json::Value::Array(vec![])),
            "created_at": row.get::<i64, _>("created_at"),
            "sig": row.get::<String, _>("sig"),
        }).to_string()
    }).collect();
    Json(serde_json::json!({ "ndjson": ndjson.join("\n") }))
}

// ==================== MAIN ====================
#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let pool = SqlitePool::connect("sqlite:nostr.db?mode=rwc").await.unwrap();
    ensure_tables(&pool).await;

    // Background sync
    let pool_clone = pool.clone();
    tokio::spawn(async move { let _ = sync::sync_npubs(pool_clone).await; });

    let app = Router::new()
        .route("/api/events", get(get_events))
        .route("/api/relays", get(get_relays))
        .route("/api/relays", post(add_relay))
        .route("/api/relays/:id", delete(delete_relay))
        .route("/api/npubs", get(get_npubs))
        .route("/api/npubs", post(add_npub))
        .route("/api/restore", post(restore_data))
        .route("/api/backup", get(backup_data))
        .with_state(pool)
        .nest_service("/", ServeDir::new("public"));

    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    let listener = TcpListener::bind(addr).await.unwrap();
    println!("🚀 Server running on http://{}", addr);
    axum::serve(listener, app.into_make_service()).await.unwrap();
}
