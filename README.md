# Tunewright

A self-hosted web application for editing audio file metadata. Inspired by [Mp3tag](https://www.mp3tag.de/en/) for Windows.

Tunewright runs as a single Docker container, serves a web UI, and operates directly on your music files via a volume mount. No database required.

## Features

- **Batch tag editing** — Edit title, artist, album, year, genre, track number and more across multiple files at once
- **Cover art** — View, add, replace, and remove embedded album artwork
- **File renaming** — Rename files using format strings like `%track% - %artist% - %title%`
- **MusicBrainz lookup** — Search releases, auto-fill tags, download cover art
- **Spreadsheet-style grid** — Virtual-scrolling file list with sortable columns and multi-select
- **Tag panel** — Quick-edit sidebar showing tags for selected files, with `< keep >` for mixed values
- **Multi-user auth** — First visitor creates admin account; invite others with shareable links
- **Dark theme** — Purpose-built UI, not a generic template

### Supported Formats

MP3, FLAC, M4A/MP4, OGG Vorbis, Opus, WAV, AIFF

### Tag Types

ID3v1, ID3v2.3, ID3v2.4, MP4/iTunes, Vorbis Comments, APE

## Quick Start

### Docker Compose (recommended)

```yaml
services:
  tunewright:
    image: ghcr.io/krabhi4/tunewright:latest
    ports:
      - "8080:8080"
    volumes:
      - /path/to/your/music:/data:rw
    restart: unless-stopped
```

```bash
docker compose up -d
```

Open `http://your-server:8080` in a browser. On first visit you'll be prompted to create your admin account.

### Docker Run

```bash
docker run -d \
  -p 8080:8080 \
  -v /path/to/your/music:/data:rw \
  ghcr.io/krabhi4/tunewright:latest
```

## Authentication

Authentication is built in and always active once an account exists.

### First-Time Setup

1. Start the container and open the web UI
2. You'll be redirected to the setup page
3. Choose a username and password — this becomes the **super admin** account
4. You're logged in and ready to go

### Inviting Users

1. Click your username in the toolbar (top-right)
2. Select **Manage Users**
3. Click **Create Invite** — a shareable link is generated (valid for 48 hours)
4. Send the link to the person you want to invite
5. They visit the link, choose a username and password, and get an **admin** account

### Roles

| Role | Access |
|------|--------|
| **Super Admin** | Full access + manage users (create invites, remove users) |
| **Admin** | Full access to all tag editing, renaming, and lookup features |

User accounts are stored in `users.json` inside your data directory and persist across container restarts. Passwords are hashed with Argon2id.

## Configuration

All configuration is via environment variables.

| Variable | Default | Description |
|----------|---------|-------------|
| `TUNEWRIGHT_DATA_DIR` | `/data` | Music directory inside the container |
| `TUNEWRIGHT_PORT` | `8080` | HTTP port |
| `TUNEWRIGHT_HOST` | `127.0.0.1` (`0.0.0.0` in the Docker image) | Bind address |
| `TUNEWRIGHT_STATIC_DIR` | `/srv/static` | Frontend build directory (set by Docker) |
| `TUNEWRIGHT_SETUP_TOKEN` | unset | Token required to claim the first admin account |
| `TUNEWRIGHT_COOKIE_SECURE` | `false` | Set to `true` when serving over HTTPS so the session cookie gets the `Secure` flag |

Authentication is managed through the web UI.

Until the first admin account exists, `/api/v1/auth/setup` is reachable without
authentication and hands the first caller super-admin. If the server binds a
non-loopback address with no `TUNEWRIGHT_SETUP_TOKEN` set, it generates one at
startup and prints it to the log — read it with `docker compose logs` and enter
it on the setup screen. Set `TUNEWRIGHT_SETUP_TOKEN` explicitly to choose your own.

Put `TUNEWRIGHT_COOKIE_SECURE=true` behind any HTTPS reverse proxy.

## Architecture

```
Rust (Axum)                          SvelteKit
┌─────────────────────┐              ┌──────────────────┐
│ tunewright-server    │  REST API    │ frontend/        │
│   routes/           │◄────────────►│   FileGrid       │
│   auth middleware    │  /api/v1/*   │   TagPanel       │
│   users.rs          │              │   UserMenu       │
│                     │              │   PathBar        │
│ tunewright-core      │              │   RenameModal    │
│   audio.rs (lofty)  │              │   LookupModal    │
│   scanner.rs        │              └──────────────────┘
│   format_string.rs  │
│   picture.rs        │
│   rename.rs         │
│                     │
│ tunewright-lookup    │
│   musicbrainz.rs    │
└─────────────────────┘
        │
    ┌───┴───┐
    │ /data │  (volume mount)
    └───────┘
```

- **tunewright-core** — Pure Rust library. Tag reading/writing via [lofty](https://github.com/Serial-ATA/lofty-rs), thumbnail generation via [image](https://github.com/image-rs/image), directory scanning, format string parser, file renaming.
- **tunewright-lookup** — MusicBrainz API client with rate limiting.
- **tunewright-server** — Axum HTTP server. Serves the SvelteKit SPA and REST API. Multi-user auth with Argon2id password hashing.

No database. Tag data lives in the audio files. User accounts live in `users.json`. UI state lives in the browser.

## Building from Source

### Prerequisites

- Rust 1.89+
- Node.js 20+ with pnpm
- Docker (for container builds)

### Development

```bash
# Backend
cargo build
cargo test

# Frontend
cd frontend
pnpm install
pnpm run dev    # Vite dev server with API proxy to localhost:8080

# Run backend
TUNEWRIGHT_DATA_DIR=./test-music cargo run -p tunewright-server
```

### Docker Build (single arch)

```bash
docker build -t tunewright:latest .
```

### Docker Build (multi-arch)

```bash
docker buildx create --name multiarch --driver docker-container --use
docker buildx build \
  --platform linux/amd64,linux/arm64 \
  -t ghcr.io/krabhi4/tunewright:latest \
  --push .
```

## API

All endpoints under `/api/v1/`. See the full API reference in [docs/api.md](docs/api.md).

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/health` | Health check |
| GET | `/files?path=/&limit=500` | List files in a directory |
| GET | `/files/tree?depth=2` | Directory tree |
| POST | `/tags/read` | Batch read tags (fast, no audio properties) |
| POST | `/tags/read-properties` | Batch read with audio properties (duration, bitrate) |
| POST | `/tags/write` | Batch write tags |
| GET | `/coverart?path=...&size=250` | Get cover art thumbnail |
| DELETE | `/coverart?path=...` | Remove cover art |
| POST | `/rename/preview` | Preview file renames |
| POST | `/rename/execute` | Execute file renames |
| GET | `/lookup/musicbrainz/search?query=...` | Search MusicBrainz |
| GET | `/lookup/musicbrainz/release/:mbid` | Get release details |
| POST | `/auth/setup` | Create first user (super admin) |
| POST | `/auth/login` | Login |
| POST | `/auth/logout` | Logout |
| GET | `/auth/check` | Check auth status / setup state |
| POST | `/auth/register` | Register via invite token |
| POST | `/auth/invites` | Create invite (super admin) |
| GET | `/auth/invites` | List active invites (super admin) |
| DELETE | `/auth/invites/:token` | Revoke invite (super admin) |
| GET | `/auth/users` | List users (super admin) |
| DELETE | `/auth/users/:id` | Remove user (super admin) |

## Tech Stack

- **Backend:** Rust, Axum, lofty, image, argon2, rayon, tokio
- **Frontend:** SvelteKit 5, TypeScript, adapter-static
- **Deployment:** Docker multi-stage build, Debian bookworm-slim runtime

## License

[MIT](LICENSE)
