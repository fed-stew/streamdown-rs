//! Inline markdown parser.
//!
//! This module handles parsing of inline markdown formatting including
//! bold, italic, underline, strikethrough, inline code, links, images,
//! and footnotes.

use crate::tokenizer::{Token, Tokenizer};
use streamdown_ansi::codes::digit_to_superscript;

/// Result of parsing inline content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InlineElement {
    /// Plain text
    Text(String),
    /// Bold text
    Bold(String),
    /// Italic text
    Italic(String),
    /// Bold and italic text
    BoldItalic(String),
    /// Underlined text
    Underline(String),
    /// Strikethrough text
    Strikeout(String),
    /// Inline code
    Code(String),
    /// A link
    Link { text: String, url: String },
    /// An image
    Image { alt: String, url: String },
    /// Footnote reference (as superscript)
    Footnote(String),
}

/// State for tracking active formatting.
#[derive(Debug, Clone, Default)]
struct FormatState {
    /// Bold is active
    bold: bool,
    /// Italic is active
    italic: bool,
    /// Underline is active
    underline: bool,
    /// Strikeout is active
    strikeout: bool,
    /// In inline code (with backtick count)
    code_backticks: Option<usize>,
    /// Code content buffer
    code_buffer: String,
}

impl FormatState {
    fn new() -> Self {
        Self::default()
    }

    #[allow(dead_code)]
    fn any_active(&self) -> bool {
        self.bold || self.italic || self.underline || self.strikeout || self.code_backticks.is_some()
    }

    fn reset(&mut self) {
        self.bold = false;
        self.italic = false;
        self.underline = false;
        self.strikeout = false;
        self.code_backticks = None;
        self.code_buffer.clear();
    }
}

/// Inline markdown parser.
///
/// Parses inline formatting and returns structured elements.
#[derive(Debug)]
pub struct InlineParser {
    tokenizer: Tokenizer,
    state: FormatState,
    /// Whether to process links
    pub process_links: bool,
    /// Whether to process images  
    pub process_images: bool,
}

impl Default for InlineParser {
    fn default() -> Self {
        Self::new()
    }
}

impl InlineParser {
    /// Create a new inline parser.
    pub fn new() -> Self {
        Self {
            tokenizer: Tokenizer::new(),
            state: FormatState::new(),
            process_links: true,
            process_images: true,
        }
    }

    /// Create parser with specific settings.
    pub fn with_settings(process_links: bool, process_images: bool) -> Self {
        Self {
            tokenizer: Tokenizer::with_settings(process_links, process_images),
            state: FormatState::new(),
            process_links,
            process_images,
        }
    }

    /// Parse a line of markdown and return inline elements.
    ///
    /// This is the main entry point for inline parsing.
    pub fn parse(&mut self, line: &str) -> Vec<InlineElement> {
        let tokens = self.tokenizer.tokenize(line);
        self.parse_tokens(&tokens)
    }

