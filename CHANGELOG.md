# Changelog

All notable changes to Tunewright are documented here.

## [1.0.0] - 2026-06-06

Tunewright 1.0. A full-workspace bug audit (69 confirmed findings: 1 critical, 17 high, 30 medium, 21 low) was completed and every finding fixed, alongside a security hardening pass and a new in-app notification system.

### Added

- **Toast Notifications** - Themed, accessible toast system replaces every blocking `alert()`. Save, lookup-apply, actions, rename, filename-to-tag, and cover-art flows now report success, partial-failure, and error outcomes with counts. Errors stay until dismissed; the styling follows all four theme families in light and dark.
- **Setup Token** - Optional `TUNEWRIGHT_SETUP_TOKEN` environment variable gates first-admin creation, protecting the setup window on network-exposed deployments (recommended for Docker). The setup page shows a token field when required, and the server logs a security warning when listening beyond loopback with setup incomplete.
- **MSRV Enforcement** - `rust-version = "1.89.0"` is declared in the workspace and checked by a dedicated CI job, and the release workflow verifies the git tag matches the crate version before publishing an image.

### Changed

- **Localhost by Default** - The bare binary now binds `127.0.0.1` instead of `0.0.0.0` (Docker still binds all interfaces via `TUNEWRIGHT_HOST`).
- **Crash-Safe Writes** - All tag and cover-art writes go through an atomic temp-copy + fsync + rename, so a crash or power loss mid-write can no longer truncate an audio file. `users.json` is fsynced the same way.
- **Serialized File Writes** - A per-file lock serializes every tag and cover-art write across all endpoints, eliminating lost updates from concurrent edits.
- **Path Handling** - All read and write endpoints operate on the validated canonical path end-to-end, closing symlink/TOCTOU windows.

### Fixed

- **Data Loss (critical)** - A cross-filesystem rename fallback could silently overwrite an existing file on filesystems without hard-link support (exFAT, SMB/NFS); destinations are now checked and never clobbered.
- **Rename Correctness** - Case-only renames work on macOS/Windows, conflict detection matches the filesystem's case sensitivity, the preview flags collisions with existing on-disk files, dot-only and unsanitized-extension targets are rejected cleanly, and trailing-dot names are avoided.
- **Tag Writing** - Writing only a track/disc total no longer fabricates a "0/N" pair on ID3v2; stale secondary tags (APE/ID3v1) are merged and removed so edits can't be shadowed on re-read; ~100 extra tag keys (ISRC, barcode, catalog number, ...) now round-trip instead of being silently dropped.
- **Format Expressions** - Deeply nested input, `$div`/`$mod` overflow, and huge `$num` pad widths no longer crash the server; empty search strings in Replace/Split no longer corrupt values; `$caps2` preserves original spacing; AutoNumber saturates instead of overflowing.
- **Lookup** - Multi-disc releases from MusicBrainz and Apple Music order correctly with sequential track numbers; one malformed row in a provider response no longer fails the whole search; all outbound requests share a client with connect/read timeouts; the MusicBrainz rate limiter rejects with 429 instead of queueing unboundedly.
- **Server Robustness** - Blocking file I/O moved off the async runtime (cover art, rename, user persistence); cover-art uploads up to the advertised 10 MB now work; unmatched `/api/v1/*` paths return JSON 404 instead of the SPA shell; startup failures (bad host, port in use, IPv6 literals, corrupt `users.json`) exit with clean errors instead of panics; invalid `TUNEWRIGHT_PORT` values log a warning.
- **Authentication** - Login throttling is per-username (normalized and memory-bounded) instead of a global counter an attacker could reset; the session cookie gains a `Secure` flag toggle (`TUNEWRIGHT_COOKIE_SECURE`); the auth middleware uses an explicit public-route allowlist so privileged `/auth/*` routes are protected in the middleware layer.
- **Frontend Correctness** - Shift-click range selection follows the sorted/filtered view; rapid folder navigation can no longer show the wrong directory's files; edits made while a save is in flight are preserved; a server error during the auth check shows "server unreachable" instead of bouncing to login; failed saves block navigation instead of silently discarding edits; numeric tag fields are clamped and accept `5/10` track/total notation; modals reset stale state on reopen; switching lookup providers clears stale results; durations under a minute display as seconds; Enter can no longer double-submit auth forms.
- **Packaging** - `docker-compose.yml` validates again, pnpm is version-pinned with `--frozen-lockfile` enforced, dead Discogs configuration was removed, and outbound User-Agent strings now derive from the crate version automatically.

