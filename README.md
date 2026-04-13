# Nostr Relay Dashboard

**Hardening candidate • Version 1.0.4**  
**Dev Team:** cryptic-node & SuperGrok • Candidate bundle recreated with ChatGPT • Powered by Rust

Nostr Relay Dashboard (NRD) is a clean, self-hosted admin and archive layer for pulling and storing events from upstream relays for any number of npubs.

This v1.0.4 candidate is the narrow hardening pass we discussed after the stable v1.0.3 release. It keeps the same overall UI and operator workflow, but tightens the dangerous edges.

## What changed from v1.0.3

- **Loopback-by-default app binding** — the app now defaults to `127.0.0.1` unless you override `HOST`
- **Optional admin token protection** — set `NRD_ADMIN_TOKEN` to protect mutating and sensitive endpoints
- **Stricter restore validation** — restore now rejects malformed NDJSON and oversized payloads
- **Consistent JSON error responses** for auth and validation failures on protected API routes
- **Minimal UI support for admin token prompts** — the browser retries protected actions after prompting for the token when needed
- **No visual redesign** — same three-panel dashboard, same basic feel

## Admin token behavior

When `NRD_ADMIN_TOKEN` is **not** set, NRD behaves much like v1.0.3.

When `NRD_ADMIN_TOKEN` **is** set, NRD requires either:

- `X-Admin-Token: <token>`
- or `Authorization: Bearer <token>`

for mutating and sensitive routes such as add / delete / toggle / sync / backup / restore / logs / restart.

## What NRD is

- A private dashboard for managing relays and monitored npubs
- A personal archive for stored Nostr events
- An HTTPS-friendly admin surface that works well behind Caddy
- A good fit for private access over Tailscale

## What NRD is not

NRD is **not** a full Nostr relay endpoint by itself.  
Use it as the admin/archive/visibility layer. If you later want a client-facing relay endpoint, pair it with a dedicated relay backend such as `nostr-rs-relay` on a separate hostname or subdomain.

## Included deployment examples

This candidate bundle includes:

- `deploy/Caddyfile.example`
- `deploy/nrd.service`

The service example binds NRD to `127.0.0.1:8080` and includes a commented `NRD_ADMIN_TOKEN` line you can enable when you want auth turned on.

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
docker build -t nostr-relay-dashboard:1.0.4 .
docker run -p 8080:8080 -v nrd-data:/app/data nostr-relay-dashboard:1.0.4
```

## Recommended private production setup

For a personal node, the clean path is:

1. Run NRD as a background service
2. Bind NRD to `127.0.0.1`
3. Put Caddy in front of it for HTTPS
4. Use Tailscale + MagicDNS for easy private access
5. Turn on `NRD_ADMIN_TOKEN` once you are ready for stricter admin protection

## Backup format

Backups are NDJSON and include typed records for:

- `relays`
- `npubs`
- `settings`
- `events`
- `sync_state`

Legacy event-only NDJSON backups are still accepted on restore.

## Candidate notes

This is a **develop-branch testing candidate**, not the stable v1.0.3 release.

## Credits

Huge thanks to SuperGrok for the Rust guidance, debugging help, and feature brainstorming that helped shape the project.

Built with passion by cryptic-node — because the wheel sometimes deserves a fresh set of rims.
