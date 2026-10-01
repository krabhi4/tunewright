# Changelog

All notable changes to Tunewright are documented here.

## [1.1.1] - 2026-10-01

A bug-fix release for audio format handling and the build pipeline. Every change was exercised end-to-end against real files in each format ffmpeg can produce (MP3, AAC and ALAC in M4A, raw AAC, FLAC, Vorbis, Opus, WAV, AIFF, WavPack, plus deliberately mislabeled files): tags, custom keys, field removal, cover embed/replace/remove and rename all pass with decoded audio unchanged.

### Fixed

- **WAV and AIFF Corruption** - Any edit that grew the ID3 chunk at the end of a WAV or AIFF file (for example embedding cover art) wrote a RIFF/FORM size that was too small, and the next write then damaged the audio. Fixed by updating lofty to 0.25.4; a regression test checks the chunk layout and samples after growing and shrinking tags.
- **Formats Detected From Contents** - The tag parser was chosen from the file extension, so an Opus file saved as `.ogg` read as untagged and could not be edited. Files are now identified by their contents, falling back to the extension.
- **Cover Art Duplicated in M4A** - Embedding a cover only replaced pictures typed "front cover". MP4 covers carry no type, so the old cover stayed alongside the new one. Untyped covers are now replaced too.
- **WavPack, APE and Musepack Covers** - Cover art embedded into APE tags was written but never read back, and removing it did nothing. APE picture items are now read, replaced and removed correctly.

### Added

- **More Formats** - AAC (`.aac`), WavPack (`.wv`), APE (`.ape`), Musepack (`.mpc`) and Speex (`.spx`) files are now listed and editable.

### Changed

- **pnpm 12** - The frontend toolchain moves from pnpm 9.15.4 to 12.8.1. The version lives only in `frontend/package.json` `packageManager`; CI and the Docker build read it from there. pnpm 12 also enforces the 7-day `minimumReleaseAge` supply-chain floor on frozen installs.
- **Node 26 Build Image** - The Docker frontend stage uses Node 26 and installs pnpm with npm, since Node 26 no longer bundles corepack.
- **Dependencies** - Svelte 5.57, lofty 0.25.4, thiserror 2.0.21 and rand 0.10.3.

### Internal

- **Release Images** - Builds now cache every stage (`cache-mode: max`), so the Rust dependency layer is reused across releases. Images carry an SBOM attestation and OCI labels/annotations (source, revision, version, license), and `latest` only moves on stable versions, never on pre-release tags. Each platform still builds natively: amd64 on `ubuntu-24.04`, arm64 on `ubuntu-24.04-arm`.
- **CI Builds Both Architectures** - Pull requests now build the amd64 and arm64 images natively with the same builder as releases, so an arm64 break shows up before tagging. Superseded PR runs are cancelled.

## [1.1.0] - 2026-10-01

A robustness release. A full review of the backend, frontend and build pipeline was followed by three rounds of fixes and re-reviews, and every tag-writing change was checked against real FLAC files compared byte-for-byte with an untouched baseline copy (audio, cover art and vendor string identical; only the requested tags changed).

**Upgrade notes:**
- Running behind a reverse proxy? Set `TUNEWRIGHT_TRUST_PROXY=true` so login throttling sees real client addresses instead of the proxy's. The server logs a one-time warning when it sees `X-Forwarded-For` without it.
- `TUNEWRIGHT_STATE_DIR` is new and optional. If you set it on an existing install, move `users.json` from the data directory into it first; the server refuses to start rather than silently reopening setup.
- The example `docker-compose.yml` now runs with `read_only: true`, `cap_drop: [ALL]` and `no-new-privileges`. The app only writes inside the data (and state) directory, so these are safe to adopt.

### Security

