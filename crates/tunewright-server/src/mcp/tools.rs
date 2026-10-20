use std::sync::LazyLock;

use axum::extract::{Path, Query, State};
use axum::response::IntoResponse;
use axum::Json;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Value};
use tunewright_core::actions::Action;
use tunewright_core::picture;
use tunewright_core::scanner;
use tunewright_core::types::{TagWriteChanges, TunewrightError};

use crate::error::{join_error, AppError};
use crate::routes::{actions, coverart, filename_to_tag, files, lookup, rename, tags};
use crate::state::AppState;

const DEFAULT_LIST_LIMIT: usize = 100;
const MAX_LIST_LIMIT: usize = 1000;

const PATH_HINT: &str = "Paths are relative to the library root, exactly as returned in `relative_path` by tunewright_list_files.";

const TAG_FIELDS: &str = "title, artist, album, album_artist, year, track_number, track_total, disc_number, disc_total, genre, comment, composer";

const FORMAT_HINT: &str = "Placeholders: %title% %artist% %album% %albumartist% %year% %track% (zero-padded) %track_total% %disc% %disc_total% %genre% %comment% %composer% %_filename% %_index%. Functions: $upper(x) $lower(x) $caps(x) $left(x,n) $right(x,n) $mid(x,start,len) $replace(x,find,with) $regex(x,pattern,with) $trim(x) $len(x).";

fn read_only(open_world: bool) -> Value {
    json!({ "readOnlyHint": true, "openWorldHint": open_world })
}

fn writes(idempotent: bool, open_world: bool) -> Value {
    json!({
        "readOnlyHint": false,
        "destructiveHint": true,
        "idempotentHint": idempotent,
        "openWorldHint": open_world
    })
}

fn tool(
    name: &str,
    title: &str,
    description: String,
    annotations: Value,
    input_schema: Value,
) -> Value {
    let mut annotations = annotations;
    annotations["title"] = json!(title);
    json!({
        "name": name,
        "title": title,
        "description": description,
        "inputSchema": input_schema,
        "annotations": annotations
    })
}

fn string_list(description: &str) -> Value {
    json!({ "type": "array", "items": { "type": "string" }, "minItems": 1, "description": description })
}

