use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{
    db,
    nostr_types::{hex_to_npub, npub_to_hex},
    AppState,
};

pub async fn get_stats(State(state): State<Arc<AppState>>) -> Json<Value> {
    let total_events = db::count_events(&state.db).await.unwrap_or(0);
    let active_connections = *state.connection_count.read().await;
    let whitelist = db::list_whitelist(&state.db).await.unwrap_or_default();
    Json(json!({
        "total_events": total_events,
        "active_connections": active_connections,
        "whitelist_count": whitelist.len(),
        "relay_name": state.relay_name,
        "relay_description": state.relay_description,
    }))
}

pub async fn get_whitelist(State(state): State<Arc<AppState>>) -> Json<Value> {
    let entries = db::list_whitelist(&state.db).await.unwrap_or_default();
    let items: Vec<Value> = entries
        .into_iter()
        .map(|(pubkey, note, added_at)| {
            let npub = hex_to_npub(&pubkey).unwrap_or_else(|_| pubkey.clone());
            json!({
                "pubkey": pubkey,
                "npub": npub,
                "note": note,
                "added_at": added_at,
            })
        })
        .collect();
    let total = items.len();
    Json(json!({ "entries": items, "total": total }))
}

#[derive(Deserialize)]
pub struct AddRequest {
    pub npub: Option<String>,
    pub pubkey: Option<String>,
    pub note: Option<String>,
}

pub async fn add_to_whitelist(
    State(state): State<Arc<AppState>>,
    Json(body): Json<AddRequest>,
) -> (StatusCode, Json<Value>) {
    let hex_key = if let Some(npub) = &body.npub {
        match npub_to_hex(npub) {
            Ok(k) => k,
            Err(e) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({ "error": format!("Invalid npub: {e}") })),
                );
            }
        }
    } else if let Some(pk) = &body.pubkey {
        if pk.len() != 64 || hex::decode(pk).is_err() {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "Invalid pubkey: must be 64-char hex" })),
            );
        }
        pk.to_lowercase()
    } else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Provide npub or pubkey" })),
        );
    };

    match db::add_to_whitelist(&state.db, &hex_key, body.note.as_deref()).await {
        Ok(true) => {
            let npub = hex_to_npub(&hex_key).unwrap_or_else(|_| hex_key.clone());
            (
                StatusCode::CREATED,
                Json(json!({ "pubkey": hex_key, "npub": npub, "note": body.note })),
            )
        }
        Ok(false) => (
            StatusCode::CONFLICT,
            Json(json!({ "error": "Already whitelisted" })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        ),
    }
}

pub async fn remove_from_whitelist(
    State(state): State<Arc<AppState>>,
    Path(pubkey): Path<String>,
) -> (StatusCode, Json<Value>) {
    match db::remove_from_whitelist(&state.db, &pubkey).await {
        Ok(true) => (StatusCode::OK, Json(json!({ "message": "removed" }))),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "Not found" })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        ),
    }
}

