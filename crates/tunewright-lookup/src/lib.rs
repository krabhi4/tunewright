pub mod applemusic;
pub mod musicbrainz;
pub mod types;

use reqwest::Client;
use serde::de::DeserializeOwned;

const MAX_BODY_BYTES: u64 = 8 * 1024 * 1024;

/// Parse the leading year from an ISO-ish date string (`"2021-05-30"` -> `2021`).
pub fn extract_year(date: &Option<String>) -> Option<u32> {
    date.as_ref()
        .and_then(|d| d.split('-').next())
        .and_then(|y| y.parse().ok())
}

/// GET `url` with the given headers and deserialize the JSON body. Error
/// messages are prefixed with `service` (e.g. `"MusicBrainz"`) for context.
pub(crate) async fn get_json<T: DeserializeOwned>(
    client: &Client,
    url: &str,
    headers: &[(&str, &str)],
    service: &str,
) -> Result<T, String> {
    let mut req = client.get(url);
    for (name, value) in headers {
        req = req.header(*name, *value);
    }

    let mut resp = req
        .send()
        .await
        .map_err(|e| format!("{service} request failed: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("{service} returned {}", resp.status()));
    }

    if resp
        .content_length()
        .is_some_and(|len| len > MAX_BODY_BYTES)
    {
        return Err(format!("{service} response too large"));
    }

    let mut body = Vec::new();
    while let Some(chunk) = resp
        .chunk()
        .await
        .map_err(|e| format!("{service} request failed: {e}"))?
    {
        if body.len() as u64 + chunk.len() as u64 > MAX_BODY_BYTES {
            return Err(format!("{service} response too large"));
        }
        body.extend_from_slice(&chunk);
    }

    serde_json::from_slice(&body).map_err(|e| format!("Failed to parse {service} response: {e}"))
}