static TOOLS: LazyLock<Vec<Value>> = LazyLock::new(|| {
    let action_schema = json!({
        "type": "object",
        "description": format!("One action, selected by `type`. Field names: {TAG_FIELDS}, or any custom tag key."),
        "oneOf": [
            { "type": "object", "required": ["type", "field", "mode"], "properties": {
                "type": { "const": "case_conversion" }, "field": { "type": "string" },
                "mode": { "enum": ["title", "upper", "lower", "sentence"] } } },
            { "type": "object", "required": ["type", "field", "search", "replace"], "properties": {
                "type": { "const": "replace" }, "field": { "type": "string" },
                "search": { "type": "string" }, "replace": { "type": "string" },
                "regex": { "type": "boolean", "default": false } } },
            { "type": "object", "required": ["type", "field", "format"], "properties": {
                "type": { "const": "format_value" }, "field": { "type": "string" },
                "format": { "type": "string", "description": FORMAT_HINT } } },
            { "type": "object", "required": ["type", "field", "value"], "properties": {
                "type": { "const": "set_field" }, "field": { "type": "string" }, "value": { "type": "string" } } },
            { "type": "object", "required": ["type", "field"], "properties": {
                "type": { "const": "remove_field" }, "field": { "type": "string" } } },
            { "type": "object", "required": ["type", "fields"], "properties": {
                "type": { "const": "remove_all_except" }, "fields": { "type": "array", "items": { "type": "string" } } } },
            { "type": "object", "required": ["type", "field"], "properties": {
                "type": { "const": "auto_number" }, "field": { "type": "string" },
                "start": { "type": "integer", "minimum": 0, "default": 1 },
                "padding": { "type": "integer", "minimum": 0, "default": 2 } } },
            { "type": "object", "required": ["type", "source", "separator", "part", "target"], "properties": {
                "type": { "const": "split_field" }, "source": { "type": "string" },
                "separator": { "type": "string" },
                "part": { "type": "integer", "minimum": 0, "description": "0-based index of the part to keep." },
                "target": { "type": "string" } } },
            { "type": "object", "required": ["type", "sources", "separator", "target"], "properties": {
                "type": { "const": "merge_fields" }, "sources": { "type": "array", "items": { "type": "string" } },
                "separator": { "type": "string" }, "target": { "type": "string" } } },
            { "type": "object", "required": ["type", "field"], "properties": {
                "type": { "const": "trim_field" }, "field": { "type": "string" } } }
        ]
    });
    let files_and_actions = json!({
        "type": "object",
        "properties": {
            "files": string_list(PATH_HINT),
            "actions": { "type": "array", "items": action_schema, "minItems": 1, "description": "Actions applied in order to each file." }
        },
        "required": ["files", "actions"]
    });
    let files_and_format = json!({
        "type": "object",
        "properties": {
            "files": string_list(PATH_HINT),
            "format": { "type": "string", "description": format!("New file name without extension; the extension is kept and the file stays in its folder. {FORMAT_HINT}") }
        },
        "required": ["files", "format"]
    });
    let paths = json!({
        "type": "object",
        "properties": { "paths": string_list(PATH_HINT) },
        "required": ["paths"]
    });
    let query = json!({
        "type": "object",
        "properties": { "query": { "type": "string", "description": "Free text, e.g. artist and album name." } },
        "required": ["query"]
    });

    vec![
        tool(
            "tunewright_list_files",
            "List library folder",
            "List the subfolders and audio files in one library folder, with format, size, duration and cover presence. Start at `/` and walk `directories` to explore.".to_string(),
            read_only(false),
            json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Folder relative to the library root. Defaults to `/`." },
                    "offset": { "type": "integer", "minimum": 0, "default": 0 },
                    "limit": { "type": "integer", "minimum": 1, "maximum": MAX_LIST_LIMIT, "default": DEFAULT_LIST_LIMIT, "description": "Files per page; use `offset` with `total` to page through large folders." }
                }
            }),
        ),
        tool(
            "tunewright_read_tags",
            "Read tags",
            format!("Read the tags of audio files ({TAG_FIELDS} and custom fields). Returns `tags` keyed by path; unreadable or missing files are left out."),
            read_only(false),
            paths.clone(),
        ),
        tool(
            "tunewright_get_audio_properties",
            "Read audio properties",
            "Read tags plus technical properties (duration, bitrate, sample rate, channels). Slower than tunewright_read_tags.".to_string(),
            read_only(false),
            paths.clone(),
        ),
        tool(
            "tunewright_write_tags",
            "Write tags",
            "Write tags to audio files. Per field: omit to keep it, a value to set it, null or an empty string to remove it.".to_string(),
            writes(true, false),
            json!({
                "type": "object",
                "properties": {
                    "changes": {
                        "type": "array",
                        "minItems": 1,
                        "items": {
                            "type": "object",
                            "properties": {
                                "path": { "type": "string", "description": PATH_HINT },
                                "tags": {
                                    "type": "object",
                                    "properties": {
                                        "title": { "type": ["string", "null"] },
                                        "artist": { "type": ["string", "null"] },
                                        "album": { "type": ["string", "null"] },
                                        "album_artist": { "type": ["string", "null"] },
                                        "year": { "type": ["integer", "null"], "minimum": 0 },
                                        "track_number": { "type": ["integer", "null"], "minimum": 0 },
                                        "track_total": { "type": ["integer", "null"], "minimum": 0 },
                                        "disc_number": { "type": ["integer", "null"], "minimum": 0 },
                                        "disc_total": { "type": ["integer", "null"], "minimum": 0 },
                                        "genre": { "type": ["string", "null"] },
                                        "comment": { "type": ["string", "null"] },
                                        "composer": { "type": ["string", "null"] },
                                        "extra": {
                                            "type": "object",
                                            "additionalProperties": { "type": ["string", "null"] },
                                            "description": "Custom tag keys; null removes the key."
                                        }
                                    }
                                }
                            },
                            "required": ["path", "tags"]
                        }
                    }
                },
                "required": ["changes"]
            }),
        ),
        tool(
            "tunewright_preview_rename",
            "Preview rename",
            "Preview renaming files from their tags without touching anything. Check `conflict` and `error` before running tunewright_execute_rename.".to_string(),
            read_only(false),
            files_and_format.clone(),
        ),
        tool(
            "tunewright_execute_rename",
            "Rename files",
            "Rename files from their tags, inside their current folder. Run tunewright_preview_rename first.".to_string(),
            writes(false, false),
            files_and_format,
        ),
        tool(
            "tunewright_preview_filename_to_tag",
            "Preview tags from filenames",
            "Preview the tag values parsed out of file names with a pattern such as `%track% - %artist% - %title%`. Apply the result with tunewright_write_tags.".to_string(),
            read_only(false),
            json!({
                "type": "object",
                "properties": {
                    "files": string_list(PATH_HINT),
                    "pattern": { "type": "string", "description": "Pattern matched against the file name without extension, using %field% placeholders." }
                },
                "required": ["files", "pattern"]
            }),
        ),
        tool(
            "tunewright_preview_actions",
            "Preview batch actions",
            "Preview batch tag actions (case conversion, find and replace, formatting, numbering, splitting, merging) without writing. Only files that would change are listed.".to_string(),
            read_only(false),
            files_and_actions.clone(),
        ),
        tool(
            "tunewright_apply_actions",
            "Apply batch actions",
            "Apply batch tag actions to files and write the result. Run tunewright_preview_actions first.".to_string(),
            writes(false, false),
            files_and_actions,
        ),
        tool(
            "tunewright_search_musicbrainz",
            "Search MusicBrainz",
            "Search MusicBrainz releases. Requests are rate limited to about one per second.".to_string(),
            read_only(true),
            query.clone(),
        ),
        tool(
            "tunewright_get_musicbrainz_release",
            "Get MusicBrainz release",
            "Fetch a MusicBrainz release with its full track list.".to_string(),
            read_only(true),
            json!({
                "type": "object",
                "properties": { "id": { "type": "string", "description": "MusicBrainz release ID (UUID) from tunewright_search_musicbrainz." } },
                "required": ["id"]
            }),
        ),
        tool(
            "tunewright_search_applemusic",
            "Search Apple Music",
            "Search Apple Music albums.".to_string(),
            read_only(true),
            query,
        ),
        tool(
            "tunewright_get_applemusic_release",
            "Get Apple Music album",
            "Fetch an Apple Music album with its full track list.".to_string(),
            read_only(true),
            json!({
                "type": "object",
                "properties": { "id": { "type": "string", "description": "Apple Music album ID from tunewright_search_applemusic." } },
                "required": ["id"]
            }),
        ),
        tool(
            "tunewright_get_coverart_info",
            "Inspect cover art",
            "Report whether a file has embedded cover art, with its MIME type, byte size and pixel dimensions.".to_string(),
            read_only(false),
            json!({
                "type": "object",
                "properties": { "path": { "type": "string", "description": PATH_HINT } },
                "required": ["path"]
            }),
        ),
        tool(
            "tunewright_embed_coverart_url",
            "Embed cover art from URL",
            "Download a JPEG or PNG (max 10 MB) and embed it as the cover of each file, replacing any existing cover. Only https URLs on coverartarchive.org, archive.org and mzstatic.com are allowed, such as the cover URLs returned by the lookup tools.".to_string(),
            writes(true, true),
            json!({
                "type": "object",
                "properties": {
                    "url": { "type": "string", "format": "uri" },
                    "paths": string_list(PATH_HINT)
                },
                "required": ["url", "paths"]
            }),
        ),
    ]
});

