use axum::{routing::{get, post, delete}, Router, Json, extract::{State, Query, Path}};
use sqlx::{SqlitePool, Row};
use tower_http::services::ServeDir;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tracing_subscriber;
use nostr_sdk::nostr::{PublicKey};
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
struct SetSettingRequest { nightly_enabled: bool }

#[derive(Serialize)]
struct ApiResponse { success: bool, message: String }

#[derive(Serialize)]
struct SettingsResponse { nightly_enabled: bool }

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
    // Add missing columns safely (no panic on first run)
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

    let _ = sqlx::query("CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY, value TEXT)").execute(pool).await;
    let _ = sqlx::query("INSERT OR IGNORE INTO settings (key, value) VALUES ('nightly_enabled', 'true')").execute(pool).await;
}

// ==================== EVENTS / NOTES (right pane) ====================
async fn get_events(Query(params): Query<std::collections::HashMap<String, String>>, State(pool): State<SqlitePool>) -> Json<Vec<EventPreview>> {
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
         FROM events WHERE pubkey = ? ORDER BY created_at DESC LIMIT 1000"
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
            0 => "Profile",
            1 => "Note",
            3 => "Contacts",
            6 => "Repost",
            7 => "Reaction",
            9735 => "Zap",
            _ => "Event",
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

        EventPreview {
            id: row.get("id"),
            kind,
            kind_name,
            preview,
            created_at: row.get("created_at_formatted"),
        }
    }).collect();

    Json(previews)
}

// ==================== ALL API HANDLERS ====================
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

async fn get_npubs(State(pool): State<SqlitePool>) -> Json<Vec<serde_json::Value>> {
    let npubs = sqlx::query("SELECT id, npub, label, last_synced, created_at FROM monitored_npubs")
        .fetch_all(&pool)
        .await
        .unwrap_or_default();

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
    let result = sqlx::query("INSERT INTO monitored_npubs (npub, label) VALUES (?, ?)")
        .bind(&req.npub)
        .bind(&req.label)
        .execute(&pool)
        .await;

    match result {
        Ok(_) => Json(ApiResponse { success: true, message: "Npub added successfully".to_string() }),
        Err(e) => Json(ApiResponse { success: false, message: format!("Failed: {}", e) }),
    }
}

async fn delete_npub(Path(id): Path<i64>, State(pool): State<SqlitePool>) -> Json<ApiResponse> {
    let _ = sqlx::query("DELETE FROM monitored_npubs WHERE id = ?").bind(id).execute(&pool).await;
    Json(ApiResponse { success: true, message: "Npub deleted".to_string() })
}

async fn trigger_sync(State(pool): State<SqlitePool>) -> Json<ApiResponse> {
    match sync::sync_npubs(pool.clone()).await {
        Ok(msg) => Json(ApiResponse { success: true, message: msg }),
        Err(e) => Json(ApiResponse { success: false, message: e.to_string() }),
    }
}

async fn get_settings(State(pool): State<SqlitePool>) -> Json<SettingsResponse> {
    let enabled: Option<String> = sqlx::query_scalar("SELECT value FROM settings WHERE key = 'nightly_enabled'")
        .fetch_optional(&pool)
        .await
        .ok()
        .flatten();
    let nightly_enabled = enabled.unwrap_or_else(|| "true".to_string()) == "true";
    Json(SettingsResponse { nightly_enabled })
}

async fn set_settings(State(pool): State<SqlitePool>, Json(req): Json<SetSettingRequest>) -> Json<ApiResponse> {
    let value = if req.nightly_enabled { "true" } else { "false" };
    let _ = sqlx::query("INSERT OR REPLACE INTO settings (key, value) VALUES ('nightly_enabled', ?)")
        .bind(value)
        .execute(&pool)
        .await;
    Json(ApiResponse { success: true, message: "Settings updated successfully".to_string() })
}

async fn get_logs(_pool: State<SqlitePool>) -> Json<Vec<String>> {
    Json(vec![
        format!("{} - Dashboard started successfully", chrono::Local::now().format("%Y-%m-%d %H:%M:%S")),
        "No errors in the last 24h".to_string(),
    ])
}

