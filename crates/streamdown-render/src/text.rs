//! Text wrapping and formatting utilities.
//!
//! This module provides ANSI-aware text wrapping that preserves escape codes
//! across line breaks, handles CJK characters correctly, and supports various
//! formatting options.

use streamdown_ansi::utils::{ansi_collapse, extract_ansi_codes, visible, visible_length};
use streamdown_parser::tokenizer::cjk_count;

/// Result of wrapping text.
#[derive(Debug, Clone)]
pub struct WrappedText {
    /// The wrapped lines
    pub lines: Vec<String>,
    /// Whether any lines were truncated
    pub truncated: bool,
}

impl WrappedText {
    /// Create empty wrapped text.
    pub fn empty() -> Self {
        Self {
            lines: Vec::new(),
            truncated: false,
        }
    }

    /// Check if there are no lines.
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// Get the number of lines.
    pub fn len(&self) -> usize {
        self.lines.len()
    }
}

/// Split text into words while preserving ANSI codes.
///
/// This is smarter than a simple split - it keeps ANSI codes attached
/// to the words they modify and handles CJK characters specially.
pub fn split_text(text: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut in_escape = false;
    let mut escape_buf = String::new();

    for ch in text.chars() {
        if in_escape {
            escape_buf.push(ch);
            if ch == 'm' {
                // End of ANSI sequence
                current.push_str(&escape_buf);
                escape_buf.clear();
                in_escape = false;
            }
            continue;
        }

        if ch == '\x1b' {
            in_escape = true;
            escape_buf.push(ch);
            continue;
        }

        if ch.is_whitespace() {
            if !current.is_empty() {
                words.push(std::mem::take(&mut current));
            }
        } else {
            current.push(ch);
        }
    }

    // Don't forget trailing escape sequence
    if !escape_buf.is_empty() {
        current.push_str(&escape_buf);
    }

    if !current.is_empty() {
        words.push(current);
    }

    words
}

