# Build stage
FROM rust:slim-bullseye AS builder

# Install SQLite and other dependencies for building
RUN apt update && apt install -y libsqlite3-dev clang && \
    rm -rf /var/lib/apt/lists/*

# Install SQLx CLI
RUN cargo install sqlx-cli --no-default-features --features sqlite-unbundled,rustls

# Set working directory
WORKDIR /app

# Copy your entire project
COPY . .

# Create database directory
RUN mkdir -p /app/data

# Create database and run migrations
RUN sqlx db create --database-url=sqlite:/app/data/database.sqlite
RUN sqlx migrate run --database-url=sqlite:/app/data/database.sqlite

# Prepare SQLx offline cache
RUN cargo sqlx prepare --database-url sqlite:/app/data/database.sqlite

# Build the application with SQLX_OFFLINE enabled
ENV SQLX_OFFLINE=true
RUN cargo build --release

# Runtime stage
FROM debian:bullseye-slim

# Install SQLite runtime
RUN apt update && apt install -y libsqlite3-0 ca-certificates && \
    rm -rf /var/lib/apt/lists/*

# Set working directory
WORKDIR /app

# Copy the binary from the build stage
COPY --from=builder /app/target/release/dick_grower_bot /app/dick_grower_bot
# Copy .env file
COPY .env /app/.env
# Copy the .sqlx directory containing the SQLx offline cache
COPY --from=builder /app/.sqlx /app/.sqlx

# Create data directory
RUN mkdir -p /app/data
# Copy the pre-populated database from build stage
COPY --from=builder /app/data/database.sqlite /app/data/database.sqlite
# Ensure proper permissions
RUN chmod 644 /app/data/database.sqlite && chmod 755 /app/data

# Create volume for data
VOLUME /app/data

# Set environment variable
ENV DATABASE_URL=sqlite:/app/data/database.sqlite

# Run the application
CMD ["/app/dick_grower_bot"]