# Stage 1: Build frontend
FROM --platform=$BUILDPLATFORM node:22-slim@sha256:43ac6c60b8f89723f746e8a92ce91abd5017e627ce1ddfe4238355d3a30b772c AS frontend-builder
RUN corepack enable && corepack prepare pnpm@9.15.4 --activate
WORKDIR /app/frontend
COPY frontend/package.json frontend/pnpm-lock.yaml* frontend/pnpm-workspace.yaml* ./
RUN --mount=type=cache,target=/root/.local/share/pnpm/store \
    pnpm install --frozen-lockfile
COPY frontend/ ./
RUN pnpm run build

# Stage 2: Build Rust backend (cargo-chef splits dependency compilation into
# its own cacheable layer).
# Stable Rust, digest-pinned; Dependabot proposes digest bumps.
FROM rust:1-bookworm@sha256:93ce27a88655056a51dbdd8f5f2d7ddc071c7b0070fb288a37b5a285fc83971e AS chef
RUN cargo install cargo-chef --version 0.1.78 --locked
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
FROM gcr.io/distroless/cc-debian12:nonroot@sha256:9dac0a79194e45a7da0158a9c6da57b217585af0786db3845d1f0ec1a0dd182f

COPY --from=backend-builder /app/tunewright-server /usr/local/bin/tunewright-server
COPY --from=frontend-builder /app/frontend/build /srv/static

ENV TUNEWRIGHT_STATIC_DIR=/srv/static
ENV TUNEWRIGHT_DATA_DIR=/data
ENV TUNEWRIGHT_PORT=8080
ENV TUNEWRIGHT_HOST=0.0.0.0

EXPOSE 8080
VOLUME ["/data"]

ENTRYPOINT ["/usr/local/bin/tunewright-server"]
