use axum::{
    extract::{Path, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
    Json, Router,
};
use chrono::{Local, Utc};
use nostr_sdk::{ClientBuilder, Filter, FromBech32, Kind, PublicKey, Timestamp};
use serde::{Deserialize, Serialize};
use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions},
    Row,
};
use std::{
    collections::HashSet,
    fs::{self, OpenOptions},
    io::Write,
    net::{IpAddr, SocketAddr},
    sync::Arc,
    time::Duration,
};
use tokio::net::TcpListener;
use tower_http::services::ServeDir;

#[derive(Deserialize)]
struct AddRelayRequest {
    url: String,
    name: Option<String>,
}

#[derive(Deserialize)]
struct AddNpubRequest {
    npub: String,
    label: Option<String>,
}

#[derive(Deserialize)]
struct ToggleRelayRequest {
    enabled: bool,
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

#[derive(Serialize)]
struct NpubResponse {
    id: i64,
    npub: String,
    label: Option<String>,
    last_synced: String,
    notes_stored: i64,
    following_count: i64,
}

#[derive(Serialize)]
struct RelayResponse {
    id: i64,
    url: String,
    name: Option<String>,
    enabled: bool,
    preloaded: bool,
    last_sync_notes: i64,
    last_synced: String,
    last_error: Option<String>,
}

#[derive(Clone)]
struct RelayRow {
    id: i64,
    url: String,
    name: Option<String>,
}

#[derive(Clone)]
struct NpubRow {
    id: i64,
    npub: String,
    pubkey_hex: String,
}

struct AppState {
    pool: SqlitePool,
}

const MAX_RESTORE_BYTES: usize = 5 * 1024 * 1024;
const MAX_RESTORE_RECORDS: usize = 100_000;
const MAX_NAME_LENGTH: usize = 80;
const MAX_LABEL_LENGTH: usize = 80;
const MAX_URL_LENGTH: usize = 512;

fn log_message(message: &str) {
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let entry = format!("{} | {}\n", timestamp, message);
    println!("{}", entry.trim_end());

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open("dashboard.log")
        .expect("Failed to open dashboard.log");

    let _ = file.write_all(entry.as_bytes());
}

async fn column_exists(pool: &SqlitePool, table: &str, column: &str) -> bool {
    let pragma = format!("PRAGMA table_info({})", table);
    match sqlx::query(&pragma).fetch_all(pool).await {
        Ok(rows) => rows
            .iter()
            .any(|row| row.get::<String, _>("name") == column),
        Err(_) => false,
    }
}

async fn ensure_column(pool: &SqlitePool, table: &str, definition: &str) {
    let column_name = definition
        .split_whitespace()
        .next()
        .unwrap_or("")
        .trim_matches('"');

    if column_name.is_empty() || column_exists(pool, table, column_name).await {
        return;
    }

    let sql = format!("ALTER TABLE {} ADD COLUMN {}", table, definition);
    if let Err(error) = sqlx::query(&sql).execute(pool).await {
        log_message(&format!(
            "Schema note: could not add column {}.{} ({})",
            table, column_name, error
        ));
    }
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
            last_synced TEXT,
            last_error TEXT
        )
        "#,
    )
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS monitored_npubs (
            id INTEGER PRIMARY KEY,
            npub TEXT UNIQUE NOT NULL,
            label TEXT,
            pubkey_hex TEXT,
            last_synced TEXT
        )
        "#,
    )
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS events (
            id TEXT PRIMARY KEY,
            pubkey TEXT NOT NULL,
            kind INTEGER NOT NULL,
            content TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            raw_json TEXT,
            source_relay TEXT,
            imported_at TEXT
        )
        "#,
    )
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS settings (
            key TEXT PRIMARY KEY,
            value TEXT
        )
        "#,
    )
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS npub_relay_sync_state (
            npub_id INTEGER NOT NULL,
            relay_id INTEGER NOT NULL,
            last_synced_unix INTEGER NOT NULL DEFAULT 0,
            last_sync_notes INTEGER NOT NULL DEFAULT 0,
            last_result TEXT,
            last_error TEXT,
            updated_at TEXT,
            PRIMARY KEY (npub_id, relay_id)
        )
        "#,
    )
    .execute(pool)
    .await
    .unwrap();

    ensure_column(pool, "upstream_relays", "last_error TEXT").await;
    ensure_column(pool, "events", "raw_json TEXT").await;
    ensure_column(pool, "events", "source_relay TEXT").await;
    ensure_column(pool, "events", "imported_at TEXT").await;

    sqlx::query("INSERT OR IGNORE INTO settings (key, value) VALUES ('nightly_enabled', 'true')")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("INSERT OR IGNORE INTO settings (key, value) VALUES ('sync_frequency', 'manual')")
        .execute(pool)
        .await
        .unwrap();

    let relay_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM upstream_relays")
        .fetch_one(pool)
        .await
        .unwrap_or(0);

    if relay_count == 0 {
        let preloaded = vec![
            ("wss://relay.damus.io", "Damus"),
            ("wss://nos.lol", "nos.lol"),
            ("wss://relay.primal.net", "Primal"),
            ("wss://nostr.wine", "Nostr Wine"),
            ("wss://relay.snort.social", "Snort"),
        ];

        for (url, name) in preloaded {
            let _ = sqlx::query(
                "INSERT OR IGNORE INTO upstream_relays (url, name, enabled, preloaded) VALUES (?, ?, 1, 1)",
            )
            .bind(url)
            .bind(name)
            .execute(pool)
            .await;
        }

        log_message("Preloaded public relays initialized");
    }

    log_message("Database ready — v1.0.4 hardening candidate");
}

