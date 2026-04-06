use axum::{routing::{get, post, delete}, Router, Json, extract::{State, Query, Path}};
use sqlx::{SqlitePool, Row};
use tower_http::services::ServeDir;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tracing_subscriber;
use nostr_sdk::nostr::PublicKey;
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

// ==================== DATABASE SETUP ====================
async fn ensure_tables(pool: &SqlitePool) {
    let has_notes: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM pragma_table_info('upstream_relays') WHERE name = 'last_sync_notes'")
        .fetch_one(pool).await.unwrap_or(0);
    if has_notes == 0 {
        let _ = sqlx::query("ALTER TABLE upstream_relays ADD COLUMN last_sync_notes INTEGER DEFAULT 0")
            .execute(pool).await;
    }
    let has_synced: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM pragma_table_info('upstream_relays') WHERE name = 'last_synced'")
        .fetch_one(pool).await.unwrap_or(0);
    if has_synced == 0 {
        let _ = sqlx::query("ALTER TABLE upstream_relays ADD COLUMN last_synced TEXT")
            .execute(pool).await;
    }
    let _ = sqlx::query("CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY, value TEXT)")
        .execute(pool).await;
    let _ = sqlx::query("INSERT OR IGNORE INTO settings (key, value) VALUES ('nightly_enabled', 'true')")
        .execute(pool).await;
    let _ = sqlx::query("INSERT OR IGNORE INTO settings (key, value) VALUES ('sync_frequency', 'nightly')")
        .execute(pool).await;
}

// ==================== EVENTS ====================
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
        "SELECT id, kind, content, tags, strftime('%Y-%m-%d %H:%M:%S', created_at, 'unixepoch') AS created_at_formatted
         FROM events 
         WHERE pubkey = ? 
         ORDER BY 
           CASE 
             WHEN kind = 1 THEN 0
             WHEN kind = 0 THEN 1
             WHEN kind = 3 THEN 99
             ELSE 2 
           END, 
           created_at DESC 
         LIMIT 800"
    )
    .bind(pubkey_hex)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    let previews: Vec<EventPreview> = events.into_iter().map(|row| {
        let kind = row.get::<i64, _>("kind") as u16;
        let content: String = row.get("content");
        let tags_str: String = row.get("tags");
        let tags: Vec<Vec<String>> = serde_json::from_str(&tags_str).unwrap_or_default();
        let kind_name = match kind {
            0 => "Profile", 1 => "Note", 3 => "Contacts", 6 => "Repost", 7 => "Reaction", 9735 => "Zap", _ => "Event",
        }.to_string();
        let preview = match kind {
            1 => if content.len() > 280 { content.chars().take(280).collect::<String>() + "…" } else { content },
            3 => {
                let following = tags.iter().filter(|t| t.first() == Some(&"p".to_string())).count();
                format!("Updated contact list ({} following)", following)
            }
            0 => "Updated profile".to_string(),
            _ => if content.len() > 200 { content.chars().take(200).collect::<String>() + "…" } else { content },
        };
        EventPreview { id: row.get("id"), kind, kind_name, preview, created_at: row.get("created_at_formatted") }
    }).collect();

    Json(previews)
}

// ==================== OTHER HANDLERS ====================
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

async fn get_npubs(State(pool): State<SqlitePool>) -> Json<Vec<serde_json::Value>> {
    let npubs = sqlx::query("SELECT id, npub, label, last_synced, created_at FROM monitored_npubs")
        .fetch_all(&pool).await.unwrap_or_default();
    let json_npubs: Vec<serde_json::Value> = npubs.into_iter().map(|row| {
        serde_json::json!({
            "id": row.get::<i64, _>("id"),
            "npub": row.get::<String, _>("npub"),
            "label": row.get::<Option<String>, _>("label"),
            "last_synced": row.get::<Option<String>, _>("last_synced").unwrap_or_default(),
            "created_at": row.get::<Option<String>, _>("created_at").unwrap_or_default(),
        })
    }).collect();
    Json(json_npubs)
}

async fn add_npub(State(pool): State<SqlitePool>, Json(req): Json<AddNpubRequest>) -> Json<ApiResponse> {
    let result = sqlx::query("INSERT OR IGNORE INTO monitored_npubs (npub, label) VALUES (?, ?)")
        .bind(&req.npub).bind(&req.label).execute(&pool).await;
    match result {
        Ok(_) => Json(ApiResponse { success: true, message: "Npub added successfully".to_string() }),
        Err(e) => Json(ApiResponse { success: false, message: format!("Failed: {}", e) }),
    }
}

// ==================== MAIN ====================
#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let pool = SqlitePool::connect("sqlite:nostr.db?mode=rwc").await.unwrap();
    ensure_tables(&pool).await;

    // Background full sync (fixed function name + pool clone)
    let pool_clone = pool.clone();
    tokio::spawn(async move {
        let _ = sync::sync_npubs(pool_clone).await;
    });

    // Load settings (fixed &pool)
    let nightly: String = sqlx::query_scalar("SELECT value FROM settings WHERE key = 'nightly_enabled'")
        .fetch_one(&pool).await.unwrap_or("true".to_string());

    let freq: String = sqlx::query_scalar("SELECT value FROM settings WHERE key = 'sync_frequency'")
        .fetch_one(&pool).await.unwrap_or("nightly".to_string());

    println!("✅ Nightly enabled: {}, Frequency: {}", nightly, freq);

    let app = Router::new()
        .route("/api/events", get(get_events))
        .route("/api/relays", get(get_relays))
        .route("/api/relays", post(add_relay))
        .route("/api/relays/:id", delete(delete_relay))
        .route("/api/npubs", get(get_npubs))
        .route("/api/npubs", post(add_npub))
        // add more routes here if you have them (delete_npub, restore, settings, etc.)
        .with_state(pool)
        .nest_service("/", ServeDir::new("public"));

    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    let listener = TcpListener::bind(addr).await.unwrap();

    println!("🚀 Server running on http://{}", addr);
    axum::serve(listener, app.into_make_service()).await.unwrap();
}
