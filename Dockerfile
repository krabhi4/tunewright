# Stage 1: Build frontend
FROM node:22-slim AS frontend-builder
RUN corepack enable && corepack prepare pnpm@latest --activate
WORKDIR /app/frontend
COPY frontend/package.json frontend/pnpm-lock.yaml* frontend/pnpm-workspace.yaml* ./
RUN --mount=type=cache,target=/root/.local/share/pnpm/store \
    pnpm install --frozen-lockfile || pnpm install
COPY frontend/ ./
RUN pnpm run build

# Stage 2: Build Rust backend
# Track latest stable Rust (matches the stable CI job; avoids MSRV-pin breakage).
FROM rust:1-bookworm AS backend-builder
WORKDIR /app

COPY Cargo.toml Cargo.lock* ./
COPY crates/ crates/

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo build --release && \
    cp target/release/tunewright-server /app/tunewright-server

# Stage 3: Runtime
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=backend-builder /app/tunewright-server /usr/local/bin/
COPY --from=frontend-builder /app/frontend/build /srv/static

ENV TUNEWRIGHT_STATIC_DIR=/srv/static
ENV TUNEWRIGHT_DATA_DIR=/data
ENV TUNEWRIGHT_PORT=8080
ENV TUNEWRIGHT_HOST=0.0.0.0

EXPOSE 8080
VOLUME ["/data"]

ENTRYPOINT ["tunewright-server"]