async fn upsert_sync_state(
    pool: &SqlitePool,
    npub_id: i64,
    relay_id: i64,
    last_synced_unix: i64,
    last_sync_notes: i64,
    last_result: &str,
    last_error: Option<&str>,
) {
    let _ = sqlx::query(
        r#"
        INSERT INTO npub_relay_sync_state
            (npub_id, relay_id, last_synced_unix, last_sync_notes, last_result, last_error, updated_at)
        VALUES (?, ?, ?, ?, ?, ?, datetime('now'))
        ON CONFLICT(npub_id, relay_id) DO UPDATE SET
            last_synced_unix = excluded.last_synced_unix,
            last_sync_notes = excluded.last_sync_notes,
            last_result = excluded.last_result,
            last_error = excluded.last_error,
            updated_at = excluded.updated_at
        "#,
    )
    .bind(npub_id)
    .bind(relay_id)
    .bind(last_synced_unix)
    .bind(last_sync_notes)
    .bind(last_result)
    .bind(last_error)
    .execute(pool)
    .await;
}

async fn perform_sync(pool: &SqlitePool) {
    log_message("=== SYNC STARTED (v1.0.4) ===");

    let relays: Vec<RelayRow> = sqlx::query(
        "SELECT id, url, name FROM upstream_relays WHERE enabled = 1 ORDER BY COALESCE(name, url)",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|row| RelayRow {
        id: row.get("id"),
        url: row.get("url"),
        name: row.get("name"),
    })
    .collect();

    let npubs: Vec<NpubRow> = sqlx::query(
        "SELECT id, npub, pubkey_hex FROM monitored_npubs ORDER BY COALESCE(label, npub)",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|row| NpubRow {
        id: row.get("id"),
        npub: row.get("npub"),
        pubkey_hex: row.get::<Option<String>, _>("pubkey_hex").unwrap_or_default(),
    })
    .collect();

    if relays.is_empty() {
        log_message("Sync skipped: no enabled relays");
        return;
    }

    if npubs.is_empty() {
        log_message("Sync skipped: no monitored npubs");
        return;
    }

    let mut npub_successes = HashSet::<i64>::new();

    for relay in relays {
        log_message(&format!(
            "Connecting to relay {} ({})",
            relay.name.clone().unwrap_or_else(|| "Unnamed relay".to_string()),
            relay.url
        ));

        let client = ClientBuilder::new().build();

        if let Err(error) = client.add_relay(&relay.url).await {
            let message = format!("Failed to add relay {}: {}", relay.url, error);
            log_message(&message);

            let _ = sqlx::query(
                "UPDATE upstream_relays SET last_error = ?, last_synced = datetime('now'), last_sync_notes = 0 WHERE id = ?",
            )
            .bind(message.clone())
            .bind(relay.id)
            .execute(pool)
            .await;

            continue;
        }

        client.connect().await;

        let mut relay_new_notes = 0_i64;
        let mut relay_error: Option<String> = None;

        for npub in &npubs {
            if npub.pubkey_hex.is_empty() {
                log_message(&format!("Skipping {} because pubkey_hex is empty", npub.npub));
                continue;
            }

            let pubkey = match PublicKey::from_hex(&npub.pubkey_hex) {
                Ok(pk) => pk,
                Err(error) => {
                    let message = format!("Invalid pubkey for {}: {}", npub.npub, error);
                    relay_error = Some(message.clone());
                    log_message(&message);
                    upsert_sync_state(
                        pool,
                        npub.id,
                        relay.id,
                        0,
                        0,
                        "error",
                        Some(&message),
                    )
                    .await;
                    continue;
                }
            };

            let last_synced_unix: i64 = sqlx::query_scalar(
                "SELECT last_synced_unix FROM npub_relay_sync_state WHERE npub_id = ? AND relay_id = ?",
            )
            .bind(npub.id)
            .bind(relay.id)
            .fetch_optional(pool)
            .await
            .unwrap_or(None)
            .unwrap_or(0);

            let now_unix = Utc::now().timestamp();
            let since_unix = if last_synced_unix > 0 {
                (last_synced_unix - 300).max(0)
            } else {
                now_unix - 604800
            };

            let filter = Filter::new()
                .authors(vec![pubkey])
                .kind(Kind::TextNote)
                .since(Timestamp::from_secs(since_unix as u64));

            match client.fetch_events(filter, Duration::from_secs(15)).await {
                Ok(events) => {
                    let mut inserted_for_pair = 0_i64;
                    let checkpoint_unix = now_unix.max(last_synced_unix);

                    for event in events {
                        let event_id = event.id.to_hex();
                        let created_at = event.created_at.as_secs() as i64;
                        let raw_json = serde_json::to_string(&event).unwrap_or_default();
                        let content = event.content.clone();

                        let result = sqlx::query(
                            r#"
                            INSERT OR IGNORE INTO events
                                (id, pubkey, kind, content, created_at, raw_json, source_relay, imported_at)
                            VALUES (?, ?, ?, ?, ?, ?, ?, datetime('now'))
                            "#,
                        )
                        .bind(event_id)
                        .bind(&npub.pubkey_hex)
                        .bind(1_i64)
                        .bind(content)
                        .bind(created_at)
                        .bind(raw_json)
                        .bind(&relay.url)
                        .execute(pool)
                        .await;

                        if let Ok(done) = result {
                            inserted_for_pair += done.rows_affected() as i64;
                        }
                    }

                    relay_new_notes += inserted_for_pair;
                    npub_successes.insert(npub.id);

                    upsert_sync_state(
                        pool,
                        npub.id,
                        relay.id,
                        checkpoint_unix,
                        inserted_for_pair,
                        "ok",
                        None,
                    )
                    .await;

                    log_message(&format!(
                        "Relay {} stored {} new notes for {}",
                        relay.url, inserted_for_pair, npub.npub
                    ));
                }
                Err(error) => {
                    let message = format!("Error pulling notes for {} from {}: {}", npub.npub, relay.url, error);
                    relay_error = Some(message.clone());
                    log_message(&message);

                    upsert_sync_state(
                        pool,
                        npub.id,
                        relay.id,
                        last_synced_unix,
                        0,
                        "error",
                        Some(&message),
                    )
                    .await;
                }
            }
        }

        let _ = sqlx::query(
            "UPDATE upstream_relays SET last_sync_notes = ?, last_synced = datetime('now'), last_error = ? WHERE id = ?",
        )
        .bind(relay_new_notes)
        .bind(relay_error)
        .bind(relay.id)
        .execute(pool)
        .await;

        let _ = client.disconnect().await;

        log_message(&format!(
            "Relay {} complete — {} new notes stored",
            relay.url, relay_new_notes
        ));
    }

    for npub_id in npub_successes {
        let _ = sqlx::query("UPDATE monitored_npubs SET last_synced = datetime('now') WHERE id = ?")
            .bind(npub_id)
            .execute(pool)
            .await;
    }

    log_message("=== SYNC COMPLETE ===");
}

