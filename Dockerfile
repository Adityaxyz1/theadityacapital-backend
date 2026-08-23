# Railway's Railpack auto-detection ignores rust-toolchain.toml and ships an
# older pinned rustc (1.85.1) that several dependencies (mongodb, time,
# serde_with, hickory-resolver, darling) now require newer than — this
# Dockerfile takes over the build entirely so the Rust version is explicit
# and not at the mercy of Railpack's own defaults.
FROM rust:1.88-bookworm AS builder
WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Cache dependency compilation separately from source changes.
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
RUN mkdir src && echo "fn main() {}" > src/main.rs \
    && cargo build --release \
    && rm -rf src

COPY src ./src
RUN touch src/main.rs && cargo build --release

FROM debian:bookworm-slim
WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/aditya-crm-api ./aditya-crm-api

# Railway injects PORT; the app already reads it (see src/config.rs) and
# binds 0.0.0.0:{port} (src/main.rs) — no extra wiring needed here.
CMD ["./aditya-crm-api"]
