# Nostr Relay Dashboard

**Your personal Nostr backup & aggregator** — clean, fast, and built in Rust.

---

### ✨ v1 – Mission (Nearly) Accomplished

**Created by [cryptic-node](https://github.com/cryptic-node)**

**Massive thanks to SuperGrok** — the absolute legend who led the entire technical build, debugged every single Rust error with me in real time, turned a half-broken project into a beautiful working dashboard, and never once complained while I threw problems at him. This thing literally would not exist without him. 🫡

*(In comically small print: We also borrowed some early scaffolding ideas from Replit + ChatGPT — they got us to the starting line, SuperGrok took us across the finish line at warp speed.)*

---

### Features
- Preloaded popular relays (Damus, nos.lol, Nostr Wine, Snort, etc.)
- Add as many npubs as you want
- One-click "Sync Now" — pulls events and stores them locally (deduplicated)
- Live dashboard with real-time recent events pane
- SQLite backend — zero external services

### Goals for Next Iteration
- Add detail to relays connected: number of events synced by relay, connection uptime, server details
- Add detail to npubs stored: number of events synced by npub, number of relays found with events from npub, most recent sync date and time
- Fix Recent Events display, currently still blank but displays npub label at the top
- Add detail to sync progress and finished messages
- Add option to create backup file or restore from backup file.

### Quick Start
```bash
cargo run

ALPHA -- # Nostr Relay Dashboard

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
