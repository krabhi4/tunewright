//! Extract tag values from filenames using a pattern.
//!
//! The inverse of format_string/rename: given a pattern like
//! `%artist% - %track% - %title%` and a filename like
//! `The Band - 03 - First Song.mp3`, extract tag values.

use crate::types::{TunewrightError, TagWriteChanges};
use serde::Serialize;
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Pattern tokenizer (simple %var% only — no $function() support)
// ---------------------------------------------------------------------------

enum PatternToken {
    Literal(String),
    Variable(String),
}

fn tokenize_pattern(pattern: &str) -> Vec<PatternToken> {
    let mut tokens = Vec::new();
    let mut chars = pattern.chars().peekable();
    let mut literal = String::new();

    while let Some(c) = chars.next() {
        if c == '%' {
            if !literal.is_empty() {
                tokens.push(PatternToken::Literal(std::mem::take(&mut literal)));
            }
            let mut var_name = String::new();
            while let Some(&next) = chars.peek() {
                if next == '%' {
                    chars.next();
                    break;
                }
                var_name.push(next);
                chars.next();
            }
            if !var_name.is_empty() {
                tokens.push(PatternToken::Variable(var_name));
            }
        } else {
            literal.push(c);
        }
    }
    if !literal.is_empty() {
        tokens.push(PatternToken::Literal(literal));
    }
    tokens
}

// ---------------------------------------------------------------------------
// Pattern → Regex conversion
// ---------------------------------------------------------------------------

/// Build a regex from a pattern string. Returns the compiled regex and the
/// ordered list of variable names that correspond to capture groups.
fn pattern_to_regex(pattern: &str) -> Result<(regex::Regex, Vec<String>), TunewrightError> {
    let tokens = tokenize_pattern(pattern);
    let mut regex_str = String::from("^");
    let mut var_names = Vec::new();

    for token in &tokens {
        match token {
            PatternToken::Literal(s) => {
                regex_str.push_str(&regex::escape(s));
            }
            PatternToken::Variable(name) => {
                let idx = var_names.len();
                regex_str.push_str(&format!("(?P<g{idx}>.+?)"));
                var_names.push(name.clone());
            }
        }
    }

    regex_str.push('$');

    let re = regex::Regex::new(&regex_str)
        .map_err(|e| TunewrightError::InvalidFormatString(format!("Invalid pattern: {e}")))?;

    Ok((re, var_names))
}

// ---------------------------------------------------------------------------
// Extraction
// ---------------------------------------------------------------------------

/// Build a `name -> value` map from regex captures using the ordered var names.
fn captures_to_values(caps: &regex::Captures, var_names: &[String]) -> HashMap<String, String> {
    let mut values = HashMap::new();
    for (i, name) in var_names.iter().enumerate() {
        if let Some(m) = caps.name(&format!("g{i}")) {
            values.insert(name.clone(), m.as_str().to_string());
        }
    }
    values
}

/// Extract tag values from a filename stem using a pattern.
/// Returns `None` if the pattern doesn't match.
pub fn extract_from_filename(
    pattern: &str,
    filename_stem: &str,
) -> Result<Option<HashMap<String, String>>, TunewrightError> {
    let (re, var_names) = pattern_to_regex(pattern)?;

    let caps = match re.captures(filename_stem) {
        Some(c) => c,
        None => return Ok(None),
    };

    Ok(Some(captures_to_values(&caps, &var_names)))
}

/// Convert extracted string values to TagWriteChanges.
pub fn values_to_changes(values: &HashMap<String, String>) -> TagWriteChanges {
    let get = |key: &str| -> Option<String> { values.get(key).filter(|s| !s.is_empty()).cloned() };

    let get_u32 =
        |key: &str| -> Option<u32> { values.get(key).and_then(|s| s.trim().parse().ok()) };

    let mut extra = HashMap::new();
    for (key, val) in values {
        let lower = key.to_lowercase();
        let is_standard = matches!(
            lower.as_str(),
            "title"
                | "artist"
                | "album"
                | "albumartist"
                | "album_artist"
                | "year"
                | "track"
                | "track_number"
                | "track_total"
                | "disc"
                | "disc_number"
                | "disc_total"
                | "genre"
                | "comment"
                | "composer"
                | "_filename"
                | "ext"
        );
        if !is_standard && !val.is_empty() {
            extra.insert(key.clone(), val.clone());
        }
    }

    TagWriteChanges {
        title: get("title"),
        artist: get("artist"),
        album: get("album"),
        album_artist: get("albumartist").or_else(|| get("album_artist")),
        year: get_u32("year"),
        track_number: get_u32("track").or(get_u32("track_number")),
        track_total: get_u32("track_total"),
        disc_number: get_u32("disc").or(get_u32("disc_number")),
        disc_total: get_u32("disc_total"),
        genre: get("genre"),
        comment: get("comment"),
        composer: get("composer"),
        extra: if extra.is_empty() { None } else { Some(extra) },
    }
}

