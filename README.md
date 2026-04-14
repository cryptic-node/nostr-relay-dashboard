# Nostr Relay Dashboard

**Feature release • Version 1.0.5**  
**Dev Team:** cryptic-node & Grok/ChatGPT collaboration • Powered by Rust

Nostr Relay Dashboard (NRD) is a clean, self-hosted admin and archive layer for pulling and storing Nostr notes from upstream relays for any number of npubs.

This **v1.0.5** build keeps the hardened private-admin posture from v1.0.4 and adds the usability/workflow upgrades that make the dashboard feel more complete in daily use.

## What changed from v1.0.4

- **Equal-height three-panel layout** — relays, npubs, and notes now share the same fixed panel height
- **Independent scrolling in all panes** — each column scrolls on its own instead of one panel growing awkwardly
- **Paginated notes loading** — notes are loaded in pages so you can browse older history without the notes pane hard-stopping at a single fixed cap
- **Sync mode selector** — choose between `recent`, `deep`, and `full` sync modes before kicking off a pull
- **Deep backfill support** — set a configurable backfill window in days for deeper history pulls
- **Full backfill mode** — request the broadest history pull the upstream relays will provide
- **Optional selected-npub sync** — target only the currently selected npub when you do not want to sync everything
- **Optional nightly backup toggle** — keep automatic NDJSON backups on a 7-day rotation

## Sync modes

### Recent
Uses the saved per-relay checkpoint when available. On first sync, recent mode defaults to the last 7 days.

### Deep
Uses an explicit backfill window in days. Default is 30 days.

### Full
Requests the broadest available history by omitting the `since` filter.

## Notes pagination

The notes pane now loads the newest page first and lets you fetch older notes on demand with **Load older notes**.

This keeps the pane responsive even when an npub has a lot of stored history.

## Nightly backups

When enabled, NRD writes a backup at **00:05 local time** to the backup directory and keeps the most recent 7 files.

Environment variable:

```text
BACKUP_DIR=/app/data/backups
```

If `BACKUP_DIR` is not set, NRD defaults to `data/backups` relative to the working directory.

## Admin token behavior

When `NRD_ADMIN_TOKEN` is **not** set, NRD behaves much like v1.0.4.

When `NRD_ADMIN_TOKEN` **is** set, NRD requires either:

- `X-Admin-Token: <token>`
- or `Authorization: Bearer <token>`

for mutating and sensitive routes such as add / delete / toggle / sync / backup / restore / logs / restart / settings.

## What NRD is

- A private dashboard for managing relays and monitored npubs
- A personal archive for stored Nostr events
- An HTTPS-friendly admin surface that works well behind Caddy
- A good fit for private access over Tailscale

## What NRD is not

NRD is **not** a full public relay endpoint by itself.  
Use it as the admin/archive/visibility layer. If you later want a client-facing relay endpoint, pair it with a dedicated relay backend such as `nostr-rs-relay` on a separate hostname or subdomain.

## Included deployment examples

This release bundle includes:

- `deploy/Caddyfile.example`
- `deploy/nrd.service`

The service example still binds NRD to `127.0.0.1:8080` and now also includes an example `BACKUP_DIR` line.

## Quick start

### Local dev

```bash
cargo run
```

Then open:

```text
http://127.0.0.1:8080
```

### Docker

```bash
docker build -t nostr-relay-dashboard:1.0.5 .
docker run -p 8080:8080 -v nrd-data:/app/data nostr-relay-dashboard:1.0.5
```

## Candidate notes

This is a **develop-branch feature candidate** built as the proposed v1.0.5 release.

## Credits

Huge thanks to Grok for idea generation and feature brainstorming, and to ChatGPT for the release packaging and implementation pass, and the NOSTR community!

## Screenshot

## Screenshot

![Nostr Relay Dashboard v1.0.5](./NRDv1.0.5.png)
