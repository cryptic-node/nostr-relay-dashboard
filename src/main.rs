mod api;
mod db;
mod gui;
mod nostr_types;
mod relay;

use std::{env, sync::Arc};

use axum::{
    extract::{State, WebSocketUpgrade},
    http::{header, HeaderMap, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::{delete, get, post},
    Router,
};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePool};
use tokio::sync::{broadcast, RwLock};
use tower_http::cors::{Any, CorsLayer};
use tracing::info;

pub struct AppState {
    pub db: db::Db,
    pub event_tx: broadcast::Sender<nostr_types::NostrEvent>,
    pub connection_count: RwLock<usize>,
    pub relay_name: String,
    pub relay_description: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "nostr_relay=info".parse().unwrap()),
        )
        .init();

    let db_path = env::var("DATABASE_PATH").unwrap_or_else(|_| "./data/relay.db".into());
    let relay_name = env::var("RELAY_NAME").unwrap_or_else(|_| "Nostr Relay".into());
    let relay_description = env::var("RELAY_DESCRIPTION")
        .unwrap_or_else(|_| "A simple Nostr relay with whitelist support".into());
    let host = env::var("HOST").unwrap_or_else(|_| "0.0.0.0".into());
    let port = env::var("PORT").unwrap_or_else(|_| "8080".into());

    // Ensure data directory exists
    if let Some(parent) = std::path::Path::new(&db_path).parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let opts = db_path
        .parse::<SqliteConnectOptions>()?
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal);

    let pool = SqlitePool::connect_with(opts).await?;
    db::run_migrations(&pool).await?;

    let (event_tx, _) = broadcast::channel(1024);

    let state = Arc::new(AppState {
        db: pool,
        event_tx,
        connection_count: RwLock::new(0),
        relay_name,
        relay_description,
    });

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/", get(root_handler))
        .route("/api/stats", get(api::get_stats))
        .route("/api/whitelist", get(api::get_whitelist))
        .route("/api/whitelist", post(api::add_to_whitelist))
        .route("/api/whitelist/:pubkey", delete(api::remove_from_whitelist))
        .layer(cors)
        .with_state(state);

    let addr = format!("{host}:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("Nostr relay listening on {addr}");
    axum::serve(listener, app).await?;

    Ok(())
}

async fn root_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    ws: Option<WebSocketUpgrade>,
) -> Response {
    // WebSocket upgrade for Nostr clients
    if let Some(ws) = ws {
        return ws.on_upgrade(move |socket| relay::handle_ws(socket, state));
    }

    // NIP-11: relay info document
    let accept = headers
        .get(header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if accept.contains("application/nostr+json") {
        let info = serde_json::json!({
            "name": state.relay_name,
            "description": state.relay_description,
            "pubkey": "",
            "contact": "",
            "supported_nips": [1, 2, 9, 11, 12, 15, 16, 20, 22],
            "software": "https://github.com/your-org/nostr-relay",
            "version": "0.1.0",
        });
        return (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/nostr+json")],
            info.to_string(),
        )
            .into_response();
    }

    // GUI
    Html(gui::INDEX_HTML).into_response()
}