pub fn find(name: &str) -> Option<&'static Value> {
    TOOLS.iter().find(|t| t["name"] == name)
}

pub fn is_read_only(tool: &Value) -> bool {
    tool["annotations"]["readOnlyHint"] == true
}

pub fn list(can_write: bool) -> Vec<&'static Value> {
    TOOLS
        .iter()
        .filter(|t| can_write || is_read_only(t))
        .collect()
}

#[derive(Deserialize)]
struct PathsArgs {
    paths: Vec<String>,
}

#[derive(Deserialize)]
struct WriteArgs {
    changes: Vec<WriteChange>,
}

#[derive(Deserialize)]
struct WriteChange {
    path: String,
    tags: TagWriteChanges,
}

#[derive(Deserialize)]
struct FormatArgs {
    files: Vec<String>,
    format: String,
}

#[derive(Deserialize)]
struct PatternArgs {
    files: Vec<String>,
    pattern: String,
}

#[derive(Deserialize)]
struct ActionArgs {
    files: Vec<String>,
    actions: Vec<Action>,
}

#[derive(Deserialize)]
struct IdArgs {
    id: String,
}

#[derive(Deserialize)]
struct PathArgs {
    path: String,
}

fn parse<T: DeserializeOwned>(args: Value) -> Result<T, String> {
    serde_json::from_value(args).map_err(|e| format!("Invalid arguments: {e}"))
}

