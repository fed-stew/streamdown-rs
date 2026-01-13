//! Tokenizer for inline markdown content.
//!
//! This module provides tokenization of markdown inline content,
//! breaking text into tokens for formatting markers, text, and special elements.

use regex::Regex;
use std::sync::LazyLock;

/// Regex for tokenizing inline markdown content.
/// Matches formatting markers (**, *, _, ~~, `) and regular text.
static INLINE_TOKEN_RE: LazyLock<Regex> = LazyLock::new(|| {
    // Match formatting markers OR runs of non-marker text
    // Order matters: longer markers first
    Regex::new(r"(```+|~~|\*\*\*|\*\*_|_\*\*|\*\*|\*|___|__|_|`+|[^~_*`]+)").unwrap()
});

/// Regex for matching links: [text](url)
static LINK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[([^\]]+)\]\(([^\)]+)\)").unwrap());

/// Regex for matching images: ![alt](url)
static IMAGE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"!\[([^\]]*)\]\(([^\)]+)\)").unwrap());

/// Regex for matching footnotes: `[^1]` or `[^1]:`
static FOOTNOTE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[\^(\d+)\]:?").unwrap());

/// Regex for matching inline code spans: `code` or ``code``
static CODE_SPAN_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"``[^`]+``|`[^`]+`").unwrap());

/// Find byte ranges of inline code spans in a line.
fn find_code_regions(line: &str) -> Vec<(usize, usize)> {
    CODE_SPAN_RE
        .find_iter(line)
        .map(|m| (m.start(), m.end()))
        .collect()
}

/// Token types for inline markdown content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    /// Plain text content
    Text(String),

    /// Triple asterisk: ***
    TripleAsterisk,

    /// Double asterisk: **
    DoubleAsterisk,

    /// Single asterisk: *
    Asterisk,

    /// Triple underscore: ___
    TripleUnderscore,

    /// Double underscore: __
    DoubleUnderscore,

    /// Single underscore: _
    Underscore,

    /// **_ combination (bold + italic start)
    DoubleAsteriskUnderscore,

    /// _** combination (italic + bold end)
    UnderscoreDoubleAsterisk,

    /// Tilde pair: ~~
    DoubleTilde,

    /// Backticks (variable count)
    Backticks(usize),

    /// A link: [text](url)
    Link { text: String, url: String },

    /// An image: ![alt](url)
    Image { alt: String, url: String },

    /// A footnote reference: `[^1]`
    Footnote(u32),
}

impl Token {
    /// Check if this token is a formatting marker.
    pub fn is_marker(&self) -> bool {
        !matches!(
            self,
            Token::Text(_) | Token::Link { .. } | Token::Image { .. } | Token::Footnote(_)
        )
    }

    /// Get the marker string for formatting tokens.
    pub fn marker_str(&self) -> Option<&'static str> {
        match self {
            Token::TripleAsterisk => Some("***"),
            Token::DoubleAsterisk => Some("**"),
            Token::Asterisk => Some("*"),
            Token::TripleUnderscore => Some("___"),
            Token::DoubleUnderscore => Some("__"),
            Token::Underscore => Some("_"),
            Token::DoubleAsteriskUnderscore => Some("**_"),
            Token::UnderscoreDoubleAsterisk => Some("_**"),
            Token::DoubleTilde => Some("~~"),
            Token::Backticks(_) => {
                // Can't return dynamic string as static
                None
            }
            _ => None,
        }
    }
}

/// Tokenizer for inline markdown content.
#[derive(Debug, Default)]
pub struct Tokenizer {
    /// Whether to process links
    pub process_links: bool,
    /// Whether to process images
    pub process_images: bool,
}

impl Tokenizer {
    /// Create a new tokenizer with default settings.
    pub fn new() -> Self {
        Self {
            process_links: true,
            process_images: true,
        }
    }

    /// Create a tokenizer with specific settings.
    pub fn with_settings(process_links: bool, process_images: bool) -> Self {
        Self {
            process_links,
            process_images,
        }
    }

    /// Tokenize a line of markdown content.
    ///
    /// This extracts links, images, footnotes, and inline formatting markers.
    pub fn tokenize(&self, line: &str) -> Vec<Token> {
        let mut tokens = Vec::new();
        self.tokenize_with_extractions(line, &mut tokens);
        tokens
    }

    /// Tokenize inline content for formatting markers.
    pub fn tokenize_inline(&self, text: &str, tokens: &mut Vec<Token>) {
        for cap in INLINE_TOKEN_RE.find_iter(text) {
            let s = cap.as_str();
            let token = match s {
                "***" => Token::TripleAsterisk,
                "**" => Token::DoubleAsterisk,
                "*" => Token::Asterisk,
                "___" => Token::TripleUnderscore,
                "__" => Token::DoubleUnderscore,
                "_" => Token::Underscore,
                "**_" => Token::DoubleAsteriskUnderscore,
                "_**" => Token::UnderscoreDoubleAsterisk,
                "~~" => Token::DoubleTilde,
                _ if s.chars().all(|c| c == '`') => Token::Backticks(s.len()),
                _ => Token::Text(s.to_string()),
            };
            tokens.push(token);
        }
    }