// ---------------------------------------------------------------------------
// Batch preview
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct FilenameTagPreview {
    pub id: String,
    pub filename: String,
    pub matched: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<TagWriteChanges>,
}

/// Preview tag extraction for a batch of files.
pub fn preview_extract(
    files: &[(String, String)], // (id, filename)
    pattern: &str,
) -> Result<Vec<FilenameTagPreview>, TunewrightError> {
    let (re, var_names) = pattern_to_regex(pattern)?;

    let previews = files
        .iter()
        .map(|(id, filename)| {
            let stem = std::path::Path::new(filename)
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy();

            let tags = re
                .captures(&stem)
                .map(|caps| values_to_changes(&captures_to_values(&caps, &var_names)));

            FilenameTagPreview {
                id: id.clone(),
                filename: filename.clone(),
                matched: tags.is_some(),
                tags,
            }
        })
        .collect();

    Ok(previews)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_extraction() {
        let values = extract_from_filename("%artist% - %title%", "The Band - First Song").unwrap();
        let values = values.unwrap();
        assert_eq!(values.get("artist").unwrap(), "The Band");
        assert_eq!(values.get("title").unwrap(), "First Song");
    }

    #[test]
    fn test_track_extraction() {
        let values =
            extract_from_filename("%track% - %artist% - %title%", "03 - The Band - First Song")
                .unwrap();
        let values = values.unwrap();
        assert_eq!(values.get("track").unwrap(), "03");
        assert_eq!(values.get("artist").unwrap(), "The Band");
        assert_eq!(values.get("title").unwrap(), "First Song");
    }

    #[test]
    fn test_no_match() {
        let values = extract_from_filename("%artist% - %title%", "NoSeparatorHere").unwrap();
        assert!(values.is_none());
    }

    #[test]
    fn test_values_to_changes() {
        let mut values = HashMap::new();
        values.insert("artist".to_string(), "The Band".to_string());
        values.insert("title".to_string(), "Song".to_string());
        values.insert("track".to_string(), "03".to_string());
        values.insert("year".to_string(), "2023".to_string());

        let changes = values_to_changes(&values);
        assert_eq!(changes.artist.as_deref(), Some("The Band"));
        assert_eq!(changes.title.as_deref(), Some("Song"));
        assert_eq!(changes.track_number, Some(3));
        assert_eq!(changes.year, Some(2023));
    }

    #[test]
    fn test_preview_extract() {
        let files = vec![
            ("id1".to_string(), "The Band - First Song.mp3".to_string()),
            ("id2".to_string(), "Another - Second Song.mp3".to_string()),
            ("id3".to_string(), "NoMatch.mp3".to_string()),
        ];

        let previews = preview_extract(&files, "%artist% - %title%").unwrap();
        assert_eq!(previews.len(), 3);

        assert!(previews[0].matched);
        let tags = previews[0].tags.as_ref().unwrap();
        assert_eq!(tags.artist.as_deref(), Some("The Band"));
        assert_eq!(tags.title.as_deref(), Some("First Song"));

        assert!(previews[1].matched);
        assert!(!previews[2].matched);
    }

    #[test]
    fn test_extra_fields_in_pattern() {
        let values =
            extract_from_filename("%artist% - %title% [%BPM%]", "The Band - Song [120]").unwrap();
        let values = values.unwrap();
        assert_eq!(values.get("BPM").unwrap(), "120");

        let changes = values_to_changes(&values);
        let extra = changes.extra.unwrap();
        assert_eq!(extra.get("BPM").unwrap(), "120");
    }
}
