# Nostr Relay

BEWARE:  A self-hosted Nostr relay vibe coded by an amateur in Rust with:

- **NIP-01** WebSocket relay protocol
- **NIP-11** relay information document
- **Npub whitelist** — restrict who can post events
- **Web GUI** styled like the Umbrel nostr-relay-rs interface
- **SQLite** storage with WAL mode
- **Docker** packaging for easy deployment

---

## Quick Start (Docker)

```bash
docker compose up -d
```

Then open **http://localhost:8080** in your browser.

Your relay WebSocket URL is: `ws://your-server:8080`

---

## Configuration

All configuration is via environment variables (set in `docker-compose.yml`):

| Variable             | Default                               | Description                    |
|----------------------|---------------------------------------|--------------------------------|
| `PORT`               | `8080`                                | HTTP/WebSocket port            |
| `HOST`               | `0.0.0.0`                             | Bind address                   |
| `DATABASE_PATH`      | `./data/relay.db`                     | SQLite database file path      |
| `RELAY_NAME`         | `Nostr Relay`                         | Relay name (shown in GUI + NIP-11) |
| `RELAY_DESCRIPTION`  | `A Nostr relay with whitelist support` | Relay description              |
| `RUST_LOG`           | `nostr_relay=info`                    | Log level                      |

---

## Whitelist

By default the relay operates in **open mode** — all valid events from any pubkey are accepted.

Once you add one or more pubkeys to the whitelist via the GUI or API, the relay switches to **whitelist mode** and only accepts events from those pubkeys.

### Via the GUI

Open `http://localhost:8080` and use the whitelist panel. You can paste either:
- `npub1...` bech32 format
- 64-character hex pubkey

### Via the API

```bash
# Add a pubkey (npub format)
curl -X POST http://localhost:8080/api/whitelist \
  -H 'Content-Type: application/json' \
  -d '{"npub": "npub1sg6plzptd64u62a878hep2kev88swjh3tw00gjsfl8f237lmu63q0uf63m", "note": "Alice"}'

# Add a pubkey (hex format)
curl -X POST http://localhost:8080/api/whitelist \
  -H 'Content-Type: application/json' \
  -d '{"pubkey": "7f3b6430c0bc3d12...", "note": "Bob"}'

# List all whitelisted pubkeys
curl http://localhost:8080/api/whitelist

# Remove a pubkey (use hex pubkey)
curl -X DELETE http://localhost:8080/api/whitelist/<hex-pubkey>

# Relay stats
curl http://localhost:8080/api/stats
```

---

## Build Without Docker

Requires Rust 1.78+:

```bash
cargo build --release
DATABASE_PATH=./relay.db ./target/release/nostr-relay
```

---

## Supported NIPs

| NIP | Description |
|-----|-------------|
| 1   | Basic protocol |
| 11  | Relay information document |

Clients can connect using any standard Nostr client (Damus, Amethyst, Snort, etc.) by pointing it at your relay URL.

---

## Data

The SQLite database is stored at `DATABASE_PATH` (default `/app/data/relay.db` in Docker). The Docker Compose setup mounts a named volume `relay-data` so data persists across container restarts.

To back up your relay data:

```bash
docker run --rm -v relay-data:/data -v $(pwd):/backup alpine \
  tar czf /backup/relay-backup.tar.gz -C /data .
```
