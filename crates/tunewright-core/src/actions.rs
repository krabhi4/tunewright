//! Action system for batch tag operations.
//!
//! Actions are typed operations that transform tag data. They can be chained
//! into saved macros (action groups) and applied to batches of files.

use crate::expr::{self, ExprContext};
use crate::types::TagData;
use serde::{Deserialize, Serialize};

/// A single action that can be applied to tag data.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    /// Convert text case: title, upper, lower, sentence
    CaseConversion { field: String, mode: CaseMode },

    /// Replace text (plain or regex)
    Replace {
        field: String,
        search: String,
        replace: String,
        #[serde(default)]
        regex: bool,
    },

    /// Set a field using a format string / expression
    FormatValue { field: String, format: String },

    /// Set a field to a literal value
    SetField { field: String, value: String },

    /// Remove a field (set to empty)
    RemoveField { field: String },

    /// Remove all fields except the listed ones
    RemoveAllExcept { fields: Vec<String> },

    /// Auto-number: set a numeric field sequentially
    AutoNumber {
        field: String,
        #[serde(default = "default_start")]
        start: u32,
        #[serde(default = "default_padding")]
        padding: u8,
    },

    /// Split a field on a separator, take one part
    SplitField {
        source: String,
        separator: String,
        /// 0-based index of the part to keep
        part: usize,
        target: String,
    },

    /// Merge multiple fields into one
    MergeFields {
        sources: Vec<String>,
        separator: String,
        target: String,
    },

    /// Trim whitespace from a field
    TrimField { field: String },
}

fn default_start() -> u32 {
    1
}
fn default_padding() -> u8 {
    2
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaseMode {
    /// Title Case
    Title,
    /// UPPER CASE
    Upper,
    /// lower case
    Lower,
    /// Sentence case (first letter of first word)
    Sentence,
}

/// Context passed to actions during batch execution.
pub struct ActionContext {
    /// 0-based index in the current batch
    pub index: usize,
    /// Original filename (without path or extension)
    pub filename: String,
}

impl Action {
    /// Apply this action to a mutable TagData, modifying it in place.
    pub fn apply(&self, tags: &mut TagData, ctx: &ActionContext) {
        match self {
            Action::CaseConversion { field, mode } => {
                let val = get_field(tags, field);
                if !val.is_empty() {
                    set_field(tags, field, &apply_case(&val, *mode));
                }
            }

            Action::Replace {
                field,
                search,
                replace,
                regex,
            } => {
                let val = get_field(tags, field);
                if val.is_empty() || search.is_empty() {
                    return;
                }
                let result = if *regex {
                    match regex::Regex::new(search) {
                        Ok(re) => re.replace_all(&val, replace.as_str()).to_string(),
                        Err(_) => val,
                    }
                } else {
                    val.replace(search.as_str(), replace.as_str())
                };
                set_field(tags, field, &result);
            }

            Action::FormatValue { field, format } => {
                let expr_ctx = ExprContext::new(tags)
                    .with_filename(&ctx.filename)
                    .with_index(ctx.index);
                let result = expr::evaluate(format, &expr_ctx);
                set_field(tags, field, &result);
            }

            Action::SetField { field, value } => {
                set_field(tags, field, value);
            }

            Action::RemoveField { field } => {
                set_field(tags, field, "");
            }

            Action::RemoveAllExcept { fields } => {
                let standard = [
                    "title",
                    "artist",
                    "album",
                    "album_artist",
                    "year",
                    "track_number",
                    "track_total",
                    "disc_number",
                    "disc_total",
                    "genre",
                    "comment",
                    "composer",
                ];
                for f in &standard {
                    if !fields.iter().any(|k| k.eq_ignore_ascii_case(f)) {
                        set_field(tags, f, "");
                    }
                }
                let extra_keys: Vec<String> = tags.extra.keys().cloned().collect();
                for key in extra_keys {
                    if !fields.iter().any(|k| k.eq_ignore_ascii_case(&key)) {
                        tags.extra.remove(&key);
                    }
                }
            }

            Action::AutoNumber {
                field,
                start,
                padding,
            } => {
                let index_u32 = u32::try_from(ctx.index).unwrap_or(u32::MAX);
                let num = start.saturating_add(index_u32);
                let formatted = format!("{:0>width$}", num, width = *padding as usize);
                set_field(tags, field, &formatted);
            }

            Action::SplitField {
                source,
                separator,
                part,
                target,
            } => {
                let val = get_field(tags, source);
                let parts: Vec<&str> = if separator.is_empty() {
                    val.split("").filter(|s| !s.is_empty()).collect()
                } else {
                    val.split(separator.as_str()).collect()
                };
                let result = parts.get(*part).unwrap_or(&"").trim().to_string();
                set_field(tags, target, &result);
            }

            Action::MergeFields {
                sources,
                separator,
                target,
            } => {
                let values: Vec<String> = sources
                    .iter()
                    .map(|f| get_field(tags, f))
                    .filter(|v| !v.is_empty())
                    .collect();
                set_field(tags, target, &values.join(separator));
            }

            Action::TrimField { field } => {
                let val = get_field(tags, field);
                set_field(tags, field, val.trim());
            }
        }
    }
}

/// A named group of actions to execute in sequence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionGroup {
    pub name: String,
    pub actions: Vec<Action>,
}

