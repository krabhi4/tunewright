# TagStudio REST API Reference

All TagStudio endpoints are prefixed with `/api/v1/` unless specified otherwise. In-flight requests are authenticated via a cookie named `tagstudio_session`.

---

## General Endpoints

### Health Check

* **Endpoint:** `GET /health`
* **Authentication:** None
* **Description:** Check if the server is running.
* **Response:** `200 OK` (text/plain)
  ```
  OK
  ```

---

## File Operations

### List Files

* **Endpoint:** `GET /files`
* **Authentication:** Authenticated Session
* **Query Parameters:**
  * `path` (string, required): The target directory path relative to the configuration root.
* **Description:** Lists all subdirectories and audio files in the specified directory path.
* **Response:** `200 OK` (application/json)
  ```json
  {
    "current_path": "/Music/Albums",
    "directories": [
      {
        "name": "Artist - AlbumName",
        "relative_path": "/Music/Albums/Artist - AlbumName"
      }
    ],
    "files": [
      {
        "id": "1a2b3c4d5e",
        "name": "01 - TrackOne.mp3",
        "relative_path": "/Music/Albums/Artist - AlbumName/01 - TrackOne.mp3",
        "extension": "mp3",
        "size_bytes": 10485760
      }
    ]
  }
  ```

### Directory Tree

* **Endpoint:** `GET /files/tree`
* **Authentication:** Authenticated Session
* **Query Parameters:**
  * `depth` (integer, optional): Maximum depth to traverse. Defaults to 2.
* **Description:** Returns the directory structure tree starting from the root directory.
* **Response:** `200 OK` (application/json)
  ```json
  {
    "name": "root",
    "relative_path": "/",
    "children": [
      {
        "name": "Music",
        "relative_path": "/Music",
        "children": []
      }
    ]
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
    "file_ids": ["1a2b3c4d5e"]
  }
  ```
* **Description:** Reads standard metadata tags (title, artist, album, track number, year, genre, etc.) for a list of file IDs. This endpoint is fast as it does not parse audio properties.
* **Response:** `200 OK` (application/json)
  ```json
  {
    "tags": {
      "1a2b3c4d5e": {
        "title": "Track One",
        "artist": "Artist Name",
        "album": "Album Name",
        "track_number": "1",
        "year": "2026",
        "genre": "Electronic",
        "has_cover": true
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
    "file_ids": ["1a2b3c4d5e"]
  }
  ```
* **Description:** Reads audio properties (duration in seconds, bitrate in kbps, sample rate in Hz, channels) for a list of file IDs.
* **Response:** `200 OK` (application/json)
  ```json
  {
    "properties": {
      "1a2b3c4d5e": {
        "duration": 240,
        "bitrate": 320,
        "sample_rate": 44100,
        "channels": 2
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
    "edits": {
      "1a2b3c4d5e": {
        "title": "New Track Title",
        "artist": "Updated Artist"
      }
    }
  }
  ```
* **Description:** Writes new tag values to the specified files in place. Specifying `null` or empty string removes the tag.
* **Response:** `200 OK` (application/json)
  ```json
  {
    "success": true
  }
  ```

---

## Cover Art Operations

### Get Cover Art Thumbnail

* **Endpoint:** `GET /coverart`
* **Authentication:** Authenticated Session
* **Query Parameters:**
  * `path` (string, required): Safe path to the audio file containing embedded cover art.
  * `size` (integer, optional): Maximum pixel dimension (e.g. 250, 500) for resizing.
* **Description:** Extracts and returns the embedded cover art image. Resizes the image on the server if `size` is specified.
* **Response:** `200 OK` (image/jpeg or image/png)

### Remove Cover Art

* **Endpoint:** `DELETE /coverart`
* **Authentication:** Authenticated Session
* **Query Parameters:**
  * `path` (string, required): Safe path to the audio file to remove cover art from.
* **Description:** Removes the embedded cover art from the target audio file.
* **Response:** `200 OK` (application/json)
  ```json
  {
    "success": true
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
    "file_ids": ["1a2b3c4d5e"],
    "pattern": "%track% - %artist% - %title%"
  }
  ```
* **Description:** Previews how files will be renamed using the given format string pattern. Resolves placeholders using the file's current tags.
* **Response:** `200 OK` (application/json)
  ```json
  {
    "previews": {
      "1a2b3c4d5e": {
        "old_name": "01 - TrackOne.mp3",
        "new_name": "01 - Artist Name - Track One.mp3",
        "safe": true
      }
    }
  }
  ```

### Execute Renames

