FROM rust:1-slim-bookworm AS builder
RUN apt-get update && apt-get install -y pkg-config libssl-dev cmake && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY backend/ .
RUN SQLX_OFFLINE=true cargo build --release

FROM debian:bookworm-slim AS api
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/api /usr/local/bin/api
CMD ["api"]

FROM debian:bookworm-slim AS worker
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/worker /usr/local/bin/worker
CMD ["worker"]
