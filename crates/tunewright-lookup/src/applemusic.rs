use crate::types::{LookupSource, ReleaseDetail, ReleaseSearchResult, TrackInfo};
use crate::{extract_year, get_json};
use reqwest::Client;
use serde::Deserialize;

const USER_AGENT: &str = "Mozilla/5.0 (compatible; Tunewright/0.5.1)";
const APPLE_HEADERS: &[(&str, &str)] = &[("User-Agent", USER_AGENT)];

#[derive(Debug, Deserialize)]
struct AppleSearchResponse {
    results: Vec<AppleSearchResult>,
}

#[derive(Debug, Deserialize)]
struct AppleSearchResult {
    #[serde(rename = "collectionId")]
    collection_id: Option<u64>,
    #[serde(rename = "collectionName")]
    collection_name: Option<String>,
    #[serde(rename = "artistName")]
    artist_name: Option<String>,
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
    wrapper_type: Option<String>,
    kind: Option<String>,
    #[serde(rename = "collectionName")]
    collection_name: Option<String>,
    #[serde(rename = "artistName")]
    artist_name: Option<String>,
    #[serde(rename = "releaseDate")]
    release_date: Option<String>,
    #[serde(rename = "primaryGenreName")]
    primary_genre_name: Option<String>,
    #[serde(rename = "trackName")]
    track_name: Option<String>,
    #[serde(rename = "trackNumber")]
    track_number: Option<u32>,
    #[serde(rename = "discNumber")]
    disc_number: Option<u32>,
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

    let body: AppleSearchResponse = get_json(client, &url, APPLE_HEADERS, "Apple Music").await?;