* **Endpoint:** `POST /rename/execute`
* **Authentication:** Authenticated Session
* **Request Body:** (application/json)
  ```json
  {
    "file_ids": ["1a2b3c4d5e"],
    "pattern": "%track% - %artist% - %title%"
  }
  ```
* **Description:** Renames the files on disk using the given format string pattern.
* **Response:** `200 OK` (application/json)
  ```json
  {
    "success": true
  }
  ```

---

## Metadata Search & Lookup

### Search MusicBrainz Releases

* **Endpoint:** `GET /lookup/musicbrainz/search`
* **Authentication:** Authenticated Session
* **Query Parameters:**
  * `query` (string, required): Free-text release search query (typically `artist album`).
* **Description:** Searches the MusicBrainz database for matching release entries.
* **Response:** `200 OK` (application/json)
  ```json
  [
    {
      "source": "MusicBrainz",
      "id": "release-uuid-here",
      "title": "Album Title",
      "artist": "Artist Name",
      "date": "2026",
      "track_count": 12,
      "cover_art_url": "https://coverartarchive.org/release/release-uuid-here/front-250"
    }
  ]
  ```

### Get MusicBrainz Release Details

* **Endpoint:** `GET /lookup/musicbrainz/release/:mbid`
* **Authentication:** Authenticated Session
* **Path Parameters:**
  * `mbid` (string, required): The MusicBrainz Release ID UUID.
* **Description:** Retrieves detailed release tracks and metadata, including cover art URLs.
* **Response:** `200 OK` (application/json)
  ```json
  {
    "source": "MusicBrainz",
    "id": "release-uuid-here",
    "title": "Album Title",
    "artist": "Artist Name",
    "date": "2026",
    "cover_art_url": "https://coverartarchive.org/release/release-uuid-here/front-500",
    "tracks": [
      {
        "position": 1,
        "title": "Track One Title",
        "artist": "Artist Name",
        "duration_secs": 240
      }
    ]
  }
  ```

---

## Authentication & User Administration

### First-Time Setup

* **Endpoint:** `POST /auth/setup`
* **Authentication:** None (Only available if no users exist in the database)
* **Request Body:** (application/json)
  ```json
  {
    "username": "admin",
    "password": "super-secure-password"
  }
  ```
* **Description:** Creates the first user account, which automatically receives the `super_admin` role. Once a user is created, this endpoint will return `400 Bad Request`.
* **Response:** `200 OK` (application/json) - sets the `tagstudio_session` cookie.
  ```json
  {
    "user": {
      "id": "user-uuid-here",
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
* **Description:** Verifies credentials and creates a session.
* **Response:** `200 OK` (application/json) - sets the `tagstudio_session` cookie.
  ```json
  {
    "user": {
      "id": "user-uuid-here",
      "username": "admin",
      "role": "super_admin"
    }
  }
  ```

### Logout

* **Endpoint:** `POST /auth/logout`
* **Authentication:** Authenticated Session
* **Description:** Invalidates the current session token and clears the cookie.
* **Response:** `200 OK` (application/json)
  ```json
  {
    "success": true
  }
  ```

### Check Auth Status

* **Endpoint:** `GET /auth/check`
* **Authentication:** None
* **Description:** Checks if setup is completed and returns the current user profile if a session cookie is present.
* **Response:** `200 OK` (application/json)
  ```json
  {
    "setup_required": false,
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
* **Description:** Registers a new administrator user using a valid, active invite link token.
* **Response:** `200 OK` (application/json)
  ```json
  {
    "user": {
      "id": "new-user-uuid",
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
    "expires_at": "2026-05-27T14:20:00Z"
  }
  ```

### List Invites

* **Endpoint:** `GET /auth/invites`
* **Authentication:** Super Admin Only
* **Description:** Returns all active, unexpired registration invite tokens.
* **Response:** `200 OK` (application/json)
  ```json
  [
    {
      "token": "invite-token-uuid",
      "created_at": "2026-05-25T14:20:00Z",
      "expires_at": "2026-05-27T14:20:00Z",
      "created_by": "super-admin-uuid"
    }
  ]
  ```

### Revoke Invite

* **Endpoint:** `DELETE /auth/invites/:token`
* **Authentication:** Super Admin Only
* **Path Parameters:**
  * `token` (string, required): The invite token to delete.
* **Description:** Revokes and deletes an active invite token.
* **Response:** `200 OK` (application/json)
  ```json
  {
    "success": true
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
* **Description:** Deletes a user account and invalidates all active sessions for that user.
* **Response:** `200 OK` (application/json)
  ```json
  {
    "success": true
  }
  ```
