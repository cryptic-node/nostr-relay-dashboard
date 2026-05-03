# syntax=docker/dockerfile:1.7
#
# Nostr Relay Dashboard — multi-stage, distroless runtime
# Shared across: docker-compose.yml (generic), docker-compose.dev.yml (dev),
#                docker-compose.prod.yml (prod)
#
# Build context expectations:
#   - Cargo.toml         (workspace root)
#   - src/               (Rust sources)
#   - public/            (static assets served by ServeDir)
#   - migrations/        (SQL — currently inert; tables created from main.rs ensure_tables)
#
# ── Build stage ─────────────────────────────────────────────────────────────
FROM rust:1.78-slim-bookworm AS builder

WORKDIR /app

# OS deps for sqlx + nostr-sdk (TLS via rustls in nostr-sdk 0.44, but libssl is
# still pulled in by some transitive paths; keep pkg-config + libssl-dev to be safe).
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        pkg-config \
        libssl-dev \
        ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# ---- Dependency cache layer ----------------------------------------------------
# The trick: build a stub binary using ONLY Cargo.toml so the dependency graph
# is compiled and cached. Subsequent rebuilds with code changes only recompile
# the leaf crate.
COPY Cargo.toml ./
RUN mkdir -p src public \
    && echo 'fn main() {}' > src/main.rs \
    && echo '<!doctype html><title>build-stub</title>' > public/index.html \
    && cargo build --release \
    && rm -rf src public target/release/deps/nostr_relay_dashboard* \
              target/release/nostr-relay-dashboard*

# ---- Real build ----------------------------------------------------------------
COPY src ./src
COPY public ./public
RUN cargo build --release \
    && strip target/release/nostr-relay-dashboard

# ── Runtime stage (distroless) ──────────────────────────────────────────────
# gcr.io/distroless/cc-debian12:nonroot — has glibc + libssl + ca-certs,
# runs as uid/gid 65532 by default, no shell, no package manager.
FROM gcr.io/distroless/cc-debian12:nonroot AS runtime

WORKDIR /app

# Copy the binary and static assets from the builder.
# /app/data is where the bind-mounted (or volume-mounted) DB lives. We do NOT
# create it here — the mount point will materialize at runtime, and distroless
# has no `mkdir`. The DATABASE_PATH default expects it to exist.
COPY --from=builder --chown=nonroot:nonroot /app/target/release/nostr-relay-dashboard /app/nostr-relay-dashboard
COPY --from=builder --chown=nonroot:nonroot /app/public /app/public

# Defaults — every compose file overrides these explicitly. They're here so
# `docker run` without compose still produces something sensible.
ENV DATABASE_PATH=/app/data/dashboard.db \
    BACKUP_DIR=/app/data/backups \
    HOST=0.0.0.0 \
    PORT=8080 \
    RUST_LOG=nostr_relay_dashboard=info

EXPOSE 8080
VOLUME ["/app/data"]

USER nonroot:nonroot

ENTRYPOINT ["/app/nostr-relay-dashboard"]