/// Wrap text to fit within a given width.
///
/// This is ANSI-aware and will preserve formatting across line breaks.
///
/// # Arguments
/// * `text` - The text to wrap
/// * `width` - Maximum width in visible characters
/// * `indent` - Indentation for continuation lines
/// * `first_prefix` - Prefix for the first line
/// * `next_prefix` - Prefix for subsequent lines
/// * `force_truncate` - If true, truncate lines that are too long
/// * `preserve_format` - If true, don't reset formatting at end of lines
pub fn text_wrap(
    text: &str,
    width: usize,
    indent: usize,
    first_prefix: &str,
    next_prefix: &str,
    force_truncate: bool,
    preserve_format: bool,
) -> WrappedText {
    if width == 0 {
        return WrappedText::empty();
    }

    let words = split_text(text);
    if words.is_empty() {
        return WrappedText::empty();
    }

    let mut lines = Vec::new();
    let mut current_line = String::new();
    let mut current_style: Vec<String> = Vec::new();
    let mut truncated = false;
    let resetter = if preserve_format { "" } else { "\x1b[0m" };

    let mut prev_word = String::new();

    for word in words.iter().chain(std::iter::once(&String::new())) {
        // Extract ANSI codes from the word
        let codes = extract_ansi_codes(word);

        // Check if word starts with an ANSI code
        if !codes.is_empty() && word.starts_with(&codes[0]) {
            current_style.push(codes[0].clone());
        }

        let word_visible_len = visible_length(word);
        let line_visible_len = visible_length(&current_line);

        // Check if word fits on current line
        let space_needed = if current_line.is_empty() || word_visible_len == 0 {
            0
        } else {
            1 // space between words
        };

        // CJK: no space needed between CJK characters
        let space_needed = if cjk_count(word) > 0 && cjk_count(&prev_word) > 0 {
            0
        } else {
            space_needed
        };

        if word_visible_len > 0 && line_visible_len + word_visible_len + space_needed <= width {
            // Word fits
            if space_needed > 0 && !current_line.is_empty() {
                current_line.push(' ');
            }
            current_line.push_str(word);
        } else if word_visible_len > 0 {
            // Word doesn't fit, finalize current line
            if !current_line.is_empty() {
                let prefix = if lines.is_empty() {
                    first_prefix
                } else {
                    next_prefix
                };
                let mut line_content = format!("{}{}", prefix, current_line);

                // Force truncate if needed
                if force_truncate {
                    while visible_length(&line_content) > width && line_content.len() > 1 {
                        // Remove last visible character and add ellipsis
                        let visible_part = visible(&line_content);
                        if visible_part.len() > 1 {
                            // Find the position to truncate
                            let target_len = visible_part.len() - 2;
                            line_content = truncate_to_visible(&line_content, target_len);
                            line_content.push('…');
                            truncated = true;
                        } else {
                            break;
                        }
                    }
                }

                // Add resetter and padding
                let padding = width.saturating_sub(visible_length(&line_content));
                line_content.push_str(resetter);
                line_content.push_str(&" ".repeat(padding));

                if !visible(&line_content).trim().is_empty() {
                    lines.push(line_content);
                }
            }

            // Start new line with current word
            let indent_str = " ".repeat(indent);
            let style_str: String = current_style.join("");
            current_line = format!("{}{}{}", indent_str, style_str, word);
        }

        // Update style tracking
        for code in codes.iter().skip(
            if word.starts_with(&codes.first().cloned().unwrap_or_default()) {
                1
            } else {
                0
            },
        ) {
            current_style.push(code.clone());
        }
        current_style = ansi_collapse(&current_style, "");

        prev_word = word.clone();
    }

    // Don't forget the last line
    if !current_line.is_empty() && !visible(&current_line).trim().is_empty() {
        let prefix = if lines.is_empty() {
            first_prefix
        } else {
            next_prefix
        };
        let mut line_content = format!("{}{}", prefix, current_line);

        if force_truncate {
            while visible_length(&line_content) > width && line_content.len() > 1 {
                let visible_part = visible(&line_content);
                if visible_part.len() > 1 {
                    let target_len = visible_part.len() - 2;
                    line_content = truncate_to_visible(&line_content, target_len);
                    line_content.push('…');
                    truncated = true;
                } else {
                    break;
                }
            }
        }

        line_content.push_str(resetter);
        lines.push(line_content);
    }

    WrappedText { lines, truncated }
}

/// Truncate a string (with ANSI codes) to a visible length.
fn truncate_to_visible(text: &str, max_visible: usize) -> String {
    let mut result = String::new();
    let mut visible_count = 0;
    let mut in_escape = false;

    for ch in text.chars() {
        if in_escape {
            result.push(ch);
            if ch == 'm' {
                in_escape = false;
            }
            continue;
        }

        if ch == '\x1b' {
            in_escape = true;
            result.push(ch);
            continue;
        }

        if visible_count >= max_visible {
            break;
        }

        result.push(ch);
        visible_count += unicode_width::UnicodeWidthChar::width(ch).unwrap_or(1);
    }

    result
}

