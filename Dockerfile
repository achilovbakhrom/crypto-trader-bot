# ── Stage 1: Dependency cache (cargo-chef) ────────────────────────────────────
FROM rust:1.96-slim AS chef
RUN cargo install cargo-chef --locked
WORKDIR /app

# ── Stage 2: Build recipe (dependency fingerprint) ───────────────────────────
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# ── Stage 3: Build dependencies (cached layer) ───────────────────────────────
FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json

# Install system deps needed for some crates (openssl, pkg-config)
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
  && rm -rf /var/lib/apt/lists/*

# Build deps only (cached unless Cargo.toml changes)
RUN cargo chef cook --release --recipe-path recipe.json

# Build the actual application
COPY . .
RUN cargo build --release --bin bot

# ── Stage 4: Minimal runtime image ───────────────────────────────────────────
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
  && rm -rf /var/lib/apt/lists/*

# Create a non-root user
RUN useradd -r -u 1001 -s /bin/false appuser

WORKDIR /app

# Copy binary and default config
COPY --from=builder /app/target/release/bot            /app/bot
COPY --from=builder /app/config/default.toml           /app/config/default.toml
COPY --from=builder /app/crates/storage/migrations     /app/migrations

RUN chown -R appuser:appuser /app
USER appuser

EXPOSE 3000

ENTRYPOINT ["/app/bot"]
