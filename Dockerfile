# Build stage - use nightly for edition2024 support
FROM rustlang/rust:nightly-bookworm AS builder

WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# ── Dependency caching layer ──────────────────────────────────────────
# Copy only manifests first so that `cargo build` fetches and compiles
# all third-party crates.  This layer is cached until Cargo.toml/lock
# change, saving minutes on incremental rebuilds.
COPY Cargo.toml Cargo.lock ./
COPY crates/olympus-common/Cargo.toml  crates/olympus-common/Cargo.toml
COPY crates/olympus-rules/Cargo.toml   crates/olympus-rules/Cargo.toml
COPY crates/vanguard/Cargo.toml        crates/vanguard/Cargo.toml
COPY crates/sisyphus/Cargo.toml        crates/sisyphus/Cargo.toml
COPY crates/minos/Cargo.toml           crates/minos/Cargo.toml
COPY crates/horus/Cargo.toml           crates/horus/Cargo.toml

# Create dummy source files so cargo can resolve the workspace and
# compile all dependencies without the real source code.
RUN mkdir -p crates/olympus-common/src && echo "pub fn _dummy(){}" > crates/olympus-common/src/lib.rs \
 && mkdir -p crates/olympus-rules/src  && echo "pub fn _dummy(){}" > crates/olympus-rules/src/lib.rs \
 && mkdir -p crates/vanguard/src       && echo "fn main(){}"       > crates/vanguard/src/main.rs \
 && mkdir -p crates/sisyphus/src       && echo "fn main(){}"       > crates/sisyphus/src/main.rs \
 && mkdir -p crates/minos/src          && echo "fn main(){}"       > crates/minos/src/main.rs \
 && mkdir -p crates/horus/src          && echo "fn main(){}"       > crates/horus/src/main.rs

RUN cargo build --release --workspace

# ── Real source build ─────────────────────────────────────────────────
# Remove the dummy sources and copy the real code.  Only our crates are
# recompiled; all dependencies stay cached from the layer above.
RUN rm -rf crates/
COPY crates/ crates/

# Touch source files so cargo sees them as newer than the dummy artifacts.
RUN find crates/ -name "*.rs" -exec touch {} +

# Build all binaries in release mode
RUN cargo build --release --workspace

# Runtime stage for Vanguard (API Gateway)
FROM debian:bookworm-slim AS vanguard

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    curl \
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
# Needs latest C/C++ runtime libraries because it directly executes
# uploaded problem binaries (generators/checkers) that may have been
# compiled with any modern GCC (17+).
FROM ubuntu:24.04 AS minos

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3t64 \
    g++ \
    && apt-get clean && rm -rf /var/lib/apt/lists/*

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
