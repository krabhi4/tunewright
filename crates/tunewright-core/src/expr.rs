//! Expression engine for Tunewright format strings and scripting.
//!
//! Supports:
//! - `%variable%` placeholders resolved against TagData
//! - `$function(arg1, arg2, ...)` calls with nesting
//! - Literal text

use crate::types::TagData;

/// AST node
#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    Literal(String),
    Variable(String),
    FuncCall { name: String, args: Vec<Vec<Node>> },
}

/// Context for expression evaluation
pub struct ExprContext<'a> {
    pub tags: &'a TagData,
    pub filename: Option<&'a str>,
    pub index: Option<usize>,
}

impl<'a> ExprContext<'a> {
    pub fn new(tags: &'a TagData) -> Self {
        Self {
            tags,
            filename: None,
            index: None,
        }
    }

    pub fn with_filename(mut self, filename: &'a str) -> Self {
        self.filename = Some(filename);
        self
    }

    pub fn with_index(mut self, index: usize) -> Self {
        self.index = Some(index);
        self
    }
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

struct Parser {
    chars: Vec<char>,
    pos: usize,
    depth: usize,
}

impl Parser {
    fn new(input: &str) -> Self {
        Self {
            chars: input.chars().collect(),
            pos: 0,
            depth: 0,
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let c = self.chars.get(self.pos).copied();
        if c.is_some() {
            self.pos += 1;
        }
        c
    }

    /// Parse an expression until one of the stop characters is reached.
    fn parse_expr(&mut self, stop_chars: &[char]) -> Vec<Node> {
        self.depth += 1;
        if self.depth > 256 {
            self.depth -= 1;
            let mut literal = String::new();
            while let Some(c) = self.peek() {
                if stop_chars.contains(&c) {
                    break;
                }
                literal.push(c);
                self.pos += 1;
            }
            return vec![Node::Literal(literal)];
        }

        let mut nodes = Vec::new();
        let mut literal = String::new();

        while let Some(c) = self.peek() {
            if stop_chars.contains(&c) {
                break;
            }
            match c {
                '%' => {
                    if !literal.is_empty() {
                        nodes.push(Node::Literal(std::mem::take(&mut literal)));
                    }
                    self.advance(); // consume opening %
                    let var = self.read_until('%');
                    self.advance(); // consume closing %
                    if !var.is_empty() {
                        nodes.push(Node::Variable(var));
                    }
                }
                '$' => {
                    if !literal.is_empty() {
                        nodes.push(Node::Literal(std::mem::take(&mut literal)));
                    }
                    self.advance(); // consume $
                    let name = self.read_identifier();
                    if self.peek() == Some('(') {
                        self.advance(); // consume (
                        let args = self.parse_args();
                        nodes.push(Node::FuncCall { name, args });
                    } else {
                        // Not a function call — emit $ + name as literal
                        literal.push('$');
                        literal.push_str(&name);
                    }
                }
                _ => {
                    literal.push(c);
                    self.advance();
                }
            }
        }

        if !literal.is_empty() {
            nodes.push(Node::Literal(literal));
        }
        self.depth -= 1;
        nodes
    }

    fn parse_args(&mut self) -> Vec<Vec<Node>> {
        let mut args = Vec::new();

        // Empty args: $func()
        if self.peek() == Some(')') {
            self.advance();
            return args;
        }

        loop {
            let arg = self.parse_expr(&[',', ')']);
            args.push(arg);
            match self.peek() {
                Some(',') => {
                    self.advance();
                }
                Some(')') => {
                    self.advance();
                    break;
                }
                _ => break, // unexpected end of input
            }
        }
        args
    }

    fn read_until(&mut self, stop: char) -> String {
        let mut s = String::new();
        while let Some(c) = self.peek() {
            if c == stop {
                break;
            }
            s.push(c);
            self.advance();
        }
        s
    }

    fn read_identifier(&mut self) -> String {
        let mut name = String::new();
        while let Some(c) = self.peek() {
            if c.is_alphanumeric() || c == '_' {
                name.push(c);
                self.advance();
            } else {
                break;
            }
        }
        name
    }
}

/// Parse a format string into an expression AST.
pub fn parse(input: &str) -> Vec<Node> {
    Parser::new(input).parse_expr(&[])
}

// ---------------------------------------------------------------------------
// Evaluator
// ---------------------------------------------------------------------------

/// Run a parsed expression against a context, producing a string.
pub fn run(nodes: &[Node], ctx: &ExprContext) -> String {
    let mut result = String::new();
    for node in nodes {
        match node {
            Node::Literal(s) => result.push_str(s),
            Node::Variable(var) => result.push_str(&resolve_variable(var, ctx)),
            Node::FuncCall { name, args } => {
                let evaluated_args: Vec<String> = args.iter().map(|a| run(a, ctx)).collect();
                result.push_str(&call_function(name, &evaluated_args, ctx));
            }
        }
    }
    result
}

/// Parse and run in one step.
pub fn evaluate(input: &str, ctx: &ExprContext) -> String {
    run(&parse(input), ctx)
}

fn resolve_variable(var: &str, ctx: &ExprContext) -> String {
    // Standard fields (case-insensitive)
    let result = match var.to_lowercase().as_str() {
        "title" => ctx.tags.title.clone(),
        "artist" => ctx.tags.artist.clone(),
        "album" => ctx.tags.album.clone(),
        "albumartist" | "album_artist" => ctx.tags.album_artist.clone(),
        "year" => ctx.tags.year.map(|y| y.to_string()),
        "genre" => ctx.tags.genre.clone(),
        "comment" => ctx.tags.comment.clone(),
        "composer" => ctx.tags.composer.clone(),
        "track" | "track_number" => ctx.tags.track_number.map(|n| format!("{:02}", n)),
        "track_total" => ctx.tags.track_total.map(|n| n.to_string()),
        "disc" | "disc_number" => ctx.tags.disc_number.map(|n| n.to_string()),
        "disc_total" => ctx.tags.disc_total.map(|n| n.to_string()),
        "_filename" => Some(ctx.filename.unwrap_or("").to_string()),
        "_index" => ctx.index.map(|i| i.to_string()),
        _ => None,
    };
    if let Some(val) = result {
        return val;
    }

    // Extra tags: try exact case first, then case-insensitive
    if let Some(val) = ctx.tags.extra.get(var) {
        return val.clone();
    }
    let lower = var.to_lowercase();
    for (key, val) in &ctx.tags.extra {
        if key.to_lowercase() == lower {
            return val.clone();
        }
    }
    String::new()
}

// ---------------------------------------------------------------------------
// Built-in functions
// ---------------------------------------------------------------------------

fn call_function(name: &str, args: &[String], ctx: &ExprContext) -> String {
    match name.to_lowercase().as_str() {
        // String
        "caps" => fn_caps(args),
        "caps2" => fn_caps2(args),
        "lower" => fn_lower(args),
        "upper" => fn_upper(args),
        "replace" => fn_replace(args),
        "regex" => fn_regex(args),
        "left" => fn_left(args),
        "right" => fn_right(args),
        "mid" => fn_mid(args),
        "len" => fn_len(args),
        "trim" | "strip" => fn_trim(args),
        "validate" => fn_validate(args),
        "char" => fn_char(args),

        // Math
        "add" => fn_math(args, i64::saturating_add),
        "sub" => fn_math(args, i64::saturating_sub),
        "mul" => fn_math(args, i64::saturating_mul),
        "div" => fn_math(args, |a, b| {
            if b == 0 {
                0
            } else if a == i64::MIN && b == -1 {
                i64::MAX
            } else {
                a / b
            }
        }),
        "mod" => fn_math(args, |a, b| {
            if b == 0 || (a == i64::MIN && b == -1) {
                0
            } else {
                a % b
            }
        }),
        "num" => fn_num(args),

        // Logic
        "if" => fn_if(args),
        "and" => fn_and(args),
        "or" => fn_or(args),
        "not" => fn_not(args),
        "strcmp" => fn_strcmp(args),
        "greater" | "grtr" => fn_compare(args, |a, b| a > b),
        "less" => fn_compare(args, |a, b| a < b),
        "geql" => fn_compare(args, |a, b| a >= b),
        "leql" => fn_compare(args, |a, b| a <= b),
        "iflonger" => fn_iflonger(args),
        "isdigit" => fn_isdigit(args),

        // Field
        "meta" => fn_meta(args, ctx),

        _ => String::new(),
    }
}

// --- String functions ---

/// Title Case: capitalize the first letter of each word, breaking on
/// whitespace and `-`, `(`, `[`. Shared by the `$caps` function and the
/// case-conversion action.
pub(crate) fn title_case(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut cap_next = true;
    for c in s.chars() {
        if c.is_whitespace() || c == '-' || c == '(' || c == '[' {
            result.push(c);
            cap_next = true;
        } else if cap_next {
            result.extend(c.to_uppercase());
            cap_next = false;
        } else {
            result.extend(c.to_lowercase());
        }
    }
    result
}

/// Title Case: capitalize first letter of each word
fn fn_caps(args: &[String]) -> String {
    title_case(&get_arg(args, 0))
}

/// Smart Title Case: keeps small words lowercase unless first word
fn fn_caps2(args: &[String]) -> String {
    let s = get_arg(args, 0);
    let small_words = [
        "a", "an", "the", "and", "but", "or", "nor", "at", "by", "for", "in", "of", "on", "to",
        "up", "as", "is", "it",
    ];
    let words: Vec<&str> = s.split_whitespace().collect();
    let mut result = Vec::with_capacity(words.len());
    for (i, word) in words.iter().enumerate() {
        let lower = word.to_lowercase();
        if i > 0 && small_words.contains(&lower.as_str()) {
            result.push(lower);
        } else {
            result.push(capitalize_word(&lower));
        }
    }
    result.join(" ")
}

fn capitalize_word(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) => {
            let mut result = String::with_capacity(s.len());
            result.extend(c.to_uppercase());
            result.push_str(chars.as_str());
            result
        }
        None => String::new(),
    }
}

