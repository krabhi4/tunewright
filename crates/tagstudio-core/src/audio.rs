use crate::types::{TagData, TagStudioError, TagWriteChanges, WriteResult};
use lofty::config::{ParseOptions, ParsingMode, WriteOptions};
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::probe::Probe;
use lofty::tag::{Accessor, Tag};
use rayon::prelude::*;
use std::collections::HashMap;
use std::path::Path;

/// Fast parse options: tags only, no audio properties, no cover art data.
/// This only reads the tag headers (first few KB of the file).
fn fast_parse_options() -> ParseOptions {
    ParseOptions::new()
        .read_properties(false)
        .read_cover_art(false)
        .parsing_mode(ParsingMode::Relaxed)
}

/// Full parse options: everything including audio properties.
/// Needed for duration, bitrate, sample rate.
fn full_parse_options() -> ParseOptions {
    ParseOptions::new()
        .read_properties(true)
        .read_cover_art(false) // still skip loading cover art bytes
        .parsing_mode(ParsingMode::BestAttempt)
}

/// Read tags FAST — skips audio properties and cover art data.
/// Returns tag text fields only (title, artist, album, etc.).
/// Use this for populating the grid quickly.
pub fn read_tags_fast(path: &Path) -> Result<TagData, TagStudioError> {
    let tagged = Probe::open(path)
        .map_err(|e| TagStudioError::TagReadError(format!("{}: {}", path.display(), e)))?
        .options(fast_parse_options())
        .read()
        .map_err(|e| TagStudioError::TagReadError(format!("{}: {}", path.display(), e)))?;

    let tag_types: Vec<String> = tagged
        .tags()
        .iter()
        .map(|t| format!("{:?}", t.tag_type()))
        .collect();

    let tags: Vec<&Tag> = tagged.tags().iter().collect();

    let title = first_string(&tags, |t| t.title());
    let artist = first_string(&tags, |t| t.artist());
    let album = first_string(&tags, |t| t.album());
    let genre = first_string(&tags, |t| t.genre());
    let comment = first_string(&tags, |t| t.comment());
    let year = tags.iter().find_map(|t| t.year());
    let track_number = tags.iter().find_map(|t| t.track());
    let track_total = tags.iter().find_map(|t| t.track_total());
    let disc_number = tags.iter().find_map(|t| t.disk());
    let disc_total = tags.iter().find_map(|t| t.disk_total());

    let album_artist = first_item_value(&tags, "ALBUMARTIST")
        .or_else(|| first_item_value(&tags, "ALBUM ARTIST"))
        .or_else(|| first_item_value(&tags, "TPE2"));
    let composer = first_item_value(&tags, "COMPOSER")
        .or_else(|| first_item_value(&tags, "TCOM"));

    // Check picture count without loading picture data
    let has_cover = tags.iter().any(|t| !t.pictures().is_empty());

    Ok(TagData {
        title,
        artist,
        album,
        album_artist,
        year,
        track_number,
        track_total,
        disc_number,
        disc_total,
        genre,
        comment,
        composer,
        // Audio properties not available in fast mode
        bitrate: None,
        sample_rate: None,
        channels: None,
        duration_secs: None,
        format: Some(format!("{:?}", tagged.file_type())),
        tag_types,
        has_cover,
    })
}

/// Read tags with full audio properties (duration, bitrate, sample rate).
/// Slower — use for detailed view or when user explicitly requests properties.
pub fn read_tags_full(path: &Path) -> Result<TagData, TagStudioError> {
    let tagged = Probe::open(path)
        .map_err(|e| TagStudioError::TagReadError(format!("{}: {}", path.display(), e)))?
        .options(full_parse_options())
        .read()
        .map_err(|e| TagStudioError::TagReadError(format!("{}: {}", path.display(), e)))?;

    let props = tagged.properties();
    let duration = props.duration();
    let duration_secs = if duration.as_secs() > 0 || duration.subsec_millis() > 0 {
        Some(duration.as_secs_f64())
    } else {
        None
    };

    let tag_types: Vec<String> = tagged
        .tags()
        .iter()
        .map(|t| format!("{:?}", t.tag_type()))
        .collect();

    let tags: Vec<&Tag> = tagged.tags().iter().collect();

    let title = first_string(&tags, |t| t.title());
    let artist = first_string(&tags, |t| t.artist());
    let album = first_string(&tags, |t| t.album());
    let genre = first_string(&tags, |t| t.genre());
    let comment = first_string(&tags, |t| t.comment());
    let year = tags.iter().find_map(|t| t.year());
    let track_number = tags.iter().find_map(|t| t.track());
    let track_total = tags.iter().find_map(|t| t.track_total());
    let disc_number = tags.iter().find_map(|t| t.disk());
    let disc_total = tags.iter().find_map(|t| t.disk_total());

    let album_artist = first_item_value(&tags, "ALBUMARTIST")
        .or_else(|| first_item_value(&tags, "ALBUM ARTIST"))
        .or_else(|| first_item_value(&tags, "TPE2"));
    let composer = first_item_value(&tags, "COMPOSER")
        .or_else(|| first_item_value(&tags, "TCOM"));

    let has_cover = tags.iter().any(|t| !t.pictures().is_empty());

    Ok(TagData {
        title,
        artist,
        album,
        album_artist,
        year,
        track_number,
        track_total,
        disc_number,
        disc_total,
        genre,
        comment,
        composer,
        bitrate: Some(props.audio_bitrate().unwrap_or(0)),
        sample_rate: Some(props.sample_rate().unwrap_or(0)),
        channels: Some(props.channels().unwrap_or(0) as u8),
        duration_secs,
        format: Some(format!("{:?}", tagged.file_type())),
        tag_types,
        has_cover,
    })
}

