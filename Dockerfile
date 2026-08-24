# syntax=docker/dockerfile:1

# So I tried to use cargo chef but it's hella broken with dioxus. So it's Build Cache Mounts time 

# -------------------------
# --- STAGE 1 : builder ---
# -------------------------

# 1. Debian 13.6 for building
FROM rust:1-slim-trixie AS builder

# 2. Set the Dioxus version
ARG DIOXUS_CLI_VERSION=0.7.10

# 3. Install build dependencies and wasm target
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    curl \
    && rm -rf /var/lib/apt/lists/* \
    && rustup target add wasm32-unknown-unknown

# 4. Install cargo-binstall
RUN curl -L --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | bash

# 5. Install Dioxus CLI
RUN cargo binstall dioxus-cli@${DIOXUS_CLI_VERSION} -y --force

# 6. Change working directory
WORKDIR /app

# 7. Copy ONLY configuration and dependency manifests
COPY Cargo.toml Cargo.lock Dioxus.toml ./

# 8. Create a dummy Dioxus fullstack app
RUN mkdir -p src && \
    cat << 'EOF' > src/main.rs
use dioxus::prelude::*;
fn main() {
    #[cfg(feature = "server")]
    dioxus::serve(|| async move { Ok(dioxus::server::router(app)) });
    #[cfg(not(feature = "server"))]
    dioxus::launch(app);
}
fn app() -> Element { rsx! { div { "dummy" } } }
EOF

# 9. Build all but ONLY dependencies
RUN dx bundle --web --release && \
    rm -rf src

# 10. NOW Copy the real source code
COPY . .

# 11. Build the real application
RUN dx bundle --web --release && \
    mkdir -p /app/dist && \
    cp -r /app/target/dx/leaderboule/release/web/* /app/dist/

# -------------------------
# --- STAGE 2 : runtime ---
# -------------------------

# 1. Still Debian 13.6 for runtime
FROM debian:trixie-slim AS runtime

# 2. Install SSL certificates and runtime libraries needed by surrealdb/reqwest
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/* \
    && useradd -m -u 10001 -s /bin/sh appuser

# 3. Change directory
WORKDIR /usr/local/app

# 4. Copy files from the builder stage to the runtime stage
COPY --from=builder --chown=appuser:appuser /app/dist /usr/local/app

# 5. Create the appuser user
USER appuser

# 6. Define PORT Variable
ENV PORT=8080

# 7. Define IP Variable
ENV IP=0.0.0.0

# 8. Exposes the 8080 port 
EXPOSE 8080

# 9. Exposes the executable of the server
ENTRYPOINT [ "/usr/local/app/server" ]
