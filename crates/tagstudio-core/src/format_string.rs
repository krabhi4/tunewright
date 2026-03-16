use crate::types::TagData;

/// Tokens from parsing a format string
#[derive(Debug, Clone, PartialEq)]
enum Token {
    Literal(String),
    Variable(String),
}

/// Parse a format string like "%artist% - %title%" into tokens
fn tokenize(format: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut chars = format.chars().peekable();
    let mut literal = String::new();

    while let Some(c) = chars.next() {
        if c == '%' {
            // Flush literal
            if !literal.is_empty() {
                tokens.push(Token::Literal(literal.clone()));
                literal.clear();
            }
            // Read variable name until next %
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
                tokens.push(Token::Variable(var_name));
            }
        } else {
            literal.push(c);
        }
    }

    if !literal.is_empty() {
        tokens.push(Token::Literal(literal));
    }

    tokens
}

/// Evaluate a format string against tag data, producing a filename (without extension)
pub fn evaluate(format: &str, tags: &TagData) -> String {
    let tokens = tokenize(format);
    let mut result = String::new();

    for token in tokens {
        match token {
            Token::Literal(s) => result.push_str(&s),
            Token::Variable(var) => {
                let value = match var.to_lowercase().as_str() {
                    "title" => tags.title.clone().unwrap_or_default(),
                    "artist" => tags.artist.clone().unwrap_or_default(),
                    "album" => tags.album.clone().unwrap_or_default(),
                    "albumartist" | "album_artist" => {
                        tags.album_artist.clone().unwrap_or_default()
                    }
                    "year" => tags.year.map(|y| y.to_string()).unwrap_or_default(),
                    "genre" => tags.genre.clone().unwrap_or_default(),
                    "comment" => tags.comment.clone().unwrap_or_default(),
                    "composer" => tags.composer.clone().unwrap_or_default(),
                    "track" | "track_number" => tags
                        .track_number
                        .map(|n| format!("{:02}", n))
                        .unwrap_or_default(),
                    "disc" | "disc_number" => tags
                        .disc_number
                        .map(|n| n.to_string())
                        .unwrap_or_default(),
                    "_filename" => String::new(), // Will be handled by the caller
                    _ => String::new(),
                };
                result.push_str(&value);
            }
        }
    }

    sanitize_filename(&result)
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
}
