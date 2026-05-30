use crate::types::{LookupSource, ReleaseDetail, ReleaseSearchResult, TrackInfo};
use crate::{extract_year, get_json};
use reqwest::Client;
use serde::Deserialize;
use std::collections::HashMap;

const MB_BASE: &str = "https://musicbrainz.org/ws/2";
const USER_AGENT: &str = "Tunewright/0.5.1 (https://github.com/tunewright)";
const MB_HEADERS: &[(&str, &str)] = &[("User-Agent", USER_AGENT), ("Accept", "application/json")];

// CoverArtArchive JSON API types
#[derive(Debug, Deserialize)]
struct CaaResponse {
    images: Vec<CaaImage>,
}

#[derive(Debug, Deserialize)]
struct CaaImage {
    front: bool,
    image: String,
    thumbnails: HashMap<String, String>,
}

/// Fetch the front cover art URL from CoverArtArchive JSON API
async fn fetch_cover_art_url(client: &Client, mbid: &str) -> Option<String> {
    let url = format!("https://coverartarchive.org/release/{}", mbid);
    let resp = client
        .get(&url)
        .header("User-Agent", USER_AGENT)
        .header("Accept", "application/json")
        .send()
        .await
        .ok()?;

    if !resp.status().is_success() {
        return None;
    }

    let caa: CaaResponse = resp.json().await.ok()?;
    let front = caa.images.iter().find(|img| img.front)?;

    // Prefer 500px thumbnail, fall back to 250, then full image
    let raw_url = front
        .thumbnails
        .get("500")
        .or_else(|| front.thumbnails.get("large"))
        .or_else(|| front.thumbnails.get("250"))
        .or_else(|| front.thumbnails.get("small"))
        .cloned()
        .or_else(|| Some(front.image.clone()))?;

    // Upgrade http to https to avoid browser Mixed Content blocking
    if raw_url.starts_with("http://") {
        Some(raw_url.replacen("http://", "https://", 1))
    } else {
        Some(raw_url)
    }
}

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

/// Search MusicBrainz for releases
pub async fn search_releases(
    client: &Client,
    query: &str,
) -> Result<Vec<ReleaseSearchResult>, String> {
    let encoded_query = urlencoding::encode(query);
    let url = format!(
        "{}/release?query={}&fmt=json&limit=20",
        MB_BASE, encoded_query
    );

    let body: MbSearchResponse = get_json(client, &url, MB_HEADERS, "MusicBrainz").await?;

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
pub async fn get_release(client: &Client, mbid: &str) -> Result<ReleaseDetail, String> {
    // Validate MBID is a valid UUID (hex + dashes) to prevent path injection
    if !mbid.chars().all(|c| c.is_ascii_hexdigit() || c == '-') || mbid.len() != 36 {
        return Err("Invalid MusicBrainz ID".to_string());
    }

    let url = format!(
        "{}/release/{}?inc=recordings+artist-credits&fmt=json",
        MB_BASE, mbid
    );

    // The release body (MusicBrainz) and the cover art URL (CoverArtArchive, a
    // separate host) are independent, so fetch them concurrently to save a round-trip.
    let (detail, cover_art_url) = tokio::join!(
        get_json::<MbReleaseDetail>(client, &url, MB_HEADERS, "MusicBrainz"),
        fetch_cover_art_url(client, mbid)
    );
    let detail = detail?;

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
        cover_art_url,
    })
}
