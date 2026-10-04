FROM lukemathwalker/cargo-chef:latest-rust-1-trixie AS chef
WORKDIR /app

FROM chef AS planner
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

COPY . .
# Queries are checked against the committed .sqlx cache instead of a live database.
ENV SQLX_OFFLINE=true
RUN cargo build --release

# Must match the builder's Debian release (glibc version).
FROM debian:trixie-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/dick_grower_bot /app/dick_grower_bot

# Secrets are passed at runtime (`--env-file`), never baked into the image.
ENV DATABASE_URL=sqlite:/app/database.sqlite
CMD ["/app/dick_grower_bot"]