    /// Parse a sequence of tokens into inline elements.
    fn parse_tokens(&mut self, tokens: &[Token]) -> Vec<InlineElement> {
        let mut elements = Vec::new();
        let mut buffer = String::new();
        let mut i = 0;

        while i < tokens.len() {
            let token = &tokens[i];

            // If we're in code mode, handle specially
            if let Some(expected_backticks) = self.state.code_backticks {
                match token {
                    Token::Backticks(n) if *n == expected_backticks => {
                        // End of inline code
                        let code = std::mem::take(&mut self.state.code_buffer);
                        // Trim single leading/trailing space (Markdown spec)
                        let code = code.strip_prefix(' ').unwrap_or(&code);
                        let code = code.strip_suffix(' ').unwrap_or(code);
                        elements.push(InlineElement::Code(code.to_string()));
                        self.state.code_backticks = None;
                    }
                    _ => {
                        // Add to code buffer
                        match token {
                            Token::Text(s) => self.state.code_buffer.push_str(s),
                            Token::Backticks(n) => {
                                self.state.code_buffer.push_str(&"`".repeat(*n));
                            }
                            _ => {
                                if let Some(marker) = token.marker_str() {
                                    self.state.code_buffer.push_str(marker);
                                }
                            }
                        }
                    }
                }
                i += 1;
                continue;
            }

            match token {
                Token::Text(s) => {
                    buffer.push_str(s);
                }

                Token::Backticks(n) => {
                    // Flush buffer
                    if !buffer.is_empty() {
                        self.emit_formatted(&mut elements, std::mem::take(&mut buffer));
                    }
                    // Start inline code
                    self.state.code_backticks = Some(*n);
                }

                Token::TripleAsterisk => {
                    // Flush buffer first
                    if !buffer.is_empty() {
                        self.emit_formatted(&mut elements, std::mem::take(&mut buffer));
                    }

                    if self.state.bold && self.state.italic {
                        // End both
                        self.state.bold = false;
                        self.state.italic = false;
                    } else if !self.state.bold && !self.state.italic {
                        // Start both
                        self.state.bold = true;
                        self.state.italic = true;
                    } else {
                        // Mixed state - just emit as text
                        buffer.push_str("***");
                    }
                }

                Token::DoubleAsterisk => {
                    if !buffer.is_empty() {
                        self.emit_formatted(&mut elements, std::mem::take(&mut buffer));
                    }
                    self.state.bold = !self.state.bold;
                }

                Token::Asterisk => {
                    if !buffer.is_empty() {
                        self.emit_formatted(&mut elements, std::mem::take(&mut buffer));
                    }
                    self.state.italic = !self.state.italic;
                }

                Token::DoubleAsteriskUnderscore => {
                    // **_ = start bold + start italic
                    if !buffer.is_empty() {
                        self.emit_formatted(&mut elements, std::mem::take(&mut buffer));
                    }
                    if !self.state.bold {
                        self.state.bold = true;
                    }
                    self.state.italic = !self.state.italic;
                }

                Token::UnderscoreDoubleAsterisk => {
                    // _** = end italic + end bold
                    if !buffer.is_empty() {
                        self.emit_formatted(&mut elements, std::mem::take(&mut buffer));
                    }
                    self.state.italic = false;
                    self.state.bold = false;
                }

                Token::TripleUnderscore => {
                    if !buffer.is_empty() {
                        self.emit_formatted(&mut elements, std::mem::take(&mut buffer));
                    }

                    if self.state.underline && self.state.italic {
                        self.state.underline = false;
                        self.state.italic = false;
                    } else if !self.state.underline && !self.state.italic {
                        self.state.underline = true;
                        self.state.italic = true;
                    } else {
                        buffer.push_str("___");
                    }
                }

                Token::DoubleUnderscore => {
                    if !buffer.is_empty() {
                        self.emit_formatted(&mut elements, std::mem::take(&mut buffer));
                    }
                    self.state.underline = !self.state.underline;
                }

                Token::Underscore => {
                    // Check context - underscore in middle of word shouldn't trigger italic.
                    // We check the ADJACENT character, not the entire token, because tokens
                    // may contain spaces (e.g., "use sem" before "_search tool").
                    let prev_char_is_alnum = i > 0
                        && matches!(&tokens[i - 1], Token::Text(s) if s.chars().last().map(|c| c.is_alphanumeric()).unwrap_or(false));
                    let next_char_is_alnum = i + 1 < tokens.len()
                        && matches!(&tokens[i + 1], Token::Text(s) if s.chars().next().map(|c| c.is_alphanumeric()).unwrap_or(false));

                    if prev_char_is_alnum && next_char_is_alnum {
                        // Underscore in middle of word - treat as text
                        buffer.push('_');
                    } else {
                        if !buffer.is_empty() {
                            self.emit_formatted(&mut elements, std::mem::take(&mut buffer));
                        }
                        self.state.italic = !self.state.italic;
                    }
                }

                Token::DoubleTilde => {
                    if !buffer.is_empty() {
                        self.emit_formatted(&mut elements, std::mem::take(&mut buffer));
                    }
                    self.state.strikeout = !self.state.strikeout;
                }

                Token::Link { text, url } => {
                    if !buffer.is_empty() {
                        self.emit_formatted(&mut elements, std::mem::take(&mut buffer));
                    }
                    elements.push(InlineElement::Link {
                        text: text.clone(),
                        url: url.clone(),
                    });
                }

                Token::Image { alt, url } => {
                    if !buffer.is_empty() {
                        self.emit_formatted(&mut elements, std::mem::take(&mut buffer));
                    }
                    elements.push(InlineElement::Image {
                        alt: alt.clone(),
                        url: url.clone(),
                    });
                }

                Token::Footnote(num) => {
                    if !buffer.is_empty() {
                        self.emit_formatted(&mut elements, std::mem::take(&mut buffer));
                    }
                    // Convert number to superscript
                    let superscript = number_to_superscript(*num);
                    elements.push(InlineElement::Footnote(superscript));
                }
            }

            i += 1;
        }

        // Flush remaining buffer
        if !buffer.is_empty() {
            self.emit_formatted(&mut elements, buffer);
        }

        // Flush any unclosed code block
        if self.state.code_backticks.is_some() {
            let code = std::mem::take(&mut self.state.code_buffer);
            if !code.is_empty() {
                elements.push(InlineElement::Code(code));
            }
            self.state.code_backticks = None;
        }

        // Reset state for next line
        self.state.reset();

        elements
    }

