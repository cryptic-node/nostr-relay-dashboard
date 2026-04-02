use anyhow::Result;
use sqlx::{sqlite::SqliteRow, Pool, Row, Sqlite};

use crate::nostr_types::{Filter, NostrEvent};

pub type Db = Pool<Sqlite>;

pub async fn run_migrations(db: &Db) -> Result<()> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS events (
            id          TEXT PRIMARY KEY,
            pubkey      TEXT NOT NULL,
            created_at  INTEGER NOT NULL,
            kind        INTEGER NOT NULL,
            tags        TEXT NOT NULL,
            content     TEXT NOT NULL,
            sig         TEXT NOT NULL
        )",
    )
    .execute(db)
    .await?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS events_pubkey ON events(pubkey)",
    )
    .execute(db)
    .await?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS events_kind ON events(kind)",
    )
    .execute(db)
    .await?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS events_created_at ON events(created_at)",
    )
    .execute(db)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS whitelist (
            pubkey      TEXT PRIMARY KEY,
            note        TEXT,
            added_at    INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
        )",
    )
    .execute(db)
    .await?;

    Ok(())
}

fn row_to_event(row: &SqliteRow) -> NostrEvent {
    let tags_str: String = row.get("tags");
    NostrEvent {
        id: row.get("id"),
        pubkey: row.get("pubkey"),
        created_at: row.get("created_at"),
        kind: row.get::<i64, _>("kind") as u64,
        tags: serde_json::from_str(&tags_str).unwrap_or_default(),
        content: row.get("content"),
        sig: row.get("sig"),
    }
}

pub async fn insert_event(db: &Db, event: &NostrEvent) -> Result<bool> {
    let tags_json = serde_json::to_string(&event.tags)?;
    let res = sqlx::query(
        "INSERT OR IGNORE INTO events (id, pubkey, created_at, kind, tags, content, sig)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&event.id)
    .bind(&event.pubkey)
    .bind(event.created_at)
    .bind(event.kind as i64)
    .bind(&tags_json)
    .bind(&event.content)
    .bind(&event.sig)
    .execute(db)
    .await?;
    Ok(res.rows_affected() > 0)
}

pub async fn query_events(db: &Db, filters: &[Filter]) -> Result<Vec<NostrEvent>> {
    let mut all: Vec<NostrEvent> = sqlx::query(
        "SELECT id, pubkey, created_at, kind, tags, content, sig
         FROM events ORDER BY created_at DESC LIMIT 5000",
    )
    .map(|row: SqliteRow| row_to_event(&row))
    .fetch_all(db)
    .await?;

    all.retain(|e| filters.iter().any(|f| f.matches(e)));

    if let Some(limit) = filters.iter().filter_map(|f| f.limit).min() {
        all.truncate(limit);
    }
    Ok(all)
}

pub async fn count_events(db: &Db) -> Result<i64> {
    let row = sqlx::query("SELECT COUNT(*) as cnt FROM events")
        .fetch_one(db)
        .await?;
    Ok(row.get::<i64, _>("cnt"))
}

pub async fn is_whitelisted(db: &Db, pubkey: &str) -> Result<bool> {
    let row = sqlx::query("SELECT COUNT(*) as cnt FROM whitelist WHERE pubkey = ?")
        .bind(pubkey)
        .fetch_one(db)
        .await?;
    Ok(row.get::<i64, _>("cnt") > 0)
}

pub async fn whitelist_is_empty(db: &Db) -> Result<bool> {
    let row = sqlx::query("SELECT COUNT(*) as cnt FROM whitelist")
        .fetch_one(db)
        .await?;
    Ok(row.get::<i64, _>("cnt") == 0)
}

pub async fn list_whitelist(db: &Db) -> Result<Vec<(String, Option<String>, i64)>> {
    let rows = sqlx::query(
        "SELECT pubkey, note, added_at FROM whitelist ORDER BY added_at DESC",
    )
    .map(|row: SqliteRow| {
        (
            row.get::<String, _>("pubkey"),
            row.get::<Option<String>, _>("note"),
            row.get::<i64, _>("added_at"),
        )
    })
    .fetch_all(db)
    .await?;
    Ok(rows)
}

pub async fn add_to_whitelist(db: &Db, pubkey: &str, note: Option<&str>) -> Result<bool> {
    let res = sqlx::query(
        "INSERT OR IGNORE INTO whitelist (pubkey, note) VALUES (?, ?)",
    )
    .bind(pubkey)
    .bind(note)
    .execute(db)
    .await?;
    Ok(res.rows_affected() > 0)
}

pub async fn remove_from_whitelist(db: &Db, pubkey: &str) -> Result<bool> {
    let res = sqlx::query("DELETE FROM whitelist WHERE pubkey = ?")
        .bind(pubkey)
        .execute(db)
        .await?;
    Ok(res.rows_affected() > 0)
}
