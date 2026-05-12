# Build stage
FROM rust:1.85-slim-bookworm as builder

WORKDIR /app

# Install dependencies needed for compiling (libssl, pkg-config, etc)
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Create a dummy project to cache dependencies
RUN cargo new --bin antri-poli
COPY Cargo.toml Cargo.lock ./antri-poli/
WORKDIR /app/antri-poli
RUN cargo build --release
RUN rm src/*.rs

# Copy the actual source code and static files
COPY src ./src
COPY static ./static

# Touch main.rs to ensure it gets rebuilt
RUN touch src/main.rs
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

WORKDIR /app

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Copy the binary from the build stage
COPY --from=builder /app/antri-poli/target/release/antri-poli .
# Copy static files (needed if not embedded in binary)
COPY static ./static

# Port to expose
EXPOSE 3030

# Command to run
CMD ["./antri-poli"]