async fn get_relays(State(state): State<Arc<AppState>>) -> Json<Vec<RelayResponse>> {
    let relays = sqlx::query(
        r#"
        SELECT id, url, name, enabled, preloaded, last_sync_notes, last_synced, last_error
        FROM upstream_relays
        ORDER BY enabled DESC, COALESCE(name, url)
        "#,
    )
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    let json = relays
        .into_iter()
        .map(|row| RelayResponse {
            id: row.get("id"),
            url: row.get("url"),
            name: row.get("name"),
            enabled: row.get::<i64, _>("enabled") != 0,
            preloaded: row.get::<i64, _>("preloaded") != 0,
            last_sync_notes: row.get::<Option<i64>, _>("last_sync_notes").unwrap_or(0),
            last_synced: row
                .get::<Option<String>, _>("last_synced")
                .unwrap_or_default(),
            last_error: row.get("last_error"),
        })
        .collect();

    Json(json)
}

async fn get_npubs(State(state): State<Arc<AppState>>) -> Json<Vec<NpubResponse>> {
    let npubs = sqlx::query(
        r#"
        SELECT n.id, n.npub, n.label, n.last_synced,
               COALESCE(COUNT(e.id), 0) AS notes_stored,
               0 AS following_count
        FROM monitored_npubs n
        LEFT JOIN events e ON e.pubkey = n.pubkey_hex AND e.kind = 1
        GROUP BY n.id, n.npub, n.label, n.last_synced
        ORDER BY COALESCE(n.label, n.npub)
        "#,
    )
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    let json = npubs
        .into_iter()
        .map(|row| NpubResponse {
            id: row.get("id"),
            npub: row.get("npub"),
            label: row.get("label"),
            last_synced: row
                .get::<Option<String>, _>("last_synced")
                .unwrap_or_default(),
            notes_stored: row.get("notes_stored"),
            following_count: row.get("following_count"),
        })
        .collect();

    Json(json)
}

