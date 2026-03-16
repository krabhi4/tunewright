# Security Policy

## Reporting a Vulnerability

If you discover a security vulnerability in TagStudio, please report it responsibly.

**Do not open a public GitHub issue for security vulnerabilities.**

Instead, email: **security@YOUR_DOMAIN** (or use GitHub's private vulnerability reporting if enabled on the repo).

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

### Out of Scope

- Denial of service via large file uploads (accepted risk for self-hosted)
- Issues requiring physical access to the server
- Social engineering

## Security Measures

### Path Traversal Protection

All file operations go through `resolve_safe_path()` which canonicalizes paths and verifies they're within the configured data root. Requests for `../../etc/passwd` or similar are rejected.

### Authentication

When `TAGSTUDIO_AUTH_ENABLED=true`:
- Credentials are checked against environment variables
- Sessions use HMAC-signed cookies
- The session secret is auto-generated if not explicitly set
- Auth endpoints are exempt from the middleware; all other `/api/*` endpoints require authentication

When disabled, all endpoints are accessible without authentication. This is intended for trusted networks only.

### External API Proxying

MusicBrainz and Discogs API calls are proxied through the server. This keeps API tokens server-side and enforces rate limiting. The frontend never contacts external services directly.

### Dependencies

- Audio file parsing uses [lofty](https://github.com/Serial-ATA/lofty-rs), a pure Rust library
- Image processing uses [image](https://github.com/image-rs/image), a pure Rust library
- No C dependencies in the audio/image pipeline reduces the attack surface for malformed files

## Supported Versions

| Version | Supported |
|---------|-----------|
| 0.1.x   | Yes       |

Only the latest release receives security updates.