impl ActionGroup {
    /// Apply all actions in sequence to tag data.
    pub fn apply(&self, tags: &mut TagData, ctx: &ActionContext) {
        for action in &self.actions {
            action.apply(tags, ctx);
        }
    }
}

// ---------------------------------------------------------------------------
// Field access helpers
// ---------------------------------------------------------------------------

fn get_field(tags: &TagData, field: &str) -> String {
    match field.to_lowercase().as_str() {
        "title" => return tags.title.clone().unwrap_or_default(),
        "artist" => return tags.artist.clone().unwrap_or_default(),
        "album" => return tags.album.clone().unwrap_or_default(),
        "album_artist" | "albumartist" => return tags.album_artist.clone().unwrap_or_default(),
        "year" => return tags.year.map(|y| y.to_string()).unwrap_or_default(),
        "track_number" | "track" => {
            return tags.track_number.map(|n| n.to_string()).unwrap_or_default()
        }
        "track_total" => return tags.track_total.map(|n| n.to_string()).unwrap_or_default(),
        "disc_number" | "disc" => {
            return tags.disc_number.map(|n| n.to_string()).unwrap_or_default()
        }
        "disc_total" => return tags.disc_total.map(|n| n.to_string()).unwrap_or_default(),
        "genre" => return tags.genre.clone().unwrap_or_default(),
        "comment" => return tags.comment.clone().unwrap_or_default(),
        "composer" => return tags.composer.clone().unwrap_or_default(),
        _ => {}
    }
    // Extra field — use original case for lookup
    tags.extra.get(field).cloned().unwrap_or_default()
}

fn set_field(tags: &mut TagData, field: &str, value: &str) {
    let opt_str = if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    };

    match field.to_lowercase().as_str() {
        "title" => {
            tags.title = opt_str;
            return;
        }
        "artist" => {
            tags.artist = opt_str;
            return;
        }
        "album" => {
            tags.album = opt_str;
            return;
        }
        "album_artist" | "albumartist" => {
            tags.album_artist = opt_str;
            return;
        }
        "year" => {
            tags.year = value.parse().ok();
            return;
        }
        "track_number" | "track" => {
            tags.track_number = value.parse().ok();
            return;
        }
        "track_total" => {
            tags.track_total = value.parse().ok();
            return;
        }
        "disc_number" | "disc" => {
            tags.disc_number = value.parse().ok();
            return;
        }
        "disc_total" => {
            tags.disc_total = value.parse().ok();
            return;
        }
        "genre" => {
            tags.genre = opt_str;
            return;
        }
        "comment" => {
            tags.comment = opt_str;
            return;
        }
        "composer" => {
            tags.composer = opt_str;
            return;
        }
        _ => {}
    }
    // Extra field — use original case for key
    if value.is_empty() {
        tags.extra.remove(field);
    } else {
        tags.extra.insert(field.to_string(), value.to_string());
    }
}