## [0.6.0] - 2026-05-30

### Changed

- **Renamed project from TagStudio to Tunewright** to avoid a naming collision with the existing [TagStudio](https://github.com/TagStudioDev/TagStudio) project. The Docker image is now `ghcr.io/krabhi4/tunewright`, and all environment variables use the `TUNEWRIGHT_` prefix (e.g. `TUNEWRIGHT_DATA_DIR` replaces `TAGSTUDIO_DATA_DIR`). Update your compose file and environment accordingly.

## [0.5.1] - 2026-05-30

### Added

- **Editorial, Terminal, and DAW Themes** - Three new theme families alongside Console, selectable from a new toolbar theme switcher (family picker plus a light/dark toggle). Terminal and DAW are dark-native and present a "Dark only" appearance.
- **Per-Theme Font Loading** - Editorial and DAW load their typefaces (Fraunces, Hanken Grotesk) on demand, keeping the default Console theme lean.

### Fixed

- **No Theme Flash on Load** - The saved theme is applied before first paint, eliminating a flash of the default theme for non-default selections.
- **Resilient Theme Storage** - All `localStorage` and `matchMedia` access is guarded, so a browser with storage blocked (for example, private mode) can no longer hang the app on load.
- **Preserved Appearance Choice** - Switching through a dark-native theme and back no longer discards a saved light-mode preference.
- **Accessibility** - The Editorial light accent now meets WCAG AA contrast.
- **Font Loading** - A failed lazy font load retries on re-activation instead of remaining on fallback fonts.

## [0.5.0] - 2026-05-29

### Added

- **Console Theme System** - New two-axis theme model (theme family plus light/dark mode) driven by a central design-token contract. Ships the "Console" theme in both dark and light, replacing the previous Sage & Stone palette, with automatic migration from the old single-key theme storage.
- **Self-Hosted Typography** - Bundled IBM Plex Sans and IBM Plex Mono locally, replacing Plus Jakarta Sans and removing all external font CDN requests.
- **Vendored Icon Set** - Local SVG icon set, core glyphs, and a new wordmark/logo, removing external icon and placeholder assets.
- **Instrument-Style Status Bar** - Reworked status bar with semantic file, selection, and edit counts.
- **Test Harness** - Added a Vitest suite covering theme resolution, the design-token contract, and a guard against reintroducing generic "AI-slop" visual tells.

### Changed

- **Semantic Dirty State** - Edited grid rows now carry a dirty-state indicator, edited tag-panel fields use an amber treatment, and `< keep >` placeholders are muted.
- **Redesigned Favicon** - New theme-aware favicon.
- Updated user agent versioning to `Tunewright/0.5.0`.
- Removed dead Google Fonts links and unused toolbar selectors.

### Performance

- **Pooled HTTP Client** - Lookup requests (MusicBrainz / Apple Music) now reuse a single connection-pooled client instead of constructing one per request.
- **Concurrent Release Fetch** - MusicBrainz release detail and its cover-art URL are fetched concurrently, saving a round-trip.
- **Parallel Tag Reads** - Batch rename and batch actions read tags in parallel across cores (writes stay serial).
- **Cheaper Directory Scans** - Listing a folder no longer issues a `canonicalize()` syscall per file; cover-art extraction skips decoding audio properties.
- **Frontend Hot Paths** - File lookups are O(1) via an id-indexed store, the grid precomputes sort keys and does one tag lookup per row, and the tag panel computes per-field state once per render.

### Internal

- **Server-Provided Format Labels** - Format display labels (e.g. "M4A") now come from the server as the single source of truth.
- **Rename Path In Responses** - Rename results include the post-rename relative path so clients no longer reconstruct it.
- Broad deduplication and simplification across the lookup providers, server routes, auth, and frontend stores and components.

## [0.4.1] — 2026-05-26

### Fixed

- **Apple Music Artwork Support** — Allowed downloading and embedding album cover art from Apple Music hosts (`mzstatic.com` and its subdomains) in the backend security policies.
- **MusicBrainz Artwork Loading** — Automatically upgraded retrieved MusicBrainz cover art archive URLs from HTTP to HTTPS, resolving browser Mixed Content blocking when the app runs in secure contexts.
- **Renamed File Cover Art Embedding** — Fixed an issue where executing a file rename during metadata confirmation caused subsequent cover art embedding to fail due to stale file path targets.

## [0.4.0] — 2026-05-25

### Added

- **Resizable Modal Dialogs** — Added native drag-and-resize handles (`resize: both`) to all modal dialogs.
- **Wider Default Modal Views** — Increased standard wide modal layout default width to `850px` for tabular-dense and file-centric dialogs (MusicBrainz Lookup, Rename Files, Filename to Tag).

### Changed

- Updated user agent versioning to `Tunewright/0.4.0`.

### Fixed

- **MusicBrainz Lookup Loading State** — Added a localized spinner inside the selected result row's cover art thumbnail box, and disabled all result rows and inputs during background data fetching to prevent duplicate clicks.
- **Asynchronous Race Condition Protection** — Safeguarded lookup and search requests against late-resolving promises if the user quickly closes/reopens the modal or updates search terms.
- **Matching State Cleanup** — Ensured matched and unmatched file state arrays are explicitly wiped when opening the MusicBrainz lookup modal to avoid stale session carryover.

## [0.3.0] — 2026-05-25

### Added

- **Sage & Stone Theme System** — Dynamic theme system that avoids generic AI gradient palettes. Dark Mode features a deep warm graphite base with organic sage green accents, and Light Mode features a sand-linen off-white base with deep forest green accents.
- **Dynamic System Preference Detection** — Detects user's OS dark/light mode preference dynamically via media query listeners, falling back to system preference by default if no override exists.
- **Theme Switcher** — Inline theme toggle switch (sun/moon SVG) in the main toolbar.
- **Plus Jakarta Sans Typography** — Replaced default font stack with a clean, premium, modern typeface.
- **Modern Brand Identity** — Custom "Tag & Waveform" SVG logo replacing placeholder assets on setup/auth screens.
- **Theme-Aware SVG Favicon** — Favicon dynamically adapts to system dark/light modes.
- **Advanced Expression Engine** — Nested recursive descent parser supporting `%variable%` placeholders and `$function(arg1, arg2)` format strings with 30+ string, math, logic, and field manipulation functions.
- **Filename-to-Tag Parser** — Extract metadata from files using custom filename pattern templates with interactive live preview.
- **Actions & Batch Processing** — Actions builder to chain operations (CaseConversion, Replace, FormatValue, SetField, RemoveField, RemoveAllExcept, AutoNumber, SplitField, MergeFields, TrimField) on multiple selected files with draggable order.

### Changed

- Updated core request/fetch User Agents to specify Tunewright/0.3.0.

### Fixed

- **Reactive Route Guarding** — Svelte 5 `$effect`-based checks prevent authenticated users from navigating back to setup or auth pages.
- **Session Persistence** — Disabled global server-side rendering (SSR) in the frontend adapter to prevent hydration mismatches and ensure persistent browser cookie authentication.
- Fixed modal layouts and cover art lookup API response behavior.

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

- **Auth is always active** — Authentication is now mandatory once a user account exists. Removed `TUNEWRIGHT_AUTH_ENABLED` toggle.
- **Session model enriched** — Sessions now store user ID, username, and role (was just a timestamp).
- **Middleware rewritten** — Setup mode blocks all non-auth API endpoints (was: allowed everything). Auth endpoints always pass through.
- **Brute-force throttling** — Consistent mutex handling; timing oracle protection with dummy argon2 verification on unknown usernames.
- **Layout auth flow** — Root layout now detects setup-required state, redirects appropriately, and populates a shared auth store.
- **Toolbar** — Now includes user menu on the right side showing username, role, and logout/manage options.
- **Corrupted `users.json` handling** — Server refuses to start if the file exists but contains invalid JSON (prevents silent data loss).
- **Save error propagation** — All user/invite mutations return errors if disk write fails, with automatic in-memory rollback.

### Removed

- `TUNEWRIGHT_AUTH_ENABLED` environment variable.
- `TUNEWRIGHT_USERNAME` environment variable.
- `TUNEWRIGHT_PASSWORD` environment variable.
- `TUNEWRIGHT_SESSION_SECRET` environment variable.
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
