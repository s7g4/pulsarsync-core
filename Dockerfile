# Multi-stage build for PulsarSync SDR-Appliance Host Daemon
FROM rust:1.82-slim AS builder

WORKDIR /usr/src/pulsarsync-core

# Install required system build dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    && rm -rf /var/lib/apt/lists/*

# Copy configuration and source files
COPY rust-toolchain.toml Cargo.toml Cargo.lock ./
COPY src/ ./src
COPY html/ ./html

# Build the host application in release mode
RUN cargo build --release --features host-testing

# Final minimal runtime image
FROM debian:bookworm-slim

WORKDIR /app

# Copy the compiled binary from the builder stage
COPY --from=builder /usr/src/pulsarsync-core/target/release/pulsarsync-core /app/pulsarsync-core

# Expose UDP socket for VITA-49 ingestion
EXPOSE 8088/udp

# Expose HTTP socket for Web Telemetry Dashboard
EXPOSE 8082

# Start the appliance daemon
ENTRYPOINT ["/app/pulsarsync-core"]