    /// Tokenize with links, images, and footnotes already extracted.
    fn tokenize_with_extractions(&self, line: &str, tokens: &mut Vec<Token>) {
        tokens.clear();

        // We need to process the line while preserving extracted elements
        let mut last_end = 0;
        let mut extractions: Vec<(usize, usize, Token)> = Vec::new();

        // Find all images
        if self.process_images {
            for cap in IMAGE_RE.captures_iter(line) {
                let m = cap.get(0).unwrap();
                let alt = cap.get(1).map(|m| m.as_str()).unwrap_or("");
                let url = cap.get(2).map(|m| m.as_str()).unwrap_or("");
                extractions.push((
                    m.start(),
                    m.end(),
                    Token::Image {
                        alt: alt.to_string(),
                        url: url.to_string(),
                    },
                ));
            }
        }

        // Find all links (that aren't part of images)
        if self.process_links {
            for cap in LINK_RE.captures_iter(line) {
                let m = cap.get(0).unwrap();
                // Check if this is part of an image (preceded by !)
                if m.start() > 0 && line.as_bytes().get(m.start() - 1) == Some(&b'!') {
                    continue;
                }
                let text = cap.get(1).map(|m| m.as_str()).unwrap_or("");
                let url = cap.get(2).map(|m| m.as_str()).unwrap_or("");
                extractions.push((
                    m.start(),
                    m.end(),
                    Token::Link {
                        text: text.to_string(),
                        url: url.to_string(),
                    },
                ));
            }
        }

        // Find all footnotes
        for cap in FOOTNOTE_RE.captures_iter(line) {
            let m = cap.get(0).unwrap();
            if let Some(num_match) = cap.get(1) {
                if let Ok(num) = num_match.as_str().parse::<u32>() {
                    extractions.push((m.start(), m.end(), Token::Footnote(num)));
                }
            }
        }

        // Filter out extractions inside code spans (backtick-delimited regions)
        let code_regions = find_code_regions(line);
        extractions.retain(|(start, end, _)| {
            !code_regions
                .iter()
                .any(|(cs, ce)| *start >= *cs && *end <= *ce)
        });

        // Sort extractions by start position
        extractions.sort_by_key(|(start, _, _)| *start);

        // Remove overlapping extractions (keep first)
        let mut filtered: Vec<(usize, usize, Token)> = Vec::new();
        for ext in extractions {
            if filtered.is_empty() || ext.0 >= filtered.last().unwrap().1 {
                filtered.push(ext);
            }
        }

        // Now tokenize, inserting extracted tokens at the right places
        for (start, end, token) in filtered {
            // Tokenize text before this extraction
            if start > last_end {
                self.tokenize_inline(&line[last_end..start], tokens);
            }
            tokens.push(token);
            last_end = end;
        }

        // Tokenize remaining text
        if last_end < line.len() {
            self.tokenize_inline(&line[last_end..], tokens);
        }
    }

    // Note: These extraction methods are kept for potential future use
    // when we need to process links/images/footnotes separately.

    #[allow(dead_code)]
    fn extract_images(&self, text: &str) -> String {
        IMAGE_RE.replace_all(text, "").to_string()
    }

    #[allow(dead_code)]
    fn extract_links(&self, text: &str) -> String {
        LINK_RE.replace_all(text, "").to_string()
    }

    #[allow(dead_code)]
    fn extract_footnotes(&self, text: &str) -> String {
        FOOTNOTE_RE.replace_all(text, "").to_string()
    }
}

/// Check if a character is CJK (Chinese, Japanese, Korean).
///
/// CJK characters don't use spaces as word separators, so we need
/// special handling for them.
pub fn is_cjk(c: char) -> bool {
    matches!(c,
        '\u{4E00}'..='\u{9FFF}' |   // CJK Unified Ideographs
        '\u{3400}'..='\u{4DBF}' |   // CJK Unified Ideographs Extension A
        '\u{20000}'..='\u{2A6DF}' | // CJK Unified Ideographs Extension B
        '\u{2A700}'..='\u{2B73F}' | // CJK Unified Ideographs Extension C
        '\u{2B740}'..='\u{2B81F}' | // CJK Unified Ideographs Extension D
        '\u{F900}'..='\u{FAFF}' |   // CJK Compatibility Ideographs
        '\u{3000}'..='\u{303F}' |   // CJK Punctuation
        '\u{3040}'..='\u{309F}' |   // Hiragana
        '\u{30A0}'..='\u{30FF}' |   // Katakana
        '\u{31F0}'..='\u{31FF}' |   // Katakana Phonetic Extensions
        '\u{AC00}'..='\u{D7AF}' |   // Hangul Syllables
        '\u{1100}'..='\u{11FF}' |   // Hangul Jamo
        '\u{FF00}'..='\u{FFEF}'     // Fullwidth Forms
    )
}

/// Count CJK characters in a string.
pub fn cjk_count(s: &str) -> usize {
    s.chars().filter(|&c| is_cjk(c)).count()
}

