# ── Build stage ─────────────────────────────────────────────────────────────
FROM rust:1.78-slim AS builder

WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Cache dependencies separately
COPY Cargo.toml ./
RUN mkdir -p src && echo 'fn main() {}' > src/main.rs
RUN cargo build --release 2>/dev/null || true
RUN rm -rf src

# Build the real binary
COPY src ./src
RUN touch src/main.rs
RUN cargo build --release

# ── Runtime stage ────────────────────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

RUN useradd -ms /bin/bash relay
WORKDIR /app

COPY --from=builder /app/target/release/nostr-relay /app/nostr-relay

RUN mkdir -p /app/data && chown -R relay:relay /app

USER relay

EXPOSE 8080

ENV DATABASE_PATH=/app/data/relay.db
ENV PORT=8080
ENV HOST=0.0.0.0
ENV RELAY_NAME="Nostr Relay"
ENV RELAY_DESCRIPTION="A Nostr relay with whitelist support"

VOLUME ["/app/data"]

ENTRYPOINT ["/app/nostr-relay"]
