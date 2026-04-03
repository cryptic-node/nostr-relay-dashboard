use std::{collections::HashMap, sync::Arc};

use axum::extract::ws::{Message, WebSocket};
use futures::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::sync::{broadcast, RwLock};
use tracing::{debug, info, warn};

use crate::{
    db,
    nostr_types::{Filter, NostrEvent},
    AppState,
};

type SubMap = Arc<RwLock<HashMap<String, Vec<Filter>>>>;

pub async fn handle_ws(socket: WebSocket, state: Arc<AppState>) {
    let (mut sender, mut receiver) = socket.split();
    let mut event_rx = state.event_tx.subscribe();

    {
        let mut count = state.connection_count.write().await;
        *count += 1;
    }

    let subs: SubMap = Arc::new(RwLock::new(HashMap::new()));
    let subs_recv = subs.clone();
    let subs_bcast = subs.clone();

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    let tx_recv = tx.clone();
    let tx_bcast = tx.clone();

    // Forward outgoing messages to the WebSocket sink
    let forward_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if sender.send(Message::Text(msg)).await.is_err() {
                break;
            }
        }
    });

    // Handle incoming messages from the client
    let state_recv = state.clone();
    let recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            let text = match msg {
                Message::Text(t) => t,
                Message::Close(_) => break,
                _ => continue,
            };

            debug!("recv: {}", text);

            let Ok(parsed) = serde_json::from_str::<Value>(&text) else {
                let _ = tx_recv
                    .send(json!(["NOTICE", "error: invalid JSON"]).to_string());
                continue;
            };

            let Some(arr) = parsed.as_array() else {
                let _ = tx_recv
                    .send(json!(["NOTICE", "error: expected array"]).to_string());
                continue;
            };

            let Some(msg_type) = arr.first().and_then(|v| v.as_str()) else {
                continue;
            };

            match msg_type {
                "EVENT" => {
                    let Some(event_val) = arr.get(1) else { continue };
                    let Ok(event) =
                        serde_json::from_value::<NostrEvent>(event_val.clone())
                    else {
                        let _ = tx_recv.send(
                            json!(["NOTICE", "error: invalid event"]).to_string(),
                        );
                        continue;
                    };

                    if !event.verify_id() {
                        let _ = tx_recv.send(
                            json!(["OK", event.id, false, "invalid: bad event id"])
                                .to_string(),
                        );
                        continue;
                    }

                    if !event.verify_sig() {
                        let _ = tx_recv.send(
                            json!(["OK", event.id, false, "invalid: bad signature"])
                                .to_string(),
                        );
                        continue;
                    }

                    let open_relay =
                        db::whitelist_is_empty(&state_recv.db).await.unwrap_or(true);
                    let allowed = open_relay
                        || db::is_whitelisted(&state_recv.db, &event.pubkey)
                            .await
                            .unwrap_or(false);

                    if !allowed {
                        let _ = tx_recv.send(
                            json!(["OK", event.id, false, "blocked: pubkey not whitelisted"])
                                .to_string(),
                        );
                        continue;
                    }

                    match db::insert_event(&state_recv.db, &event).await {
                        Ok(true) => {
                            let _ = tx_recv.send(
                                json!(["OK", event.id, true, ""]).to_string(),
                            );
                            let _ = state_recv.event_tx.send(event);
                        }
                        Ok(false) => {
                            let _ = tx_recv.send(
                                json!(["OK", event.id, true, "duplicate: already have this event"])
                                    .to_string(),
                            );
                        }
                        Err(e) => {
                            warn!("db insert error: {e}");
                            let _ = tx_recv.send(
                                json!(["OK", event.id, false, "error: database error"])
                                    .to_string(),
                            );
                        }
                    }
                }

                "REQ" => {
                    let Some(sub_id) = arr.get(1).and_then(|v| v.as_str()) else {
                        continue;
                    };
                    let sub_id = sub_id.to_string();

                    let filters: Vec<Filter> = arr[2..]
                        .iter()
                        .filter_map(|v| serde_json::from_value(v.clone()).ok())
                        .collect();

                    {
                        let mut map = subs_recv.write().await;
                        map.insert(sub_id.clone(), filters.clone());
                    }

                    match db::query_events(&state_recv.db, &filters).await {
                        Ok(events) => {
                            for event in events {
                                let _ = tx_recv.send(
                                    json!(["EVENT", sub_id, event]).to_string(),
                                );
                            }
                        }
                        Err(e) => warn!("query error: {e}"),
                    }

                    let _ = tx_recv.send(json!(["EOSE", sub_id]).to_string());
                }

                "CLOSE" => {
                    let Some(sub_id) = arr.get(1).and_then(|v| v.as_str()) else {
                        continue;
                    };
                    let mut map = subs_recv.write().await;
                    map.remove(sub_id);
                }

                _ => {
                    let _ = tx_recv.send(
                        json!(["NOTICE", format!("unknown message type: {msg_type}")])
                            .to_string(),
                    );
                }
            }
        }
    });

    // Broadcast incoming events to matching subscribers
    let bcast_task = tokio::spawn(async move {
        while let Ok(event) = event_rx.recv().await {
            let map = subs_bcast.read().await;
            for (sub_id, filters) in map.iter() {
                if filters.iter().any(|f| f.matches(&event)) {
                    let _ =
                        tx_bcast.send(json!(["EVENT", sub_id, event]).to_string());
                }
            }
        }
    });

    tokio::select! {
        _ = recv_task => {}
        _ = bcast_task => {}
    }

    forward_task.abort();

    {
        let mut count = state.connection_count.write().await;
        *count = count.saturating_sub(1);
    }

    info!("client disconnected");
}
