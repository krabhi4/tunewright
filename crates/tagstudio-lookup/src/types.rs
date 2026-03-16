use serde::{Deserialize, Serialize};

/// A release found via MusicBrainz or Discogs search
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseSearchResult {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub year: Option<u32>,
    pub track_count: Option<u32>,
    pub source: LookupSource,
    pub cover_art_url: Option<String>,
}

/// Detailed release info with track listing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseDetail {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub year: Option<u32>,
    pub genre: Option<String>,
    pub tracks: Vec<TrackInfo>,
    pub source: LookupSource,
    pub cover_art_url: Option<String>,
}

/// A single track from a release
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackInfo {
    pub position: u32,
    pub title: String,
    pub artist: Option<String>,
    pub duration_secs: Option<f64>,
}

/// Which online source the data comes from
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LookupSource {
    MusicBrainz,
    Discogs,
}
