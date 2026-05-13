# syntax=docker/dockerfile:1

ARG RUST_VERSION=1

FROM rust:${RUST_VERSION}-slim-bookworm AS builder
RUN apt-get update && apt-get install -y --no-install-recommends pkg-config libssl-dev cmake && rm -rf /var/lib/apt/lists/*
WORKDIR /app

RUN --mount=type=bind,source=crates,target=crates \
    --mount=type=bind,source=migrations,target=migrations \
    --mount=type=bind,source=Cargo.toml,target=Cargo.toml \
    --mount=type=bind,source=Cargo.lock,target=Cargo.lock \
    --mount=type=bind,source=.sqlx,target=.sqlx \
    --mount=type=cache,target=/app/target/ \
    --mount=type=cache,target=/usr/local/cargo/git/db \
    --mount=type=cache,target=/usr/local/cargo/registry/ \
    SQLX_OFFLINE=true cargo build --locked --release && \
    cp ./target/release/api /bin/api && \
    cp ./target/release/worker /bin/worker

FROM debian:bookworm-slim AS base
RUN apt-get update && apt-get install -y --no-install-recommends \
    adduser \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

ARG UID=10001
RUN adduser \
    --disabled-password \
    --gecos "" \
    --home "/nonexistent" \
    --shell "/usr/sbin/nologin" \
    --no-create-home \
    --uid "${UID}" \
    appuser
USER appuser

FROM base AS api
COPY --from=builder /bin/api /bin/
CMD ["/bin/api"]

FROM base AS worker
COPY --from=builder /bin/worker /bin/
CMD ["/bin/worker"]
