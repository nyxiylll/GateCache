FROM rust:1.82-slim AS builder
WORKDIR /app

# Cache dependencies layer separately for faster rebuilds
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs \
    && cargo build --release \
    && rm -rf src

# Build the real source
COPY src ./src
RUN touch src/main.rs && cargo build --release

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/Gatecache /usr/local/bin/gatecache

EXPOSE 3000
ENV RUST_LOG=info
CMD ["gatecache"]
