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
struct AddNpubRequest { npub: String; label: Option<String>; }

#[derive(Deserialize)]
struct AddRelayRequest { url: String; name: Option<String>; }

#[derive(Serialize)]
struct ApiResponse { success: bool; message: String; }

#[derive(Serialize)]
struct EventPreview {
    id: String, kind: u16, kind_name: String,
    content: String, created_at: String,
}

async fn get_events(Query(params): Query<HashMap<String, String>>, State(pool): State<SqlitePool>) -> Json<Vec<EventPreview>> {
    let npub_str = match params.get("npub") { Some(n) => n, None => return Json(vec![]), };
    let pubkey = match PublicKey::parse(npub_str) { Ok(pk) => pk, Err(_) => return Json(vec![]), };
    let pubkey_hex = pubkey.to_hex();

    let events = sqlx::query("SELECT id, kind, content, created_at FROM events WHERE pubkey = ? ORDER BY created_at DESC LIMIT 10")
        .bind(pubkey_hex)
        .fetch_all(&pool)
        .await
        .unwrap_or_default();

    let previews: Vec<EventPreview> = events.into_iter().map(|row| {
        let kind = row.get::<i64, _>("kind") as u16;
        let kind_name = match kind { 0 => "Profile", 1 => "Text", 3 => "Contacts", 6 => "Repost", 7 => "Reaction", 9735 => "Zap", _ => "Event" }.to_string();
        EventPreview {
            id: row.get::<String, _>("id"),
            kind,
            kind_name,
            content: { let c = row.get::<String, _>("content"); if c.len() > 120 { c.chars().take(120).collect::<String>() + "…" } else { c } },
            created_at: row.get::<Option<i64>, _>("created_at").map_or("".to_string(), |v| v.to_string()),
        }
    }).collect();

    Json(previews)
}

// ... (the rest of the file is the same as the last main.rs I gave you — get_relays, add_relay, delete_relay, get_npubs, add_npub, delete_npub, trigger_sync, main() )

// (I'm keeping the full file short here to save space — paste the entire thing from my previous message if you need it, or tell me and I'll repaste the whole thing.)

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    let pool = SqlitePool::connect("sqlite:nostr_relay.db?mode=rwc").await.expect("Failed to connect");
    sqlx::migrate!().run(&pool).await.expect("Migrations failed");

    // Seed preloaded relays (only once)
    let _ = sqlx::query("INSERT OR IGNORE INTO upstream_relays (url, name, enabled, preloaded) VALUES 
        ('wss://relay.damus.io', 'Damus', 1, 1),
        ('wss://nos.lol', 'nos.lol', 1, 1),
        ('wss://nostr.wine', 'Nostr Wine', 1, 1),
        ('wss://relay.snort.social', 'Snort', 1, 1),
        ('wss://nostr.mutinywallet.com', 'Mutiny', 1, 1)")
        .execute(&pool).await;

    println!("✅ Database ready (preloaded relays seeded)");

    let app = Router::new()
        .route("/api/relays", get(get_relays).post(add_relay))
        .route("/api/relays/:id", delete(delete_relay))
        .route("/api/npubs", get(get_npubs).post(add_npub))
        .route("/api/npubs/:id", delete(delete_npub))
        .route("/api/sync", post(trigger_sync))
        .route("/api/events", get(get_events))
        .nest_service("/", ServeDir::new("public"))
        .with_state(pool);

    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    println!("🚀 Dashboard running on http://0.0.0.0:8080");

    let listener = TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app.into_make_service()).await.unwrap();
}
