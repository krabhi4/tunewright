use crate::types::{TagData, TagStudioError, TagWriteChanges, WriteResult};
use lofty::config::WriteOptions;
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::probe::Probe;
use lofty::tag::{Accessor, Tag};
use std::collections::HashMap;
use std::path::Path;

/// Read full tag data from an audio file
pub fn read_tags(path: &Path) -> Result<TagData, TagStudioError> {
    let tagged = Probe::open(path)
        .map_err(|e| TagStudioError::TagReadError(format!("{}: {}", path.display(), e)))?
        .read()
        .map_err(|e| TagStudioError::TagReadError(format!("{}: {}", path.display(), e)))?;

    // Get audio properties
    let props = tagged.properties();
    let duration = props.duration();
    let duration_secs = if duration.as_secs() > 0 || duration.subsec_millis() > 0 {
        Some(duration.as_secs_f64())
    } else {
        None
    };

    // Collect tag type names
    let tag_types: Vec<String> = tagged
        .tags()
        .iter()
        .map(|t| format!("{:?}", t.tag_type()))
        .collect();

    // Merge all tags (prefer first non-empty value across tag types)
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

    // Get album artist and composer from item keys
    let album_artist = first_item_value(&tags, "ALBUMARTIST")
        .or_else(|| first_item_value(&tags, "ALBUM ARTIST"))
        .or_else(|| first_item_value(&tags, "TPE2"));
    let composer = first_item_value(&tags, "COMPOSER")
        .or_else(|| first_item_value(&tags, "TCOM"));

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
    })
}

/// Batch read tags for multiple files. Returns a map of relative_path -> TagData.
/// Errors for individual files are silently skipped (logged).
pub fn batch_read_tags(
    data_root: &Path,
    relative_paths: &[String],
) -> HashMap<String, TagData> {
    let mut results = HashMap::new();

    for rel_path in relative_paths {
        let full_path = data_root.join(rel_path);
        match read_tags(&full_path) {
            Ok(tags) => {
                results.insert(rel_path.clone(), tags);
            }
            Err(e) => {
                tracing::warn!("Failed to read tags for {}: {}", rel_path, e);
            }
        }
    }

    results
}

/// Write tag changes to a single audio file
pub fn write_tags(path: &Path, changes: &TagWriteChanges) -> Result<(), TagStudioError> {
    let mut tagged = Probe::open(path)
        .map_err(|e| TagStudioError::TagWriteError(format!("{}: {}", path.display(), e)))?
        .read()
        .map_err(|e| TagStudioError::TagWriteError(format!("{}: {}", path.display(), e)))?;

    // Get or create the primary tag
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

    // Apply changes (only fields that are Some)
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

    // Save the file
    tagged
        .save_to_path(path, WriteOptions::default())
        .map_err(|e| TagStudioError::TagWriteError(format!("{}: {}", path.display(), e)))?;

    Ok(())
}

/// Batch write tags. Returns per-file results.
pub fn batch_write_tags(
    data_root: &Path,
    changes: &[(String, String, TagWriteChanges)], // (id, relative_path, changes)
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
