use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Supported audio formats
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AudioFormat {
    Mp3,
    Flac,
    Mp4,
    Ogg,
    Opus,
    Wav,
    Aiff,
}

impl AudioFormat {
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "mp3" => Some(Self::Mp3),
            "flac" => Some(Self::Flac),
            "m4a" | "m4b" | "mp4" | "m4v" => Some(Self::Mp4),
            "ogg" | "oga" => Some(Self::Ogg),
            "opus" => Some(Self::Opus),
            "wav" | "wave" => Some(Self::Wav),
            "aif" | "aiff" | "aifc" => Some(Self::Aiff),
            _ => None,
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Mp3 => "MP3",
            Self::Flac => "FLAC",
            Self::Mp4 => "M4A",
            Self::Ogg => "OGG",
            Self::Opus => "Opus",
            Self::Wav => "WAV",
            Self::Aiff => "AIFF",
        }
    }
}

/// Lightweight file entry returned by directory scanning (no tag data)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub id: String,
    pub filename: String,
    pub relative_path: String,
    pub format: AudioFormat,
    /// Human-readable format label (e.g. "M4A"); the single source of truth is
    /// `AudioFormat::display_name`, sent to clients so they don't re-map it.
    pub format_label: String,
    pub size: u64,
    pub duration_secs: Option<f64>,
    pub has_cover: bool,
    pub modified_at: String,
}

/// Full tag data for a single audio file
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TagData {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artist: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub album: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub album_artist: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub year: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub track_number: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub track_total: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disc_number: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disc_total: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub genre: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub composer: Option<String>,

    // Read-only audio properties
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bitrate: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sample_rate: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channels: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_secs: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tag_types: Vec<String>,
    #[serde(default)]
    pub has_cover: bool,

    /// Extra/custom tag fields not covered by the standard fields above
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub extra: HashMap<String, String>,
}

/// Directory tree node for folder picker
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirNode {
    pub name: String,
    pub path: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<DirNode>,
}

/// Result of listing files in a directory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileListResult {
    pub path: String,
    pub files: Vec<FileEntry>,
    pub total: usize,
    pub directories: Vec<String>,
}

/// Changes to write to a single file's tags. Per field: absent leaves it
/// unchanged, `null` (or an empty string) removes it, a value sets it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct TagWriteChanges {
    #[serde(
        default,
        deserialize_with = "double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub title: Option<Option<String>>,
    #[serde(
        default,
        deserialize_with = "double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub artist: Option<Option<String>>,
    #[serde(
        default,
        deserialize_with = "double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub album: Option<Option<String>>,
    #[serde(
        default,
        deserialize_with = "double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub album_artist: Option<Option<String>>,
    #[serde(
        default,
        deserialize_with = "double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub year: Option<Option<u32>>,
    #[serde(
        default,
        deserialize_with = "double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub track_number: Option<Option<u32>>,
    #[serde(
        default,
        deserialize_with = "double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub track_total: Option<Option<u32>>,
    #[serde(
        default,
        deserialize_with = "double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub disc_number: Option<Option<u32>>,
    #[serde(
        default,
        deserialize_with = "double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub disc_total: Option<Option<u32>>,
    #[serde(
        default,
        deserialize_with = "double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub genre: Option<Option<String>>,
    #[serde(
        default,
        deserialize_with = "double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub comment: Option<Option<String>>,
    #[serde(
        default,
        deserialize_with = "double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub composer: Option<Option<String>>,

    /// Extra/custom tag fields to write; a `null` value removes the key
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extra: Option<HashMap<String, Option<String>>>,
}

fn double_option<'de, T, D>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}

impl TagWriteChanges {
    pub fn diff(old: &TagData, new: &TagData) -> Self {
        fn field<T: PartialEq + Clone>(old: &Option<T>, new: &Option<T>) -> Option<Option<T>> {
            (old != new).then(|| new.clone())
        }
        let extra: HashMap<String, Option<String>> = old
            .extra
            .keys()
            .chain(new.extra.keys())
            .filter(|k| old.extra.get(*k) != new.extra.get(*k))
            .map(|k| (k.clone(), new.extra.get(k).cloned()))
            .collect();
        Self {
            title: field(&old.title, &new.title),
            artist: field(&old.artist, &new.artist),
            album: field(&old.album, &new.album),
            album_artist: field(&old.album_artist, &new.album_artist),
            year: field(&old.year, &new.year),
            track_number: field(&old.track_number, &new.track_number),
            track_total: field(&old.track_total, &new.track_total),
            disc_number: field(&old.disc_number, &new.disc_number),
            disc_total: field(&old.disc_total, &new.disc_total),
            genre: field(&old.genre, &new.genre),
            comment: field(&old.comment, &new.comment),
            composer: field(&old.composer, &new.composer),
            extra: (!extra.is_empty()).then_some(extra),
        }
    }
}

/// Result of writing tags to a single file
#[derive(Debug, Clone, Serialize)]
pub struct WriteResult {
    pub id: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Error types for tunewright-core
#[derive(Debug, thiserror::Error)]
pub enum TunewrightError {
    #[error("File not found: {0}")]
    FileNotFound(PathBuf),

    #[error("Unsupported format: {0}")]
    UnsupportedFormat(String),

    #[error("Tag read error: {0}")]
    TagReadError(String),

    #[error("Tag write error: {0}")]
    TagWriteError(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(PathBuf),

    #[error("Path traversal denied: {0}")]
    PathTraversal(String),

    #[error("Image processing error: {0}")]
    ImageError(String),

    #[error("Rename conflict: {0} already exists")]
    RenameConflict(PathBuf),

    #[error("Invalid format string: {0}")]
    InvalidFormatString(String),

    #[error("Request too large: {0}")]
    RequestTooLarge(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tag_write_changes_is_tri_state() {
        let c: TagWriteChanges =
            serde_json::from_str(r#"{"title":"T","year":null,"extra":{"Bpm":null,"Mood":"x"}}"#)
                .unwrap();
        assert_eq!(c.title, Some(Some("T".to_string())));
        assert_eq!(c.year, Some(None));
        assert_eq!(c.artist, None);
        let extra = c.extra.as_ref().unwrap();
        assert_eq!(extra.get("Bpm"), Some(&None));
        assert_eq!(extra.get("Mood"), Some(&Some("x".to_string())));

        let json = serde_json::to_value(&c).unwrap();
        assert_eq!(json["year"], serde_json::Value::Null);
        assert!(json.get("artist").is_none());
    }

    #[test]
    fn diff_touches_only_changed_fields() {
        let mut old = TagData {
            title: Some("a".into()),
            artist: Some("b".into()),
            year: Some(2021),
            ..Default::default()
        };
        old.extra.insert("Bpm".into(), "120".into());
        let mut new = old.clone();
        new.title = Some("A".into());
        new.year = None;
        new.extra.clear();

        let c = TagWriteChanges::diff(&old, &new);
        assert_eq!(c.title, Some(Some("A".into())));
        assert_eq!(c.year, Some(None));
        assert_eq!(c.artist, None);
        assert_eq!(c.extra.unwrap().get("Bpm"), Some(&None));
        assert_eq!(
            TagWriteChanges::diff(&old, &old),
            TagWriteChanges::default()
        );
    }
}
