#!/usr/bin/env bash
set -Eeuo pipefail

# NRD install script: Local-only
# ------------------------------
# What it does:
#   1. Updates the droplet.
#   2. Installs build tools, Rust, Caddy, and optional Tailscale.
#   3. Clones or refreshes the NRD repo.
#   4. Installs a local-only Caddy config.
#
# What it does NOT do:
#   - It does not automatically authenticate Tailscale.
#   - It does not start NRD under systemd; you can run it manually or add your own service.

REPO_URL="${REPO_URL:-https://github.com/cryptic-node/nostr-relay-dashboard.git}"
BRANCH="${BRANCH:-main}"
REPO_DIR="${REPO_DIR:-$HOME/nostr-relay-dashboard}"

sudo apt-get update
sudo DEBIAN_FRONTEND=noninteractive apt-get install -y \
  ca-certificates curl git tmux unzip build-essential pkg-config \
  libssl-dev libsqlite3-dev sqlite3 gnupg debian-keyring debian-archive-keyring apt-transport-https

if ! command -v rustup >/dev/null 2>&1; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
fi
source "$HOME/.cargo/env"
rustup default stable

if ! command -v tailscale >/dev/null 2>&1; then
  curl -fsSL https://tailscale.com/install.sh | sh
fi
sudo systemctl enable --now tailscaled

if ! command -v caddy >/dev/null 2>&1; then
  curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/gpg.key' | sudo gpg --dearmor -o /usr/share/keyrings/caddy-stable-archive-keyring.gpg
  curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/debian.deb.txt' | sudo tee /etc/apt/sources.list.d/caddy-stable.list >/dev/null
  sudo chmod o+r /usr/share/keyrings/caddy-stable-archive-keyring.gpg
  sudo chmod o+r /etc/apt/sources.list.d/caddy-stable.list
  sudo apt-get update
  sudo DEBIAN_FRONTEND=noninteractive apt-get install -y caddy
fi

if [ ! -d "$REPO_DIR/.git" ]; then
  git clone "$REPO_URL" "$REPO_DIR"
fi
git -C "$REPO_DIR" fetch origin
if git -C "$REPO_DIR" show-ref --verify --quiet "refs/heads/$BRANCH"; then
  git -C "$REPO_DIR" switch "$BRANCH"
else
  git -C "$REPO_DIR" switch -c "$BRANCH" --track "origin/$BRANCH"
fi
git -C "$REPO_DIR" reset --hard "origin/$BRANCH"

sudo cp "$REPO_DIR/deploy/caddy/Caddyfile.LOCAL_ONLY.example" /etc/caddy/Caddyfile
sudo systemctl enable caddy
sudo systemctl restart caddy

cat <<MSG
Install complete.

Next steps:
  cd "$REPO_DIR"
  source "$HOME/.cargo/env"
  cargo run

For local-only mode, browse on the droplet itself:
  http://127.0.0.1
  http://localhost

Optional:
  sudo tailscale up
MSG
