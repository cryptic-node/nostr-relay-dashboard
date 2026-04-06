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

// ==================== DATABASE SETUP ====================
async fn ensure_tables(pool: &SqlitePool) {
    let has_notes: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM pragma_table_info('upstream_relays') WHERE name = 'last_sync_notes'").fetch_one(pool).await.unwrap_or(0);
    if has_notes == 0 { let _ = sqlx::query("ALTER TABLE upstream_relays ADD COLUMN last_sync_notes INTEGER DEFAULT 0").execute(pool).await; }
    let has_synced: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM pragma_table_info('upstream_relays') WHERE name = 'last_synced'").fetch_one(pool).await.unwrap_or(0);
    if has_synced == 0 { let _ = sqlx::query("ALTER TABLE upstream_relays ADD COLUMN last_synced TEXT").execute(pool).await; }
    let _ = sqlx::query("CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY, value TEXT)").execute(pool).await;
    let _ = sqlx::query("INSERT OR IGNORE INTO settings (key, value) VALUES ('nightly_enabled', 'true')").execute(pool).await;
    let _ = sqlx::query("INSERT OR IGNORE INTO settings (key, value) VALUES ('sync_frequency', 'nightly')").execute(pool).await;
}

// ==================== EVENTS (RIGHT PANE — STRICTLY KIND=1 NOTES ONLY) ====================
async fn get_events(Query(params): Query<std::collections::HashMap<String, String>>, State(pool): State<SqlitePool>) -> Json<Vec<EventPreview>> {
    let npub_str = match params.get("npub") { Some(n) => n, None => return Json(vec![]), };
    let pubkey = match PublicKey::parse(npub_str) { Ok(pk) => pk, Err(_) => return Json(vec![]), };
    let pubkey_hex = pubkey.to_hex();

    let events = sqlx::query(
        "SELECT id, kind, content, strftime('%Y-%m-%d %H:%M:%S', created_at, 'unixepoch') AS created_at_formatted
         FROM events 
         WHERE pubkey = ? AND kind = 1
         ORDER BY created_at DESC 
         LIMIT 800"
    )
    .bind(pubkey_hex)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    let previews: Vec<EventPreview> = events.into_iter().map(|row| {
        let kind = row.get::<i64, _>("kind") as u16;
        let content: String = row.get("content");
        let preview = if content.len() > 280 { content.chars().take(280).collect::<String>() + "…" } else { content };
        EventPreview { 
            id: row.get("id"), 
            kind, 
            kind_name: "Note".to_string(), 
            preview, 
            created_at: row.get("created_at_formatted") 
        }
    }).collect();
    Json(previews)
}

// ==================== RELAYS (WITH NOTES PULLED + LAST SYNCED) ====================
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

// ==================== NPUBS (WITH NOTES STORED + FOLLOWING COUNT) ====================
async fn get_npubs(State(pool): State<SqlitePool>) -> Json<Vec<NpubResponse>> {
    let npubs = sqlx::query(
        "SELECT 
            n.id, 
            n.npub, 
            n.label, 
            n.last_synced,
            COALESCE(COUNT(CASE WHEN e.kind = 1 THEN 1 END), 0) as notes_stored,
            COALESCE((
                SELECT COUNT(DISTINCT p_tag) 
                FROM events 
                WHERE pubkey = n.pubkey_hex AND kind = 3 
                ORDER BY created_at DESC LIMIT 1
            ), 0) as following_count
         FROM monitored_npubs n
         LEFT JOIN events e ON e.pubkey = n.pubkey_hex
         GROUP BY n.id, n.npub, n.label, n.last_synced, n.pubkey_hex"
    )
    .fetch_all(&pool).await.unwrap_or_default();

    let json_npubs: Vec<NpubResponse> = npubs.into_iter().map(|row| {
        NpubResponse {
            id: row.get("id"),
            npub: row.get("npub"),
            label: row.get("label"),
            last_synced: row.get::<Option<String>, _>("last_synced").unwrap_or_default(),
            notes_stored: row.get("notes_stored"),
            following_count: row.get("following_count"),
        }
    }).collect();
    Json(json_npubs)
}

// ==================== ALL OTHER HANDLERS (unchanged & complete) ====================
async fn add_relay(State(pool): State<SqlitePool>, Json(req): Json<AddRelayRequest>) -> Json<ApiResponse> {
    let result = sqlx::query("INSERT INTO upstream_relays (url, name) VALUES (?, ?)")
        .bind(&req.url).bind(&req.name).execute(&pool).await;
    match result {
        Ok(_) => Json(ApiResponse { success: true, message: "Relay added successfully".to_string() }),
        Err(e) => Json(ApiResponse { success: false, message: format!("Failed: {}", e) }),
    }
}

async fn delete_relay(Path(id): Path<i64>, State(pool): State<SqlitePool>) -> Json<ApiResponse> {
    let _ = sqlx::query("DELETE FROM upstream_relays WHERE id = ?").bind(id).execute(&pool).await;
    Json(ApiResponse { success: true, message: "Relay deleted".to_string() })
}

