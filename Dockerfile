# Build stage
FROM rust:1.85-bookworm AS builder

WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/

# Build all binaries in release mode
RUN cargo build --release --workspace

# Runtime stage for Vanguard (API Gateway)
FROM debian:bookworm-slim AS vanguard

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/vanguard /usr/local/bin/vanguard

ENV RUST_LOG=vanguard=info,sqlx=warn
EXPOSE 8080

CMD ["vanguard"]

# Runtime stage for Sisyphus (Compiler)
FROM debian:bookworm-slim AS sisyphus

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    docker.io \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/sisyphus /usr/local/bin/sisyphus

ENV RUST_LOG=sisyphus=info,sqlx=warn
CMD ["sisyphus"]

# Runtime stage for Minos (Judge)
FROM debian:bookworm-slim AS minos

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/minos /usr/local/bin/minos

ENV RUST_LOG=minos=info,sqlx=warn
EXPOSE 9091

CMD ["minos"]

# Runtime stage for Horus (Cleaner)
FROM debian:bookworm-slim AS horus

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/horus /usr/local/bin/horus

ENV RUST_LOG=horus=info,sqlx=warn
CMD ["horus"]
