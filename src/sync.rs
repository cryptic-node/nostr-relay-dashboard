use nostr_sdk::{Client, Filter, Kind, ClientBuilder};
use nostr::PublicKey;
use sqlx::SqlitePool;
use std::str::FromStr;
use std::time::Duration;

pub async fn sync_npubs(pool: SqlitePool) -> Result<String, String> {
    let npubs: Vec<(String, Option<String>)> = sqlx::query_as(
        "SELECT npub, label FROM monitored_npubs"
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| e.to_string())?;

    if npubs.is_empty() {
        return Ok("No monitored npubs configured yet.".to_string());
    }

    let relays: Vec<String> = sqlx::query_scalar(
        "SELECT url FROM upstream_relays WHERE enabled = 1"
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| e.to_string())?;

    if relays.is_empty() {
        return Ok("No enabled upstream relays.".to_string());
    }

    println!("🔄 Starting sync for {} npubs from {} relays...", npubs.len(), relays.len());

    // Create client WITHOUT signer (we only read, no signing needed)
    let client = ClientBuilder::default().build();

    // Add all enabled relays
    for url in &relays {
        if let Err(e) = client.add_relay(url).await {
            println!("⚠️ Failed to add relay {}: {}", url, e);
        }
    }

    client.connect().await;

    let mut total_inserted = 0usize;

    for (npub_str, label) in npubs {
        let pubkey = match PublicKey::from_str(&npub_str) {
            Ok(pk) => pk,
            Err(_) => {
                println!("❌ Invalid npub: {}", npub_str);
                continue;
            }
        };

        let filter = Filter::new()
            .pubkey(pubkey)
            .kinds(vec![
                Kind::Metadata,
                Kind::TextNote,
                Kind::ContactList,
                Kind::Repost,
                Kind::Reaction,
                Kind::ZapReceipt,   // corrected from Zap
            ])
            .limit(300);

        // Use fetch_events (the correct method in current nostr-sdk)
        match client.fetch_events(filter, Duration::from_secs(15)).await {
            Ok(events) => {
                for event in events {
                    let inserted = sqlx::query(
                        "INSERT OR IGNORE INTO events 
                         (id, pubkey, kind, content, created_at, tags, sig) 
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
                    .await
                    .is_ok();

                    if inserted {
                        total_inserted += 1;
                    }
                }
                println!("✅ Fetched {} events for npub {}", events.len(), npub_str);
            }
            Err(e) => println!("⚠️ Error fetching for {}: {}", npub_str, e),
        }

        // Update last_synced timestamp
        let _ = sqlx::query("UPDATE monitored_npubs SET last_synced = CURRENT_TIMESTAMP WHERE npub = ?")
            .bind(&npub_str)
            .execute(&pool)
            .await;
    }

    Ok(format!(
        "✅ Sync complete! Inserted {} new events from {} npubs across {} relays.",
        total_inserted, npubs.len(), relays.len()
    ))
}
