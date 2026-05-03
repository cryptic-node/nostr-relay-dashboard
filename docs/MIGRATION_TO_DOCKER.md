# NRD Migration: systemd → Docker

This runbook covers the cutover of an existing NRD installation from a
bare-metal systemd unit to a Docker container managed by `docker compose`
(optionally adopted into Portainer afterward).

**Goal:** zero-downtime cutover for any clients hitting the Caddy hostname
(e.g. mobile Nostr clients pointed at `nrd.your-tailnet.ts.net`).

The runbook assumes a two-box homelab: a **prod** box that serves your
stable instance and a **dev** box where you prove the container before
touching prod. If you only have one box, do the dev validation steps in
a separate compose project on the same host (different ports, different
data path).

## Pre-flight (do these first, days before cutover if possible)

### 1. Prove the container on the dev box

```bash
# On the dev box:
git clone <your repo> nrd && cd nrd

# Host data dir, owned by the distroless nonroot uid:
sudo mkdir -p /srv/nrd-dev/data/backups
sudo chown -R 65532:65532 /srv/nrd-dev/data

# Copy a SNAPSHOT of the prod DB into the dev mount for realistic testing.
# (Replace prod-host with your own SSH alias / Tailscale name.)
sudo scp prod-host:/opt/nostr-relay-dashboard/dashboard.db \
    /srv/nrd-dev/data/dashboard.db
sudo chown 65532:65532 /srv/nrd-dev/data/dashboard.db

# Build + run:
docker compose -f docker-compose.dev.yml up -d --build
docker compose -f docker-compose.dev.yml logs -f
```

Verify:

- `curl http://127.0.0.1:8080/` returns the dashboard HTML
- `curl http://127.0.0.1:8080/api/relays` returns expected JSON
- Caddy on the dev box serves the dashboard at the configured path
- `sqlite3 /srv/nrd-dev/data/dashboard.db 'SELECT count(*) FROM events;'`
  matches the count on prod

### 2. Generate the production admin token

```bash
openssl rand -hex 32
```

Save it somewhere durable (password manager). You'll need it on the prod
box in `.env` and in any client/script that hits admin endpoints.

## Cutover on the prod box

### 3. Stage the repo and host paths

```bash
# On the prod box:
git clone <your repo> /opt/nrd-docker && cd /opt/nrd-docker

sudo mkdir -p /srv/nrd-prod/data/backups
sudo chown -R 65532:65532 /srv/nrd-prod/data

cp .env.example .env
nano .env                  # paste the token from step 2
chmod 600 .env
```

### 4. Build the image WITHOUT starting the container

```bash
docker compose -f docker-compose.prod.yml --env-file .env build
```

This compiles the Rust binary in the builder stage. It does NOT touch the
running systemd service or port 8080. Verify:

```bash
docker images nrd:1.0.5
```

### 5. Stop systemd, copy the DB, start the container

This is the only step with downtime — should take well under 30 seconds.

```bash
# Stop the running service:
sudo systemctl stop nrd

# Copy the live DB into the container's bind-mount path. -p preserves
# timestamps; use rsync if you want progress feedback on a big DB.
sudo cp -p /opt/nostr-relay-dashboard/dashboard.db \
           /srv/nrd-prod/data/dashboard.db

# If WAL/SHM files exist, copy them too — the SQLite checkpoint may have
# left uncommitted writes; copying them lets SQLite finish replay on first
# open in the container.
sudo cp -p /opt/nostr-relay-dashboard/dashboard.db-wal \
           /srv/nrd-prod/data/ 2>/dev/null || true
sudo cp -p /opt/nostr-relay-dashboard/dashboard.db-shm \
           /srv/nrd-prod/data/ 2>/dev/null || true

# Fix ownership for the distroless nonroot uid:
sudo chown -R 65532:65532 /srv/nrd-prod/data

# Start the container:
docker compose -f docker-compose.prod.yml --env-file .env up -d
```

### 6. Verify

```bash
# Container running, listening:
docker ps --filter name=nrd
docker logs nrd

# HTTP serving locally:
curl -s http://127.0.0.1:8080/ | head -20
curl -s http://127.0.0.1:8080/api/relays

# Through Caddy on the tailnet:
curl -s https://nrd.your-tailnet.ts.net/ | head -20
```

Then open your Nostr client — notes should load from
`nrd.your-tailnet.ts.net` exactly as before.

### 7. Disable the systemd unit (do NOT delete it yet)

```bash
sudo systemctl disable nrd
# Leave the unit file in place for a few days as a rollback option.
```

## Rollback (if anything looks wrong)

```bash
# Stop the container:
docker compose -f docker-compose.prod.yml down

# Restart systemd:
sudo systemctl start nrd
sudo systemctl enable nrd
```

The systemd unit still points at `/opt/nostr-relay-dashboard/dashboard.db`,
which is untouched by the migration (we copied FROM it, not over it).
Rollback loses any writes that happened to the container's DB during its
uptime — acceptable for a rollback in the first hours.

## Cleanup (after a week of stable container operation)

```bash
sudo rm -rf /opt/nostr-relay-dashboard          # remove the bare-metal install
sudo rm /etc/systemd/system/nrd.service         # remove the unit
sudo systemctl daemon-reload
sudo userdel nrd                                # remove the system user
```

## Portainer adoption

Once the stack is running via `docker compose`, you can import it into
Portainer:

- Stacks → Add stack → "Web editor"
- Name: `nrd`
- Paste the contents of `docker-compose.prod.yml`
- Add `NRD_ADMIN_TOKEN` under Environment variables (Portainer's `.env`
  equivalent)
- Deploy

Portainer will adopt the existing container if names match.

## Future: registry-based deploys

When you have a private container registry (Gitea, Forgejo, Harbor, plain
`registry:2`), you can move builds off the prod box:

1. Build on the dev box, tag with version:
   `docker build -t <registry>/nrd:1.0.5 .`
2. Push: `docker push <registry>/nrd:1.0.5`
3. On the prod box, replace the `build:` block in `docker-compose.prod.yml`
   with `image: <registry>/nrd:1.0.5` and remove the local build step.
   Pull + restart is now a single registry pull, no Rust toolchain needed
   on prod.