async fn get_events(
    State(state): State<Arc<AppState>>,
    Path(npub_id): Path<i64>,
) -> Json<Vec<EventPreview>> {
    let events = sqlx::query(
        r#"
        SELECT id, kind, content, created_at
        FROM events
        WHERE pubkey = (SELECT pubkey_hex FROM monitored_npubs WHERE id = ?)
          AND kind = 1
        ORDER BY created_at DESC
        LIMIT 100
        "#,
    )
    .bind(npub_id)
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    let json = events
        .into_iter()
        .map(|row| {
            let content: String = row.get("content");
            let preview = if content.chars().count() > 280 {
                content.chars().take(277).collect::<String>() + "..."
            } else {
                content
            };

            EventPreview {
                id: row.get("id"),
                kind: row.get::<i64, _>("kind") as u16,
                kind_name: "Text Note".to_string(),
                preview,
                created_at: chrono::DateTime::from_timestamp(row.get::<i64, _>("created_at"), 0)
                    .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
                    .unwrap_or_default(),
            }
        })
        .collect();

    Json(json)
}

async fn add_relay(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<AddRelayRequest>,
) -> Response {
    if let Some(response) = require_admin(&headers) {
        return response;
    }

    let url = payload.url.trim();
    if url.is_empty() {
        return json_response(StatusCode::BAD_REQUEST, false, "Relay URL is required");
    }
    if url.len() > MAX_URL_LENGTH {
        return json_response(StatusCode::BAD_REQUEST, false, "Relay URL is too long");
    }
    if !is_valid_relay_url(url) {
        return json_response(StatusCode::BAD_REQUEST, false, "Relay URL must start with ws:// or wss://");
    }

    let trimmed_name = payload.name.as_deref().map(str::trim).filter(|value| !value.is_empty());
    if trimmed_name.map(|value| value.chars().count() > MAX_NAME_LENGTH).unwrap_or(false) {
        return json_response(StatusCode::BAD_REQUEST, false, "Relay name is too long");
    }

    let result = sqlx::query(
        "INSERT OR IGNORE INTO upstream_relays (url, name, enabled, preloaded, last_sync_notes) VALUES (?, ?, 1, 0, 0)",
    )
    .bind(url)
    .bind(trimmed_name)
    .execute(&state.pool)
    .await;

    match result {
        Ok(done) if done.rows_affected() > 0 => {
            log_message(&format!("Added new relay: {}", url));
            json_response(StatusCode::OK, true, "Relay added successfully")
        }
        Ok(_) => json_response(StatusCode::CONFLICT, false, "Relay already exists"),
        Err(error) => json_response(StatusCode::INTERNAL_SERVER_ERROR, false, format!("Failed to add relay: {}", error)),
    }
}

async fn add_npub(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<AddNpubRequest>,
) -> Response {
    if let Some(response) = require_admin(&headers) {
        return response;
    }

    let trimmed_npub = payload.npub.trim();
    if trimmed_npub.is_empty() {
        return json_response(StatusCode::BAD_REQUEST, false, "Npub is required");
    }

    let trimmed_label = payload.label.as_deref().map(str::trim).filter(|value| !value.is_empty());
    if trimmed_label.map(|value| value.chars().count() > MAX_LABEL_LENGTH).unwrap_or(false) {
        return json_response(StatusCode::BAD_REQUEST, false, "Label is too long");
    }

    let pubkey = match PublicKey::from_bech32(trimmed_npub) {
        Ok(pk) => pk.to_hex(),
        Err(_) => return json_response(StatusCode::BAD_REQUEST, false, "Invalid npub format"),
    };

    let result = sqlx::query(
        "INSERT OR IGNORE INTO monitored_npubs (npub, label, pubkey_hex) VALUES (?, ?, ?)",
    )
    .bind(trimmed_npub)
    .bind(trimmed_label)
    .bind(pubkey)
    .execute(&state.pool)
    .await;

    match result {
        Ok(done) if done.rows_affected() > 0 => {
            log_message(&format!("Added npub: {}", trimmed_npub));
            json_response(StatusCode::OK, true, "Npub added successfully")
        }
        Ok(_) => json_response(StatusCode::CONFLICT, false, "Npub already exists"),
        Err(error) => json_response(StatusCode::INTERNAL_SERVER_ERROR, false, format!("Failed to add npub: {}", error)),
    }
}

