# Security Policy

## Reporting a Vulnerability

If you discover a security vulnerability in TagStudio, please report it responsibly.

**Do not open a public GitHub issue for security vulnerabilities.**

Instead, please use GitHub's private vulnerability reporting feature on the repository to submit reports securely.

Include:
- Description of the vulnerability
- Steps to reproduce
- Potential impact
- Suggested fix (if you have one)

You should receive a response within 72 hours. We will work with you to understand the issue and coordinate a fix before any public disclosure.

## Scope

TagStudio is a self-hosted application typically run on a private network. The threat model assumes:

- The server is behind a VPN, reverse proxy, or firewall
- Users with access are semi-trusted
- The primary risk is unauthorized modification of music files

### In Scope

- Path traversal (reading/writing files outside the data directory)
- Authentication bypass
- Cross-site scripting (XSS) in the web UI
- Arbitrary code execution via malformed audio files
- Information disclosure (leaking file paths, server internals)
- Privilege escalation (admin gaining super admin access)

### Out of Scope

- Denial of service via large file uploads (accepted risk for self-hosted)
- Issues requiring physical access to the server
- Social engineering

## Security Measures

### Path Traversal Protection

All file operations go through `resolve_safe_path()` which canonicalizes paths and verifies they're within the configured data root. Requests for `../../etc/passwd` or similar are rejected.

### Authentication

Authentication is always active once a user account exists:

- First visitor creates a super admin account via the web UI
- Additional users are added via invite links (48-hour expiry)
- Passwords are hashed with Argon2id (via the `argon2` crate)
- Sessions use random 256-bit tokens stored in HttpOnly, SameSite=Lax cookies
- Brute-force throttling with exponential backoff on failed logins
- Timing oracle protection: dummy Argon2 verification on unknown usernames
- User accounts stored in `users.json` with atomic writes (temp file + rename)
- Server refuses to start if `users.json` exists but contains invalid JSON (prevents silent data wipe)

### Setup Mode

Before any user account exists, only `/auth/*` and `/health` API endpoints are accessible. All other endpoints return 503 until setup is complete.

### External API Proxying

MusicBrainz API calls are proxied through the server. This keeps API tokens server-side and enforces rate limiting. The frontend never contacts external services directly.

### Dependencies

- Audio file parsing uses [lofty](https://github.com/Serial-ATA/lofty-rs), a pure Rust library
- Image processing uses [image](https://github.com/image-rs/image), a pure Rust library
- Password hashing uses [argon2](https://crates.io/crates/argon2), a pure Rust library
- No C dependencies in the audio/image/crypto pipeline reduces the attack surface

## Supported Versions

| Version | Supported |
|---------|-----------|
| 0.2.x   | Yes       |
| 0.1.x   | No        |

Only the latest release receives security updates.
