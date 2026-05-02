<div align="center">

<img src="NRDv1.0.5.png" alt="Nostr Relay Dashboard" width="720">

# Nostr Relay Dashboard

**A clean, self-hosted admin and archive layer for Nostr.**
Pull notes from any set of upstream relays for any number of npubs, store them locally, and manage the whole thing from a fast dashboard.

[![Version](https://img.shields.io/badge/version-1.0.5-8b5cf6?style=for-the-badge)](https://github.com/cryptic-node/nostr-relay-dashboard/releases)
[![Rust](https://img.shields.io/badge/built_with-Rust-dea584?style=for-the-badge&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![License: Unlicense](https://img.shields.io/badge/license-Unlicense-blue?style=for-the-badge)](LICENSE)
[![Tailscale Friendly](https://img.shields.io/badge/tailnet-friendly-7c3aed?style=for-the-badge)](https://tailscale.com/)

</div>

-----

## Why NRD

NRD is the admin and archive layer you actually want sitting in front of your Nostr identity.

- 🗂️ **Multi-npub archive.** Monitor as many npubs as you want. NRD stores their notes locally in SQLite.
- 🎛️ **Three sync modes.** `recent`, `deep`, and `full` — pick how far back you want to pull history.
- 🔁 **Optional nightly backups.** NDJSON, 7-day rotation, fully automatic.
- 🔐 **Hardened admin.** Optional bearer token gate on every mutating route.
- 🛰️ **Tailnet-native.** Designed to live quietly on your tailnet — no public exposure required.
- 📱 **Works as an iOS web app.** Add to Home Screen and it runs fullscreen like a native app.

## What NRD is — and isn’t

✅ A private dashboard for managing relays and monitored npubs
✅ A personal archive for stored Nostr events
✅ An HTTPS-friendly admin surface that pairs well with Caddy
✅ A good fit for private access over Tailscale

❌ **Not** a public-facing relay endpoint. If you want client-facing relay service, pair NRD with a dedicated backend like `nostr-rs-relay` on a separate hostname.

-----

## ✨ What’s new in v1.0.5

- **Equal-height three-panel layout** — relays, npubs, and notes share a fixed panel height
- **Independent scrolling** — every pane scrolls on its own
- **Paginated notes loading** — browse older history without hitting a fixed cap
- **Sync mode selector** — `recent` / `deep` / `full` chosen per sync
- **Deep backfill window** — configurable in days (default 30)
- **Full backfill** — drops the `since` filter entirely for broadest history
- **Selected-npub sync** — target one npub or sync everything
- **Nightly backup toggle** — automatic NDJSON snapshots, 7-day rotation

-----

## 🚀 Default setup: fresh Ubuntu Server

These directions assume a **clean Ubuntu 24.04 LTS** install (Server or Desktop, both fine). If you’re on Debian or another distro the commands are nearly identical.

### 1. System packages

```bash
sudo apt update && sudo apt upgrade -y
sudo apt install -y \
  curl git build-essential pkg-config libssl-dev \
  sqlite3 ca-certificates ufw
```

### 2. Install Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"
rustc --version   # sanity check
```

### 3. Clone and build NRD

```bash
git clone https://github.com/cryptic-node/nostr-relay-dashboard.git
cd nostr-relay-dashboard
cargo build --release
```

First build takes a few minutes. After that, cargo caches everything and rebuilds are fast.

### 4. First run

```bash
cargo run --release
```

Then open `http://127.0.0.1:8080` in a browser on the same machine. You should see the dashboard.

Stop it with `Ctrl+C` once you’ve confirmed it works. The next sections set it up to run as a proper service.

### 5. Environment variables

NRD reads these from the environment. Defaults in parentheses.

|Variable         |Purpose                                                                                         |Default               |
|-----------------|------------------------------------------------------------------------------------------------|----------------------|
|`HOST`           |Bind address                                                                                    |`127.0.0.1`           |
|`PORT`           |Bind port                                                                                       |`8080`                |
|`NRD_ADMIN_TOKEN`|Bearer token for mutating routes. **Strongly recommended** for any deployment beyond local-only.|*(unset = permissive)*|
|`BACKUP_DIR`     |Where nightly backups are written                                                               |`data/backups`        |

Generate a solid admin token with:

```bash
openssl rand -hex 32
```

### 6. Run as a systemd service

A ready-to-go unit lives at `deploy/nrd.service`. Copy, edit, enable:

```bash
sudo cp deploy/nrd.service /etc/systemd/system/nrd.service
sudo nano /etc/systemd/system/nrd.service   # set User, paths, and NRD_ADMIN_TOKEN
sudo systemctl daemon-reload
sudo systemctl enable --now nrd
sudo systemctl status nrd
```

Tail the logs:

```bash
journalctl -u nrd -f
```

-----

## 🛰️ Tailscale setup

This is the part that turns NRD into *your* private dashboard, reachable from any device you own without opening a single port on your router.

### 1. Install Tailscale on the NRD host

```bash
curl -fsSL https://tailscale.com/install.sh | sh
sudo tailscale up
```

Tailscale prints an auth URL. Open it, log in, approve the machine. Done.

```bash
tailscale status
tailscale ip -4   # your machine's 100.x.x.x address
```

### 2. (Optional but recommended) Enable MagicDNS

In the Tailscale admin console → **DNS** → enable **MagicDNS**. Now every tailnet machine is reachable by hostname. Your NRD box becomes something like `http://nrd-optiplex` instead of a numeric IP.

### 3. Install Tailscale on every device you want to reach NRD from

Laptops, phones, tablets — same command on Linux, or grab the app on iOS / macOS / Android / Windows. Log in with the same account, approve the machine, and those devices can now talk to NRD directly.

### 4. (Optional) HTTPS inside your tailnet with Tailscale certs

Tailscale can issue real Let’s Encrypt certs for your `*.ts.net` hostname. This is what makes iOS happy about “add to home screen” as a real web app with a valid padlock.

In the admin console → **DNS** → enable **HTTPS Certificates**. Then on the NRD host:

```bash
sudo tailscale cert nrd-optiplex.your-tailnet.ts.net
```

Replace with your actual MagicDNS hostname. Tailscale drops a cert + key in the current directory.

-----

## 🌐 Caddy reverse proxy

Caddy handles HTTPS and sits in front of NRD. An example config lives at `deploy/Caddyfile.example`.

### 1. Install Caddy

```bash
sudo apt install -y debian-keyring debian-archive-keyring apt-transport-https
curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/gpg.key' \
  | sudo gpg --dearmor -o /usr/share/keyrings/caddy-stable-archive-keyring.gpg
curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/debian.deb.txt' \
  | sudo tee /etc/apt/sources.list.d/caddy-stable.list
sudo apt update
sudo apt install -y caddy
```

### 2. Tailnet-only Caddyfile

Drop this into `/etc/caddy/Caddyfile`, replacing the hostname with your actual MagicDNS name:

```caddyfile
nrd-optiplex.your-tailnet.ts.net {
    tls /var/lib/caddy/nrd-optiplex.your-tailnet.ts.net.crt \
        /var/lib/caddy/nrd-optiplex.your-tailnet.ts.net.key

    reverse_proxy 127.0.0.1:8080

    encode gzip
    header {
        Strict-Transport-Security "max-age=31536000"
        X-Content-Type-Options "nosniff"
        Referrer-Policy "no-referrer"
    }
}
```

Copy the Tailscale-issued cert into place where Caddy can read it, then:

```bash
sudo systemctl enable --now caddy
sudo systemctl reload caddy
```

> **Note:** if you set `admin off` anywhere in your Caddyfile, use `systemctl restart caddy` instead of `reload`.

### 3. Visit your dashboard

From any tailnet device:

```
https://nrd-optiplex.your-tailnet.ts.net
```

You should see the NRD dashboard with a valid padlock. 🔒

-----

## 📱 Add NRD to your iPhone Home Screen

Since you’re already on the tailnet, NRD behaves exactly like a web app — and iOS lets you launch it fullscreen, no Safari chrome, straight from your Home Screen.

### Step by step

1. **Open Safari** on your iPhone or iPad. *(Chrome and other browsers on iOS don’t support “Add to Home Screen” the same way — use Safari.)*
1. Navigate to your NRD address, e.g.
   `https://nrd-optiplex.your-tailnet.ts.net`
1. Tap the **Share** button (the square with the arrow pointing up, in the bottom toolbar).
1. Scroll down in the share sheet and tap **Add to Home Screen**.
1. On iOS 16.4 and newer you’ll see an **“Open as Web App”** toggle — **leave it on**. This is what makes NRD launch fullscreen like a native app instead of opening in Safari.
1. Edit the name if you want (“NRD” is already clean), then tap **Add**.

Your Home Screen now has an NRD icon. Tap it and it opens fullscreen — no address bar, no tabs, just the dashboard.

> The app icon is served by NRD itself via `apple-touch-icon` in the dashboard’s HTML head. Swap in your own PNG if you’d like a different look.

-----

## 🏃 Quick start — just run it

If you’ve already got Rust and just want to see it work:

```bash
git clone https://github.com/cryptic-node/nostr-relay-dashboard.git
cd nostr-relay-dashboard
cargo run --release
```

Then hit `http://127.0.0.1:8080`.

### Docker

```bash
docker build -t nostr-relay-dashboard:1.0.5 .
docker run -p 8080:8080 -v nrd-data:/app/data nostr-relay-dashboard:1.0.5
```

-----

## 🔄 Sync modes

**Recent** — uses the saved per-relay checkpoint when available. On first sync, defaults to the last 7 days.

**Deep** — explicit backfill window in days. Default 30.

**Full** — requests the broadest available history by omitting the `since` filter.

## 📜 Notes pagination

The notes pane loads the newest page first. Use **Load older notes** to page further back. The pane stays responsive no matter how much history an npub has.

## 💾 Nightly backups

When enabled, NRD writes a backup at **00:05 local time** to `BACKUP_DIR` and keeps the most recent 7 files.

```bash
export BACKUP_DIR=/app/data/backups
```

Default is `data/backups` relative to the working directory.

## 🔐 Admin token behavior

When `NRD_ADMIN_TOKEN` is **unset**, NRD behaves like v1.0.4 — permissive.

When `NRD_ADMIN_TOKEN` **is set**, mutating and sensitive routes (add, delete, toggle, sync, backup, restore, logs, restart, settings) require one of:

```
X-Admin-Token: <token>
Authorization: Bearer <token>
```

-----

## 🗂️ Repository layout

```
.
├── deploy/                 # Caddyfile.example, nrd.service
├── migrations/             # SQLite schema migrations
├── public/                 # Frontend (HTML/JS/CSS + icons)
├── src/                    # Rust source (Axum + SQLx)
├── Cargo.toml
├── Dockerfile
├── docker-compose.yml
├── NRDv1.0.5.png
├── RELEASE_MANIFEST.txt
└── README.md
```

-----

## 🙌 Credits

Huge thanks to **Grok** for idea generation and feature brainstorming, **ChatGPT** for release packaging and implementation passes, **Claude** for docs and UX polish, and the **Nostr community** for making this ecosystem worth building for.

## 📄 License

[Unlicense](LICENSE) — public domain, do whatever you want with it.

-----

<div align="center">

Built with 🦀 by [**cryptic-node**](https://github.com/cryptic-node) — part of the [**Cryptic Forge**](https://github.com/cryptic-node) toolkit.

</div>

## Support development

If NERD is useful to you, Lightning tips are welcome.

*Lightning address orchidcheetah29@primal.net and QR is QR.png*

<p align="center">
  <img src="QR.png" alt="Lightning donation QR code" width="200">
</p>

