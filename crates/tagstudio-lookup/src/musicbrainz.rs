use crate::types::{LookupSource, ReleaseDetail, ReleaseSearchResult, TrackInfo};
use reqwest::Client;
use serde::Deserialize;

const MB_BASE: &str = "https://musicbrainz.org/ws/2";
const USER_AGENT: &str = "TagStudio/0.1.0 (https://github.com/tagstudio)";

#[derive(Debug, Deserialize)]
struct MbSearchResponse {
    releases: Option<Vec<MbRelease>>,
}

#[derive(Debug, Deserialize)]
struct MbRelease {
    id: String,
    title: String,
    #[serde(rename = "artist-credit")]
    artist_credit: Option<Vec<MbArtistCredit>>,
    date: Option<String>,
    #[serde(rename = "track-count")]
    track_count: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct MbArtistCredit {
    artist: MbArtist,
}

#[derive(Debug, Deserialize)]
struct MbArtist {
    name: String,
}

#[derive(Debug, Deserialize)]
struct MbReleaseDetail {
    id: String,
    title: String,
    #[serde(rename = "artist-credit")]
    artist_credit: Option<Vec<MbArtistCredit>>,
    date: Option<String>,
    media: Option<Vec<MbMedia>>,
}

#[derive(Debug, Deserialize)]
struct MbMedia {
    tracks: Option<Vec<MbTrack>>,
}

#[derive(Debug, Deserialize)]
struct MbTrack {
    position: u32,
    title: String,
    #[serde(rename = "artist-credit")]
    artist_credit: Option<Vec<MbArtistCredit>>,
    length: Option<u64>,
}

fn extract_artist(credits: &Option<Vec<MbArtistCredit>>) -> String {
    credits
        .as_ref()
        .and_then(|c| c.first())
        .map(|c| c.artist.name.clone())
        .unwrap_or_default()
}

fn extract_year(date: &Option<String>) -> Option<u32> {
    date.as_ref()
        .and_then(|d| d.split('-').next())
        .and_then(|y| y.parse().ok())
}

/// Search MusicBrainz for releases
pub async fn search_releases(
    client: &Client,
    query: &str,
) -> Result<Vec<ReleaseSearchResult>, String> {
    let url = format!("{}/release?query={}&fmt=json&limit=20", MB_BASE, query);

    let resp = client
        .get(&url)
        .header("User-Agent", USER_AGENT)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| format!("MusicBrainz request failed: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("MusicBrainz returned {}", resp.status()));
    }

    let body: MbSearchResponse = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    let results = body
        .releases
        .unwrap_or_default()
        .into_iter()
        .map(|r| ReleaseSearchResult {
            id: r.id.clone(),
            title: r.title,
            artist: extract_artist(&r.artist_credit),
            year: extract_year(&r.date),
            track_count: r.track_count,
            source: LookupSource::MusicBrainz,
            cover_art_url: Some(format!(
                "https://coverartarchive.org/release/{}/front-250",
                r.id
            )),
        })
        .collect();

    Ok(results)
}

/// Get detailed release info with track listing
pub async fn get_release(
    client: &Client,
    mbid: &str,
) -> Result<ReleaseDetail, String> {
    let url = format!(
        "{}/release/{}?inc=recordings+artist-credits&fmt=json",
        MB_BASE, mbid
    );

    let resp = client
        .get(&url)
        .header("User-Agent", USER_AGENT)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| format!("MusicBrainz request failed: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("MusicBrainz returned {}", resp.status()));
    }

    let detail: MbReleaseDetail = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse release: {}", e))?;

    let tracks: Vec<TrackInfo> = detail
        .media
        .unwrap_or_default()
        .into_iter()
        .flat_map(|m| m.tracks.unwrap_or_default())
        .map(|t| TrackInfo {
            position: t.position,
            title: t.title,
            artist: Some(extract_artist(&t.artist_credit)),
            duration_secs: t.length.map(|ms| ms as f64 / 1000.0),
        })
        .collect();

    Ok(ReleaseDetail {
        id: detail.id.clone(),
        title: detail.title,
        artist: extract_artist(&detail.artist_credit),
        year: extract_year(&detail.date),
        genre: None,
        tracks,
        source: LookupSource::MusicBrainz,
        cover_art_url: Some(format!(
            "https://coverartarchive.org/release/{}/front-500",
            detail.id
        )),
    })
}
