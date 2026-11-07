FROM rust:1.99-slim AS builder

RUN apt-get update \
    && apt-get install -y --no-install-recommends build-essential pkg-config ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY migrations ./migrations
COPY seed ./seed

RUN cargo build --release --bin lavunio-api

FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/lavunio-api /usr/local/bin/lavunio-api

ENV HOST=0.0.0.0
ENV PORT=8080
ENV STORAGE_DIR=/app/data/uploads

EXPOSE 8080
CMD ["lavunio-api"]
