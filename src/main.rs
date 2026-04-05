use axum::{routing::{get, post, delete}, Router, Json, extract::{State, Query, Path}, response::IntoResponse};
use sqlx::{SqlitePool, Row};
use tower_http::services::ServeDir;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tracing_subscriber;
use std::collections::HashMap;
use nostr::PublicKey;
use nostr::Event as NostrEvent;
use nostr::JsonUtil; // ← needed for .from_json()

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

#[derive(Deserialize)]
struct RestoreRequest {
    ndjson: String,
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

async fn ensure_relay_stats_columns(pool: &SqlitePool) {
    let has_notes: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pragma_table_info('upstream_relays') WHERE name = 'last_sync_notes'"
    ).fetch_one(pool).await.unwrap_or(0);
    if has_notes == 0 {
        let _ = sqlx::query("ALTER TABLE upstream_relays ADD COLUMN last_sync_notes INTEGER DEFAULT 0")
            .execute(pool).await;
        println!("✅ Added column: last_sync_notes");
    }
    let has_synced: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pragma_table_info('upstream_relays') WHERE name = 'last_synced'"
    ).fetch_one(pool).await.unwrap_or(0);
    if has_synced == 0 {
        let _ = sqlx::query("ALTER TABLE upstream_relays ADD COLUMN last_synced TEXT")
            .execute(pool).await;
        println!("✅ Added column: last_synced");
    }
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
        "SELECT id, kind, content, tags, 
         strftime('%Y-%m-%d %H:%M:%S', created_at, 'unixepoch') AS created_at_formatted 
         FROM events 
         WHERE pubkey = ? 
         ORDER BY created_at DESC LIMIT 10"
    )
    .bind(pubkey_hex)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    let previews: Vec<EventPreview> = events.into_iter().map(|row| {
        let kind = row.get::<i64, _>("kind") as u16;
        let content: String = row.get("content");
        let kind_name = match kind {
            0 => "Profile", 1 => "Text Note", 3 => "Contacts",
            6 => "Repost", 7 => "Reaction", 9735 => "Zap", _ => "Event",
        }.to_string();

        let preview = match kind {
            3 => "Updated contact list".to_string(),
            0 => "Updated profile".to_string(),
            _ => if content.len() > 200 { content.chars().take(200).collect::<String>() + "…" } else { content },
        };

        EventPreview {
            id: row.get::<String, _>("id"),
            kind,
            kind_name,
            preview,
            created_at: row.get::<String, _>("created_at_formatted"),
        }
    }).collect();

    Json(previews)
}

async fn backup(State(pool): State<SqlitePool>) -> impl IntoResponse {
    let events = sqlx::query(
        "SELECT id, pubkey, kind, content, created_at, tags, sig FROM events"
    )
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    let mut ndjson = String::new();
    for row in events {
        let tags_str: String = row.get("tags");
        let tags: Vec<Vec<String>> = serde_json::from_str(&tags_str).unwrap_or_default();

        let event_json = serde_json::json!({
            "id": row.get::<String, _>("id"),
            "pubkey": row.get::<String, _>("pubkey"),
            "kind": row.get::<i64, _>("kind") as u16,
            "content": row.get::<String, _>("content"),
            "created_at": row.get::<i64, _>("created_at"),
            "tags": tags,
            "sig": row.get::<String, _>("sig"),
        });

        ndjson.push_str(&event_json.to_string());
        ndjson.push('\n');
    }

    ([(axum::http::header::CONTENT_TYPE, "application/x-ndjson")], ndjson)
}

