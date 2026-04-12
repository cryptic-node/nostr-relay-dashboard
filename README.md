# Nostr Relay Dashboard

**Personal backup & aggregator • Version 1.0.3**  
**Dev Team:** cryptic-node, ChatGPT Pro & SuperGrok • Proposed release bundle refined by ChatGPT Pro • Powered by Rust

A clean, self-hosted Nostr relay dashboard that lets you pull and store events from upstream relays for any number of npubs. This release keeps the existing three-panel feel, but fixes the parts that matter most for a trustworthy personal archive: real per-relay sync stats, safer frontend rendering, fuller backups, and a working Docker image.

## What changed in v1.0.3

- **Incremental sync checkpoints** per npub + relay pair, with a small overlap window to avoid missing edge-of-sync events
- **Truthful relay stats** – each relay now reports what it actually stored on the last run instead of sharing one global total
- **Full NDJSON backup / restore** – relays, npubs, settings, sync state, and events are all included
- **Frontend hardening** – event previews, labels, and relay names are escaped before rendering
- **Relay controls in the UI** – enable / disable relays and delete relays / npubs without leaving the dashboard
- **Dockerfile fixed** – correct binary name, static assets copied into the image, persistent database path supported
- **Removed hardcoded private relay preload** from the public default seed list

## Features

- **Three-panel layout** – Upstream relays (left), monitored npubs (center), recent events (right)
- **Manual sync button** – Pull fresh events anytime
- **Multi-npub support**
- **Human-readable note previews**
- **SQLite persistence**
- **Downloadable NDJSON backups**
- **Download Logs**
- **Restart Server** placeholder hook for tmux/systemd-driven restarts
- **Mobile-friendly dark UI** with Tailwind + Font Awesome

## Quick Start

### Local

```bash
git clone -b develop https://github.com/cryptic-node/nostr-relay-dashboard.git
cd nostr-relay-dashboard
cargo run
```

Then open:

```text
http://your-server-ip:8080
```

### Docker

```bash
docker build -t nostr-relay-dashboard:1.0.3 .
docker run -p 8080:8080 -v nrd-data:/app/data nostr-relay-dashboard:1.0.3
```

## Backup format

Backups are NDJSON and now include typed records for:

- relays
- npubs
- settings
- events
- sync_state

Legacy event-only NDJSON backups are still accepted on restore.

## Next up after this release

- HTTP auth / admin mode
- Search and pagination
- Profile metadata previews
- Zap / reaction summaries
- Real scheduler configuration instead of fixed/manual sync behavior
- Docker + Umbrel polish

## Credits

Huge thanks to SuperGrok for the tireless Rust guidance, error fixing, and feature brainstorming that made the project possible.

Built with passion by cryptic-node — because the wheel sometimes deserves a fresh set of rims.

Star the repo if you find it useful.  
Feedback, issues, and pull requests are always welcome.

cryptic-node npub: npub1axr49qkexxmcm0g2tac3uawzmk59gaupsgy5fw5sfuscumq79h9qjh47gn