fn apply_case(s: &str, mode: CaseMode) -> String {
    match mode {
        CaseMode::Upper => s.to_uppercase(),
        CaseMode::Lower => s.to_lowercase(),
        CaseMode::Title => expr::title_case(s),
        CaseMode::Sentence => {
            let mut chars = s.chars();
            let mut result = String::with_capacity(s.len());
            if let Some(first) = chars.next() {
                result.extend(first.to_uppercase());
            }
            for c in chars {
                result.extend(c.to_lowercase());
            }
            result
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(index: usize) -> ActionContext {
        ActionContext {
            index,
            filename: "test.mp3".to_string(),
        }
    }

    fn sample_tags() -> TagData {
        TagData {
            title: Some("hello world".to_string()),
            artist: Some("the band".to_string()),
            album: Some("great album".to_string()),
            track_number: Some(1),
            year: Some(2023),
            ..Default::default()
        }
    }

    #[test]
    fn test_case_conversion_title() {
        let mut tags = sample_tags();
        let action = Action::CaseConversion {
            field: "title".to_string(),
            mode: CaseMode::Title,
        };
        action.apply(&mut tags, &ctx(0));
        assert_eq!(tags.title.as_deref(), Some("Hello World"));
    }

    #[test]
    fn test_case_conversion_upper() {
        let mut tags = sample_tags();
        let action = Action::CaseConversion {
            field: "artist".to_string(),
            mode: CaseMode::Upper,
        };
        action.apply(&mut tags, &ctx(0));
        assert_eq!(tags.artist.as_deref(), Some("THE BAND"));
    }

    #[test]
    fn test_replace() {
        let mut tags = sample_tags();
        let action = Action::Replace {
            field: "title".to_string(),
            search: "world".to_string(),
            replace: "earth".to_string(),
            regex: false,
        };
        action.apply(&mut tags, &ctx(0));
        assert_eq!(tags.title.as_deref(), Some("hello earth"));
    }

    #[test]
    fn test_replace_regex() {
        let mut tags = sample_tags();
        let action = Action::Replace {
            field: "title".to_string(),
            search: r"\bworld\b".to_string(),
            replace: "WORLD".to_string(),
            regex: true,
        };
        action.apply(&mut tags, &ctx(0));
        assert_eq!(tags.title.as_deref(), Some("hello WORLD"));
    }

    #[test]
    fn test_format_value() {
        let mut tags = sample_tags();
        let action = Action::FormatValue {
            field: "title".to_string(),
            format: "$upper(%artist%) - %title%".to_string(),
        };
        action.apply(&mut tags, &ctx(0));
        assert_eq!(tags.title.as_deref(), Some("THE BAND - hello world"));
    }

    #[test]
    fn test_set_field() {
        let mut tags = sample_tags();
        let action = Action::SetField {
            field: "genre".to_string(),
            value: "Rock".to_string(),
        };
        action.apply(&mut tags, &ctx(0));
        assert_eq!(tags.genre.as_deref(), Some("Rock"));
    }

    #[test]
    fn test_remove_field() {
        let mut tags = sample_tags();
        let action = Action::RemoveField {
            field: "title".to_string(),
        };
        action.apply(&mut tags, &ctx(0));
        assert_eq!(tags.title, None);
    }

    #[test]
    fn test_auto_number() {
        let mut tags1 = sample_tags();
        let mut tags2 = sample_tags();
        let action = Action::AutoNumber {
            field: "track_number".to_string(),
            start: 1,
            padding: 2,
        };
        action.apply(&mut tags1, &ctx(0));
        action.apply(&mut tags2, &ctx(1));
        assert_eq!(tags1.track_number, Some(1)); // "01" parsed as 1
        assert_eq!(tags2.track_number, Some(2)); // "02" parsed as 2
    }

    #[test]
    fn test_split_field() {
        let mut tags = TagData {
            title: Some("Part A / Part B".to_string()),
            ..Default::default()
        };
        let action = Action::SplitField {
            source: "title".to_string(),
            separator: " / ".to_string(),
            part: 1,
            target: "album".to_string(),
        };
        action.apply(&mut tags, &ctx(0));
        assert_eq!(tags.album.as_deref(), Some("Part B"));
    }

    #[test]
    fn test_split_field_empty_separator() {
        let mut tags = TagData {
            title: Some("Hello".to_string()),
            ..Default::default()
        };
        let action = Action::SplitField {
            source: "title".to_string(),
            separator: "".to_string(),
            part: 1,
            target: "album".to_string(),
        };
        action.apply(&mut tags, &ctx(0));
        assert_eq!(tags.album.as_deref(), Some("e"));
    }

    #[test]
    fn test_merge_fields() {
        let mut tags = sample_tags();
        let action = Action::MergeFields {
            sources: vec!["artist".to_string(), "album".to_string()],
            separator: " - ".to_string(),
            target: "comment".to_string(),
        };
        action.apply(&mut tags, &ctx(0));
        assert_eq!(tags.comment.as_deref(), Some("the band - great album"));
    }

    #[test]
    fn test_trim_field() {
        let mut tags = TagData {
            title: Some("  spaces  ".to_string()),
            ..Default::default()
        };
        let action = Action::TrimField {
            field: "title".to_string(),
        };
        action.apply(&mut tags, &ctx(0));
        assert_eq!(tags.title.as_deref(), Some("spaces"));
    }

    #[test]
    fn test_action_group() {
        let mut tags = sample_tags();
        let group = ActionGroup {
            name: "Fix Title".to_string(),
            actions: vec![
                Action::CaseConversion {
                    field: "title".to_string(),
                    mode: CaseMode::Title,
                },
                Action::TrimField {
                    field: "title".to_string(),
                },
            ],
        };
        group.apply(&mut tags, &ctx(0));
        assert_eq!(tags.title.as_deref(), Some("Hello World"));
    }

    #[test]
    fn test_extra_field_operations() {
        let mut tags = TagData::default();
        tags.extra.insert("BPM".to_string(), "120".to_string());

        let action = Action::SetField {
            field: "BPM".to_string(),
            value: "130".to_string(),
        };
        action.apply(&mut tags, &ctx(0));
        assert_eq!(tags.extra.get("BPM").unwrap(), "130");

        let action = Action::RemoveField {
            field: "BPM".to_string(),
        };
        action.apply(&mut tags, &ctx(0));
        assert!(!tags.extra.contains_key("BPM"));
    }

    #[test]
    fn test_replace_empty_search() {
        let mut tags = sample_tags();
        let action = Action::Replace {
            field: "title".to_string(),
            search: "".to_string(),
            replace: "X".to_string(),
            regex: false,
        };
        action.apply(&mut tags, &ctx(0));
        assert_eq!(tags.title.as_deref(), Some("hello world"));
    }

    #[test]
    fn test_replace_empty_search_regex() {
        let mut tags = sample_tags();
        let action = Action::Replace {
            field: "title".to_string(),
            search: "".to_string(),
            replace: "X".to_string(),
            regex: true,
        };
        action.apply(&mut tags, &ctx(0));
        assert_eq!(tags.title.as_deref(), Some("hello world"));
    }

    #[test]
    fn test_auto_number_overflow() {
        let mut tags = sample_tags();
        let action = Action::AutoNumber {
            field: "track_number".to_string(),
            start: u32::MAX,
            padding: 2,
        };
        action.apply(&mut tags, &ctx(1));
        // u32::MAX + 1 saturates to u32::MAX rather than panicking or wrapping
        assert_eq!(tags.track_number, Some(u32::MAX));
    }
}
