# TagStudio

A self-hosted web application for editing audio file metadata. Inspired by [Mp3tag](https://www.mp3tag.de/en/) for Windows.

TagStudio runs as a single Docker container, serves a web UI, and operates directly on your music files via a volume mount. No database required.

## Features

- **Batch tag editing** — Edit title, artist, album, year, genre, track number and more across multiple files at once
- **Cover art** — View, add, replace, and remove embedded album artwork
- **File renaming** — Rename files using format strings like `%track% - %artist% - %title%`
- **MusicBrainz lookup** — Search releases, auto-fill tags, download cover art
- **Spreadsheet-style grid** — Virtual-scrolling file list with sortable columns and multi-select
- **Tag panel** — Quick-edit sidebar showing tags for selected files, with `< keep >` for mixed values
- **Dark theme** — Purpose-built UI, not a generic template

### Supported Formats

MP3, FLAC, M4A/MP4, OGG Vorbis, Opus, WAV, AIFF

### Tag Types

ID3v1, ID3v2.3, ID3v2.4, MP4/iTunes, Vorbis Comments, APE

## Quick Start

### Docker Compose (recommended)

```yaml
services:
  tagstudio:
    image: ghcr.io/YOUR_USERNAME/tagstudio:latest
    ports:
      - "8080:8080"
    volumes:
      - /path/to/your/music:/data:rw
    environment:
      - TAGSTUDIO_AUTH_ENABLED=false
    restart: unless-stopped
```

```bash
docker compose up -d
```

Open `http://your-server:8080` in a browser.

### Docker Run

```bash
docker run -d \
  -p 8080:8080 \
  -v /path/to/your/music:/data:rw \
  -e TAGSTUDIO_AUTH_ENABLED=false \
  ghcr.io/YOUR_USERNAME/tagstudio:latest
```

### With Authentication

```yaml
services:
  tagstudio:
    image: ghcr.io/YOUR_USERNAME/tagstudio:latest
    ports:
      - "8080:8080"
    volumes:
      - /path/to/your/music:/data:rw
    environment:
      - TAGSTUDIO_AUTH_ENABLED=true
      - TAGSTUDIO_USERNAME=admin
      - TAGSTUDIO_PASSWORD=your-secure-password
    restart: unless-stopped
```

## Configuration

All configuration is via environment variables.

| Variable | Default | Description |
|----------|---------|-------------|
| `TAGSTUDIO_DATA_DIR` | `/data` | Music directory inside the container |
| `TAGSTUDIO_PORT` | `8080` | HTTP port |
| `TAGSTUDIO_HOST` | `0.0.0.0` | Bind address |
| `TAGSTUDIO_AUTH_ENABLED` | `false` | Enable username/password authentication |
| `TAGSTUDIO_USERNAME` | `admin` | Login username (when auth enabled) |
| `TAGSTUDIO_PASSWORD` | `changeme` | Login password (when auth enabled) |
| `TAGSTUDIO_SESSION_SECRET` | auto-generated | HMAC key for session cookies |
| `DISCOGS_API_TOKEN` | — | Optional Discogs API token for lookups |
| `TAGSTUDIO_STATIC_DIR` | `/srv/static` | Frontend build directory (set by Docker) |

## Architecture

```
Rust (Axum)                          SvelteKit
┌─────────────────────┐              ┌──────────────────┐
│ tagstudio-server    │  REST API    │ frontend/        │
│   routes/           │◄────────────►│   FileGrid       │
│   auth middleware    │  /api/v1/*   │   TagPanel       │
│                     │              │   PathBar        │
│ tagstudio-core      │              │   RenameModal    │
│   audio.rs (lofty)  │              │   LookupModal    │
│   scanner.rs        │              └──────────────────┘
│   format_string.rs  │
│   picture.rs        │
│   rename.rs         │
│                     │
│ tagstudio-lookup    │
│   musicbrainz.rs    │
└─────────────────────┘
        │
    ┌───┴───┐
    │ /data │  (volume mount)
    └───────┘
```

- **tagstudio-core** — Pure Rust library. Tag reading/writing via [lofty](https://github.com/Serial-ATA/lofty-rs), thumbnail generation via [image](https://github.com/image-rs/image), directory scanning, format string parser, file renaming.
- **tagstudio-lookup** — MusicBrainz API client with rate limiting.
- **tagstudio-server** — Axum HTTP server. Serves the SvelteKit SPA and REST API. Optional cookie-based auth.

No database. Tag data lives in the audio files. UI state lives in the browser. Server is stateless.

## Building from Source

### Prerequisites

- Rust 1.86+
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
TAGSTUDIO_DATA_DIR=./test-music cargo run -p tagstudio-server
```

### Docker Build (single arch)

```bash
docker build -t tagstudio:latest .
```

### Docker Build (multi-arch)

```bash
docker buildx create --name multiarch --driver docker-container --use
docker buildx build \
  --platform linux/amd64,linux/arm64 \
  -t ghcr.io/YOUR_USERNAME/tagstudio:latest \
  --push .
```

## API

All endpoints under `/api/v1/`. See the full API reference in [docs/api.md](docs/api.md) (TODO).

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
| POST | `/auth/login` | Login |
| POST | `/auth/logout` | Logout |
| GET | `/auth/check` | Check auth status |

## Tech Stack

- **Backend:** Rust, Axum, lofty, image, rayon, tokio
- **Frontend:** SvelteKit 5, TypeScript, adapter-static
- **Deployment:** Docker multi-stage build, Debian bookworm-slim runtime

## License

[MIT](LICENSE)