    /// Emit formatted text based on current state.
    fn emit_formatted(&self, elements: &mut Vec<InlineElement>, text: String) {
        if text.is_empty() {
            return;
        }

        if self.state.bold && self.state.italic {
            elements.push(InlineElement::BoldItalic(text));
        } else if self.state.bold {
            elements.push(InlineElement::Bold(text));
        } else if self.state.italic {
            elements.push(InlineElement::Italic(text));
        } else if self.state.underline {
            elements.push(InlineElement::Underline(text));
        } else if self.state.strikeout {
            elements.push(InlineElement::Strikeout(text));
        } else {
            elements.push(InlineElement::Text(text));
        }
    }

    /// Reset the parser state.
    pub fn reset(&mut self) {
        self.state.reset();
    }
}

/// Convert a number to superscript string.
fn number_to_superscript(num: u32) -> String {
    num.to_string()
        .chars()
        .map(|c| {
            let digit = c.to_digit(10).unwrap_or(0) as u8;
            digit_to_superscript(digit)
        })
        .collect()
}

/// Format a line with inline markdown.
///
/// This is a convenience function that parses a line and returns
/// the formatted result as ANSI-styled text.
pub fn format_line(line: &str, process_links: bool, process_images: bool) -> String {
    use streamdown_ansi::codes::*;
    use streamdown_ansi::style::*;

    let mut parser = InlineParser::with_settings(process_links, process_images);
    let elements = parser.parse(line);

    let mut result = String::new();

    for element in elements {
        match element {
            InlineElement::Text(s) => result.push_str(&s),
            InlineElement::Bold(s) => {
                result.push_str(BOLD.0);
                result.push_str(&s);
                result.push_str(BOLD.1);
            }
            InlineElement::Italic(s) => {
                result.push_str(ITALIC.0);
                result.push_str(&s);
                result.push_str(ITALIC.1);
            }
            InlineElement::BoldItalic(s) => {
                result.push_str(BOLD.0);
                result.push_str(ITALIC.0);
                result.push_str(&s);
                result.push_str(ITALIC.1);
                result.push_str(BOLD.1);
            }
            InlineElement::Underline(s) => {
                result.push_str(UNDERLINE.0);
                result.push_str(&s);
                result.push_str(UNDERLINE.1);
            }
            InlineElement::Strikeout(s) => {
                result.push_str(STRIKEOUT.0);
                result.push_str(&s);
                result.push_str(STRIKEOUT.1);
            }
            InlineElement::Code(s) => {
                result.push_str(DIM_ON);
                result.push_str(&s);
                result.push_str(DIM_OFF);
            }
            InlineElement::Link { text, url } => {
                result.push_str(LINK.0);
                result.push_str(&url);
                result.push_str("\x1b");
                result.push_str(UNDERLINE.0);
                result.push_str(&text);
                result.push_str(UNDERLINE.1);
                result.push_str(LINK.1);
            }
            InlineElement::Image { alt, url: _ } => {
                result.push_str(DIM_ON);
                result.push_str("[\u{1F5BC} ");
                result.push_str(&alt);
                result.push_str("]");
                result.push_str(DIM_OFF);
            }
            InlineElement::Footnote(s) => {
                result.push_str(&s);
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_plain_text() {
        let mut parser = InlineParser::new();
        let elements = parser.parse("Hello world");
        assert_eq!(elements, vec![InlineElement::Text("Hello world".to_string())]);
    }

    #[test]
    fn test_parse_bold() {
        let mut parser = InlineParser::new();
        let elements = parser.parse("Hello **bold** world");
        assert_eq!(
            elements,
            vec![
                InlineElement::Text("Hello ".to_string()),
                InlineElement::Bold("bold".to_string()),
                InlineElement::Text(" world".to_string()),
            ]
        );
    }

    #[test]
    fn test_parse_italic() {
        let mut parser = InlineParser::new();
        let elements = parser.parse("Hello *italic* world");
        assert_eq!(
            elements,
            vec![
                InlineElement::Text("Hello ".to_string()),
                InlineElement::Italic("italic".to_string()),
                InlineElement::Text(" world".to_string()),
            ]
        );
    }

    #[test]
    fn test_parse_bold_italic() {
        let mut parser = InlineParser::new();
        let elements = parser.parse("Hello ***bold italic*** world");
        assert_eq!(
            elements,
            vec![
                InlineElement::Text("Hello ".to_string()),
                InlineElement::BoldItalic("bold italic".to_string()),
                InlineElement::Text(" world".to_string()),
            ]
        );
    }

    #[test]
    fn test_parse_strikethrough() {
        let mut parser = InlineParser::new();
        let elements = parser.parse("Hello ~~strike~~ world");
        assert_eq!(
            elements,
            vec![
                InlineElement::Text("Hello ".to_string()),
                InlineElement::Strikeout("strike".to_string()),
                InlineElement::Text(" world".to_string()),
            ]
        );
    }

    #[test]
    fn test_parse_inline_code() {
        let mut parser = InlineParser::new();
        let elements = parser.parse("Use `code` here");
        assert_eq!(
            elements,
            vec![
                InlineElement::Text("Use ".to_string()),
                InlineElement::Code("code".to_string()),
                InlineElement::Text(" here".to_string()),
            ]
        );
    }

    #[test]
    fn test_parse_double_backtick_code() {
        let mut parser = InlineParser::new();
        let elements = parser.parse("Use `` `backticks` `` here");
        assert_eq!(
            elements,
            vec![
                InlineElement::Text("Use ".to_string()),
                InlineElement::Code("`backticks`".to_string()),
                InlineElement::Text(" here".to_string()),
            ]
        );
    }

    #[test]
    fn test_parse_link() {
        let mut parser = InlineParser::new();
        let elements = parser.parse("Check [this](http://example.com) out");

        assert!(elements.iter().any(|e| matches!(
            e,
            InlineElement::Link { text, url }
            if text == "this" && url == "http://example.com"
        )));
    }

    #[test]
    fn test_parse_image() {
        let mut parser = InlineParser::new();
        let elements = parser.parse("See ![alt text](http://img.png) here");

        assert!(elements.iter().any(|e| matches!(
            e,
            InlineElement::Image { alt, url }
            if alt == "alt text" && url == "http://img.png"
        )));
    }

    #[test]
    fn test_parse_footnote() {
        let mut parser = InlineParser::new();
        let elements = parser.parse("Some text[^1] here");

        assert!(elements
            .iter()
            .any(|e| matches!(e, InlineElement::Footnote(s) if s == "¹")));
    }

    #[test]
    fn test_parse_footnote_multi_digit() {
        let mut parser = InlineParser::new();
        let elements = parser.parse("Reference[^42]");

        assert!(elements
            .iter()
            .any(|e| matches!(e, InlineElement::Footnote(s) if s == "⁴²")));
    }

    #[test]
    fn test_underscore_in_word() {
        let mut parser = InlineParser::new();
        let elements = parser.parse("some_variable_name");
        // Underscores in middle of word should not trigger formatting
        assert_eq!(
            elements,
            vec![InlineElement::Text("some_variable_name".to_string())]
        );
    }

    #[test]
    fn test_underscore_in_word_with_surrounding_text() {
        // This is the key test case - underscore in word with spaces around
        // Previously this would incorrectly parse "_search tool" as italic
        let mut parser = InlineParser::new();
        let elements = parser.parse("use sem_search tool");
        assert_eq!(
            elements,
            vec![InlineElement::Text("use sem_search tool".to_string())]
        );
    }

    #[test]
    fn test_underscore_at_start_of_text() {
        let mut parser = InlineParser::new();
        let elements = parser.parse("sem_search");
        assert_eq!(
            elements,
            vec![InlineElement::Text("sem_search".to_string())]
        );
    }

    #[test]
    fn test_underscore_at_end_of_text() {
        let mut parser = InlineParser::new();
        let elements = parser.parse("sem_search is useful");
        assert_eq!(
            elements,
            vec![InlineElement::Text("sem_search is useful".to_string())]
        );
    }

    #[test]
    fn test_multiple_underscores_in_text() {
        let mut parser = InlineParser::new();
        let elements = parser.parse("use my_var_name here");
        assert_eq!(
            elements,
            vec![InlineElement::Text("use my_var_name here".to_string())]
        );
    }

    #[test]
    fn test_underscore_italic_still_works() {
        // Real italic with underscores should still work
        let mut parser = InlineParser::new();
        let elements = parser.parse("this is _italic_ text");
        assert_eq!(
            elements,
            vec![
                InlineElement::Text("this is ".to_string()),
                InlineElement::Italic("italic".to_string()),
                InlineElement::Text(" text".to_string()),
            ]
        );
    }

    #[test]
    fn test_underscore_italic_at_boundaries() {
        // Italic at word boundaries (space before underscore)
        let mut parser = InlineParser::new();
        let elements = parser.parse("word _italic_");
        assert_eq!(
            elements,
            vec![
                InlineElement::Text("word ".to_string()),
                InlineElement::Italic("italic".to_string()),
            ]
        );
    }

    #[test]
    fn test_mixed_underscore_scenarios() {
        // Mix of variable names and real italic
        let mut parser = InlineParser::new();
        let elements = parser.parse("use my_func for _emphasis_");
        assert_eq!(
            elements,
            vec![
                InlineElement::Text("use my_func for ".to_string()),
                InlineElement::Italic("emphasis".to_string()),
            ]
        );
    }

    #[test]
    fn test_format_line() {
        let result = format_line("Hello **bold** world", true, true);
        assert!(result.contains("bold"));
        assert!(result.contains("\x1b[1m")); // Bold on
        assert!(result.contains("\x1b[22m")); // Bold off
    }

    #[test]
    fn test_number_to_superscript() {
        assert_eq!(number_to_superscript(0), "⁰");
        assert_eq!(number_to_superscript(1), "¹");
        assert_eq!(number_to_superscript(2), "²");
        assert_eq!(number_to_superscript(42), "⁴²");
        assert_eq!(number_to_superscript(123), "¹²³");
    }
}