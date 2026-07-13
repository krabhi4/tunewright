# Stage 1: Build frontend
FROM node:22-slim AS frontend-builder
RUN corepack enable && corepack prepare pnpm@9.15.4 --activate
WORKDIR /app/frontend
COPY frontend/package.json frontend/pnpm-lock.yaml* frontend/pnpm-workspace.yaml* ./
RUN --mount=type=cache,target=/root/.local/share/pnpm/store \
    pnpm install --frozen-lockfile
COPY frontend/ ./
RUN pnpm run build

# Stage 2: Build Rust backend (cargo-chef splits dependency compilation into
# its own cacheable layer).
# Track latest stable Rust (matches the stable CI job; avoids MSRV-pin breakage).
FROM rust:1-bookworm AS chef
RUN cargo install cargo-chef --locked
WORKDIR /app

FROM chef AS planner
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS backend-builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --locked --recipe-path recipe.json

COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
RUN cargo build --release --locked && \
    cp target/release/tunewright-server /app/tunewright-server

# Stage 3: Runtime (distroless ships glibc + CA certs; runs as uid 65532)
FROM gcr.io/distroless/cc-debian12:nonroot

COPY --from=backend-builder /app/tunewright-server /usr/local/bin/tunewright-server
COPY --from=frontend-builder /app/frontend/build /srv/static

ENV TUNEWRIGHT_STATIC_DIR=/srv/static
ENV TUNEWRIGHT_DATA_DIR=/data
ENV TUNEWRIGHT_PORT=8080
ENV TUNEWRIGHT_HOST=0.0.0.0

EXPOSE 8080
VOLUME ["/data"]

ENTRYPOINT ["/usr/local/bin/tunewright-server"]
