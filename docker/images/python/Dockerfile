# AlgoJudge Python Benchmark Container
FROM python:3.12-bookworm

# Install required tools
RUN apt-get update && apt-get install -y \
    time \
    && rm -rf /var/lib/apt/lists/*

# Create workspace
WORKDIR /workspace

# Create non-root user
RUN useradd -m -u 1000 judge
RUN chown -R judge:judge /workspace

USER judge

CMD ["/bin/bash"]
