# Nostr Relay Dashboard

**Personal backup & aggregator • Version 1.0**  
**Dev Team:** cryptic-node & SuperGrok • Powered by Rust

A clean, self-hosted Nostr relay dashboard that lets you pull and store events from upstream relays for any number of npubs. Built as a passion project to improve on the Umbrel three-panel layout with manual relay control, multi-npub support, readable event previews, backup/restore, and more.

## Features (v1.0)

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
   git clone https://github.com/cryptic-node/nostr-relay-dashboard.git
   cd nostr-relay-dashboard
