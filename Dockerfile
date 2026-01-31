# Build stage
FROM rust:alpine AS builder

RUN apk add --no-cache musl-dev openssl-dev openssl-libs-static pkgconfig

WORKDIR /app

# Copy everything and build
COPY Cargo.toml Cargo.lock ./
COPY src ./src

# Build for release
RUN cargo build --release

# Runtime stage
FROM alpine:3.19

RUN apk add --no-cache ca-certificates libgcc docker-cli

WORKDIR /app

# Copy binary from builder
COPY --from=builder /app/target/release/algojudge /app/algojudge

# Create data directories
RUN mkdir -p /data/submissions /data/test_cases

# Create non-root user
RUN adduser -D -u 1000 algojudge
RUN chown -R algojudge:algojudge /app /data

# Note: Running as root for Docker socket access
# In production, consider using Docker socket proxy

EXPOSE 8081

CMD ["/app/algojudge"]
