use nostr_sdk::{Client, Filter, Kind, RelayPool, Event};
use nostr::PublicKey;
use sqlx::SqlitePool;
use std::str::FromStr;
use tokio::time::{sleep, Duration};

pub async fn sync_npubs(pool: SqlitePool) -> Result<String, String> {
    let npubs: Vec<(String, Option<String>)> = sqlx::query_as("SELECT npub, label FROM monitored_npubs")
        .fetch_all(&pool)
        .await
        .map_err(|e| e.to_string())?;

    if npubs.is_empty() {
        return Ok("No npubs to sync.".to_string());
    }

    let relays: Vec<String> = sqlx::query_scalar("SELECT url FROM upstream_relays WHERE enabled = true")
        .fetch_all(&pool)
        .await
        .map_err(|e| e.to_string())?;

    if relays.is_empty() {
        return Ok("No upstream relays configured.".to_string());
    }

    let client = Client::new(RelayPool::new());
    for relay_url in &relays {
        client.add_relay(relay_url).await.map_err(|e| e.to_string())?;
    }
    client.connect().await;

    let mut total_events = 0;

    for (npub_str, label) in npubs {
        let pubkey = PublicKey::from_str(&npub_str).map_err(|e| e.to_string())?;
        let filter = Filter::new()
            .pubkey(pubkey)
            .kinds(vec![Kind::TextNote, Kind::Metadata, Kind::ContactList, Kind::Repost, Kind::Reaction /* add more as needed */])
            .limit(500);  // Reasonable batch size; increase or paginate if needed

        let events = client.get_events_of(vec![filter], None).await.map_err(|e| e.to_string())?;

        for event in events {
            // Insert into your existing events table (adjust table/column names to match your relay DB)
            let result = sqlx::query(
                "INSERT OR IGNORE INTO events (id, pubkey, kind, content, created_at, tags, sig) 
                 VALUES (?, ?, ?, ?, ?, ?, ?)"
            )
            .bind(event.id.to_hex())
            .bind(event.pubkey.to_hex())
            .bind(event.kind.as_u64() as i64)
            .bind(&event.content)
            .bind(event.created_at.as_i64())
            .bind(serde_json::to_string(&event.tags).unwrap_or_default())
            .bind(event.sig.to_hex())
            .execute(&pool)
            .await;

            if result.is_ok() {
                total_events += 1;
            }
        }

        // Update last_synced
        sqlx::query("UPDATE monitored_npubs SET last_synced = CURRENT_TIMESTAMP WHERE npub = ?")
            .bind(&npub_str)
            .execute(&pool)
            .await
            .ok();
    }

    Ok(format!("Synced {} events from {} npubs across {} relays.", total_events, npubs.len(), relays.len()))
}

// Optional: background task (add to main later)
pub async fn start_background_sync(pool: SqlitePool) {
    loop {
        if let Err(e) = sync_npubs(pool.clone()).await {
            tracing::error!("Background sync failed: {}", e);
        }
        sleep(Duration::from_secs(3600)).await; // Every hour
    }
}