fn fn_lower(args: &[String]) -> String {
    get_arg(args, 0).to_lowercase()
}

fn fn_upper(args: &[String]) -> String {
    get_arg(args, 0).to_uppercase()
}

fn fn_replace(args: &[String]) -> String {
    if args.len() < 3 {
        return get_arg(args, 0);
    }
    if args[1].is_empty() {
        return args[0].clone();
    }
    args[0].replace(args[1].as_str(), args[2].as_str())
}

fn fn_regex(args: &[String]) -> String {
    if args.len() < 3 {
        return get_arg(args, 0);
    }
    if args[1].is_empty() {
        return args[0].clone();
    }
    match regex::Regex::new(&args[1]) {
        Ok(re) => re.replace_all(&args[0], args[2].as_str()).to_string(),
        Err(_) => args[0].clone(),
    }
}

fn fn_left(args: &[String]) -> String {
    let s = get_arg(args, 0);
    let n = parse_int(&get_arg(args, 1)) as usize;
    s.chars().take(n).collect()
}

fn fn_right(args: &[String]) -> String {
    let s = get_arg(args, 0);
    let n = parse_int(&get_arg(args, 1)) as usize;
    let chars: Vec<char> = s.chars().collect();
    if n >= chars.len() {
        return s;
    }
    chars[chars.len() - n..].iter().collect()
}

