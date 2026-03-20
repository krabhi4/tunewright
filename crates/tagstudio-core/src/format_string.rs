use crate::expr::{self, ExprContext};
use crate::types::TagData;

/// Evaluate a format string against tag data, producing a sanitized filename
/// (without extension). Supports both `%variable%` and `$function()` syntax.
pub fn evaluate(format: &str, tags: &TagData) -> String {
    let ctx = ExprContext::new(tags);
    let raw = expr::evaluate(format, &ctx);
    sanitize_filename(&raw)
}

/// Evaluate with filename context (for patterns that reference `%_filename%`).
pub fn evaluate_with_filename(format: &str, tags: &TagData, filename: &str) -> String {
    let ctx = ExprContext::new(tags).with_filename(filename);
    let raw = expr::evaluate(format, &ctx);
    sanitize_filename(&raw)
}

/// Remove or replace characters that are invalid in filenames
fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect::<String>()
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_format() {
        let tags = TagData {
            artist: Some("The Band".to_string()),
            title: Some("First Song".to_string()),
            track_number: Some(1),
            ..Default::default()
        };
        assert_eq!(
            evaluate("%artist% - %title%", &tags),
            "The Band - First Song"
        );
    }

    #[test]
    fn test_track_padding() {
        let tags = TagData {
            track_number: Some(3),
            artist: Some("Artist".to_string()),
            title: Some("Song".to_string()),
            ..Default::default()
        };
        assert_eq!(
            evaluate("%track% - %artist% - %title%", &tags),
            "03 - Artist - Song"
        );
    }

    #[test]
    fn test_missing_fields() {
        let tags = TagData {
            title: Some("Song".to_string()),
            ..Default::default()
        };
        assert_eq!(evaluate("%artist% - %title%", &tags), "- Song");
    }

    #[test]
    fn test_sanitize() {
        let tags = TagData {
            title: Some("Song: The Remix".to_string()),
            ..Default::default()
        };
        assert_eq!(evaluate("%title%", &tags), "Song_ The Remix");
    }

    #[test]
    fn test_format_with_functions() {
        let tags = TagData {
            artist: Some("the band".to_string()),
            title: Some("first song".to_string()),
            track_number: Some(3),
            ..Default::default()
        };
        assert_eq!(
            evaluate("$num(%track%,2) - $caps(%artist%) - $caps(%title%)", &tags),
            "03 - The Band - First Song"
        );
    }
}
