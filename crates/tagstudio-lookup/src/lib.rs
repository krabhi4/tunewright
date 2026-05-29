pub mod applemusic;
pub mod musicbrainz;
pub mod types;

/// Parse the leading year from an ISO-ish date string (`"2021-05-30"` -> `2021`).
pub fn extract_year(date: &Option<String>) -> Option<u32> {
    date.as_ref()
        .and_then(|d| d.split('-').next())
        .and_then(|y| y.parse().ok())
}