fn fn_mid(args: &[String]) -> String {
    let s = get_arg(args, 0);
    let start = parse_int(&get_arg(args, 1)) as usize;
    let len = parse_int(&get_arg(args, 2)) as usize;
    s.chars().skip(start).take(len).collect()
}

fn fn_len(args: &[String]) -> String {
    get_arg(args, 0).chars().count().to_string()
}

fn fn_trim(args: &[String]) -> String {
    get_arg(args, 0).trim().to_string()
}

fn fn_validate(args: &[String]) -> String {
    let s = get_arg(args, 0);
    let replacement = if args.len() > 1 { &args[1] } else { "_" };
    s.chars()
        .map(|c| {
            if "\\/:*?\"<>|".contains(c) {
                replacement.to_string()
            } else {
                c.to_string()
            }
        })
        .collect()
}

fn fn_char(args: &[String]) -> String {
    get_arg(args, 0)
        .parse::<u32>()
        .ok()
        .and_then(char::from_u32)
        .map(|c| c.to_string())
        .unwrap_or_default()
}

// --- Math functions ---

fn fn_math(args: &[String], op: fn(i64, i64) -> i64) -> String {
    let a = parse_int(&get_arg(args, 0));
    let b = parse_int(&get_arg(args, 1));
    op(a, b).to_string()
}

