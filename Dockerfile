# Build stage
FROM rust:1.87-slim AS builder

WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

# Cache dependencies layer
COPY Cargo.toml Cargo.lock ./
COPY src ./src

# Build server binary only (skip wasm/cli features for smaller image)
RUN cargo build --release --bin zkp

# Runtime stage
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/zkp /usr/local/bin/zkp

# Create data directory for sled
RUN mkdir -p /app/data

ENV RUST_LOG=info
EXPOSE 3000

CMD ["zkp"]
