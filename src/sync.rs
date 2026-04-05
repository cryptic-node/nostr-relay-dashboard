use nostr_sdk::{ClientBuilder, Filter, Kind};
use nostr_sdk::nostr::PublicKey;
use sqlx::SqlitePool;
use std::time::Duration;

pub async fn sync_npubs(pool: SqlitePool) -> Result<String, String> {
    let npubs: Vec<(String, Option<String>)> = sqlx::query_as(
        "SELECT npub, label FROM monitored_npubs"
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| format!("Failed to load npubs: {}", e))?;

    if npubs.is_empty() {
        return Ok("No monitored npubs configured yet. Add some in the dashboard!".to_string());
    }

    let relays: Vec<(i64, String)> = sqlx::query_as(
        "SELECT id, url FROM upstream_relays WHERE enabled = 1"
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| format!("Failed to load relays: {}", e))?;

    if relays.is_empty() {
        return Ok("No enabled upstream relays. Add some in the dashboard!".to_string());
    }

    let num_relays = relays.len();

    println!("🔄 Starting sync — {} npubs from {} relays...", npubs.len(), num_relays);

    let mut total_inserted = 0usize;

    for (relay_id, url) in relays {
        println!("   Trying relay: {}", url);

        let client = ClientBuilder::default().build();
        if let Err(e) = client.add_relay(&url).await {
            println!("   ❌ Failed to add {}: {}", url, e);
            continue;
        }

        client.connect().await;

        let mut relay_inserted = 0usize;

        for (npub_str, _label) in &npubs {
            let pubkey = match PublicKey::from_str(npub_str) {
                Ok(pk) => pk,
                Err(_) => continue,
            };

            let filter = Filter::new()
                .pubkey(pubkey)
                .kinds(vec![
                    Kind::Metadata,
                    Kind::TextNote,
                    Kind::ContactList,
                    Kind::Repost,
                    Kind::Reaction,
                    Kind::ZapReceipt,
                ])
                .limit(300);

            match client.fetch_events(filter, Duration::from_secs(20)).await {
                Ok(events) => {
                    let num_fetched = events.len();

                    for event in events {
                        let inserted = sqlx::query(
                            "INSERT OR IGNORE INTO events 
                             (id, pubkey, kind, content, created_at, tags, sig)
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
                        .await
                        .is_ok();

                        if inserted {
                            total_inserted += 1;
                            relay_inserted += 1;
                        }
                    }

                    if num_fetched > 0 {
                        println!("   ✅ {} events from this relay for npub {}", num_fetched, npub_str);
                    }
                }
                Err(_) => {}
            }
        }

        let _ = sqlx::query(
            "UPDATE upstream_relays 
             SET last_sync_notes = ?, last_synced = CURRENT_TIMESTAMP 
             WHERE id = ?"
        )
        .bind(relay_inserted as i64)
        .bind(relay_id)
        .execute(&pool)
        .await;
    }

    let msg = format!("🎉 Sync finished! Inserted {} new events from {} npubs across {} relays.", 
                      total_inserted, npubs.len(), num_relays);
    println!("{}", msg);
    Ok(msg)
}
