# Tunewright REST API Reference

All Tunewright endpoints are prefixed with `/api/v1/` unless specified otherwise. In-flight requests are authenticated via a cookie named `tunewright_session`.

---

## General Endpoints

### Health Check

* **Endpoint:** `GET /health`
* **Authentication:** None
* **Description:** Check if the server is running.
* **Response:** `200 OK` (application/json)
  ```json
  {
    "status": "ok",
    "version": "1.0.0"
  }
  ```

---

## File Operations

### List Files

* **Endpoint:** `GET /files`
* **Authentication:** Authenticated Session
* **Query Parameters:**
  * `path` (string, optional): The target directory path relative to the data root. Defaults to the root.
  * `offset` (integer, optional): Number of entries (directories + files) to skip. Defaults to 0.
  * `limit` (integer, optional): Maximum number of entries to return. Defaults to 500.
* **Description:** Lists subdirectories and audio files in the specified directory path. Directories are paginated before files; `total` counts both. Does not read tags, so `duration_secs` is always `null` and `has_cover` is always `false` here (fetch them via the tag endpoints).
* **Response:** `200 OK` (application/json)
  ```json
  {
    "path": "/Music/Albums",
    "directories": ["Artist - AlbumName"],
    "files": [
      {
        "id": "1a2b3c4d5e6f7a8b9c0d1e2f",
        "filename": "01 - TrackOne.mp3",
        "relative_path": "Music/Albums/01 - TrackOne.mp3",
        "format": "mp3",
        "format_label": "MP3",
        "size": 10485760,
        "duration_secs": null,
        "has_cover": false,
        "modified_at": "2026-05-25T14:20:00Z"
      }
    ],
    "total": 2
  }
  ```

---

## Tag Operations

### Batch Read Tags (Fast)

* **Endpoint:** `POST /tags/read`
* **Authentication:** Authenticated Session
* **Request Body:** (application/json)
  ```json
  {
    "ids": ["1a2b3c4d5e6f7a8b9c0d1e2f"],
    "paths": {
      "1a2b3c4d5e6f7a8b9c0d1e2f": "Music/Albums/01 - TrackOne.mp3"
    }
  }
  ```
* **Description:** Reads standard metadata tags (title, artist, album, track number, year, genre, etc.) for a list of file IDs, resolving each ID through the `paths` map. This endpoint is fast as it does not parse audio properties, so `bitrate`, `sample_rate`, `channels` and `duration_secs` are never included. Optional fields are omitted when absent; `tag_types` and `extra` (custom tag fields) are omitted when empty. Files that fail to read or resolve to an unsafe path are left out of the `tags` map.
* **Response:** `200 OK` (application/json)
  ```json
  {
    "tags": {
      "1a2b3c4d5e6f7a8b9c0d1e2f": {
        "title": "Track One",
        "artist": "Artist Name",
        "album": "Album Name",
        "album_artist": "Artist Name",
        "track_number": 1,
        "track_total": 12,
        "year": 2026,
        "genre": "Electronic",
        "format": "Mpeg",
        "tag_types": ["Id3v2"],
        "has_cover": true,
        "extra": {
          "BPM": "128"
        }
      }
    }
  }
  ```

### Batch Read Audio Properties

* **Endpoint:** `POST /tags/read-properties`
* **Authentication:** Authenticated Session
* **Request Body:** (application/json)
  ```json
  {
    "ids": ["1a2b3c4d5e6f7a8b9c0d1e2f"],
    "paths": {
      "1a2b3c4d5e6f7a8b9c0d1e2f": "Music/Albums/01 - TrackOne.mp3"
    }
  }
  ```
* **Description:** Same as `/tags/read` but also parses audio properties (duration in seconds, bitrate in kbps, sample rate in Hz, channels). Slower; only call it for files the user wants to inspect.
* **Response:** `200 OK` (application/json)
  ```json
  {
    "tags": {
      "1a2b3c4d5e6f7a8b9c0d1e2f": {
        "title": "Track One",
        "artist": "Artist Name",
        "album": "Album Name",
        "bitrate": 320,
        "sample_rate": 44100,
        "channels": 2,
        "duration_secs": 240.5,
        "format": "Mpeg",
        "tag_types": ["Id3v2"],
        "has_cover": true
      }
    }
  }
  ```

