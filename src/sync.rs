use nostr_sdk::{ClientBuilder, Filter, Kind};
use nostr::PublicKey;
use sqlx::SqlitePool;
use std::str::FromStr;
use std::time::Duration;

pub async fn sync_npubs(pool: SqlitePool) -> Result<String, String> {
    // Load monitored npubs
    let npubs: Vec<(String, Option<String>)> = sqlx::query_as(
        "SELECT npub, label FROM monitored_npubs"
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| format!("Failed to load npubs: {}", e))?;

    if npubs.is_empty() {
        return Ok("No monitored npubs configured yet. Add some in the dashboard!".to_string());
    }

    // Load enabled upstream relays
    let relays: Vec<String> = sqlx::query_scalar(
        "SELECT url FROM upstream_relays WHERE enabled = 1"
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| format!("Failed to load relays: {}", e))?;

    if relays.is_empty() {
        return Ok("No enabled upstream relays. Add some in the dashboard!".to_string());
    }

    println!("🔄 Starting sync — {} npubs from {} relays...", npubs.len(), relays.len());

    let client = ClientBuilder::default().build();

    // Add all relays
    for url in &relays {
        if let Err(e) = client.add_relay(url).await {
            println!("⚠️ Could not add relay {}: {}", url, e);
        }
    }

    if let Err(e) = client.connect().await {
        return Err(format!("Failed to connect to relays: {}", e));
    }

    let mut total_inserted = 0usize;

    for (npub_str, _label) in &npubs {
        let pubkey = match PublicKey::from_str(npub_str) {
            Ok(pk) => pk,
            Err(_) => {
                println!("❌ Skipping invalid npub: {}", npub_str);
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
                Kind::ZapReceipt,
            ])
            .limit(300);

        match client.fetch_events(filter, Duration::from_secs(25)).await {
            Ok(events) => {
                let mut inserted_count = 0;

                for event in &events.events {
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
                    .bind(event.sig.to_hex())           // better to use .to_hex() for consistency
                    .execute(&pool)
                    .await
                    .is_ok();

                    if inserted {
                        inserted_count += 1;
                        total_inserted += 1;
                    }
                }

                println!("✅ Fetched {} events for npub {} → {} new inserted", 
                         events.events.len(), npub_str, inserted_count);
            }
            Err(e) => {
                println!("⚠️ Fetch error for {}: {}", npub_str, e);
            }
        }

        // Update last sync timestamp
        let _ = sqlx::query(
            "UPDATE monitored_npubs SET last_synced = CURRENT_TIMESTAMP WHERE npub = ?"
        )
        .bind(npub_str)
        .execute(&pool)
        .await;
    }

    Ok(format!(
        "🎉 Sync finished! Inserted {} new events from {} npubs across {} relays.",
        total_inserted, npubs.len(), relays.len()
    ))
}
