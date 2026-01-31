# AlgoJudge Zig Benchmark Container
FROM debian:bookworm-slim

# Install Zig and required tools
RUN apt-get update && apt-get install -y \
    curl \
    xz-utils \
    time \
    && rm -rf /var/lib/apt/lists/*

# Install Zig
RUN curl -L https://ziglang.org/download/0.13.0/zig-linux-x86_64-0.13.0.tar.xz | tar -xJ -C /opt \
    && ln -s /opt/zig-linux-x86_64-0.13.0/zig /usr/local/bin/zig

# Create workspace
WORKDIR /workspace

# Create non-root user
RUN useradd -m -u 1000 judge
RUN chown -R judge:judge /workspace

USER judge

CMD ["/bin/bash"]
