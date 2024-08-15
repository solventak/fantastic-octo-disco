# Start from the official Rust image for building
FROM rust:latest as builder

WORKDIR /blocky

# Copy the Cargo.toml and Cargo.lock files
COPY ./Cargo.toml ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release
RUN rm src/*.rs

# Copy the source code
COPY ./src src

# add this here so that we "touch" main.rs and do a rebuild when needed
RUN cat src/main.rs > src/main2.rs
RUN rm src/main.rs && mv src/main2.rs src/main.rs
RUN cargo build --release

# Create a minimal final image
FROM debian:latest

# Install necessary runtime dependencies (if any)
RUN apt-get update && apt-get install -y \
    openssl \
    ca-certificates

# Copy the compiled binary from the build stage
COPY --from=builder /blocky/target/release/blocky /usr/local/bin/blocky
#
## Set the startup command
CMD ["blocky"]