/// Parallel batch read tags (fast mode — tags only, no audio properties).
/// Uses rayon to read multiple files concurrently across CPU cores.
pub fn batch_read_tags(
    data_root: &Path,
    relative_paths: &[String],
) -> HashMap<String, TagData> {
    relative_paths
        .par_iter()
        .filter_map(|rel_path| {
            let full_path = data_root.join(rel_path);
            match read_tags_fast(&full_path) {
                Ok(tags) => Some((rel_path.clone(), tags)),
                Err(e) => {
                    tracing::warn!("Failed to read tags for {}: {}", rel_path, e);
                    None
                }
            }
        })
        .collect()
}

/// Parallel batch read tags with full audio properties.
pub fn batch_read_tags_full(
    data_root: &Path,
    relative_paths: &[String],
) -> HashMap<String, TagData> {
    relative_paths
        .par_iter()
        .filter_map(|rel_path| {
            let full_path = data_root.join(rel_path);
            match read_tags_full(&full_path) {
                Ok(tags) => Some((rel_path.clone(), tags)),
                Err(e) => {
                    tracing::warn!("Failed to read tags for {}: {}", rel_path, e);
                    None
                }
            }
        })
        .collect()
}

/// Write tag changes to a single audio file
pub fn write_tags(path: &Path, changes: &TagWriteChanges) -> Result<(), TagStudioError> {
    let mut tagged = Probe::open(path)
        .map_err(|e| TagStudioError::TagWriteError(format!("{}: {}", path.display(), e)))?
        .read()
        .map_err(|e| TagStudioError::TagWriteError(format!("{}: {}", path.display(), e)))?;

    let primary_type = tagged
        .primary_tag()
        .map(|t| t.tag_type())
        .unwrap_or(lofty::tag::TagType::Id3v2);

    let tag = match tagged.tag_mut(primary_type) {
        Some(t) => t,
        None => {
            tagged.insert_tag(Tag::new(primary_type));
            tagged.tag_mut(primary_type).unwrap()
        }
    };

    if let Some(ref v) = changes.title {
        tag.set_title(v.clone());
    }
    if let Some(ref v) = changes.artist {
        tag.set_artist(v.clone());
    }
    if let Some(ref v) = changes.album {
        tag.set_album(v.clone());
    }
    if let Some(ref v) = changes.genre {
        tag.set_genre(v.clone());
    }
    if let Some(ref v) = changes.comment {
        tag.set_comment(v.clone());
    }
    if let Some(v) = changes.year {
        tag.set_year(v);
    }
    if let Some(v) = changes.track_number {
        tag.set_track(v);
    }
    if let Some(v) = changes.track_total {
        tag.set_track_total(v);
    }
    if let Some(v) = changes.disc_number {
        tag.set_disk(v);
    }
    if let Some(v) = changes.disc_total {
        tag.set_disk_total(v);
    }

    tagged
        .save_to_path(path, WriteOptions::default())
        .map_err(|e| TagStudioError::TagWriteError(format!("{}: {}", path.display(), e)))?;

    Ok(())
}

/// Batch write tags. Returns per-file results.
pub fn batch_write_tags(
    data_root: &Path,
    changes: &[(String, String, TagWriteChanges)],
) -> Vec<WriteResult> {
    changes
        .iter()
        .map(|(id, rel_path, ch)| {
            let full_path = data_root.join(rel_path);
            match write_tags(&full_path, ch) {
                Ok(()) => WriteResult {
                    id: id.clone(),
                    status: "ok".to_string(),
                    error: None,
                },
                Err(e) => WriteResult {
                    id: id.clone(),
                    status: "error".to_string(),
                    error: Some(e.to_string()),
                },
            }
        })
        .collect()
}

fn first_string<F>(tags: &[&Tag], accessor: F) -> Option<String>
where
    F: Fn(&Tag) -> Option<std::borrow::Cow<'_, str>>,
{
    tags.iter()
        .find_map(|t| accessor(t).map(|s| s.to_string()))
        .filter(|s| !s.is_empty())
}

fn first_item_value(tags: &[&Tag], key: &str) -> Option<String> {
    for tag in tags {
        for item in tag.items() {
            let item_key = match item.key() {
                lofty::tag::ItemKey::Unknown(k) => k.to_string(),
                other => format!("{:?}", other),
            };
            if item_key.eq_ignore_ascii_case(key) {
                if let lofty::tag::ItemValue::Text(val) = item.value() {
                    if !val.is_empty() {
                        return Some(val.to_string());
                    }
                }
            }
        }
    }
    None
}
