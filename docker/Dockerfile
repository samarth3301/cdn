# Use the official Rust image as the base image
FROM rust:latest as builder

# Set the working directory
WORKDIR /app

# Copy the Cargo.toml and Cargo.lock files
COPY Cargo.toml Cargo.lock ./

# Copy the source code
COPY src ./src

# Build the application in release mode
RUN cargo build --release

# Use a smaller base image for the final image
FROM ubuntu:24.04

# Install necessary dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create a non-root user
RUN useradd -r -s /bin/false cdnuser

# Set the working directory
WORKDIR /app

# Copy the binary from the builder stage
COPY --from=builder /app/target/release/s3-cdn-network /app/s3-cdn-network

# Change ownership of the binary
RUN chown cdnuser:cdnuser /app/s3-cdn-network

# Switch to the non-root user
USER cdnuser

# Expose the port the app runs on
EXPOSE 9876

# Enable unbuffered output and set logging
ENV RUST_LOG=s3_cdn_network=info,tower_http=debug,axum=debug
ENV RUST_BACKTRACE=1

# Set the default command to run the application
CMD ["./s3-cdn-network"]
