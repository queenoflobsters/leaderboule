# syntax=docker/dockerfile:1

# So I tried to use cargo chef but it's hella broken with dioxus. So it's Build Cache Mounts time 

# --- STAGE 1 : builder ---
FROM rust:1-slim-trixie AS builder

ARG DIOXUS_CLI_VERSION=0.7.10

# 1. Install build dependencies needed for crates like reqwest/surrealdb
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    curl \
    && rm -rf /var/lib/apt/lists \
    && rustup target add wasm32-unknown-unknown

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    curl -L --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | bash
RUN cargo binstall dioxus-cli@${DIOXUS_CLI_VERSION} -y --force

WORKDIR /app

COPY . .

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/app/target \
    dx bundle --web --release && \
    mkdir -p /app/dist && \
    cp -r /app/target/dx/leaderboule/release/web/* /app/dist/

# --- STAGE 2 : runtime ---
FROM debian:trixie-slim AS runtime

# Install SSL certificates and runtime libraries needed by surrealdb/reqwest
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/* \
    && useradd -m -u 10001 -s /bin/sh appuser

WORKDIR /usr/local/app

COPY --from=builder --chown=appuser:appuser /app/dist /usr/local/app

USER appuser

ENV PORT=8080
ENV IP=0.0.0.0
EXPOSE 8080
ENTRYPOINT [ "/usr/local/app/server" ]