/// Zero-pad a number: $num(value, width)
fn fn_num(args: &[String]) -> String {
    let n = parse_int(&get_arg(args, 0));
    let width = parse_int(&get_arg(args, 1)).clamp(1, 1000) as usize;
    format!("{:0>width$}", n)
}

// --- Logic functions ---

/// $if(condition, then, else) — empty string and "0" are falsy.
fn fn_if(args: &[String]) -> String {
    let cond = get_arg(args, 0);
    if is_truthy(&cond) {
        get_arg(args, 1)
    } else {
        get_arg(args, 2)
    }
}

fn fn_and(args: &[String]) -> String {
    let a = is_truthy(&get_arg(args, 0));
    let b = is_truthy(&get_arg(args, 1));
    bool_to_string(a && b)
}

fn fn_or(args: &[String]) -> String {
    let a = is_truthy(&get_arg(args, 0));
    let b = is_truthy(&get_arg(args, 1));
    bool_to_string(a || b)
}

fn fn_not(args: &[String]) -> String {
    bool_to_string(!is_truthy(&get_arg(args, 0)))
}

/// $strcmp(a, b) — returns "1" if equal, empty otherwise
fn fn_strcmp(args: &[String]) -> String {
    if get_arg(args, 0) == get_arg(args, 1) {
        "1".to_string()
    } else {
        String::new()
    }
}

fn fn_compare(args: &[String], op: fn(i64, i64) -> bool) -> String {
    let a = parse_int(&get_arg(args, 0));
    let b = parse_int(&get_arg(args, 1));
    bool_to_string(op(a, b))
}

/// $iflonger(text, n, then, else)
fn fn_iflonger(args: &[String]) -> String {
    let text = get_arg(args, 0);
    let n = parse_int(&get_arg(args, 1)) as usize;
    if text.chars().count() > n {
        get_arg(args, 2)
    } else {
        get_arg(args, 3)
    }
}

fn fn_isdigit(args: &[String]) -> String {
    let s = get_arg(args, 0);
    bool_to_string(!s.is_empty() && s.chars().all(|c| c.is_ascii_digit()))
}

// --- Field functions ---

/// $meta(fieldname) — look up any tag field by name
fn fn_meta(args: &[String], ctx: &ExprContext) -> String {
    resolve_variable(&get_arg(args, 0), ctx)
}

// --- Helpers ---

fn get_arg(args: &[String], index: usize) -> String {
    args.get(index).cloned().unwrap_or_default()
}

fn parse_int(s: &str) -> i64 {
    s.trim().parse::<i64>().unwrap_or(0)
}

fn is_truthy(s: &str) -> bool {
    !s.is_empty() && s != "0"
}