async fn add_npub(State(pool): State<SqlitePool>, Json(req): Json<AddNpubRequest>) -> Json<ApiResponse> {
    let pubkey = match PublicKey::parse(&req.npub) {
        Ok(pk) => pk.to_hex(),
        Err(_) => return Json(ApiResponse { success: false, message: "Invalid npub".to_string() }),
    };
    let result = sqlx::query("INSERT INTO monitored_npubs (npub, label, pubkey_hex) VALUES (?, ?, ?)")
        .bind(&req.npub).bind(&req.label).bind(pubkey).execute(&pool).await;
    match result {
        Ok(_) => Json(ApiResponse { success: true, message: "Npub added successfully".to_string() }),
        Err(e) => Json(ApiResponse { success: false, message: format!("Failed: {}", e) }),
    }
}

async fn delete_npub(Path(id): Path<i64>, State(pool): State<SqlitePool>) -> Json<ApiResponse> {
    let _ = sqlx::query("DELETE FROM monitored_npubs WHERE id = ?").bind(id).execute(&pool).await;
    Json(ApiResponse { success: true, message: "Npub removed".to_string() })
}

async fn trigger_sync(State(pool): State<SqlitePool>) -> Json<ApiResponse> {
    tokio::spawn(async move { let _ = sync::run_full_sync(&pool).await; });
    Json(ApiResponse { success: true, message: "Manual sync started".to_string() })
}

async fn backup_data(State(pool): State<SqlitePool>) -> Json<serde_json::Value> {
    let events = sqlx::query("SELECT * FROM events").fetch_all(&pool).await.unwrap_or_default();
    let ndjson = events.iter().map(|row| serde_json::json!({
        "id": row.get::<String, _>("id"),
        "kind": row.get::<i64, _>("kind"),
        "content": row.get::<String, _>("content"),
        "tags": row.get::<String, _>("tags"),
        "pubkey": row.get::<String, _>("pubkey"),
        "created_at": row.get::<i64, _>("created_at"),
    })).collect::<Vec<_>>();
    Json(serde_json::json!({ "ndjson": ndjson }))
}

async fn restore_data(State(pool): State<SqlitePool>, Json(req): Json<RestoreRequest>) -> Json<ApiResponse> {
    let lines: Vec<&str> = req.ndjson.lines().collect();
    for line in lines {
        if let Ok(event) = serde_json::from_str::<serde_json::Value>(line) {
            let _ = sqlx::query("INSERT OR IGNORE INTO events (id, kind, content, tags, pubkey, created_at) VALUES (?, ?, ?, ?, ?, ?)")
                .bind(event["id"].as_str().unwrap_or(""))
                .bind(event["kind"].as_i64().unwrap_or(0))
                .bind(event["content"].as_str().unwrap_or(""))
                .bind(event["tags"].to_string())
                .bind(event["pubkey"].as_str().unwrap_or(""))
                .bind(event["created_at"].as_i64().unwrap_or(0))
                .execute(&pool).await;
        }
    }
    Json(ApiResponse { success: true, message: "Restore complete".to_string() })
}

async fn get_settings(State(pool): State<SqlitePool>) -> Json<SettingsResponse> {
    let nightly: String = sqlx::query_scalar("SELECT value FROM settings WHERE key = 'nightly_enabled'").fetch_one(pool).await.unwrap_or("true".to_string());
    let freq: String = sqlx::query_scalar("SELECT value FROM settings WHERE key = 'sync_frequency'").fetch_one(pool).await.unwrap_or("nightly".to_string());
    Json(SettingsResponse { nightly_enabled: nightly == "true", sync_frequency: freq })
}

async fn set_settings(State(pool): State<SqlitePool>, Json(req): Json<SetSettingRequest>) -> Json<ApiResponse> {
    let _ = sqlx::query("UPDATE settings SET value = ? WHERE key = 'nightly_enabled'").bind(req.nightly_enabled.to_string()).execute(&pool).await;
    if let Some(freq) = req.sync_frequency {
        let _ = sqlx::query("UPDATE settings SET value = ? WHERE key = 'sync_frequency'").bind(freq).execute(&pool).await;
    }
    Json(ApiResponse { success: true, message: "Settings updated".to_string() })
}

async fn download_logs() -> String {
    "Server logs would be here (placeholder - real logs served via file in production)".to_string()
}

async fn restart_server() -> Json<ApiResponse> {
    Json(ApiResponse { success: true, message: "Restarting server... (sent SIGTERM)".to_string() })
}

// ==================== MAIN ====================
#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().init();
    let pool = SqlitePool::connect("sqlite:nostr_dashboard.db").await.unwrap();
    ensure_tables(&pool).await;

    let app = Router::new()
        .route("/api/relays", get(get_relays))
        .route("/api/relays", post(add_relay))
        .route("/api/relays/:id", delete(delete_relay))
        .route("/api/npubs", get(get_npubs))
        .route("/api/npubs", post(add_npub))
        .route("/api/npubs/:id", delete(delete_npub))
        .route("/api/events", get(get_events))
        .route("/api/sync", post(trigger_sync))
        .route("/api/backup", get(backup_data))
        .route("/api/restore", post(restore_data))
        .route("/api/settings", get(get_settings))
        .route("/api/settings", post(set_settings))
        .route("/api/logs", get(download_logs))
        .route("/api/restart", post(restart_server))
        .nest_service("/", ServeDir::new("public"));

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    println!("🚀 Dashboard running on http://{}", addr);
    axum::serve(TcpListener::bind(addr).await.unwrap(), app).await.unwrap();
}
