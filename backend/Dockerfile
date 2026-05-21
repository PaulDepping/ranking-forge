# syntax=docker/dockerfile:1

ARG RUST_VERSION=1

FROM rust:${RUST_VERSION}-slim-bookworm AS chef
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev cmake \
    && rm -rf /var/lib/apt/lists/* \
    && cargo install cargo-chef
WORKDIR /app

FROM chef AS planner
RUN --mount=type=bind,source=crates,target=crates \
    --mount=type=bind,source=Cargo.toml,target=Cargo.toml \
    --mount=type=bind,source=Cargo.lock,target=Cargo.lock \
    cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
COPY Cargo.lock Cargo.lock
# Compile dependencies only — this layer is cached until Cargo.lock changes
RUN --mount=type=cache,target=/usr/local/cargo/git/db \
    --mount=type=cache,target=/usr/local/cargo/registry/ \
    cargo chef cook --release --locked --recipe-path recipe.json
# Compile source — starts from pre-built deps above, so only our code is compiled
RUN --mount=type=bind,source=crates,target=crates \
    --mount=type=bind,source=migrations,target=migrations \
    --mount=type=bind,source=Cargo.toml,target=Cargo.toml \
    --mount=type=bind,source=Cargo.lock,target=Cargo.lock \
    --mount=type=bind,source=.sqlx,target=.sqlx \
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