async fn respond(response: impl IntoResponse) -> Result<String, String> {
    let response = response.into_response();
    let ok = response.status().is_success();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .map_err(|e| format!("Failed to read result: {e}"))?;
    let text = String::from_utf8_lossy(&body).into_owned();
    if ok {
        return Ok(text);
    }
    Err(serde_json::from_str::<Value>(&text)
        .ok()
        .and_then(|v| v["error"].as_str().map(String::from))
        .unwrap_or(text))
}

fn read_request(paths: Vec<String>) -> tags::ReadTagsRequest {
    tags::ReadTagsRequest {
        paths: paths.iter().map(|p| (p.clone(), p.clone())).collect(),
        ids: paths,
    }
}

fn rename_request(args: FormatArgs) -> rename::RenameRequest {
    rename::RenameRequest {
        files: args
            .files
            .into_iter()
            .map(|path| rename::RenameFileEntry {
                id: path.clone(),
                path,
            })
            .collect(),
        format: args.format,
    }
}

fn actions_request(args: ActionArgs) -> actions::ExecuteActionsRequest {
    actions::ExecuteActionsRequest {
        files: args
            .files
            .into_iter()
            .map(|path| actions::ActionFileEntry {
                id: path.clone(),
                path,
            })
            .collect(),
        actions: args.actions,
    }
}

async fn cover_info(state: AppState, path: String) -> Result<String, String> {
    let data_root = state.data_root.clone();
    let result = tokio::task::spawn_blocking(move || {
        let art = picture::extract_cover_art(&scanner::resolve_safe_path(&data_root, &path)?)?;
        Ok::<_, TunewrightError>(Json(match art {
            Some((data, mime)) => {
                let dims = image::ImageReader::new(std::io::Cursor::new(&data))
                    .with_guessed_format()
                    .ok()
                    .and_then(|r| r.into_dimensions().ok());
                json!({
                    "has_cover": true,
                    "mime": picture::sanitize_mime(&mime),
                    "size_bytes": data.len(),
                    "width": dims.map(|d| d.0),
                    "height": dims.map(|d| d.1)
                })
            }
            None => json!({ "has_cover": false }),
        }))
    })
    .await
    .map_err(join_error)
    .and_then(|r| r.map_err(AppError));
    respond(result).await
}

