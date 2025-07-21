# Chef planner stage
FROM lukemathwalker/cargo-chef:latest-rust-1 AS chef
WORKDIR /app

# Planner stage - create recipe.json for dependencies
FROM chef AS planner
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo chef prepare --recipe-path recipe.json

# Builder stage with cached dependencies
FROM chef AS builder

# Install SQLite and other build dependencies
RUN apt update && apt install -y libsqlite3-dev clang && rm -rf /var/lib/apt/lists/*
RUN cargo install sqlx-cli --no-default-features --features sqlite-unbundled,rustls

WORKDIR /app

# Build dependencies (using caching)
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# Copy actual source code and assets
COPY . .

# (No database file is generated in the image here!)

# Prepare SQLx offline cache
# For offline support; assumes you included .sqlx/ in source or it will create it (needs a DB to prepare)
# You could comment this out if unnecessary
# If you want to build with offline mode, it requires a DB file, you can prepare it locally and copy .sqlx if needed

ENV SQLX_OFFLINE=true
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

# Install only SQLite runtime
RUN apt update && apt install -y libsqlite3-0 ca-certificates && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy binary and assets
COPY --from=builder /app/target/release/dick_grower_bot /app/dick_grower_bot
COPY .env /app/.env
COPY --from=builder /app/.sqlx /app/.sqlx

# VOLUME declaration lets Docker users know this path should be mapped
VOLUME ["/app/database.sqlite"]

# Set environment variable for SQLite file
ENV DATABASE_URL=sqlite:/app/database.sqlite

# Start the application
CMD ["/app/dick_grower_bot"]