/// Simple text wrap without ANSI awareness (for plain text).
pub fn simple_wrap(text: &str, width: usize) -> Vec<String> {
    if width == 0 || text.is_empty() {
        return vec![text.to_string()];
    }

    let mut lines = Vec::new();
    let mut current = String::new();

    for word in text.split_whitespace() {
        let word_len = unicode_width::UnicodeWidthStr::width(word);
        let current_len = unicode_width::UnicodeWidthStr::width(current.as_str());

        if current.is_empty() {
            current = word.to_string();
        } else if current_len + 1 + word_len <= width {
            current.push(' ');
            current.push_str(word);
        } else {
            lines.push(std::mem::take(&mut current));
            current = word.to_string();
        }
    }

    if !current.is_empty() {
        lines.push(current);
    }

    if lines.is_empty() {
        lines.push(String::new());
    }

    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_text() {
        let words = split_text("hello world");
        assert_eq!(words, vec!["hello", "world"]);
    }

    #[test]
    fn test_split_text_with_ansi() {
        let text = "\x1b[1mhello\x1b[0m world";
        let words = split_text(text);
        assert_eq!(words.len(), 2);
        assert!(words[0].contains("\x1b[1m"));
    }

    #[test]
    fn test_simple_wrap() {
        let lines = simple_wrap("hello world foo bar", 10);
        assert!(lines.len() >= 2);
    }

    #[test]
    fn test_text_wrap_basic() {
        let result = text_wrap("hello world", 20, 0, "", "", false, false);
        assert_eq!(result.lines.len(), 1);
    }

    #[test]
    fn test_text_wrap_multiline() {
        let result = text_wrap("hello world foo bar baz", 10, 0, "", "", false, false);
        assert!(result.lines.len() > 1);
    }

    #[test]
    fn test_text_wrap_with_prefix() {
        let result = text_wrap("hello world", 20, 0, "> ", "  ", false, false);
        assert!(!result.lines.is_empty());
        assert!(result.lines[0].starts_with("> "));
    }

    #[test]
    fn test_truncate_to_visible() {
        let text = "hello world";
        let truncated = truncate_to_visible(text, 5);
        assert_eq!(truncated, "hello");
    }

    #[test]
    fn test_truncate_with_ansi() {
        let text = "\x1b[1mhello\x1b[0m world";
        let truncated = truncate_to_visible(text, 5);
        assert!(truncated.contains("\x1b["));
    }

    #[test]
    fn test_truncate_to_visible_box_drawing() {
        // ═ is 3 bytes, 1 display column. Byte-based slicing would corrupt.
        let text = "═══════════";
        assert_eq!(text.len(), 33);
        assert_eq!(text.chars().count(), 11);

        let truncated = truncate_to_visible(text, 5);

        assert_eq!(truncated.chars().count(), 5);
        assert_eq!(truncated, "═════");
    }

    #[test]
    fn test_truncate_to_visible_emojis() {
        // 🎉 = 4 bytes, 2 display columns.
        let text = "🎉🎉🎉🎉🎉";
        assert_eq!(text.len(), 20);
        assert_eq!(text.chars().count(), 5);

        let truncated = truncate_to_visible(text, 4);

        assert_eq!(truncated.chars().count(), 2);
        assert_eq!(truncated, "🎉🎉");
    }

    #[test]
    fn test_truncate_to_visible_zwj_emojis() {
        // 👨‍💻 = 11 bytes, 3 code points. Should not split ZWJ sequence.
        let text = format!("{}{}{}", "👨‍💻", "👨‍💻", "👨‍💻");
        assert_eq!(text.chars().count(), 9);

        let truncated = truncate_to_visible(&text, 4);

        let _ = truncated.chars().count(); // Should not panic
    }

    #[test]
    fn test_simple_wrap_cjk() {
        // CJK: 3 bytes, 2 display columns per char.
        let text = "你好 世界 你好 世界";

        let lines = simple_wrap(text, 10);

        assert_eq!(lines.len(), 2);
        for line in &lines {
            let _ = line.chars().count();
        }
    }

    #[test]
    fn test_simple_wrap_emojis() {
        // 🎉 = 4 bytes, 2 display columns.
        let text = "🎉🎉🎉🎉 🎉🎉🎉🎉";

        let lines = simple_wrap(text, 10);

        assert!(!lines.is_empty());
    }

    #[test]
    fn test_text_wrap_mixed_multibyte() {
        let text = "Hello 你好 ═══ 🎉 world";

        let result = text_wrap(text, 15, 0, "", "", false, false);

        assert!(!result.lines.is_empty());
    }

    #[test]
    fn test_split_text_emoji_integrity() {
        let text = "hello 🎉 world 🌟 test";

        let words = split_text(text);

        assert_eq!(words.len(), 5);
        assert_eq!(words[1], "🎉");
        assert_eq!(words[3], "🌟");
    }

    #[test]
    fn test_split_text_zwj_emoji() {
        // ZWJ emoji 👨‍👩‍👧‍👦 should stay together (7 code points).
        let text = "Family: 👨‍👩‍👧‍👦 done";

        let words = split_text(text);

        assert_eq!(words.len(), 3);
        assert_eq!(words[1].chars().count(), 7);
    }
}