    let results = body
        .results
        .into_iter()
        .filter_map(|r| {
            let id = r.collection_id?.to_string();
            let title = r.collection_name?;
            Some(ReleaseSearchResult {
                id,
                title,
                artist: r
                    .artist_name
                    .unwrap_or_else(|| "Unknown Artist".to_string()),
                year: extract_year(&r.release_date),
                track_count: r.track_count,
                source: LookupSource::AppleMusic,
                cover_art_url: upgrade_cover_url(&r.artwork_url_100),
            })
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

    let body: AppleLookupResponse = get_json(client, &url, APPLE_HEADERS, "Apple Music").await?;

    // Find the collection (album wrapper) result
    let collection = body
        .results
        .iter()
        .find(|r| r.wrapper_type.as_deref() == Some("collection"))
        .ok_or_else(|| "Album details not found in Apple Music response".to_string())?;

    // Extract all tracks (where wrapper_type is "track" and kind is "song")
    let mut temp_tracks: Vec<(u32, u32, TrackInfo)> = body
        .results
        .iter()
        .filter(|r| r.wrapper_type.as_deref() == Some("track") && r.kind.as_deref() == Some("song"))
        .filter_map(|r| {
            let disc = r.disc_number.unwrap_or(1);
            let track = r.track_number?;
            let title = r.track_name.clone()?;
            Some((
                disc,
                track,
                TrackInfo {
                    position: track,
                    title,
                    artist: r.artist_name.clone(),
                    duration_secs: r.track_time_millis.map(|ms| ms as f64 / 1000.0),
                },
            ))
        })
        .collect();

    // Sort tracks by (disc, track)
    temp_tracks.sort_by_key(|t| (t.0, t.1));

    // Assign a global sequential position (1..N) to prevent duplicates and ordering issues
    let tracks: Vec<TrackInfo> = temp_tracks
        .into_iter()
        .enumerate()
        .map(|(i, (_, _, mut t))| {
            t.position = (i + 1) as u32;
            t
        })
        .collect();

    Ok(ReleaseDetail {
        id: id.to_string(),
        title: collection.collection_name.clone().unwrap_or_default(),
        artist: collection
            .artist_name
            .clone()
            .unwrap_or_else(|| "Unknown Artist".to_string()),
        year: extract_year(&collection.release_date),
        genre: collection.primary_genre_name.clone(),
        tracks,
        source: LookupSource::AppleMusic,
        cover_art_url: upgrade_cover_url(&collection.artwork_url_100),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apple_music_multi_disc_sorting() {
        let json_data = r#"{
            "results": [
                {
                    "wrapperType": "collection",
                    "collectionId": 12345,
                    "collectionName": "Greatest Hits",
                    "artistName": "Famous Artist",
                    "releaseDate": "2020-05-15T07:00:00Z",
                    "primaryGenreName": "Pop",
                    "artworkUrl100": "http://example.com/100x100bb.jpg"
                },
                {
                    "wrapperType": "track",
                    "kind": "song",
                    "artistName": "Famous Artist",
                    "trackName": "Disc 2 Track 1",
                    "trackNumber": 1,
                    "discNumber": 2,
                    "trackTimeMillis": 180000
                },
                {
                    "wrapperType": "track",
                    "kind": "song",
                    "artistName": "Famous Artist",
                    "trackName": "Disc 1 Track 2",
                    "trackNumber": 2,
                    "discNumber": 1,
                    "trackTimeMillis": 240000
                },
                {
                    "wrapperType": "track",
                    "kind": "song",
                    "artistName": "Famous Artist",
                    "trackName": "Disc 1 Track 1",
                    "trackNumber": 1,
                    "discNumber": 1,
                    "trackTimeMillis": 200000
                }
            ]
        }"#;

        let response: AppleLookupResponse = serde_json::from_str(json_data).unwrap();

        let mut temp_tracks: Vec<(u32, u32, TrackInfo)> = response
            .results
            .iter()
            .filter(|r| {
                r.wrapper_type.as_deref() == Some("track") && r.kind.as_deref() == Some("song")
            })
            .filter_map(|r| {
                let disc = r.disc_number.unwrap_or(1);
                let track = r.track_number?;
                let title = r.track_name.clone()?;
                Some((
                    disc,
                    track,
                    TrackInfo {
                        position: track,
                        title,
                        artist: r.artist_name.clone(),
                        duration_secs: r.track_time_millis.map(|ms| ms as f64 / 1000.0),
                    },
                ))
            })
            .collect();

        temp_tracks.sort_by_key(|t| (t.0, t.1));

        let tracks: Vec<TrackInfo> = temp_tracks
            .into_iter()
            .enumerate()
            .map(|(i, (_, _, mut t))| {
                t.position = (i + 1) as u32;
                t
            })
            .collect();

        assert_eq!(tracks.len(), 3);
        // Track 1: Disc 1 Track 1
        assert_eq!(tracks[0].title, "Disc 1 Track 1");
        assert_eq!(tracks[0].position, 1);
        // Track 2: Disc 1 Track 2
        assert_eq!(tracks[1].title, "Disc 1 Track 2");
        assert_eq!(tracks[1].position, 2);
        // Track 3: Disc 2 Track 1
        assert_eq!(tracks[2].title, "Disc 2 Track 1");
        assert_eq!(tracks[2].position, 3);
    }

    #[test]
    fn test_apple_music_partial_garbage_response() {
        let json_data = r#"{
            "results": [
                {
                    "wrapperType": "collection",
                    "collectionId": 12345,
                    "collectionName": "Valid Album",
                    "artistName": "Artist"
                },
                {
                    "wrapperType": null,
                    "collectionId": 67890
                },
                {
                    "wrapperType": "track",
                    "kind": "song",
                    "trackName": "Valid Track",
                    "trackNumber": 1,
                    "discNumber": 1,
                    "trackTimeMillis": 200000
                }
            ]
        }"#;

        let response: AppleLookupResponse = serde_json::from_str(json_data).unwrap();
        assert_eq!(response.results.len(), 3);

        let collection = response
            .results
            .iter()
            .find(|r| r.wrapper_type.as_deref() == Some("collection"))
            .unwrap();
        assert_eq!(collection.collection_name.as_deref(), Some("Valid Album"));

        let tracks: Vec<&AppleLookupResult> = response
            .results
            .iter()
            .filter(|r| r.wrapper_type.as_deref() == Some("track"))
            .collect();
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].track_name.as_deref(), Some("Valid Track"));
    }
}
