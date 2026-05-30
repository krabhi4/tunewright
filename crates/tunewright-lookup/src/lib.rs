pub mod applemusic;
pub mod musicbrainz;
pub mod types;

use reqwest::Client;
use serde::de::DeserializeOwned;

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

    let resp = req
        .send()
        .await
        .map_err(|e| format!("{service} request failed: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("{service} returned {}", resp.status()));
    }

    resp.json::<T>()
        .await
        .map_err(|e| format!("Failed to parse {service} response: {e}"))
}
