# ── Build stage ─────────────────────────────────────────────────────────────
FROM rust:1.78-slim AS builder

WORKDIR /app

RUN apt-get update && apt-get install -y     pkg-config     libssl-dev     && rm -rf /var/lib/apt/lists/*

# Cache dependencies first
COPY Cargo.toml ./
RUN mkdir -p src public     && echo 'fn main() {}' > src/main.rs     && echo '<!doctype html><title>build-stub</title>' > public/index.html     && cargo build --release 2>/dev/null || true     && rm -rf src public

# Build the real app
COPY src ./src
COPY public ./public
RUN cargo build --release

# ── Runtime stage ───────────────────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y     ca-certificates     libssl3     && rm -rf /var/lib/apt/lists/*

RUN useradd -ms /bin/bash relay
WORKDIR /app

COPY --from=builder /app/target/release/nostr-relay-dashboard /app/nostr-relay-dashboard
COPY --from=builder /app/public /app/public

RUN mkdir -p /app/data /app/data/backups && chown -R relay:relay /app

USER relay

EXPOSE 8080
ENV DATABASE_PATH=/app/data/dashboard.db
ENV PORT=8080
ENV HOST=0.0.0.0
ENV BACKUP_DIR=/app/data/backups

VOLUME ["/app/data"]

ENTRYPOINT ["/app/nostr-relay-dashboard"]