pub async fn call(state: AppState, name: &str, args: Value) -> Result<String, String> {
    let s = State(state.clone());
    match name {
        "tunewright_list_files" => {
            let mut args = args;
            if args.get("limit").is_none() {
                args["limit"] = json!(DEFAULT_LIST_LIMIT);
            }
            let mut query: files::ListFilesQuery = parse(args)?;
            query.limit = query.limit.min(MAX_LIST_LIMIT);
            respond(files::list_files(s, Query(query)).await).await
        }
        "tunewright_read_tags" => {
            let args: PathsArgs = parse(args)?;
            respond(tags::read_tags(s, Json(read_request(args.paths))).await).await
        }
        "tunewright_get_audio_properties" => {
            let args: PathsArgs = parse(args)?;
            respond(tags::read_properties(s, Json(read_request(args.paths))).await).await
        }
        "tunewright_write_tags" => {
            let args: WriteArgs = parse(args)?;
            let changes = args
                .changes
                .into_iter()
                .map(|c| tags::WriteTagsEntry {
                    id: c.path.clone(),
                    path: c.path,
                    tags: c.tags,
                })
                .collect();
            respond(tags::write_tags(s, Json(tags::WriteTagsRequest { changes })).await).await
        }
        "tunewright_preview_rename" => {
            respond(rename::preview(s, Json(rename_request(parse(args)?))).await).await
        }
        "tunewright_execute_rename" => {
            respond(rename::execute(s, Json(rename_request(parse(args)?))).await).await
        }
        "tunewright_preview_filename_to_tag" => {
            let args: PatternArgs = parse(args)?;
            let request = filename_to_tag::FilenameToTagRequest {
                files: args
                    .files
                    .into_iter()
                    .map(|path| filename_to_tag::FileEntry {
                        id: path.clone(),
                        path,
                    })
                    .collect(),
                pattern: args.pattern,
            };
            respond(filename_to_tag::preview(s, Json(request)).await).await
        }
        "tunewright_preview_actions" => {
            respond(actions::preview(s, Json(actions_request(parse(args)?))).await).await
        }
        "tunewright_apply_actions" => {
            respond(actions::execute(s, Json(actions_request(parse(args)?))).await).await
        }
        "tunewright_search_musicbrainz" => {
            respond(lookup::musicbrainz_search(s, Query(parse(args)?)).await).await
        }
        "tunewright_get_musicbrainz_release" => {
            let args: IdArgs = parse(args)?;
            respond(lookup::musicbrainz_release(s, Path(args.id)).await).await
        }
        "tunewright_search_applemusic" => {
            respond(lookup::applemusic_search(s, Query(parse(args)?)).await).await
        }
        "tunewright_get_applemusic_release" => {
            let args: IdArgs = parse(args)?;
            respond(lookup::applemusic_release(s, Path(args.id)).await).await
        }
        "tunewright_get_coverart_info" => {
            let args: PathArgs = parse(args)?;
            cover_info(state, args.path).await
        }
        "tunewright_embed_coverart_url" => {
            respond(coverart::embed_cover_art_from_url(s, Json(parse(args)?)).await).await
        }
        unknown => Err(format!("Unknown tool: {unknown}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_listed_tool_is_dispatched_and_annotated() {
        let source = include_str!("tools.rs");
        for t in TOOLS.iter() {
            let name = t["name"].as_str().unwrap();
            assert!(
                source.contains(&format!("\"{name}\" =>")),
                "{name} has no dispatch arm"
            );
            assert!(
                t["annotations"]["readOnlyHint"].is_boolean(),
                "{name} lacks readOnlyHint"
            );
            assert_eq!(t["inputSchema"]["type"], "object");
        }
        assert_eq!(
            list(false).len(),
            TOOLS.iter().filter(|t| is_read_only(t)).count()
        );
        assert!(list(false).iter().all(|t| is_read_only(t)));
        assert_eq!(list(true).len(), TOOLS.len());
    }
}
