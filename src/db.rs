use sqlx::SqlitePool;

pub async fn init_pool() -> SqlitePool {
    let pool = SqlitePool::connect("sqlite:nostr_relay.db?mode=rwc")
        .await
        .expect("Failed to connect to SQLite database");

    println!("✅ Database connected: nostr_relay.db");
    pool
}