fn bool_to_string(b: bool) -> String {
    if b {
        "1".to_string()
    } else {
        String::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_ctx(tags: &TagData) -> ExprContext<'_> {
        ExprContext::new(tags)
    }

    fn tags() -> TagData {
        TagData {
            title: Some("First Song".to_string()),
            artist: Some("The Band".to_string()),
            album: Some("Great Album".to_string()),
            album_artist: Some("The Band".to_string()),
            year: Some(2023),
            track_number: Some(3),
            track_total: Some(12),
            genre: Some("Rock".to_string()),
            ..Default::default()
        }
    }

    // --- Parser tests ---

    #[test]
    fn test_parse_literal() {
        assert_eq!(parse("hello"), vec![Node::Literal("hello".to_string())]);
    }

    #[test]
    fn test_parse_variable() {
        assert_eq!(
            parse("%artist%"),
            vec![Node::Variable("artist".to_string())]
        );
    }

    #[test]
    fn test_parse_mixed() {
        assert_eq!(
            parse("%artist% - %title%"),
            vec![
                Node::Variable("artist".to_string()),
                Node::Literal(" - ".to_string()),
                Node::Variable("title".to_string()),
            ]
        );
    }

    #[test]
    fn test_parse_function() {
        let nodes = parse("$upper(%artist%)");
        assert_eq!(
            nodes,
            vec![Node::FuncCall {
                name: "upper".to_string(),
                args: vec![vec![Node::Variable("artist".to_string())]],
            }]
        );
    }

    #[test]
    fn test_parse_nested_function() {
        let nodes = parse("$left($upper(%artist%),3)");
        assert_eq!(
            nodes,
            vec![Node::FuncCall {
                name: "left".to_string(),
                args: vec![
                    vec![Node::FuncCall {
                        name: "upper".to_string(),
                        args: vec![vec![Node::Variable("artist".to_string())]],
                    }],
                    vec![Node::Literal("3".to_string())],
                ],
            }]
        );
    }

    #[test]
    fn test_parse_deeply_nested_recursion_limit() {
        // Construct a string with 300 levels of nested function calls
        let mut input = String::new();
        for _ in 0..300 {
            input.push_str("$a(");
        }
        input.push_str("x");
        for _ in 0..300 {
            input.push(')');
        }

        // Parsing should succeed without stack overflow
        let nodes = parse(&input);
        assert!(!nodes.is_empty());
    }

    #[test]
    fn test_parse_dollar_not_function() {
        assert_eq!(parse("$50"), vec![Node::Literal("$50".to_string())]);
    }

    // --- Evaluation: variables ---

    #[test]
    fn test_run_variable() {
        let t = tags();
        assert_eq!(evaluate("%artist%", &make_ctx(&t)), "The Band");
    }

    #[test]
    fn test_run_mixed() {
        let t = tags();
        assert_eq!(
            evaluate("%artist% - %title%", &make_ctx(&t)),
            "The Band - First Song"
        );
    }

    #[test]
    fn test_run_track_padding() {
        let t = tags();
        assert_eq!(evaluate("%track%", &make_ctx(&t)), "03");
    }

    #[test]
    fn test_run_missing_field() {
        let t = TagData::default();
        assert_eq!(evaluate("%artist%", &make_ctx(&t)), "");
    }

    #[test]
    fn test_run_extra_field() {
        let mut t = TagData::default();
        t.extra.insert("Lyrics".to_string(), "La la la".to_string());
        assert_eq!(evaluate("%Lyrics%", &make_ctx(&t)), "La la la");
    }

    // --- Evaluation: string functions ---

    #[test]
    fn test_fn_upper() {
        let t = tags();
        assert_eq!(evaluate("$upper(%artist%)", &make_ctx(&t)), "THE BAND");
    }

    #[test]
    fn test_fn_lower() {
        let t = tags();
        assert_eq!(evaluate("$lower(%artist%)", &make_ctx(&t)), "the band");
    }

    #[test]
    fn test_fn_caps() {
        let t = TagData {
            title: Some("hello world".to_string()),
            ..Default::default()
        };
        assert_eq!(evaluate("$caps(%title%)", &make_ctx(&t)), "Hello World");
    }

    #[test]
    fn test_fn_caps2() {
        let t = TagData {
            title: Some("the lord of the rings".to_string()),
            ..Default::default()
        };
        assert_eq!(
            evaluate("$caps2(%title%)", &make_ctx(&t)),
            "The Lord of the Rings"
        );
    }

    #[test]
    fn test_fn_replace() {
        let t = tags();
        assert_eq!(
            evaluate("$replace(%artist%,Band,Group)", &make_ctx(&t)),
            "The Group"
        );
        assert_eq!(
            evaluate("$replace(%artist%,,Group)", &make_ctx(&t)),
            "The Band"
        );
    }

    #[test]
    fn test_fn_regex() {
        let t = TagData {
            title: Some("Song - Remix".to_string()),
            ..Default::default()
        };
        // Note: ) in regex patterns conflicts with function call parsing,
        // so avoid literal ) in patterns passed to $regex().
        assert_eq!(evaluate("$regex(%title%, - .*,)", &make_ctx(&t)), "Song");
        assert_eq!(
            evaluate("$regex(%title%,,Replacement)", &make_ctx(&t)),
            "Song - Remix"
        );
    }

    #[test]
    fn test_fn_left() {
        let t = tags();
        assert_eq!(evaluate("$left(%artist%,3)", &make_ctx(&t)), "The");
    }

    #[test]
    fn test_fn_right() {
        let t = tags();
        assert_eq!(evaluate("$right(%artist%,4)", &make_ctx(&t)), "Band");
    }

    #[test]
    fn test_fn_mid() {
        let t = tags();
        assert_eq!(evaluate("$mid(%artist%,4,4)", &make_ctx(&t)), "Band");
    }

    #[test]
    fn test_fn_len() {
        let t = tags();
        assert_eq!(evaluate("$len(%artist%)", &make_ctx(&t)), "8");
    }

    #[test]
    fn test_fn_trim() {
        let t = TagData {
            title: Some("  hello  ".to_string()),
            ..Default::default()
        };
        assert_eq!(evaluate("$trim(%title%)", &make_ctx(&t)), "hello");
    }

    // --- Evaluation: math functions ---

    #[test]
    fn test_fn_add() {
        let t = tags();
        assert_eq!(evaluate("$add(%year%,1)", &make_ctx(&t)), "2024");
    }

    #[test]
    fn test_fn_sub() {
        let t = tags();
        assert_eq!(evaluate("$sub(%year%,23)", &make_ctx(&t)), "2000");
    }

    #[test]
    fn test_fn_num() {
        let t = tags();
        assert_eq!(evaluate("$num(%track%,3)", &make_ctx(&t)), "003");

        // Large width should not panic and should be capped at 1000
        let large_res = evaluate("$num(%track%,100000)", &make_ctx(&t));
        assert_eq!(large_res.len(), 1000);
    }

    #[test]
    fn test_fn_div_by_zero() {
        let t = TagData::default();
        assert_eq!(evaluate("$div(10,0)", &make_ctx(&t)), "0");
    }

    #[test]
    fn test_fn_div_mod_overflow() {
        let t = TagData::default();
        // i64::MIN is -9223372036854775808
        // dividing it by -1 should saturate to i64::MAX (9223372036854775807)
        assert_eq!(
            evaluate("$div(-9223372036854775808,-1)", &make_ctx(&t)),
            "9223372036854775807"
        );
        // modulo should return 0
        assert_eq!(
            evaluate("$mod(-9223372036854775808,-1)", &make_ctx(&t)),
            "0"
        );
    }

    #[test]
    fn test_recursion_limit_boundary() {
        let t = TagData::default();
        let make_expr = |depth: usize| {
            let mut s = String::new();
            for _ in 0..depth {
                s.push_str("$upper(");
            }
            s.push_str("a");
            for _ in 0..depth {
                s.push(')');
            }
            s
        };

        // Under the limit (255): should evaluate to "A" (since $upper is applied 255 times)
        let under = make_expr(255);
        assert_eq!(evaluate(&under, &make_ctx(&t)), "A");

        // Over the limit (260): should not panic and should evaluate gracefully
        let over = make_expr(260);
        let result = evaluate(&over, &make_ctx(&t));
        assert!(!result.is_empty());
    }

    #[test]
    fn test_fn_math_safety_bounds() {
        let t = TagData::default();
        // Division by zero
        assert_eq!(evaluate("$div(5,0)", &make_ctx(&t)), "0");
        assert_eq!(evaluate("$mod(5,0)", &make_ctx(&t)), "0");

        // Addition overflow
        assert_eq!(
            evaluate("$add(9223372036854775807,1)", &make_ctx(&t)),
            "9223372036854775807"
        );
        // Subtraction underflow
        assert_eq!(
            evaluate("$sub(-9223372036854775808,1)", &make_ctx(&t)),
            "-9223372036854775808"
        );
        // Multiplication overflow
        assert_eq!(
            evaluate("$mul(4611686018427387904,2)", &make_ctx(&t)),
            "9223372036854775807"
        );
        assert_eq!(
            evaluate("$mul(-4611686018427387904,3)", &make_ctx(&t)),
            "-9223372036854775808"
        );
    }

    // --- Evaluation: logic functions ---

    #[test]
    fn test_fn_if_truthy() {
        let t = tags();
        assert_eq!(
            evaluate("$if(%artist%,has artist,no artist)", &make_ctx(&t)),
            "has artist"
        );
    }

    #[test]
    fn test_fn_if_falsy() {
        let t = TagData::default();
        assert_eq!(
            evaluate("$if(%artist%,has artist,no artist)", &make_ctx(&t)),
            "no artist"
        );
    }

    #[test]
    fn test_fn_strcmp_equal() {
        let t = tags();
        assert_eq!(
            evaluate(
                "$if($strcmp(%artist%,%albumartist%),same,different)",
                &make_ctx(&t)
            ),
            "same"
        );
    }

    #[test]
    fn test_fn_greater() {
        let t = tags();
        assert_eq!(
            evaluate("$if($greater(%year%,2000),modern,classic)", &make_ctx(&t)),
            "modern"
        );
    }

    #[test]
    fn test_fn_iflonger() {
        let t = tags();
        assert_eq!(
            evaluate("$iflonger(%artist%,5,long,short)", &make_ctx(&t)),
            "long"
        );
    }

    #[test]
    fn test_fn_and_or() {
        let t = tags();
        assert_eq!(
            evaluate("$if($and(%artist%,%album%),both,not both)", &make_ctx(&t)),
            "both"
        );
    }

    #[test]
    fn test_fn_not() {
        let t = TagData::default();
        assert_eq!(
            evaluate("$if($not(%artist%),empty,filled)", &make_ctx(&t)),
            "empty"
        );
    }

    // --- Evaluation: nested and complex ---

    #[test]
    fn test_nested_functions() {
        let t = tags();
        assert_eq!(evaluate("$left($upper(%artist%),3)", &make_ctx(&t)), "THE");
    }

    #[test]
    fn test_complex_format() {
        let t = tags();
        let result = evaluate(
            "$num(%track%,2). $caps(%artist%) - $caps(%title%) (%year%)",
            &make_ctx(&t),
        );
        assert_eq!(result, "03. The Band - First Song (2023)");
    }

    #[test]
    fn test_fn_meta() {
        let mut t = tags();
        t.extra.insert("BPM".to_string(), "120".to_string());
        assert_eq!(evaluate("$meta(BPM)", &make_ctx(&t)), "120");
        assert_eq!(evaluate("$meta(artist)", &make_ctx(&t)), "The Band");
    }

    #[test]
    fn test_empty_function_args() {
        let t = TagData::default();
        assert_eq!(evaluate("$len()", &make_ctx(&t)), "0");
    }

    #[test]
    fn test_backwards_compat() {
        // Pure %variable% expressions still work
        let t = tags();
        assert_eq!(
            evaluate("%track% - %artist% - %title%", &make_ctx(&t)),
            "03 - The Band - First Song"
        );
    }

    #[test]
    fn test_filename_context() {
        let t = tags();
        let ctx = ExprContext::new(&t).with_filename("track03.mp3");
        assert_eq!(evaluate("%_filename%", &ctx), "track03.mp3");
    }

    #[test]
    fn test_index_context() {
        let t = tags();
        let ctx = ExprContext::new(&t).with_index(5);
        assert_eq!(evaluate("%_index%", &ctx), "5");
    }
}
