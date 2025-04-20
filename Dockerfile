FROM messense/rust-musl-cross:x86_64-musl AS chef
ENV SQLX_OFFLINE=true
RUN cargo install cargo-chef
WORKDIR /dick_grower_bot

FROM chef AS planner
# Copy source code from previous stage
COPY . .
# Generate info for caching dependencies
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /dick_grower_bot/recipe.json recipe.json
# Build & cache dependencies
RUN cargo chef cook --release --target x86_64-unknown-linux-musl --recipe-path recipe.json
# Copy source code from previous stage
COPY . .
# Build application
RUN cargo build --release --target x86_64-unknown-linux-musl
# Install sqlx-cli in the builder stage so we can copy it later
# Adjust features based on your database (e.g., mysql, sqlite)
RUN cargo install sqlx-cli --no-default-features --features rustls,postgres

# Create a new stage with a minimal Alpine image
FROM alpine:latest
# Install runtime dependencies (e.g., ca-certificates for TLS)
RUN apk add --no-cache ca-certificates

WORKDIR /app

# Copy necessary artifacts from the builder stage
COPY --from=builder /dick_grower_bot/target/x86_64-unknown-linux-musl/release/dick_grower_bot /app/dick_grower_bot
# Copy sqlx-cli from the builder stage's cargo bin directory
COPY --from=builder /root/.cargo/bin/sqlx /usr/local/bin/sqlx
# Ensure the migrations directory exists in the source and copy it
COPY --from=builder /dick_grower_bot/migrations /app/migrations

# Add and make executable the entrypoint script
COPY entrypoint.sh /usr/local/bin/entrypoint.sh
RUN chmod +x /usr/local/bin/entrypoint.sh

# Set the entrypoint script to run on container start
ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]
# The command to run the application (will be executed by the entrypoint script)
CMD ["/app/dick_grower_bot"]