async fn restore(State(pool): State<SqlitePool>, Json(req): Json<RestoreRequest>) -> Json<ApiResponse> {
    let mut inserted = 0usize;

    for line in req.ndjson.lines() {
        let line = line.trim();
        if line.is_empty() { continue; }

        if let Ok(event) = NostrEvent::from_json(line) {
            let _ = sqlx::query(
                "INSERT OR IGNORE INTO events (id, pubkey, kind, content, created_at, tags, sig)
                 VALUES (?, ?, ?, ?, ?, ?, ?)"
            )
            .bind(event.id.to_hex())
            .bind(event.pubkey.to_hex())
            .bind(event.kind.as_u16() as i64)
            .bind(&event.content)
            .bind(event.created_at.as_secs() as i64)
            .bind(serde_json::to_string(&event.tags).unwrap_or_default())
            .bind(event.sig.to_string())
            .execute(&pool)
            .await;

            inserted += 1;
        }
    }

    Json(ApiResponse {
        success: true,
        message: format!("✅ Restored {} events successfully!", inserted),
    })
}

async fn get_relays(State(pool): State<SqlitePool>) -> Json<Vec<serde_json::Value>> {
    let relays = sqlx::query(
        "SELECT id, url, name, enabled, preloaded, created_at, last_sync_notes, last_synced 
         FROM upstream_relays"
    )
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
            "created_at": row.get::<Option<String>, _>("created_at"),
            "last_sync_notes": row.get::<Option<i64>, _>("last_sync_notes").unwrap_or(0),
            "last_synced": row.get::<Option<String>, _>("last_synced"),
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
        Err(e) => Json(ApiResponse { success: false, message: format!("Failed to add relay: {}", e) }),
    }
}

async fn delete_relay(Path(id): Path<i64>, State(pool): State<SqlitePool>) -> Json<ApiResponse> {
    let result = sqlx::query("DELETE FROM upstream_relays WHERE id = ?")
        .bind(id)
        .execute(&pool)
        .await;
    match result {
        Ok(r) if r.rows_affected() > 0 => Json(ApiResponse { success: true, message: "Relay deleted".to_string() }),
        _ => Json(ApiResponse { success: false, message: "Relay not found".to_string() }),
    }
}

async fn get_npubs(State(pool): State<SqlitePool>) -> Json<Vec<serde_json::Value>> {
    let npubs = sqlx::query(
        "SELECT id, npub, label, last_synced, created_at FROM monitored_npubs"
    )
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    let json_npubs: Vec<serde_json::Value> = npubs.into_iter().map(|row| {
        serde_json::json!({
            "id": row.get::<i64, _>("id"),
            "npub": row.get::<String, _>("npub"),
            "label": row.get::<Option<String>, _>("label"),
            "last_synced": row.get::<Option<String>, _>("last_synced"),
            "created_at": row.get::<Option<String>, _>("created_at"),
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
        Err(e) => Json(ApiResponse { success: false, message: format!("Failed to add npub: {}", e) }),
    }
}

async fn delete_npub(Path(id): Path<i64>, State(pool): State<SqlitePool>) -> Json<ApiResponse> {
    let result = sqlx::query("DELETE FROM monitored_npubs WHERE id = ?")
        .bind(id)
        .execute(&pool)
        .await;
    match result {
        Ok(r) if r.rows_affected() > 0 => Json(ApiResponse { success: true, message: "Npub deleted".to_string() }),
        _ => Json(ApiResponse { success: false, message: "Npub not found".to_string() }),
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

    let pool = SqlitePool::connect("sqlite:nostr_relay.db?mode=rwc")
        .await
        .expect("Failed to connect to SQLite");

    sqlx::migrate!()
        .run(&pool)
        .await
        .expect("Failed to run database migrations");

    ensure_relay_stats_columns(&pool).await;

    println!("Database connected and migrations applied.");

    let app = Router::new()
        .route("/api/relays", get(get_relays).post(add_relay))
        .route("/api/relays/:id", delete(delete_relay))
        .route("/api/npubs", get(get_npubs).post(add_npub))
        .route("/api/npubs/:id", delete(delete_npub))
        .route("/api/sync", post(trigger_sync))
        .route("/api/events", get(get_events))
        .route("/api/backup", get(backup))
        .route("/api/restore", post(restore))
        .nest_service("/", ServeDir::new("public"))
        .with_state(pool);

    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    println!("Nostr Relay Dashboard running on http://0.0.0.0:8080");

    let listener = TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app.into_make_service())
        .await
        .unwrap();
}
