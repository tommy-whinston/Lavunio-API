FROM rust:1.99-slim AS builder

RUN apt-get update \
    && apt-get install -y --no-install-recommends build-essential pkg-config ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src/bin \
    && printf '' > src/lib.rs \
    && printf 'fn main() {}' > src/main.rs \
    && for name in seed role token migrate_assets; do printf 'fn main() {}' > "src/bin/$name.rs"; done \
    && cargo build --release --bins \
    && rm -rf src

COPY src ./src
COPY migrations ./migrations
COPY seed ./seed

RUN cargo build --release --bin lavunio-api --bin seed --bin role --bin token --bin migrate_assets

FROM debian:trixie-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/lavunio-api /usr/local/bin/lavunio-api
COPY --from=builder /app/target/release/seed /usr/local/bin/seed
COPY --from=builder /app/target/release/role /usr/local/bin/role
COPY --from=builder /app/target/release/token /usr/local/bin/token
COPY --from=builder /app/target/release/migrate_assets /usr/local/bin/migrate_assets
COPY seed ./seed

ENV HOST=0.0.0.0
ENV PORT=8080
ENV STORAGE_DIR=/app/data/uploads

EXPOSE 8080
CMD ["lavunio-api"]
