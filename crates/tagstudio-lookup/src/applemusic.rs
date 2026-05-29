use crate::extract_year;
use crate::types::{LookupSource, ReleaseDetail, ReleaseSearchResult, TrackInfo};
use reqwest::Client;
use serde::Deserialize;

const USER_AGENT: &str = "Mozilla/5.0 (compatible; TagStudio/0.4.1)";

#[derive(Debug, Deserialize)]
struct AppleSearchResponse {
    results: Vec<AppleSearchResult>,
}

#[derive(Debug, Deserialize)]
struct AppleSearchResult {
    #[serde(rename = "collectionId")]
    collection_id: u64,
    #[serde(rename = "collectionName")]
    collection_name: String,
    #[serde(rename = "artistName")]
    artist_name: String,
    #[serde(rename = "releaseDate")]
    release_date: Option<String>,
    #[serde(rename = "trackCount")]
    track_count: Option<u32>,
    #[serde(rename = "artworkUrl100")]
    artwork_url_100: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AppleLookupResponse {
    results: Vec<AppleLookupResult>,
}

#[derive(Debug, Deserialize)]
struct AppleLookupResult {
    #[serde(rename = "wrapperType")]
    wrapper_type: String,
    kind: Option<String>,
    #[serde(rename = "collectionName")]
    collection_name: Option<String>,
    #[serde(rename = "artistName")]
    artist_name: String,
    #[serde(rename = "releaseDate")]
    release_date: Option<String>,
    #[serde(rename = "primaryGenreName")]
    primary_genre_name: Option<String>,
    #[serde(rename = "trackName")]
    track_name: Option<String>,
    #[serde(rename = "trackNumber")]
    track_number: Option<u32>,
    #[serde(rename = "trackTimeMillis")]
    track_time_millis: Option<u64>,
    #[serde(rename = "artworkUrl100")]
    artwork_url_100: Option<String>,
}

fn upgrade_cover_url(url: &Option<String>) -> Option<String> {
    url.as_ref()
        .map(|u| u.replace("100x100bb.jpg", "800x800bb.jpg"))
}

/// Search iTunes/Apple Music for albums
pub async fn search_releases(
    client: &Client,
    query: &str,
) -> Result<Vec<ReleaseSearchResult>, String> {
    let encoded_query = urlencoding::encode(query);
    let url = format!(
        "https://itunes.apple.com/search?term={}&media=music&entity=album&limit=20",
        encoded_query
    );

    let resp = client
        .get(&url)
        .header("User-Agent", USER_AGENT)
        .send()
        .await
        .map_err(|e| format!("Apple Music request failed: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("Apple Music returned {}", resp.status()));
    }

    let body: AppleSearchResponse = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    let results = body
        .results
        .into_iter()
        .map(|r| ReleaseSearchResult {
            id: r.collection_id.to_string(),
            title: r.collection_name,
            artist: r.artist_name,
            year: extract_year(&r.release_date),
            track_count: r.track_count,
            source: LookupSource::AppleMusic,
            cover_art_url: upgrade_cover_url(&r.artwork_url_100),
        })
        .collect();

    Ok(results)
}

/// Get detailed album information and tracks via Apple Music Lookup API
pub async fn get_release(client: &Client, id: &str) -> Result<ReleaseDetail, String> {
    // Validate ID is numeric to prevent path injection
    if !id.chars().all(|c| c.is_ascii_digit()) {
        return Err("Invalid Apple Music ID".to_string());
    }

    let url = format!("https://itunes.apple.com/lookup?id={}&entity=song", id);

    let resp = client
        .get(&url)
        .header("User-Agent", USER_AGENT)
        .send()
        .await
        .map_err(|e| format!("Apple Music request failed: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("Apple Music returned {}", resp.status()));
    }

    let body: AppleLookupResponse = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse release detail: {}", e))?;

    // Find the collection (album wrapper) result
    let collection = body
        .results
        .iter()
        .find(|r| r.wrapper_type == "collection")
        .ok_or_else(|| "Album details not found in Apple Music response".to_string())?;

    // Extract all tracks (where wrapper_type is "track" and kind is "song")
    let mut tracks: Vec<TrackInfo> = body
        .results
        .iter()
        .filter(|r| r.wrapper_type == "track" && r.kind.as_deref() == Some("song"))
        .filter_map(|r| {
            let position = r.track_number?;
            let title = r.track_name.clone()?;
            Some(TrackInfo {
                position,
                title,
                artist: Some(r.artist_name.clone()),
                duration_secs: r.track_time_millis.map(|ms| ms as f64 / 1000.0),
            })
        })
        .collect();

    // Sort tracks by position to ensure sequential ordering
    tracks.sort_by_key(|t| t.position);

    Ok(ReleaseDetail {
        id: id.to_string(),
        title: collection.collection_name.clone().unwrap_or_default(),
        artist: collection.artist_name.clone(),
        year: extract_year(&collection.release_date),
        genre: collection.primary_genre_name.clone(),
        tracks,
        source: LookupSource::AppleMusic,
        cover_art_url: upgrade_cover_url(&collection.artwork_url_100),
    })
}
