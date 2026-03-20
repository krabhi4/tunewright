# Changelog

All notable changes to TagStudio are documented here.

## [0.2.0] — 2026-03-20

### Added

- **Multi-user authentication** — First visitor creates a super admin account via web UI. No more environment variable credentials.
- **Invite system** — Super admins can generate 48-hour invite links for new users.
- **User management UI** — Toolbar dropdown with user menu; super admins get a modal to list users, create invites, and remove accounts.
- **Setup page** — `/setup` route for first-run account creation with password confirmation.
- **Registration page** — `/register?token=...` route for invited users to create accounts.
- **Persistent user storage** — User accounts and invites stored in `users.json` (in data directory), surviving container restarts.
- **Password hashing** — Argon2id for all stored passwords (via `argon2` crate).
- **Role-based access** — Two roles: `super_admin` (full access + user management) and `admin` (full access, no user management).
- **Session purge on user deletion** — Removing a user immediately invalidates all their active sessions.
- **Atomic file operations** — User data writes use temp file + rename for crash safety, with in-memory rollback on write failure.
- New API endpoints: `/auth/setup`, `/auth/register`, `/auth/invites`, `/auth/users`.
- New frontend components: `UserMenu`, `UserManagementModal`.
- New frontend stores: `auth.ts` for reactive auth state.
- New frontend API module: `auth.ts` with typed functions for all auth endpoints.

### Changed

- **Auth is always active** — Authentication is now mandatory once a user account exists. Removed `TAGSTUDIO_AUTH_ENABLED` toggle.
- **Session model enriched** — Sessions now store user ID, username, and role (was just a timestamp).
- **Middleware rewritten** — Setup mode blocks all non-auth API endpoints (was: allowed everything). Auth endpoints always pass through.
- **Brute-force throttling** — Consistent mutex handling; timing oracle protection with dummy argon2 verification on unknown usernames.
- **Layout auth flow** — Root layout now detects setup-required state, redirects appropriately, and populates a shared auth store.
- **Toolbar** — Now includes user menu on the right side showing username, role, and logout/manage options.
- **Corrupted `users.json` handling** — Server refuses to start if the file exists but contains invalid JSON (prevents silent data loss).
- **Save error propagation** — All user/invite mutations return errors if disk write fails, with automatic in-memory rollback.

### Removed

- `TAGSTUDIO_AUTH_ENABLED` environment variable.
- `TAGSTUDIO_USERNAME` environment variable.
- `TAGSTUDIO_PASSWORD` environment variable.
- `TAGSTUDIO_SESSION_SECRET` environment variable.
- Plain-text password comparison.

### Security

- Passwords hashed with Argon2id (was: plain-text comparison against env vars).
- Atomic first-user setup prevents race condition where two users could both become super admin.
- Atomic invite registration prevents race condition on username uniqueness.
- Setup mode no longer exposes file/tag/rename API endpoints to unauthenticated users.
- Deleted users' sessions are immediately purged.
- Consistent mutex poison recovery across all lock sites.

## [0.1.0] — 2026-03-18

### Added

- Initial release.
- Batch tag editing for MP3, FLAC, M4A/MP4, OGG Vorbis, Opus, WAV, AIFF.
- Cover art viewing, adding, replacing, and removing.
- File renaming with format strings (`%track% - %artist% - %title%`).
- MusicBrainz lookup with auto-fill and cover art download.
- Spreadsheet-style virtual-scrolling file grid with sortable columns.
- Tag panel sidebar with batch editing and `< keep >` for mixed values.
- Dark navy theme with indigo accents.
- Optional cookie-based authentication via environment variables.
- Docker multi-stage build for amd64 and arm64.
- Path traversal protection via `resolve_safe_path()`.
- REST API under `/api/v1/`.
