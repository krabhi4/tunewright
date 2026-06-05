# Contributing to Tunewright

Thanks for your interest in contributing. This document covers how to get started.

## Development Setup

### Prerequisites

- Rust 1.89+ (`rustup` recommended)
- Node.js 20+ with pnpm
- Some audio files for testing (MP3, FLAC, etc.)

### Clone and Build

```bash
git clone https://github.com/YOUR_USERNAME/tunewright.git
cd tunewright

# Backend
cargo build
cargo test

# Frontend
cd frontend
pnpm install
pnpm run build
```

### Running Locally

Terminal 1 — backend:
```bash
mkdir -p test-music
# Put some audio files in test-music/
TUNEWRIGHT_DATA_DIR=./test-music TUNEWRIGHT_STATIC_DIR=./frontend/build cargo run -p tunewright-server
```

Terminal 2 — frontend dev server (hot reload):
```bash
cd frontend
pnpm run dev
```

The Vite dev server proxies `/api` requests to `localhost:8080`.

## Project Structure

```
crates/
  tunewright-core/       Pure Rust library (no HTTP)
    src/
      audio.rs          Tag reading/writing (lofty wrapper)
      picture.rs        Cover art extraction/embedding
      scanner.rs        Directory scanning
      format_string.rs  %artist% - %title% parser
      rename.rs         File renaming with collision detection
      types.rs          Shared types and error enum

  tunewright-lookup/     External API clients
    src/
      musicbrainz.rs    MusicBrainz search + release lookup

  tunewright-server/     Axum HTTP server
    src/
      main.rs           Entry point
      routes/           API endpoint handlers
      auth.rs           Multi-user authentication endpoints + middleware
      users.rs          User/invite storage (JSON file, Argon2 hashing)
      state.rs          Shared server state (sessions, config)
      config.rs         Environment variable parsing

frontend/               SvelteKit SPA
  src/
    lib/api/            Typed API client functions (incl. auth.ts)
    lib/stores/         Svelte stores (files, tags, ui, auth)
    lib/components/     UI components (incl. UserMenu, UserManagementModal)
    routes/             SvelteKit pages (/, /login, /setup, /register)
```

## Making Changes

### Backend

- `tunewright-core` contains all domain logic and should have no HTTP awareness
- All filesystem operations must go through `resolve_safe_path()` to prevent path traversal
- Blocking I/O in route handlers must use `tokio::task::spawn_blocking`
- Use `read_tags_fast()` for grid display, `read_tags_full()` only when audio properties are needed
- Add tests for new functionality in `tunewright-core`

### Frontend

- Components go in `src/lib/components/` organized by feature
- API calls go in `src/lib/api/` as typed functions
- State management uses Svelte stores in `src/lib/stores/`
- Keep the design system CSS variables in `app.css` — don't hardcode colors
- The grid uses virtual scrolling — avoid operations that touch all rows

### Commit Messages

Use conventional-ish commits:

```
feat: add batch cover art import
fix: handle files with missing ID3 header
refactor: extract tag merging into shared function
docs: add API endpoint documentation
```

## Pull Requests

1. Fork the repo and create a branch from `main`
2. Make your changes
3. Run `cargo test` and `cargo clippy`
4. Run `cd frontend && pnpm run build` to verify frontend compiles
5. Open a PR with a clear description of what and why

### What Makes a Good PR

- Focused on a single change
- Includes tests for new backend functionality
- Doesn't introduce new dependencies without justification
- Follows existing code patterns

## Reporting Bugs

Open an issue with:

- What you expected to happen
- What actually happened
- Steps to reproduce
- Your setup (Docker version, architecture, browser, music directory size)
- Container logs if relevant (`docker logs tunewright`)

## Feature Requests

Open an issue describing the feature and why it's useful. If it's a large feature, discuss before implementing.

## Areas Where Help Is Welcome

- **Audio format support** — Adding support for more formats (WavPack, Musepack, APE, etc.)
- **Actions system** — Batch operations like case conversion, text replacement (like Mp3tag's Actions)
- **Discogs integration** — The lookup module has a placeholder for Discogs
- **Performance** — Optimizing tag reading for very large libraries (10k+ files)
- **Accessibility** — Keyboard navigation, screen reader support
- **Tests** — Integration tests for API endpoints, frontend component tests
- **Documentation** — API docs, user guide
