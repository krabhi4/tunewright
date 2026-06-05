use serde::{Deserialize, Serialize};
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

/// Changes to write to a single file's tags
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TagWriteChanges {
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

    /// Extra/custom tag fields to write
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extra: Option<HashMap<String, String>>,
}

impl From<&TagData> for TagWriteChanges {
    /// Project a full `TagData` onto the writable subset of tag fields.
    fn from(tags: &TagData) -> Self {
        Self {
            title: tags.title.clone(),
            artist: tags.artist.clone(),
            album: tags.album.clone(),
            album_artist: tags.album_artist.clone(),
            year: tags.year,
            track_number: tags.track_number,
            track_total: tags.track_total,
            disc_number: tags.disc_number,
            disc_total: tags.disc_total,
            genre: tags.genre.clone(),
            comment: tags.comment.clone(),
            composer: tags.composer.clone(),
            extra: if tags.extra.is_empty() {
                None
            } else {
                Some(tags.extra.clone())
            },
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

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
