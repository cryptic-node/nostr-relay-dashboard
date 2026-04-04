use axum::{routing::{get, post}, Router, Json, extract::{State, Query}};
use sqlx::{SqlitePool, Row};
use tower_http::services::ServeDir;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tracing_subscriber;
use std::collections::HashMap;

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
    let npub = match params.get("npub") {
        Some(n) => n,
        None => return Json(vec![]),
    };

    let events = sqlx::query(
        "SELECT id, kind, content, created_at FROM events 
         WHERE pubkey = ? 
         ORDER BY created_at DESC LIMIT 10"
    )
    .bind(npub)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    let previews: Vec<EventPreview> = events.into_iter().map(|row| {
        let kind = row.get::<i64, _>("kind") as u16;
        let kind_name = match kind {
            0 => "Profile",
            1 => "Text",
            3 => "Contacts",
            6 => "Repost",
            7 => "Reaction",
            9735 => "Zap",
            _ => "Event",
        }.to_string();

        EventPreview {
            id: row.get::<String, _>("id"),
            kind,
            kind_name,
            content: row.get::<String, _>("content").chars().take(120).collect::<String>() + if row.get::<String, _>("content").len() > 120 { "…" } else { "" },
            created_at: row.get::<String, _>("created_at"),
        }
    }).collect();

    Json(previews)
}

// ... (the rest of your main.rs stays exactly the same until the router)

async fn get_relays(...) { ... }   // keep your existing functions
async fn add_relay(...) { ... }
async fn get_npubs(...) { ... }
async fn add_npub(...) { ... }
async fn trigger_sync(...) { ... }

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

    println!("✅ Database connected and migrations applied.");

    let app = Router::new()
        .route("/api/relays", get(get_relays).post(add_relay))
        .route("/api/npubs", get(get_npubs).post(add_npub))
        .route("/api/sync", post(trigger_sync))
        .route("/api/events", get(get_events))           // ← NEW
        .nest_service("/", ServeDir::new("public"))
        .with_state(pool);

    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    println!("🚀 Nostr Relay Dashboard running on http://0.0.0.0:8080");

    let listener = TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app.into_make_service())
        .await
        .unwrap();
}
