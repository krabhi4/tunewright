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
    name: Option<String>,
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
#[allow(dead_code)]
struct MbTrack {
    position: u32,
    title: Option<String>,
    #[serde(rename = "artist-credit")]
    artist_credit: Option<Vec<MbArtistCredit>>,
    length: Option<u64>,
}

fn extract_artist(credits: &Option<Vec<MbArtistCredit>>) -> String {
    credits
        .as_ref()
        .and_then(|c| c.first())
        .and_then(|c| c.artist.name.clone())
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
        .enumerate()
        .map(|(i, t)| TrackInfo {
            position: (i + 1) as u32,
            title: t.title.unwrap_or_else(|| "Unknown Track".to_string()),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_musicbrainz_multi_disc_and_missing_artist() {
        let json_data = r#"{
            "id": "release-id-123",
            "title": "Double Album",
            "artist-credit": [
                {
                    "artist": {
                        "name": "Cool Artist"
                    }
                }
            ],
            "date": "2024-03-20",
            "media": [
                {
                    "tracks": [
                        {
                            "position": 1,
                            "title": "Disc 1 Song 1",
                            "artist-credit": [
                                {
                                    "artist": {
                                        "name": null
                                    }
                                }
                            ]
                        },
                        {
                            "position": 2,
                            "title": null,
                            "artist-credit": []
                        }
                    ]
                },
                {
                    "tracks": [
                        {
                            "position": 1,
                            "title": "Disc 2 Song 1"
                        }
                    ]
                }
            ]
        }"#;

        let detail: MbReleaseDetail = serde_json::from_str(json_data).unwrap();
        assert_eq!(detail.title, "Double Album");

        let tracks: Vec<TrackInfo> = detail
            .media
            .unwrap_or_default()
            .into_iter()
            .flat_map(|m| m.tracks.unwrap_or_default())
            .enumerate()
            .map(|(i, t)| TrackInfo {
                position: (i + 1) as u32,
                title: t.title.unwrap_or_else(|| "Unknown Track".to_string()),
                artist: Some(extract_artist(&t.artist_credit)),
                duration_secs: t.length.map(|ms| ms as f64 / 1000.0),
            })
            .collect();

        assert_eq!(tracks.len(), 3);

        // Track 1
        assert_eq!(tracks[0].title, "Disc 1 Song 1");
        assert_eq!(tracks[0].position, 1);
        assert_eq!(tracks[0].artist, Some("".to_string())); // null name falls back to empty

        // Track 2
        assert_eq!(tracks[1].title, "Unknown Track"); // null title falls back
        assert_eq!(tracks[1].position, 2);

        // Track 3
        assert_eq!(tracks[2].title, "Disc 2 Song 1");
        assert_eq!(tracks[2].position, 3);
    }
}
