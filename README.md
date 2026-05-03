# Nostr Relay Dashboard (NRD)

**Version 1.0.5** — feature-complete archival milestone.

NRD is a clean, self-hosted admin and archive layer for pulling and storing
Nostr notes from upstream relays for any number of npubs. It runs as a small
Rust service (axum + sqlx + SQLite) with a single static HTML/JS dashboard,
and is designed to live behind a reverse proxy on a private network
(Tailscale, WireGuard, LAN).

This v1.0.5 release ships in two flavors:

- **Bare-metal** — `cargo build --release` plus a systemd unit
- **Docker** — multi-stage build to a distroless runtime

Pick whichever fits your homelab. Both run the same binary.

## What's in this release

- **Equal-height three-panel layout** — relays, npubs, and notes share a
  fixed panel height
- **Independent scrolling in all panes** — each column scrolls on its own
- **Paginated notes loading** — notes load in pages with a *Load older notes*
  control, so the pane stays responsive even with deep history
- **Sync mode selector** — choose between `recent`, `deep`, and `full`
  before kicking off a pull
- **Deep backfill support** — configurable backfill window in days
  (default 30)
- **Full backfill mode** — broadest history pull the upstream relays will
  provide (omits the `since` filter)
- **Selected-npub-only sync** — target one npub when you don't want to
  sync everything
- **Optional nightly backups** — NDJSON dumps with 7-day rotation

## Sync modes

| Mode     | Behavior                                                                |
| -------- | ----------------------------------------------------------------------- |
| `recent` | Uses the saved per-relay checkpoint; defaults to last 7 days on first sync |
| `deep`   | Explicit backfill window in days (default 30)                           |
| `full`   | Omits the `since` filter; broadest history available                    |

## Nightly backups

When enabled, NRD writes a backup at **00:05 local time** to the backup
directory and keeps the most recent 7 files.

```text
BACKUP_DIR=/app/data/backups
```

If `BACKUP_DIR` is not set, NRD defaults to `data/backups` relative to the
working directory.

## Admin token

When `NRD_ADMIN_TOKEN` is **not** set, mutating routes are unauthenticated
(fine for purely localhost binds). When it **is** set, NRD requires either:

- `X-Admin-Token: <token>`, or
- `Authorization: Bearer <token>`

on mutating and sensitive routes (add/delete/toggle/sync/backup/restore/
logs/restart/settings).

Generate a token with:

```bash
openssl rand -hex 32
```

Put it in `.env` (see `.env.example`). Production deployments **must** set
this — the production compose file refuses to start without it.

## What NRD is

- A private dashboard for managing relays and monitored npubs
- A personal archive for stored Nostr events
- An HTTPS-friendly admin surface that works well behind Caddy
- A good fit for private access over Tailscale or VPN

## What NRD is not

NRD is **not** a public-facing relay endpoint. Stored events live in the
dashboard's SQLite DB; standard Nostr clients can't read from it directly.
If you want a client-facing relay, pair NRD with a dedicated relay backend
like [`nostr-rs-relay`](https://github.com/scsibug/nostr-rs-relay) on a
separate hostname or subdomain.

(This pairing is the planned direction for NRD v1.0.6.)

## Quick start

### Local dev

```bash
cargo run
# open http://127.0.0.1:8080
```

### Docker — generic single host

The default `docker-compose.yml` uses a Docker named volume and works
out of the box:

```bash
docker compose up -d --build
docker compose logs -f
```

### Docker — separate prod / dev tiers

If you run a homelab with separate boxes for production and experimentation,
use the dedicated compose files:

```bash
# Production tier — bind mounts, admin token required, tight resource limits
cp .env.example .env && nano .env       # paste a fresh token
docker compose -f docker-compose.prod.yml --env-file .env up -d --build

# Dev tier — bind mounts, looser limits, debug logging
docker compose -f docker-compose.dev.yml up -d --build
```

See [`docs/MIGRATION_TO_DOCKER.md`](docs/MIGRATION_TO_DOCKER.md) for a
detailed runbook covering migration from a bare-metal systemd install to
the containerized deployment.

## Repository layout

```
.
├── Cargo.toml                       Rust crate manifest
├── src/                             Rust sources (axum + sqlx)
├── public/                          Single-file dashboard UI
├── migrations/                      SQL (schema is created from main.rs at startup)
├── Dockerfile                       Multi-stage, distroless runtime
├── .dockerignore
├── .env.example
├── docker-compose.yml               Generic single-host (named volume)
├── docker-compose.prod.yml          Production tier (bind mount, token required)
├── docker-compose.dev.yml           Dev tier (bind mount, debug logging)
├── deploy/
│   ├── Caddyfile.example            Generic Caddy reverse-proxy template
│   ├── Caddyfile.prod               Caddy on the prod box (single hostname)
│   └── Caddyfile.dev                Caddy on the dev box (path-based)
└── docs/
    └── MIGRATION_TO_DOCKER.md       systemd → Docker cutover runbook
```

## License

Apache-2.0. See `LICENSE`.

## Acknowledgments

Developed with significant AI-assisted iteration. NRD is part of a broader
self-hosted Nostr ecosystem under active exploration.
