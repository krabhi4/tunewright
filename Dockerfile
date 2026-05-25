# Stage 1: Build frontend
FROM node:22-slim AS frontend-builder
RUN corepack enable && corepack prepare pnpm@latest --activate
WORKDIR /app/frontend
COPY frontend/package.json frontend/pnpm-lock.yaml* frontend/pnpm-workspace.yaml* ./
RUN pnpm install --frozen-lockfile || pnpm install
COPY frontend/ ./
RUN pnpm run build

# Stage 2: Build Rust backend
FROM rust:1.86-bookworm AS backend-builder
WORKDIR /app

# Cache dependencies by building with stub sources first
COPY Cargo.toml Cargo.lock* ./
COPY crates/tagstudio-server/Cargo.toml crates/tagstudio-server/
COPY crates/tagstudio-core/Cargo.toml crates/tagstudio-core/
COPY crates/tagstudio-lookup/Cargo.toml crates/tagstudio-lookup/
RUN mkdir -p crates/tagstudio-server/src crates/tagstudio-core/src crates/tagstudio-lookup/src \
    && echo "fn main(){}" > crates/tagstudio-server/src/main.rs \
    && echo "" > crates/tagstudio-core/src/lib.rs \
    && echo "" > crates/tagstudio-lookup/src/lib.rs \
    && cargo build --release 2>/dev/null || true \
    && rm -rf crates/

# Build real source
COPY crates/ crates/
RUN touch crates/tagstudio-server/src/main.rs \
    && touch crates/tagstudio-core/src/lib.rs \
    && touch crates/tagstudio-lookup/src/lib.rs \
    && cargo build --release

# Stage 3: Runtime
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=backend-builder /app/target/release/tagstudio-server /usr/local/bin/
COPY --from=frontend-builder /app/frontend/build /srv/static

ENV TAGSTUDIO_STATIC_DIR=/srv/static
ENV TAGSTUDIO_DATA_DIR=/data
ENV TAGSTUDIO_PORT=8080
ENV TAGSTUDIO_HOST=0.0.0.0

EXPOSE 8080
VOLUME ["/data"]

ENTRYPOINT ["tagstudio-server"]