async fn toggle_relay(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(payload): Json<ToggleRelayRequest>,
) -> Response {
    if let Some(response) = require_admin(&headers) {
        return response;
    }

    let enabled_value = if payload.enabled { 1 } else { 0 };
    let result = sqlx::query("UPDATE upstream_relays SET enabled = ? WHERE id = ?")
        .bind(enabled_value)
        .bind(id)
        .execute(&state.pool)
        .await;

    match result {
        Ok(done) if done.rows_affected() > 0 => {
            let action = if payload.enabled { "enabled" } else { "disabled" };
            log_message(&format!("Relay ID {} {}", id, action));
            json_response(StatusCode::OK, true, format!("Relay {}", action))
        }
        Ok(_) => json_response(StatusCode::NOT_FOUND, false, "Relay not found"),
        Err(error) => json_response(StatusCode::INTERNAL_SERVER_ERROR, false, format!("Failed to update relay: {}", error)),
    }
}

async fn delete_relay(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Response {
    if let Some(response) = require_admin(&headers) {
        return response;
    }

    let _ = sqlx::query("DELETE FROM npub_relay_sync_state WHERE relay_id = ?")
        .bind(id)
        .execute(&state.pool)
        .await;

    let result = sqlx::query("DELETE FROM upstream_relays WHERE id = ?")
        .bind(id)
        .execute(&state.pool)
        .await;

    match result {
        Ok(done) if done.rows_affected() > 0 => {
            log_message(&format!("Deleted relay ID {} (stored notes retained)", id));
            json_response(StatusCode::OK, true, "Relay deleted (stored notes retained)")
        }
        Ok(_) => json_response(StatusCode::NOT_FOUND, false, "Relay not found"),
        Err(error) => json_response(StatusCode::INTERNAL_SERVER_ERROR, false, format!("Failed to delete relay: {}", error)),
    }
}

async fn delete_npub(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Response {
    if let Some(response) = require_admin(&headers) {
        return response;
    }

    let _ = sqlx::query("DELETE FROM npub_relay_sync_state WHERE npub_id = ?")
        .bind(id)
        .execute(&state.pool)
        .await;

    let result = sqlx::query("DELETE FROM monitored_npubs WHERE id = ?")
        .bind(id)
        .execute(&state.pool)
        .await;

    match result {
        Ok(done) if done.rows_affected() > 0 => {
            log_message(&format!("Deleted npub ID {} (archive events retained for future re-add)", id));
            json_response(StatusCode::OK, true, "Npub deleted (stored notes retained)")
        }
        Ok(_) => json_response(StatusCode::NOT_FOUND, false, "Npub not found"),
        Err(error) => json_response(StatusCode::INTERNAL_SERVER_ERROR, false, format!("Failed to delete npub: {}", error)),
    }
}

async fn sync_now(State(state): State<Arc<AppState>>, headers: HeaderMap) -> Response {
    if let Some(response) = require_admin(&headers) {
        return response;
    }

    tokio::spawn({
        let pool = state.pool.clone();
        async move { perform_sync(&pool).await }
    });

    json_response(StatusCode::OK, true, "Sync started in background — refresh relay stats in a few seconds")
}

async fn backup_data(State(state): State<Arc<AppState>>, headers: HeaderMap) -> Response {
    if let Some(response) = require_admin(&headers) {
        return response;
    }

    let relays = sqlx::query(
        "SELECT url, name, enabled, preloaded, last_sync_notes, last_synced, last_error FROM upstream_relays ORDER BY COALESCE(name, url)",
    )
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    let npubs = sqlx::query(
        "SELECT npub, label, pubkey_hex, last_synced FROM monitored_npubs ORDER BY COALESCE(label, npub)",
    )
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    let settings = sqlx::query("SELECT key, value FROM settings ORDER BY key")
        .fetch_all(&state.pool)
        .await
        .unwrap_or_default();

    let events = sqlx::query(
        "SELECT id, pubkey, kind, content, created_at, raw_json, source_relay, imported_at FROM events ORDER BY created_at DESC",
    )
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    let sync_state = sqlx::query(
        r#"
        SELECT n.npub, r.url AS relay_url, s.last_synced_unix, s.last_sync_notes, s.last_result, s.last_error, s.updated_at
        FROM npub_relay_sync_state s
        JOIN monitored_npubs n ON n.id = s.npub_id
        JOIN upstream_relays r ON r.id = s.relay_id
        ORDER BY n.npub, r.url
        "#,
    )
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    let mut ndjson = String::new();

    for row in relays {
        let json = serde_json::json!({
            "type": "relay",
            "url": row.get::<String, _>("url"),
            "name": row.get::<Option<String>, _>("name"),
            "enabled": row.get::<i64, _>("enabled") != 0,
            "preloaded": row.get::<i64, _>("preloaded") != 0,
            "last_sync_notes": row.get::<Option<i64>, _>("last_sync_notes").unwrap_or(0),
            "last_synced": row.get::<Option<String>, _>("last_synced"),
            "last_error": row.get::<Option<String>, _>("last_error")
        });
        ndjson.push_str(&format!("{}
", json));
    }

    for row in npubs {
        let json = serde_json::json!({
            "type": "npub",
            "npub": row.get::<String, _>("npub"),
            "label": row.get::<Option<String>, _>("label"),
            "pubkey_hex": row.get::<Option<String>, _>("pubkey_hex"),
            "last_synced": row.get::<Option<String>, _>("last_synced")
        });
        ndjson.push_str(&format!("{}
", json));
    }

    for row in settings {
        let json = serde_json::json!({
            "type": "setting",
            "key": row.get::<String, _>("key"),
            "value": row.get::<String, _>("value")
        });
        ndjson.push_str(&format!("{}
", json));
    }

    for row in events {
        let json = serde_json::json!({
            "type": "event",
            "id": row.get::<String, _>("id"),
            "pubkey": row.get::<String, _>("pubkey"),
            "kind": row.get::<i64, _>("kind"),
            "content": row.get::<String, _>("content"),
            "created_at": row.get::<i64, _>("created_at"),
            "raw_json": row.get::<Option<String>, _>("raw_json"),
            "source_relay": row.get::<Option<String>, _>("source_relay"),
            "imported_at": row.get::<Option<String>, _>("imported_at")
        });
        ndjson.push_str(&format!("{}
", json));
    }

    for row in sync_state {
        let json = serde_json::json!({
            "type": "sync_state",
            "npub": row.get::<String, _>("npub"),
            "relay_url": row.get::<String, _>("relay_url"),
            "last_synced_unix": row.get::<i64, _>("last_synced_unix"),
            "last_sync_notes": row.get::<i64, _>("last_sync_notes"),
            "last_result": row.get::<Option<String>, _>("last_result"),
            "last_error": row.get::<Option<String>, _>("last_error"),
            "updated_at": row.get::<Option<String>, _>("updated_at")
        });
        ndjson.push_str(&format!("{}
", json));
    }

    let mut headers = header::HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, header::HeaderValue::from_static("application/x-ndjson"));
    headers.insert(header::CONTENT_DISPOSITION, header::HeaderValue::from_static("attachment; filename="nostr-dashboard-backup.ndjson""));

    (headers, ndjson).into_response()
}

async fn restore_data(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<RestoreRequest>,
) -> Response {
    if let Some(response) = require_admin(&headers) {
        return response;
    }

    if payload.ndjson.as_bytes().len() > MAX_RESTORE_BYTES {
        return json_response(StatusCode::PAYLOAD_TOO_LARGE, false, format!("Restore rejected: payload exceeds {} MiB", MAX_RESTORE_BYTES / 1024 / 1024));
    }

    let mut records: Vec<serde_json::Value> = Vec::new();
    let mut invalid_lines: Vec<usize> = Vec::new();
    let mut unsupported_type_lines: Vec<usize> = Vec::new();

    for (index, line) in payload.ndjson.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        match serde_json::from_str::<serde_json::Value>(trimmed) {
            Ok(record) => {
                if let Some(record_type) = record.get("type").and_then(|value| value.as_str()) {
                    if !matches!(record_type, "relay" | "npub" | "setting" | "event" | "sync_state") {
                        unsupported_type_lines.push(index + 1);
                    }
                }
                records.push(record);
                if records.len() > MAX_RESTORE_RECORDS {
                    return json_response(StatusCode::PAYLOAD_TOO_LARGE, false, format!("Restore rejected: too many records (max {})", MAX_RESTORE_RECORDS));
                }
            }
            Err(_) => invalid_lines.push(index + 1),
        }
    }

    if !invalid_lines.is_empty() {
        return json_response(StatusCode::BAD_REQUEST, false, format!("Restore rejected: invalid NDJSON on line(s) {}", format_line_numbers(&invalid_lines)));
    }
    if !unsupported_type_lines.is_empty() {
        return json_response(StatusCode::BAD_REQUEST, false, format!("Restore rejected: unsupported record type on line(s) {}", format_line_numbers(&unsupported_type_lines)));
    }
    if records.is_empty() {
        return json_response(StatusCode::BAD_REQUEST, false, "Restore rejected: no valid records found");
    }

    let mut relays_imported = 0_i64;
    let mut npubs_imported = 0_i64;
    let mut settings_imported = 0_i64;
    let mut events_imported = 0_i64;
    let mut sync_states_imported = 0_i64;

    for record in records.iter().filter(|record| record["type"] == "relay") {
        let url = record["url"].as_str().unwrap_or("").trim();
        if url.is_empty() || !is_valid_relay_url(url) {
            continue;
        }
        let name = record["name"].as_str();
        let enabled = record["enabled"].as_bool().unwrap_or(true);
        let preloaded = record["preloaded"].as_bool().unwrap_or(false);
        let last_sync_notes = record["last_sync_notes"].as_i64().unwrap_or(0);
        let last_synced = record["last_synced"].as_str();
        let last_error = record["last_error"].as_str();

        let result = sqlx::query(
            r#"
            INSERT INTO upstream_relays
                (url, name, enabled, preloaded, last_sync_notes, last_synced, last_error)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(url) DO UPDATE SET
                name = excluded.name,
                enabled = excluded.enabled,
                preloaded = excluded.preloaded,
                last_sync_notes = excluded.last_sync_notes,
                last_synced = excluded.last_synced,
                last_error = excluded.last_error
            "#,
        )
        .bind(url)
        .bind(name)
        .bind(if enabled { 1 } else { 0 })
        .bind(if preloaded { 1 } else { 0 })
        .bind(last_sync_notes)
        .bind(last_synced)
        .bind(last_error)
        .execute(&state.pool)
        .await;
        if result.is_ok() { relays_imported += 1; }
    }

    for record in records.iter().filter(|record| record["type"] == "npub") {
        let npub = record["npub"].as_str().unwrap_or("").trim();
        if npub.is_empty() { continue; }
        let pubkey_hex = record["pubkey_hex"].as_str().map(|s| s.to_string()).or_else(|| PublicKey::from_bech32(npub).ok().map(|pk| pk.to_hex()));
        let result = sqlx::query(
            r#"
            INSERT INTO monitored_npubs
                (npub, label, pubkey_hex, last_synced)
            VALUES (?, ?, ?, ?)
            ON CONFLICT(npub) DO UPDATE SET
                label = excluded.label,
                pubkey_hex = excluded.pubkey_hex,
                last_synced = excluded.last_synced
            "#,
        )
        .bind(npub)
        .bind(record["label"].as_str())
        .bind(pubkey_hex)
        .bind(record["last_synced"].as_str())
        .execute(&state.pool)
        .await;
        if result.is_ok() { npubs_imported += 1; }
    }

    for record in records.iter().filter(|record| record["type"] == "setting") {
        let key = record["key"].as_str().unwrap_or("").trim();
        if key.is_empty() { continue; }
        let result = sqlx::query("INSERT OR REPLACE INTO settings (key, value) VALUES (?, ?)")
            .bind(key)
            .bind(record["value"].as_str().unwrap_or(""))
            .execute(&state.pool)
            .await;
        if result.is_ok() { settings_imported += 1; }
    }

    for record in records.iter().filter(|record| record.get("type").is_none() || record["type"] == "event") {
        let id = record["id"].as_str().unwrap_or("").trim();
        let pubkey = record["pubkey"].as_str().unwrap_or("").trim();
        if id.is_empty() || pubkey.is_empty() { continue; }
        let result = sqlx::query(
            r#"
            INSERT OR IGNORE INTO events
                (id, pubkey, kind, content, created_at, raw_json, source_relay, imported_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, COALESCE(?, datetime('now')))
            "#,
        )
        .bind(id)
        .bind(pubkey)
        .bind(record["kind"].as_i64().unwrap_or(1))
        .bind(record["content"].as_str().unwrap_or(""))
        .bind(record["created_at"].as_i64().unwrap_or(0))
        .bind(record["raw_json"].as_str())
        .bind(record["source_relay"].as_str())
        .bind(record["imported_at"].as_str())
        .execute(&state.pool)
        .await;
        if let Ok(done) = result { events_imported += done.rows_affected() as i64; }
    }

    for record in records.iter().filter(|record| record["type"] == "sync_state") {
        let npub = record["npub"].as_str().unwrap_or("").trim();
        let relay_url = record["relay_url"].as_str().unwrap_or("").trim();
        if npub.is_empty() || relay_url.is_empty() { continue; }
        let npub_id: Option<i64> = sqlx::query_scalar("SELECT id FROM monitored_npubs WHERE npub = ?")
            .bind(npub)
            .fetch_optional(&state.pool)
            .await
            .unwrap_or(None);
        let relay_id: Option<i64> = sqlx::query_scalar("SELECT id FROM upstream_relays WHERE url = ?")
            .bind(relay_url)
            .fetch_optional(&state.pool)
            .await
            .unwrap_or(None);
        let (Some(npub_id), Some(relay_id)) = (npub_id, relay_id) else { continue; };
        let result = sqlx::query(
            r#"
            INSERT INTO npub_relay_sync_state
                (npub_id, relay_id, last_synced_unix, last_sync_notes, last_result, last_error, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(npub_id, relay_id) DO UPDATE SET
                last_synced_unix = excluded.last_synced_unix,
                last_sync_notes = excluded.last_sync_notes,
                last_result = excluded.last_result,
                last_error = excluded.last_error,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(npub_id)
        .bind(relay_id)
        .bind(record["last_synced_unix"].as_i64().unwrap_or(0))
        .bind(record["last_sync_notes"].as_i64().unwrap_or(0))
        .bind(record["last_result"].as_str())
        .bind(record["last_error"].as_str())
        .bind(record["updated_at"].as_str())
        .execute(&state.pool)
        .await;
        if result.is_ok() { sync_states_imported += 1; }
    }

    let message = format!("Restore complete — {} relays, {} npubs, {} settings, {} events, {} sync states applied", relays_imported, npubs_imported, settings_imported, events_imported, sync_states_imported);
    log_message(&message);
    json_response(StatusCode::OK, true, message)
}

async fn download_logs(headers: HeaderMap) -> Response {
    if let Some(response) = require_admin(&headers) {
        return response;
    }

    let content = match fs::read_to_string("dashboard.log") {
        Ok(c) => c,
        Err(_) => "No logs yet.".to_string(),
    };

    let mut headers = header::HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, header::HeaderValue::from_static("text/plain"));
    headers.insert(header::CONTENT_DISPOSITION, header::HeaderValue::from_static("attachment; filename="dashboard.log""));

    (headers, content).into_response()
}

async fn restart_server(headers: HeaderMap) -> Response {
    if let Some(response) = require_admin(&headers) {
        return response;
    }

    log_message("Restart requested via dashboard — external supervisor must handle the actual restart");
    json_response(StatusCode::OK, true, "Restart request logged — use tmux/systemd/docker to restart the process")
}

#[tokio::main]
async fn main() {
    let db_path = std::env::var("DATABASE_PATH").unwrap_or_else(|_| "dashboard.db".to_string());
    let host = std::env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(8080);

    let connect_options = SqliteConnectOptions::new()
        .filename(&db_path)
        .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(connect_options)
        .await
        .expect("Failed to connect to DB");

    ensure_tables(&pool).await;

    let state = Arc::new(AppState { pool });

    let ip = host
        .parse::<IpAddr>()
        .unwrap_or(IpAddr::from([0, 0, 0, 0]));
    let addr = SocketAddr::new(ip, port);

    let app = Router::new()
        .route("/api/relays", get(get_relays).post(add_relay))
        .route("/api/relays/:id", delete(delete_relay))
        .route("/api/relays/:id/toggle", post(toggle_relay))
        .route("/api/npubs", get(get_npubs).post(add_npub))
        .route("/api/npubs/:id", delete(delete_npub))
        .route("/api/npubs/:id/events", get(get_events))
        .route("/api/sync", post(sync_now))
        .route("/api/backup", get(backup_data))
        .route("/api/restore", post(restore_data))
        .route("/api/logs", get(download_logs))
        .route("/api/restart", post(restart_server))
        .nest_service("/", ServeDir::new("public"))
        .with_state(state);

    log_message("🚀 Nostr Relay Dashboard v1.0.4 starting...");

    if configured_admin_token().is_some() {
        log_message("🔐 Admin token protection enabled for mutating and sensitive endpoints");
    } else {
        log_message("⚠️ NRD_ADMIN_TOKEN not set — admin protection is disabled");
    }

    let listener = match TcpListener::bind(&addr).await {
        Ok(listener) => listener,
        Err(error) if error.kind() == std::io::ErrorKind::AddrInUse => {
            log_message(&format!(
                "❌ Port {} is already in use. Another instance is likely running.",
                port
            ));
            std::process::exit(1);
        }
        Err(error) => {
            log_message(&format!("❌ Failed to bind to {}: {}", addr, error));
            std::process::exit(1);
        }
    };

    log_message(&format!("✅ Server listening on http://{}", addr));
    axum::serve(listener, app).await.expect("Server error");
}