### Batch Write Tags

* **Endpoint:** `POST /tags/write`
* **Authentication:** Authenticated Session
* **Request Body:** (application/json)
  ```json
  {
    "changes": [
      {
        "id": "1a2b3c4d5e6f7a8b9c0d1e2f",
        "path": "Music/Albums/01 - TrackOne.mp3",
        "tags": {
          "title": "New Track Title",
          "artist": "Updated Artist"
        }
      }
    ]
  }
  ```
* **Description:** Writes new tag values to the specified files in place. Writable fields: `title`, `artist`, `album`, `album_artist`, `year`, `track_number`, `track_total`, `disc_number`, `disc_total`, `genre`, `comment`, `composer` and `extra` (a string map of custom fields). Fields omitted from `tags` are left unchanged. Entries with unsafe paths are dropped and produce no result.
* **Response:** `200 OK` (application/json). `status` is `ok` or `error` (with an `error` message).
  ```json
  {
    "results": [
      {
        "id": "1a2b3c4d5e6f7a8b9c0d1e2f",
        "status": "ok"
      }
    ]
  }
  ```

---

## Cover Art Operations

### Get Cover Art Thumbnail

* **Endpoint:** `GET /coverart`
* **Authentication:** Authenticated Session
* **Query Parameters:**
  * `path` (string, required): Safe path to the audio file containing embedded cover art.
  * `size` (integer, optional): Maximum pixel dimension for resizing. Defaults to 250; `0` returns the original image without resizing.
