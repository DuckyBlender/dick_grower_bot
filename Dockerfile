# Build stage
FROM rust:slim-bullseye AS builder

# Install SQLite for building
RUN apt update && apt install -y libsqlite3-dev clang
# Install SQLx CLI
RUN cargo install sqlx-cli --no-default-features --features sqlite-unbundled,rustls

# Set working directory
WORKDIR /app

# Copy your entire project
COPY . .

# Create database and run migrations
RUN mkdir -p /app/data && \
    sqlx db create --database-url=sqlite:/app/data/dick_growth.db && \
    sqlx migrate run --database-url=sqlite:/app/data/dick_growth.db

# Build the application
RUN cargo build --release

# Runtime stage
FROM debian:bullseye-slim

# Install SQLite runtime
RUN apt update && apt install -y libsqlite3-0 && \
    rm -rf /var/lib/apt/lists/*

# Set working directory
WORKDIR /app

# Copy the binary from the build stage
COPY --from=builder /app/target/release/dick_grower_bot /app/dick_grower_bot
# Copy .env file
COPY .env /app/.env
# Copy the database
COPY --from=builder /app/data /app/data

# Create volume for data
VOLUME /app/data

# Set environment variable
ENV DATABASE_URL=sqlite:/app/data/dick_growth.db

# Run the application
CMD ["/app/dick_grower_bot"]