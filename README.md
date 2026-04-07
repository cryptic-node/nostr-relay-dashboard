Nostr Relay Dashboard v1.0.1

Self-hosted Nostr event aggregator and personal backup dashboard. Pulls real Kind=1 text notes (signed by your monitored npubs) from configurable upstream relays, stores them locally, and gives you a clean, fast, mobile-friendly dark UI to browse, backup, and restore everything.

Live on main branch (stable production release): http://159.89.49.4:8080

Current Features (v1.0.1)

• Three-panel clean layout (exactly as locked in preferences):

• Left panel: Upstream relays (name/URL at top-left, “X notes pulled” + last-synced timestamp on right). Preloaded relays included (Damus, nos.lol, Primal, Nostr Wine, Snort, and Umbrel private relay at ws://100.72.15.19:4848).

• Middle panel: Monitored npubs (label top-left, truncated npub below, notes count + last-synced on right). Click any npub for purple highlight/ring. Add Npub section stacked at bottom.

• Right panel: Recent Kind=1 notes only for the selected npub. Human-readable previews, safe UTF-8 truncation with “...” for long notes. Panel is height-capped with a clean scrollbar so the page never becomes ridiculously long.

• Real Nostr pulling: Only Kind=1 text notes signed by your monitored npubs are fetched from enabled upstream relays using the official nostr-sdk.

• Sync options: Manual “Sync Now” button + automatic nightly sync (configurable via settings table).

• Full NDJSON backup & restore: One-click backup of all events. Restore accepts any valid NDJSON file with full validation and import count.

• Verbose logging: Real-time logs written to dashboard.log with timestamps. “Download Logs” button included.

• Restart server: One-click restart (works with your tmux setup).

• Safe & polished UX:

• No confirmation popups on delete (instant and clean).

• Green/grey color scheme with emerald accents locked in.

• Mobile-friendly responsive dark UI.

• Zero panics on UTF-8 notes.

• Status messages under buttons are exact and green.

• Database: SQLite (dashboard.db) with proper indexes and migrations handled automatically on first run.

• Production ready: Runs forever in tmux, port-bind safety, graceful error handling, and clean shutdown.

Bottom Control Bar (exactly as specified)

• Sync Now

• Backup (NDJSON)

• Restore

• Download Logs

• Restart Server

Quick Start (on your Droplet)

    git clone https://github.com/cryptic-node/nostr-relay-dashboard.git
    cd nostr-relay-dashboard
    git checkout main (for v1.0.1 stable)
    cargo build --release
    tmux new-session -d -s nostr-relay-dashboard './target/release/nostr-relay-dashboard'
    Open http://159.89.49.4:8080
    To update later: git pull origin main, kill tmux session, rebuild, and restart.