* **Description:** Extracts and returns the embedded cover art image, resized on the server if larger than `size`. Responses carry an `ETag` (derived from the file's mtime, size and the requested thumbnail size) and `Cache-Control: private, max-age=3600`; requests with a matching `If-None-Match` header receive `304 Not Modified`.
* **Response:** `200 OK` (image/jpeg or image/png), `304 Not Modified` on ETag match, or `404 Not Found` if the file has no embedded cover art.

### Upload Cover Art

* **Endpoint:** `POST /coverart`
* **Authentication:** Authenticated Session
* **Request Body:** (multipart/form-data)
  * `path` (text, required): Safe path to the audio file to embed the image into.
  * `image` (file, required): JPEG or PNG image, max 10 MB.
* **Description:** Embeds the uploaded image as the file's cover art.
* **Response:** `200 OK` (application/json)
  ```json
  {
    "status": "ok"
  }
  ```

### Embed Cover Art from URL

* **Endpoint:** `POST /coverart/from-url`
* **Authentication:** Authenticated Session
* **Request Body:** (application/json)
  ```json
  {
    "url": "https://coverartarchive.org/release/release-uuid-here/front-500",
    "paths": ["Music/Albums/01 - TrackOne.mp3"]
  }
  ```
* **Description:** Downloads the image (only `coverartarchive.org` and `mzstatic.com` URLs are allowed; JPEG or PNG; max 10 MB) and embeds it into all listed files.
* **Response:** `200 OK` (application/json)
  ```json
  {
    "status": "ok",
    "embedded": 1,
    "errors": []
  }
  ```

### Remove Cover Art

* **Endpoint:** `DELETE /coverart`
* **Authentication:** Authenticated Session
* **Query Parameters:**
  * `path` (string, required): Safe path to the audio file to remove cover art from.
* **Description:** Removes the embedded cover art from the target audio file.
* **Response:** `200 OK` (application/json)
  ```json
  {
    "status": "ok"
  }
  ```

---

## File Renaming

### Preview Renames

* **Endpoint:** `POST /rename/preview`
* **Authentication:** Authenticated Session
* **Request Body:** (application/json)
  ```json
  {
    "files": [
      {
        "id": "1a2b3c4d5e6f7a8b9c0d1e2f",
        "path": "Music/Albums/01 - TrackOne.mp3"
      }
    ],
    "format": "%track% - %artist% - %title%"
  }
  ```
* **Description:** Previews how files will be renamed using the given format string. Resolves placeholders using the file's current tags; `conflict` flags names that would collide with an existing file or another rename in the batch.
* **Response:** `200 OK` (application/json)
  ```json
  {
    "previews": [
      {
        "id": "1a2b3c4d5e6f7a8b9c0d1e2f",
        "old_name": "01 - TrackOne.mp3",
        "new_name": "01 - Artist Name - Track One.mp3",
        "conflict": false
      }
    ]
  }
  ```

### Execute Renames

* **Endpoint:** `POST /rename/execute`
* **Authentication:** Authenticated Session
* **Request Body:** (application/json)
  ```json
  {
    "files": [
      {
        "id": "1a2b3c4d5e6f7a8b9c0d1e2f",
        "path": "Music/Albums/01 - TrackOne.mp3"
      }
    ],
    "format": "%track% - %artist% - %title%"
  }
  ```
* **Description:** Renames the files on disk using the given format string. `status` is `ok`, `skipped` (name unchanged) or `error` (with an `error` message); `new_relative_path` is the file's path after the operation.
* **Response:** `200 OK` (application/json)
  ```json
  {
    "results": [
      {
        "id": "1a2b3c4d5e6f7a8b9c0d1e2f",
        "status": "ok",
        "old_name": "01 - TrackOne.mp3",
        "new_name": "01 - Artist Name - Track One.mp3",
        "new_relative_path": "Music/Albums/01 - Artist Name - Track One.mp3"
      }
    ]
  }
  ```

---

## Filename to Tag

### Preview Tag Extraction

* **Endpoint:** `POST /filename-to-tag/preview`
* **Authentication:** Authenticated Session
* **Request Body:** (application/json)
  ```json
  {
    "files": [
      {
        "id": "1a2b3c4d5e6f7a8b9c0d1e2f",
        "path": "Music/Albums/01 - Artist Name - Track One.mp3"
      }
    ],
    "pattern": "%track% - %artist% - %title%"
  }
  ```
* **Description:** Previews tag values extracted from each filename using the given pattern. `tags` (the writable tag fields, ready for `/tags/write`) is omitted when the filename does not match. An invalid pattern returns `400 Bad Request` with `{"error": "..."}`.
* **Response:** `200 OK` (application/json)
  ```json
  {
    "previews": [
      {
        "id": "1a2b3c4d5e6f7a8b9c0d1e2f",
        "filename": "01 - Artist Name - Track One.mp3",
        "matched": true,
        "tags": {
          "title": "Track One",
          "artist": "Artist Name",
          "track_number": 1
        }
      }
    ]
  }
  ```

---

## Batch Actions

### Preview Actions

* **Endpoint:** `POST /actions/preview`
* **Authentication:** Authenticated Session
* **Request Body:** (application/json)
  ```json
  {
    "files": [
      {
        "id": "1a2b3c4d5e6f7a8b9c0d1e2f",
        "path": "Music/Albums/01 - TrackOne.mp3"
      }
    ],
    "actions": [
      {
        "type": "case_conversion",
        "field": "title",
        "mode": "title"
      }
    ]
  }
  ```
* **Description:** Applies the action chain to each file's tags in memory and returns the per-field diffs without writing. Files with no resulting changes are omitted. An invalid regex in a `replace` action fails the whole request with `400 Bad Request` and `{"error": "..."}`. Each action is tagged by `type` (snake_case):
  * `case_conversion` `{field, mode}` where `mode` is `title`, `upper`, `lower` or `sentence`
  * `replace` `{field, search, replace, regex?}` (`regex` defaults to `false`)
  * `format_value` `{field, format}`
  * `set_field` `{field, value}`
  * `remove_field` `{field}`
  * `remove_all_except` `{fields}`
  * `auto_number` `{field, start?, padding?}` (defaults: start 1, padding 2)
  * `split_field` `{source, separator, part, target}` (`part` is 0-based)
  * `merge_fields` `{sources, separator, target}`
  * `trim_field` `{field}`
* **Response:** `200 OK` (application/json)
  ```json
  {
    "previews": [
      {
        "id": "1a2b3c4d5e6f7a8b9c0d1e2f",
        "filename": "01 - TrackOne.mp3",
        "changes": [
          {
            "field": "title",
            "old_value": "track one",
            "new_value": "Track One"
          }
        ]
      }
    ]
  }
  ```

### Execute Actions

* **Endpoint:** `POST /actions/execute`
* **Authentication:** Authenticated Session
* **Request Body:** Same shape as `/actions/preview`.
* **Description:** Applies the action chain to each file's tags and writes the result back to disk. An invalid regex in a `replace` action fails the whole request with `400 Bad Request` and `{"error": "..."}`.
* **Response:** `200 OK` (application/json). `status` is `ok` or `error` (with an `error` message).
  ```json
  {
    "results": [
      {
        "id": "1a2b3c4d5e6f7a8b9c0d1e2f",
        "status": "ok"
      }
    ]
  }
  ```

---

## Metadata Search & Lookup

### Search MusicBrainz Releases

* **Endpoint:** `GET /lookup/musicbrainz/search`
* **Authentication:** Authenticated Session
* **Query Parameters:**
  * `query` (string, required): Free-text release search query (typically `artist album`).
* **Description:** Searches the MusicBrainz database for matching release entries. MusicBrainz requests are rate-limited globally (minimum 1.1s gap); when the backlog exceeds 10 seconds the server returns `429 Too Many Requests`, and upstream failures return `502 Bad Gateway` with `{"error": "..."}`.
* **Response:** `200 OK` (application/json)
  ```json
  [
    {
      "id": "release-uuid-here",
      "title": "Album Title",
      "artist": "Artist Name",
      "year": 2026,
      "track_count": 12,
      "source": "musicbrainz",
      "cover_art_url": "https://coverartarchive.org/release/release-uuid-here/front-250"
    }
  ]
  ```

### Get MusicBrainz Release Details

* **Endpoint:** `GET /lookup/musicbrainz/release/:mbid`
* **Authentication:** Authenticated Session
* **Path Parameters:**
  * `mbid` (string, required): The MusicBrainz Release ID UUID.
* **Description:** Retrieves detailed release tracks and metadata, including a cover art URL. Subject to the same rate limiting and `502` handling as the search endpoint.
* **Response:** `200 OK` (application/json)
  ```json
  {
    "id": "release-uuid-here",
    "title": "Album Title",
    "artist": "Artist Name",
    "year": 2026,
    "genre": "Electronic",
    "tracks": [
      {
        "position": 1,
        "title": "Track One Title",
        "artist": "Artist Name",
        "duration_secs": 240.0
      }
    ],
    "source": "musicbrainz",
    "cover_art_url": "https://coverartarchive.org/release/release-uuid-here/front-500"
  }
  ```

### Search Apple Music Releases

* **Endpoint:** `GET /lookup/applemusic/search`
* **Authentication:** Authenticated Session
* **Query Parameters:**
  * `query` (string, required): Free-text release search query.
* **Description:** Searches the Apple Music (iTunes) catalog for matching release entries. Upstream failures return `502 Bad Gateway` with `{"error": "..."}`.
* **Response:** `200 OK` (application/json). Same shape as the MusicBrainz search, with `"source": "applemusic"`.

### Get Apple Music Release Details

* **Endpoint:** `GET /lookup/applemusic/release/:id`
* **Authentication:** Authenticated Session
* **Path Parameters:**
  * `id` (string, required): The Apple Music collection ID.
* **Description:** Retrieves detailed release tracks and metadata, including a cover art URL.
* **Response:** `200 OK` (application/json). Same shape as the MusicBrainz release details, with `"source": "applemusic"`.

---

## Authentication & User Administration

### First-Time Setup

* **Endpoint:** `POST /auth/setup`
* **Authentication:** None (Only available if no users exist in the database)
* **Request Body:** (application/json)
  ```json
  {
    "username": "admin",
    "password": "super-secure-password",
    "setup_token": "optional-setup-token"
  }
  ```
* **Description:** Creates the first user account, which automatically receives the `super_admin` role. `setup_token` is only required when the server is configured with one (`403 Forbidden` otherwise). Once a user is created, this endpoint will return `409 Conflict`.
* **Response:** `200 OK` (application/json) - sets the `tunewright_session` cookie.
  ```json
  {
    "status": "ok",
    "user": {
      "username": "admin",
      "role": "super_admin"
    }
  }
  ```

### Login

* **Endpoint:** `POST /auth/login`
* **Authentication:** None
* **Request Body:** (application/json)
  ```json
  {
    "username": "admin",
    "password": "super-secure-password"
  }
  ```
* **Description:** Verifies credentials and creates a session. Repeated failures for a username are throttled; invalid credentials return `401 Unauthorized`.
* **Response:** `200 OK` (application/json) - sets the `tunewright_session` cookie.
  ```json
  {
    "status": "ok",
    "user": {
      "username": "admin",
      "role": "super_admin"
    }
  }
  ```

### Logout

* **Endpoint:** `POST /auth/logout`
* **Authentication:** None
* **Description:** Invalidates the current session token and clears the cookie.
* **Response:** `200 OK` (application/json)
  ```json
  {
    "status": "ok"
  }
  ```

### Check Auth Status

* **Endpoint:** `GET /auth/check`
* **Authentication:** None
* **Description:** Checks if setup is completed and returns the current user profile if a valid session cookie is present. When no users exist yet, returns `200 OK` with `{"setup_required": true, "setup_token_required": <bool>}`. Without a valid session, returns `401 Unauthorized` with `{"authenticated": false}`.
* **Response:** `200 OK` (application/json)
  ```json
  {
    "authenticated": true,
    "user": {
      "username": "admin",
      "role": "super_admin"
    }
  }
  ```

### Register via Invite Token

* **Endpoint:** `POST /auth/register`
* **Authentication:** None
* **Request Body:** (application/json)
  ```json
  {
    "token": "invite-token-uuid",
    "username": "new_user",
    "password": "password123"
  }
  ```
* **Description:** Registers a new `admin` user using a valid, unused, unexpired invite token.
* **Response:** `200 OK` (application/json) - sets the `tunewright_session` cookie.
  ```json
  {
    "status": "ok",
    "user": {
      "username": "new_user",
      "role": "admin"
    }
  }
  ```

### Create Invite

* **Endpoint:** `POST /auth/invites`
* **Authentication:** Super Admin Only
* **Description:** Generates a new 48-hour registration invite link token.
* **Response:** `200 OK` (application/json)
  ```json
  {
    "token": "invite-token-uuid",
    "created_by": "super-admin-uuid",
    "expires_at": "2026-05-27T14:20:00Z",
    "link": "/register?token=invite-token-uuid"
  }
  ```

### List Invites

* **Endpoint:** `GET /auth/invites`
* **Authentication:** Super Admin Only
* **Description:** Returns all active (unused, unexpired) registration invite tokens.
* **Response:** `200 OK` (application/json)
  ```json
  [
    {
      "token": "invite-token-uuid",
      "created_by": "super-admin-uuid",
      "expires_at": "2026-05-27T14:20:00Z",
      "link": "/register?token=invite-token-uuid"
    }
  ]
  ```

### Revoke Invite

* **Endpoint:** `DELETE /auth/invites/:token`
* **Authentication:** Super Admin Only
* **Path Parameters:**
  * `token` (string, required): The invite token to delete.
* **Description:** Revokes and deletes an active invite token. Returns `404 Not Found` if the token does not exist.
* **Response:** `200 OK` (application/json)
  ```json
  {
    "status": "ok"
  }
  ```

### List Users

* **Endpoint:** `GET /auth/users`
* **Authentication:** Super Admin Only
* **Description:** Lists all registered users (excluding password hashes).
* **Response:** `200 OK` (application/json)
  ```json
  [
    {
      "id": "user-uuid-here",
      "username": "admin",
      "role": "super_admin",
      "created_at": "2026-05-25T14:20:00Z"
    }
  ]
  ```

### Remove User

* **Endpoint:** `DELETE /auth/users/:id`
* **Authentication:** Super Admin Only
* **Path Parameters:**
  * `id` (string, required): The ID of the user to delete.
* **Description:** Deletes a user account and invalidates all active sessions for that user. Deleting your own account returns `400 Bad Request`.
* **Response:** `200 OK` (application/json)
  ```json
  {
    "status": "ok"
  }
  ```