/// Check if a token string is "not text" (is a potential marker boundary).
///
/// Returns true if the token is not alphanumeric and not a quote/backslash,
/// OR if it contains CJK characters.
pub fn not_text(token: &str) -> bool {
    if cjk_count(token) > 0 {
        return true;
    }

    !token
        .chars()
        .all(|c| c.is_alphanumeric() || c == '\\' || c == '"')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenize_plain_text() {
        let tokenizer = Tokenizer::new();
        let tokens = tokenizer.tokenize("Hello world");
        assert_eq!(tokens, vec![Token::Text("Hello world".to_string())]);
    }

    #[test]
    fn test_tokenize_bold() {
        let tokenizer = Tokenizer::new();
        let mut tokens = Vec::new();
        tokenizer.tokenize_inline("**bold**", &mut tokens);
        assert_eq!(
            tokens,
            vec![
                Token::DoubleAsterisk,
                Token::Text("bold".to_string()),
                Token::DoubleAsterisk,
            ]
        );
    }

    #[test]
    fn test_tokenize_italic() {
        let tokenizer = Tokenizer::new();
        let mut tokens = Vec::new();
        tokenizer.tokenize_inline("*italic*", &mut tokens);
        assert_eq!(
            tokens,
            vec![
                Token::Asterisk,
                Token::Text("italic".to_string()),
                Token::Asterisk,
            ]
        );
    }

    #[test]
    fn test_tokenize_triple_asterisk() {
        let tokenizer = Tokenizer::new();
        let mut tokens = Vec::new();
        tokenizer.tokenize_inline("***bold italic***", &mut tokens);
        assert_eq!(
            tokens,
            vec![
                Token::TripleAsterisk,
                Token::Text("bold italic".to_string()),
                Token::TripleAsterisk,
            ]
        );
    }

    #[test]
    fn test_tokenize_strikethrough() {
        let tokenizer = Tokenizer::new();
        let mut tokens = Vec::new();
        tokenizer.tokenize_inline("~~strike~~", &mut tokens);
        assert_eq!(
            tokens,
            vec![
                Token::DoubleTilde,
                Token::Text("strike".to_string()),
                Token::DoubleTilde,
            ]
        );
    }

    #[test]
    fn test_tokenize_backticks() {
        let tokenizer = Tokenizer::new();
        let mut tokens = Vec::new();
        tokenizer.tokenize_inline("`code`", &mut tokens);
        assert_eq!(
            tokens,
            vec![
                Token::Backticks(1),
                Token::Text("code".to_string()),
                Token::Backticks(1),
            ]
        );
    }

    #[test]
    fn test_tokenize_double_backticks() {
        let tokenizer = Tokenizer::new();
        let mut tokens = Vec::new();
        tokenizer.tokenize_inline("`` `code` ``", &mut tokens);
        // The tokenizer just splits on markers, the InlineParser handles matching
        assert_eq!(
            tokens,
            vec![
                Token::Backticks(2),
                Token::Text(" ".to_string()),
                Token::Backticks(1),
                Token::Text("code".to_string()),
                Token::Backticks(1),
                Token::Text(" ".to_string()),
                Token::Backticks(2),
            ]
        );
    }

    #[test]
    fn test_tokenize_link() {
        let tokenizer = Tokenizer::new();
        let tokens = tokenizer.tokenize("Check [this](http://example.com) out");

        // Should contain a Link token
        assert!(tokens.iter().any(|t| matches!(t, Token::Link { .. })));
    }

    #[test]
    fn test_tokenize_image() {
        let tokenizer = Tokenizer::new();
        let tokens = tokenizer.tokenize("See ![alt](http://img.png) here");

        // Should contain an Image token
        assert!(tokens.iter().any(|t| matches!(t, Token::Image { .. })));
    }

    #[test]
    fn test_tokenize_footnote() {
        let tokenizer = Tokenizer::new();
        let tokens = tokenizer.tokenize("Some text[^1] here");

        // Should contain a Footnote token
        assert!(tokens.iter().any(|t| matches!(t, Token::Footnote(1))));
    }

    #[test]
    fn test_is_cjk() {
        assert!(is_cjk('中'));
        assert!(is_cjk('日'));
        assert!(is_cjk('한'));
        assert!(is_cjk('あ'));
        assert!(!is_cjk('A'));
        assert!(!is_cjk('1'));
    }

    #[test]
    fn test_cjk_count() {
        assert_eq!(cjk_count("Hello"), 0);
        assert_eq!(cjk_count("中文"), 2);
        assert_eq!(cjk_count("Hello世界"), 2);
    }

    #[test]
    fn test_not_text() {
        assert!(!not_text("hello"));
        assert!(!not_text("Hello123"));
        assert!(not_text("**"));
        assert!(not_text("*"));
        assert!(not_text("中文")); // CJK
    }

    #[test]
    fn test_link_inside_code_not_extracted() {
        let tokenizer = Tokenizer::new();
        let tokens = tokenizer.tokenize("`[text](url)`");
        // Link inside backticks should NOT be extracted
        assert!(!tokens.iter().any(|t| matches!(t, Token::Link { .. })));
    }
}
