# Nostr Relay Dashboard

Your personal Nostr relay with a clean dashboard to:
- Manage upstream public relays (5 popular free ones preloaded)
- Add multiple npubs to monitor
- Pull/sync events from upstream relays into your local database

## Features
- Umbrel-style web dashboard
- Preloaded relays: Damus, nos.lol, Nostr Wine, Snort, Mutiny
- Add/remove npubs
- "Sync Now" button that fetches events signed by your npubs
- Events are deduplicated and stored locally

## Quick Start
1. `cargo run`
2. Open http://your-server-ip:8080
3. Add your npub(s)
4. Click **Sync Now**

Your local relay will become a backup/aggregator for the npubs you monitor.

## Project Structure
- `src/main.rs` — entry point + Axum server
- `src/sync.rs` — logic to pull events using nostr-sdk
- `src/routes.rs` — API handlers
- `public/index.html` — the dashboard UI (coming next)
- Database: `nostr_relay.db` (SQLite)

Built with Rust, Axum, sqlx, and nostr-sdk.