- **Rename Could Move the Library Root** - A rename request for `/` resolved to the music root itself, and a directory rename fell back to `fs::rename`, moving the whole library (and `users.json`) out from under the app. Rename now only accepts regular files with a supported audio extension, never the root or a directory.
- **`users.json` Hardening** - The password-hash store is created with mode `0600` and existing files are tightened at startup. It can live outside the music folder via `TUNEWRIGHT_STATE_DIR`.
- **Login Throttle Lockout** - Throttling is now keyed on username plus client address (IPv6 grouped by /64), with one verification in flight per client and a non-queuing hash pool that answers `503` instead of stalling. An attacker failing logins from one address can no longer lock the owner out from another, and parallel guesses from one address get `429`.
- **Stricter Content Security Policy** - The header now sets `default-src 'self'` and `script-src` with the SvelteKit bootstrap hashes read from the built `index.html`. The inline theme script moved to `/theme-init.js` so it is no longer blocked.
- **Username Limits** - New accounts are limited to 1-64 characters with no control characters. Used and expired invites are pruned.
- **Dependency Advisories** - Resolved RUSTSEC-2026-0258 (h2, removed entirely by dropping reqwest's unused HTTP/2 support), RUSTSEC-2026-0204 (crossbeam-epoch) and RUSTSEC-2026-0190 (anyhow), and updated yanked `chacha20` and `spin`.

### Fixed

- **Actions Rewrote Every Field** - Running any action (for example "Trim title") wrote every tag back, truncating full dates like `2021-05-30` to `2021` and collapsing multi-value fields such as two `ARTIST` entries into one. Actions now read, apply and write under one file lock and only write fields that actually changed; a no-op action leaves the file byte-identical.
- **Removals Did Nothing** - Remove field, remove all except, setting a field to empty, and clearing Year or Track reported success but left the value on disk. Tag writes are now tri-state: an absent key is unchanged, `null` or `""` removes it.
- **Custom Tags Deleted on Every Write** - Any Vorbis comment lofty has no name for (custom keys, `ENCODER`) was dropped from FLAC/Ogg/Opus files on every save, and APE, ID3v1 and RIFF INFO secondary tags were stripped. Writes now go through each format's native tag type, preserving unknown keys, multi-value order, the vendor string and secondary tags. Custom keys can be written and removed on Vorbis, APE, ID3v2 (TXXX) and MP4 (freeform) tags.
- **ID3v2.3 Silently Upgraded** - Files tagged as ID3v2.3 were saved as v2.4 on every write; they now stay v2.3.
- **Removed Cover Art Reappearing** - Removing cover art only cleared the primary tag, so art in a secondary tag came back. It is now removed from every tag.
- **Multi-Disc Lookups** - MusicBrainz and Apple Music track numbers ran across the whole release (disc 2 track 1 became track 13). Tracks now carry `disc_number` and a per-disc position, auto-match pairs on disc and track, and multi-disc renames use `%disc%-%track% - %title%`. Featured artists are kept from MusicBrainz artist credits.
- **Hidden Files From Renames** - Titles starting with dots (e.g. "...And Justice for All") produced dot-files the file list never showed. Leading dots now become `_`. Files whose tags can't be read are reported instead of being renamed to `-.mp3`.
- **Unsaved Edits Lost** - Rename, Actions, Filename-to-Tag and Lookup now ask to save or discard pending edits first. Pressing Ctrl+S while typing in a field saves the typed value. An expired session keeps your edits, a different user logging in clears them, and closing the tab with unsaved edits asks for confirmation.
- **Session Expiry Redirect Loop** - A `401` sent the browser to `/login` without clearing the session state, which bounced it straight back in an endless loop.
- **Double Saves and Stale Previews** - Saves are de-duplicated and edits made during a save are written afterwards. Rename and Filename-to-Tag can no longer apply a preview for an older pattern. Background property loading no longer overwrites freshly saved tags.
- **Mixed Values Wiped** - Clearing a field that showed `< keep >` across files with different values removed it from all of them; it now leaves them unchanged.
- **Grid** - Numeric columns (track, year, duration, size) sort numerically. Keyboard focus survives scrolling, PageUp/PageDown work, and rows use proper grid roles. Copy Filename/Path works over plain HTTP.
- **Deep Links Returned 404** - Loading `/login` or any other client route directly served the app with a `404` status; it is now `200`, while missing `/_app` assets still return `404`.
- **Graceful Shutdown** - The server ignored `SIGTERM` (as PID 1 in a container), so `docker stop` waited 10 seconds and killed it mid-request. It now drains connections, exits on a second signal or after 25 seconds.
- **Memory Exhaustion via Expressions** - Nested `$replace`/`$regex` expressions could build gigabytes of output per file. Results are capped at 64 KiB, and action preview values at 1 KiB.
- **Secondary Tag Errors** - Failures removing secondary tags are logged instead of swallowed.

### Changed

- **Login Responses** - Login can now return `429 Too Many Requests` (with `Retry-After`) or `503 Service Unavailable`.
- **Rename Preview Errors** - Preview entries that can't be renamed include an `error` message.
- **New Settings** - `TUNEWRIGHT_TRUST_PROXY` and `TUNEWRIGHT_STATE_DIR`, documented in the README.
- **Track Numbers** - Writing a FLAC/Ogg file normalizes `TRACKNUMBER=1/10` to `TRACKNUMBER=1` plus `TRACKTOTAL=10`.

### Internal

- **CI** - Added `cargo fmt --check`, clippy over all targets, `--locked` everywhere, read-only workflow permissions and a separate weekly `cargo audit` workflow. The release workflow pins every action by SHA, reads the tag from the environment, and also checks `frontend/package.json` against the tag.
- **Docker** - Base images are pinned by digest with Dependabot updates, `cargo-chef` is pinned, and the frontend stage builds natively on multi-arch builds.
- **Dependencies** - Routine Rust, frontend and GitHub Actions updates via Dependabot, with TypeScript held on 6.x until `svelte-check` supports 7.
- **Tests** - 163 Rust tests (up from 131) and 76 frontend tests (up from 62).

## [1.0.3] - 2026-08-09

A security release. An existing audit report was verified finding-by-finding against the code (7 of its 38 findings turned out to be incorrect and were withdrawn, and 15 real issues it had missed were found), then everything genuine was fixed and the fixes were themselves adversarially reviewed twice more.

**Migration:** if you run the Docker image with its default `TUNEWRIGHT_HOST=0.0.0.0` and have *not* yet created your admin account, the server now refuses to hand that account to whoever connects first. It generates a one-time setup token at startup and prints it to the log; read it with `docker compose logs` and enter it on the setup screen. Set `TUNEWRIGHT_SETUP_TOKEN` to choose your own instead. Existing installations that already have a user are unaffected.

### Security

- **Setup Window No Longer Open to the Network** - `POST /auth/setup` is unauthenticated by design and grants the first caller super-admin over the whole music library. With the container's default `0.0.0.0` bind and no token configured, anyone who could reach the port could claim it. Previously this only logged a warning; the server now mints a setup token and prints it, so the window is closed by default.
- **Stored XSS via Cover Art** - The `Content-Type` served by `GET /coverart` was taken verbatim from the audio file's embedded picture frame, which is attacker-controlled and which lofty preserves unvalidated. A crafted file requested with `size=0` returned arbitrary bytes as `text/html` on the application's own origin. The MIME is now derived from the payload's magic bytes and constrained to an image allowlist at both the source and the response, with `nosniff` and `Content-Disposition: inline`.
- **Unauthenticated Argon2id Amplification** - `/auth/setup`, `/auth/register`, and `/auth/login` are public and each ran a full Argon2id hash (~19 MiB, t=2) per request, with setup and register hashing *before* their cheap rejection checks. Those checks now run first, and all password hashing goes through a concurrency limiter.
- **Login Throttle Rewritten** - The previous throttle was bypassable by issuing guesses concurrently (all attempts read the same counter and slept in parallel) and was keyed only on username. Attempts for a username now serialize, and failed responses are delayed on an escalating, capped schedule. A correct password is always verified and always accepted, so an attacker cannot lock an account's owner out by failing repeatedly.
- **Image Decompression Bomb** - Cover-art thumbnailing decoded attacker-supplied bytes with the image crate's defaults: no dimension ceiling and a 512 MB allocation budget. A 476 KB file decoding to 9000x9000 is now refused in ~15 ms with flat memory, and art too large to expand is served as stored rather than failing.
- **SSRF via Lookup Redirects** - The HTTP client used for MusicBrainz and Apple Music had no redirect policy, so it followed up to 10 hops to any host including link-local metadata addresses. It now re-validates scheme and host against an allowlist on every hop, matching the cover-art client, which additionally now requires https.
- **Filesystem Existence Oracle** - `GET /files` returned a different status for a traversal path that exists on the host versus one that does not, letting an authenticated user probe the filesystem outside the data root. Parent-directory components are now rejected before the filesystem is touched, so both cases are identical.
- **Security Headers** - Responses now carry `Content-Security-Policy`, `X-Frame-Options`, `X-Content-Type-Options`, `Referrer-Policy`, and `Permissions-Policy`. `script-src` is delivered as hashes by the SvelteKit build so the app's own inline bootstrap stays trusted.
- **Inter-Process File Locking** - Per-file locking synchronized threads within one process only, so two instances sharing a data directory could interleave writes. Writes now also take an OS advisory lock, with a bounded wait so a stuck external process cannot wedge the worker pool, and the crash-safe temp file is per-process.
- **Batch Limits** - Tag, action, rename, and cover-art endpoints had no explicit ceiling. They are now bounded, with actions capped separately and by the `files x actions` product, since that is what actually drives the work.
- **Bounded Lookup Responses** - Provider responses were buffered into memory with no size limit; they are now capped and read in chunks, matching the cover-art download path.
- **Tag Value Limits** - Per-value and per-file caps when collecting non-standard tags from untrusted audio files.
- **Error Message Sanitization** - Internal error strings, absolute filesystem paths, and provider errors are no longer echoed to clients; they are logged server-side and replaced with generic messages.
- **Invite Tokens Out of the Query String** - Invite links are now `/register#token=...`, so the token stays in the fragment and out of reverse-proxy access logs and `Referer` headers. Existing `?token=` links continue to work.
- **Frontend Hardening** - Third-party cover-art URLs from lookup providers are validated against an allowlist before rendering and carry `referrerpolicy="no-referrer"`; the icon component's `{@html}` sink is now typed to its own icon set so an arbitrary string cannot reach it.
- **Constant-Time Setup Token Comparison** - Plus the removal of a 403-versus-409 response oracle that revealed whether a supplied setup token was correct.

### Fixed

- **Client Errors Returned 5xx** - A malformed cover-art URL, a rejected host, a non-image upload, and a malformed MusicBrainz or Apple Music id all returned `500` or `502` for what are plainly client mistakes. They now return `400`, and oversized batches return `413`, each with an accurate message.
- **Setup Impossible at `RUST_LOG=error`** - The generated setup token was only emitted through `tracing`, so a stricter log filter silently swallowed it and left the instance unable to complete setup. It is now written to stderr as well.
- **Filename Sanitization** - Control characters are stripped from generated filenames, and names are bounded to fit within filesystem limits without splitting a multi-byte character.
- **Flaky Test Suite** - Test temporary directories were named from a nanosecond timestamp and collided under parallel execution, failing a different test on roughly one run in four.

### Changed

- **`TUNEWRIGHT_COOKIE_SECURE` and `TUNEWRIGHT_SETUP_TOKEN` Documented** - Both are now in the README configuration table, which also no longer claims `TUNEWRIGHT_HOST` defaults to `0.0.0.0` (the binary defaults to `127.0.0.1`; only the container image overrides it).
- **Compiled Regex Caching** - `$regex()` compiled its pattern once per file inside the batch loops; patterns are now cached with a bounded budget and a size limit chosen by measuring real-world patterns against automaton-blowup ones.
- **Logout Cookie** - The session-clearing cookie now honours `TUNEWRIGHT_COOKIE_SECURE` like the session cookie does.

### Removed

- **Unused `walkdir` Dependency** - Declared but never used; the scanner uses `std::fs::read_dir` directly.

### Internal

- **Verification** - 131 unit tests (up from 103) and a new 81-case end-to-end suite covering every route plus each security property claimed above, run against a freshly provisioned container. Two tests were found to be asserting conditions that could no longer occur and were repaired and mutation-tested.
- **Audit Report Corrected** - `SECURITY_AUDIT.md` now records the verification verdict for every original finding, the newly discovered issues, and an explicit list of the report's own suggested fixes that must not be applied because they would not compile, are no-ops, or would cause data loss.

## [1.0.2] - 2026-07-15

### Fixed

- **Metadata Lookup Showed No Results** - MusicBrainz and Apple Music searches returned matches (HTTP 200 with a full result set) but the modal rendered nothing, or briefly showed results that vanished a beat later. Two reactive effects cleared `searchResults` out from under a completed search: the provider-reset effect also tracked the in-flight `searching`/`loadingRelease` flags (wiping results the instant a search finished), and the auto-fill effect re-ran every time the selected files' tags/properties finished loading in the background. Each now fires only on its intended trigger (an explicit provider change; the modal opening).
- **Apply-with-Rename Failed on Repeat** - After applying a looked-up release with "rename files" enabled, the file list was not refreshed, so the renamed files' paths went stale in the browser and a subsequent apply or save wrote to the pre-rename paths and failed with "File not found". The directory now reloads after a rename, matching the Rename tool.

## [1.0.1] - 2026-07-14

A full-workspace optimization audit (34 verified findings across runtime, build, Docker, CI, and frontend) with every finding applied, plus correctness fixes discovered along the way.

### Fixed

- **Embedded Cover Art Now Displays** - Tag reads skipped picture frames entirely, so `has_cover` was always false and existing embedded artwork never appeared in the tag panel; the thumbnail now loads unconditionally and hides itself only when the file truly has no art.
- **Large Directory Truncation** - Directories with more than 5000 entries silently truncated in the grid while the status bar showed the full count; the file list now pages through the entire directory.
- **Safari Keyboard Navigation** - URL state syncing called `history.replaceState` on every arrow-key press, tripping Safari's 100-calls-per-30-seconds limit and breaking the app during key-repeat; syncing is now debounced.
- **Invalid Patterns Return 400** - An invalid regex in a Replace action or an invalid filename-to-tag pattern now fails the request with a single clear `400 Bad Request` instead of being silently swallowed per file or returning a 500.
- **API Documentation Drift** - `docs/api.md` request/response shapes are regenerated from the actual handlers; the tags, files, and rename endpoints all documented shapes the server never accepted.

### Performance

- **Parallel Batch Writes** - Batch tag writes and batch action executions now run across all cores (reads already did), cutting large-batch write time substantially on SSDs.
- **Cover-Art Caching** - `GET /coverart` responses carry an `ETag` and answer `If-None-Match` with `304`, with `max-age` raised from 60s to 1h; thumbnail resizing switched from Lanczos3 to the several-times-faster Triangle filter; embedding one cover into many files shares the downloaded bytes and runs concurrently instead of copying and writing serially.
- **One Regex Compile Per Request** - Replace actions compiled their regex once per file, so a 1000-file preview compiled the same pattern 1000 times; patterns now compile once per request.
- **Leaner Tag Writes** - Write paths no longer parse the duration/bitrate/sample-rate data they discard.
- **Cheaper Directory Scans** - One stat syscall per entry instead of two, and per-file metadata (hash id, timestamp formatting) is computed only for the requested page instead of the whole directory; the data root is canonicalized once at startup instead of once per file per batch request.
- **Smaller Tag Payloads** - Batch tag reads omit multi-kilobyte lyrics fields the grid never displays (full single-file reads keep them).
- **Frontend Hot Paths** - Scrolling no longer fires duplicate overlapping tag-read requests (in-flight tracking plus a debounce); filename/size/format sorts no longer resort on every tag fetch; tag-edit overlays copy the loaded map instead of rebuilding it per update.
- **Lookup Thumbnails** - Apple Music search results load 100px artwork instead of 800px for 36px thumbnails, and result thumbnails lazy-load.

### Changed

- **Non-Root Distroless Docker Image** - The runtime image is now `gcr.io/distroless/cc-debian12:nonroot` (was `debian:bookworm-slim` running as root): 64.5 MB, no shell or package manager, and the server runs as uid 65532. **Migration:** the mounted `/data` directory must be writable by uid 65532, or set `user:` in your compose file to match your library's owner (see the note in `docker-compose.yml`).
- **Smaller Release Binary** - Release builds are stripped and use thin LTO with a single codegen unit.

### Internal

- **CI Speedups** - Docker builds use cargo-chef so dependency compilation is cached across CI runs (the old BuildKit cache mounts were never persisted by the GitHub Actions cache); the docker job runs in parallel with the test jobs instead of after them; a redundant `cargo check` step was removed; github-actions dependabot updates are grouped into one PR.
- **Dependency Trims** - reqwest now uses ring instead of aws-lc-rs for TLS (drops the slowest-compiling crate in the tree); tokio narrowed from `full` to the features in use; chrono drops its time-zone machinery; sha2 pinned to dedupe a duplicated RustCrypto stack; reqwest, rayon, and hex moved to workspace dependencies.
- **Dead Code Removal** - The unused `GET /files/tree` endpoint and its recursive tree walker are gone.
- **Build Hygiene** - Docker builds enforce `Cargo.lock` with `--locked`; `.dockerignore` covers nested `.env` files, `docs/`, and `.github/`.

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
