# Multi-stage Dockerfile for building and running ghop

# 1) Build stage
FROM rust:1-bookworm AS builder

# Create a new empty shell project
WORKDIR /app

# Pre-copy manifest files to leverage Docker layer caching
COPY Cargo.toml Cargo.lock ./
COPY build.rs ./

# Create an empty src to allow dependency build caching
RUN mkdir -p src && echo "fn main() {}" > src/main.rs

# Build dependencies (this will be cached unless Cargo.toml changes)
RUN cargo build --release || true

# Now copy the actual source
COPY src ./src
COPY README.md ./README.md
COPY ghop.yml ./ghop.yml

# Build the real binary
RUN cargo build --release


# 2) Runtime stage
FROM debian:bookworm-slim AS runtime

# Install minimal runtime essentials
# - ca-certificates: for any HTTPS subprocesses/tools
# - bash: ghop launches commands via a shell; ensure a familiar shell exists
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        bash \
    && rm -rf /var/lib/apt/lists/*

# Create an unprivileged user to run the app
RUN useradd -m -u 1000 ghop

# Work directory where you can mount your project (e.g., with ghop.yml)
WORKDIR /work

# Copy the compiled binary
COPY --from=builder /app/target/release/ghop /usr/local/bin/ghop

# Drop privileges
USER ghop

# Default entrypoint shows help if no args are provided
ENTRYPOINT ["ghop"]
CMD ["--help"]

# Usage examples:
#
# Build the image:
#   docker build -t ghop:latest .
#
# Show help:
#   docker run --rm -it ghop:latest --help
#
# Run a set from a mounted ghop.yml:
#   docker run --rm -it -v "$PWD":/work ghop:latest -f ghop.yml dev