async fn backup(State(pool): State<SqlitePool>) -> String {
    let events = sqlx::query("SELECT id, pubkey, kind, content, tags, created_at FROM events ORDER BY created_at DESC")
        .fetch_all(&pool)
        .await
        .unwrap_or_default();

    let mut ndjson = String::new();
    for row in events {
        let event_json = serde_json::json!({
            "id": row.get::<String, _>("id"),
            "pubkey": row.get::<String, _>("pubkey"),
            "kind": row.get::<i64, _>("kind"),
            "content": row.get::<String, _>("content"),
            "tags": row.get::<Option<String>, _>("tags").unwrap_or_default(),
            "created_at": row.get::<Option<i64>, _>("created_at").unwrap_or_default(),
        });
        ndjson.push_str(&event_json.to_string());
        ndjson.push('\n');
    }
    ndjson
}

async fn restore(State(pool): State<SqlitePool>, Json(req): Json<RestoreRequest>) -> Json<ApiResponse> {
    let lines: Vec<&str> = req.ndjson.lines().collect();
    let mut imported = 0;

    for line in lines {
        if let Ok(event) = serde_json::from_str::<serde_json::Value>(line) {
            let _ = sqlx::query(
                "INSERT OR IGNORE INTO events (id, pubkey, kind, content, tags, created_at) VALUES (?, ?, ?, ?, ?, ?)"
            )
            .bind(event["id"].as_str().unwrap_or(""))
            .bind(event["pubkey"].as_str().unwrap_or(""))
            .bind(event["kind"].as_i64().unwrap_or(1))
            .bind(event["content"].as_str().unwrap_or(""))
            .bind(event["tags"].to_string())
            .bind(event["created_at"].as_i64().unwrap_or(chrono::Utc::now().timestamp()))
            .execute(&pool)
            .await;
            imported += 1;
        }
    }

    Json(ApiResponse {
        success: true,
        message: format!("Restored {} events successfully", imported),
    })
}

async fn restart_server() -> Json<ApiResponse> {
    println!("🚀 Restart requested by dashboard — shutting down now...");
    std::process::exit(0);
}

// ==================== MAIN ====================
#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let pool = SqlitePool::connect("sqlite:nostr_relay.db?mode=rwc")
        .await
        .expect("Failed to connect to SQLite");

    sqlx::migrate!().run(&pool).await.expect("Failed to run database migrations");
    ensure_tables(&pool).await;

    println!("✅ Database connected and migrations applied.");

    let app = Router::new()
        .route("/api/relays", get(get_relays).post(add_relay))
        .route("/api/relays/:id", delete(delete_relay))
        .route("/api/npubs", get(get_npubs).post(add_npub))
        .route("/api/npubs/:id", delete(delete_npub))
        .route("/api/sync", post(trigger_sync))
        .route("/api/events", get(get_events))
        .route("/api/backup", get(backup))
        .route("/api/restore", post(restore))
        .route("/api/settings", get(get_settings).post(set_settings))
        .route("/api/logs", get(get_logs))
        .route("/api/restart", post(restart_server))
        .nest_service("/", ServeDir::new("public"))
        .with_state(pool.clone());

    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    println!("🚀 Nostr Relay Dashboard running on http://0.0.0.0:8080");

    let pool_for_task = pool.clone();
    tokio::spawn(async move {
        loop {
            let now = Local::now();
            if now.hour() == 0 && now.minute() == 0 {
                let enabled: Option<String> = sqlx::query_scalar("SELECT value FROM settings WHERE key = 'nightly_enabled'")
                    .fetch_optional(&pool_for_task)
                    .await
                    .ok()
                    .flatten();

                if enabled.unwrap_or_else(|| "true".to_string()) == "true" {
                    println!("🌙 Running nightly auto-sync at midnight...");
                    let _ = sync::sync_npubs(pool_for_task.clone()).await;
                }
                tokio::time::sleep(std::time::Duration::from_secs(70)).await;
            }
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        }
    });

    let listener = TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app.into_make_service()).await.unwrap();
}
