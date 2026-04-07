# Nostr Relay Dashboard

**Personal backup & aggregator • Version 1.0.2**  
**Dev Team:** cryptic-node & SuperGrok • Powered by Rust

A clean, self-hosted Nostr relay dashboard that lets you pull and store events from upstream relays for any number of npubs. Built as a passion project to improve on the Umbrel three-panel layout with manual relay control, multi-npub support, readable event previews, backup/restore, and more.

## Features (v1.0.2)

- Real note sync and Kind 1 notes displayed on npub selection.
- **Three-panel layout** – Upstream relays (left), monitored npubs (center), recent events (right)
- **Human-readable events** – Text notes show actual content; contacts show “Updated contact list (X following)”
- **Full backup & restore** – Everything (relays, npubs, settings, events) saved as NDJSON
- **Nightly auto-sync** – Toggleable midnight sync (00:00 local time)
- **Manual sync button** – Pull fresh events anytime
- **Download Logs** – One-click server log export for debugging
- **Restart Server** – One-click graceful restart request
- **SQLite persistence** – All data survives restarts and upgrades
- **Mobile-friendly dark UI** with Tailwind + Font Awesome

## Quick Start

1. Clone the repo:
   ```bash
   git clone https://github.com/cryptic-node/nostr-relay-dashboard.git develop
   cd nostr-relay-dashboard

2. Build & run:
cargo run
Open http://your-server-ip:8080
Add relays and npubs → hit Sync Now → enjoy your personal Nostr archive.

Version 2.0 Goals (Roadmap)

Custom sync schedule (choose any time instead of only midnight)
Infinite scroll / pagination for the Recent Events pane
Search & filter events by kind, date, or keyword
Profile metadata preview (show name/avatar for npubs)
Zap & reaction summaries
Optional Lightning address / NIP-57 zap support
Docker + Umbrel app manifest for one-click install
Optional public read-only mode

Credits
Huge thanks to SuperGrok for the tireless Rust guidance, error fixing, and feature brainstorming that made this possible.
Built with passion by cryptic-node — because the wheel sometimes deserves a fresh set of rims.

Star the repo if you find it useful!
Feedback, issues, and pull requests are always welcome.

cryptic-node npub: npub1axr49qkexxmcm0g2tac3uawzmk59gaupsgy5fw5sfuscumq79h9qjh47gn
