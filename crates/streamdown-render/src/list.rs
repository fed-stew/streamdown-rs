//! List rendering.
//!
//! Renders markdown lists with:
//! - Bullet points (•, ◦, ▪)
//! - Ordered numbers with configurable style
//! - Nested indentation
//! - Proper text wrapping for long items
//! - Inline formatting (bold, italic, strikethrough, etc.)

use crate::text::text_wrap;
use crate::RenderStyle;
use crate::{bg_color, fg_color};
use streamdown_ansi::codes::{
    BOLD_OFF, BOLD_ON, DIM_ON, ITALIC_OFF, ITALIC_ON, RESET, STRIKEOUT_OFF, STRIKEOUT_ON,
    UNDERLINE_OFF, UNDERLINE_ON,
};
use streamdown_parser::{decode_html_entities, InlineElement, InlineParser, ListBullet};

/// Bullet characters for different nesting levels.
pub const BULLETS: [&str; 4] = [
    "•", // Level 0: Filled circle
    "◦", // Level 1: Empty circle
    "▪", // Level 2: Small filled square
    "‣", // Level 3: Triangular bullet
];

/// List rendering state.
#[derive(Debug, Clone, Default)]
pub struct ListState {
    /// Stack of (indent, is_ordered) for nested lists
    pub stack: Vec<(usize, bool)>,
    /// Current ordered list numbers at each level
    pub numbers: Vec<usize>,
}

impl ListState {
    /// Create a new list state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the current nesting level.
    pub fn level(&self) -> usize {
        self.stack.len()
    }

    /// Push a new list level.
    pub fn push(&mut self, indent: usize, ordered: bool) {
        self.stack.push((indent, ordered));
        self.numbers.push(0);
    }

    /// Pop a list level.
    pub fn pop(&mut self) {
        self.stack.pop();
        self.numbers.pop();
    }

    /// Get the next number for an ordered list.
    pub fn next_number(&mut self) -> usize {
        if let Some(n) = self.numbers.last_mut() {
            *n += 1;
            *n
        } else {
            1
        }
    }

    /// Adjust stack for a new item at given indent.
    pub fn adjust_for_indent(&mut self, indent: usize, ordered: bool) {
        // Pop levels that are deeper than current
        while let Some((stack_indent, _)) = self.stack.last() {
            if *stack_indent > indent {
                self.pop();
            } else {
                break;
            }
        }

        // Check if we need to push a new level
        let need_push = self.stack.last().map(|(i, _)| indent > *i).unwrap_or(true);
        if need_push {
            self.push(indent, ordered);
        }
    }

    /// Check if we're in a list.
    pub fn is_in_list(&self) -> bool {
        !self.stack.is_empty()
    }

    /// Reset the state.
    pub fn reset(&mut self) {
        self.stack.clear();
        self.numbers.clear();
    }
}

/// Render inline elements to a string with ANSI codes.
///
/// Parses markdown inline formatting (bold, italic, strikethrough, etc.)
/// and renders them with appropriate ANSI escape codes.
fn render_inline_content(content: &str, style: &RenderStyle) -> String {
    let mut parser = InlineParser::new();
    let elements = parser.parse(content);

    let mut result = String::new();

    for element in elements {
        match element {
            InlineElement::Text(text) => {
                result.push_str(&decode_html_entities(&text));
            }
            InlineElement::Bold(text) => {
                result.push_str(BOLD_ON);
                result.push_str(&decode_html_entities(&text));
                result.push_str(BOLD_OFF);
            }
            InlineElement::Italic(text) => {
                result.push_str(ITALIC_ON);
                result.push_str(&decode_html_entities(&text));
                result.push_str(ITALIC_OFF);
            }
            InlineElement::BoldItalic(text) => {
                result.push_str(BOLD_ON);
                result.push_str(ITALIC_ON);
                result.push_str(&decode_html_entities(&text));
                result.push_str(ITALIC_OFF);
                result.push_str(BOLD_OFF);
            }
            InlineElement::Strikeout(text) => {
                result.push_str(STRIKEOUT_ON);
                result.push_str(&decode_html_entities(&text));
                result.push_str(STRIKEOUT_OFF);
            }
            InlineElement::Underline(text) => {
                result.push_str(UNDERLINE_ON);
                result.push_str(&decode_html_entities(&text));
                result.push_str(UNDERLINE_OFF);
            }
            InlineElement::Code(text) => {
                // Inline code with background
                let bg = bg_color(&style.dark);
                result.push_str(&bg);
                result.push_str(DIM_ON);
                result.push(' ');
                result.push_str(&text);
                result.push(' ');
                result.push_str(RESET);
            }
            InlineElement::Link { text, url } => {
                // Underlined text with URL in parens
                let fg = fg_color(&style.grey);
                result.push_str(UNDERLINE_ON);
                result.push_str(&decode_html_entities(&text));
                result.push_str(UNDERLINE_OFF);
                result.push_str(&format!(" {}({}){}", fg, url, RESET));
            }
            InlineElement::Image { alt, .. } => {
                let fg = fg_color(&style.symbol);
                result.push_str(&format!("{}[🖼 {}]{}", fg, alt, RESET));
            }
            InlineElement::Footnote(text) => {
                let fg = fg_color(&style.symbol);
                result.push_str(&format!("{}{}{}", fg, text, RESET));
            }
        }
    }

    result
}

/// Render a list item.
///
/// # Arguments
/// * `indent` - Indentation level in spaces
/// * `bullet` - Bullet type
/// * `content` - Item content (may be inline-formatted)
/// * `width` - Available width
/// * `left_margin` - Left margin string
/// * `style` - Render style
/// * `list_state` - List state for tracking numbers
///
/// # Returns
/// Vector of rendered lines (may be multiple if content wraps)
pub fn render_list_item(
    indent: usize,
    bullet: &ListBullet,
    content: &str,
    width: usize,
    left_margin: &str,
    style: &RenderStyle,
    list_state: &mut ListState,
) -> Vec<String> {
    // Adjust list state for current indent
    let ordered = matches!(bullet, ListBullet::Ordered(_));
    list_state.adjust_for_indent(indent, ordered);

    let level = list_state.level().saturating_sub(1);

    // Calculate marker
    let marker = match bullet {
        ListBullet::Ordered(_) => {
            let num = list_state.next_number();
            format!("{}.", num)
        }
        ListBullet::PlusExpand => "⊞".to_string(), // Squared plus
        _ => {
            // Cycle through bullet styles based on level
            BULLETS[level % BULLETS.len()].to_string()
        }
    };

    // Calculate indentation
    let indent_spaces = indent * 2;
    let marker_width = unicode_width::UnicodeWidthStr::width(marker.as_str());
    let content_indent = indent_spaces + marker_width + 1; // +1 for space after marker

    // Color the marker
    let marker_fg = fg_color(&style.symbol);
    let colored_marker = format!("{}{}{}", marker_fg, marker, RESET);

    // Parse and render inline content with formatting (bold, italic, strikethrough, etc.)
    let rendered_content = render_inline_content(content, style);

    // Calculate content width (use visible length for margin with ANSI codes)
    let margin_width = streamdown_ansi::utils::visible_length(left_margin);
    let content_width = width.saturating_sub(margin_width + content_indent);

    // Wrap the content
    let first_prefix = format!(
        "{}{}{} ",
        left_margin,
        " ".repeat(indent_spaces),
        colored_marker
    );
    let next_prefix = format!("{}{}", left_margin, " ".repeat(content_indent));

    // Note: text_wrap handles ANSI codes properly via strip_ansi option
    let wrapped = text_wrap(
        &rendered_content,
        content_width,
        0,
        &first_prefix,
        &next_prefix,
        false,
        true,
    );

    if wrapped.is_empty() {
        vec![first_prefix]
    } else {
        wrapped.lines
    }
}

/// Render the end of a list.
pub fn render_list_end(list_state: &mut ListState) -> Vec<String> {
    list_state.reset();
    Vec::new() // No visible output, just state cleanup
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_style() -> RenderStyle {
        RenderStyle::default()
    }

    #[test]
    fn test_list_state_new() {
        let state = ListState::new();
        assert!(!state.is_in_list());
        assert_eq!(state.level(), 0);
    }

    #[test]
    fn test_list_state_push_pop() {
        let mut state = ListState::new();
        state.push(0, false);
        assert!(state.is_in_list());
        assert_eq!(state.level(), 1);

        state.pop();
        assert!(!state.is_in_list());
    }

    #[test]
    fn test_list_state_numbers() {
        let mut state = ListState::new();
        state.push(0, true);

        assert_eq!(state.next_number(), 1);
        assert_eq!(state.next_number(), 2);
        assert_eq!(state.next_number(), 3);
    }

    #[test]
    fn test_render_bullet_item() {
        let mut state = ListState::new();
        let lines = render_list_item(
            0,
            &ListBullet::Dash,
            "Item one",
            80,
            "",
            &default_style(),
            &mut state,
        );

        assert!(!lines.is_empty());
        assert!(lines[0].contains("•")); // First level bullet
        assert!(lines[0].contains("Item one"));
    }

    #[test]
    fn test_render_ordered_item() {
        let mut state = ListState::new();
        let lines = render_list_item(
            0,
            &ListBullet::Ordered(1),
            "First item",
            80,
            "",
            &default_style(),
            &mut state,
        );

        assert!(!lines.is_empty());
        assert!(lines[0].contains("1.")); // Number
        assert!(lines[0].contains("First item"));
    }

    #[test]
    fn test_render_nested_items() {
        let mut state = ListState::new();

        // First level
        let lines1 = render_list_item(
            0,
            &ListBullet::Dash,
            "Level 1",
            80,
            "",
            &default_style(),
            &mut state,
        );
        assert!(lines1[0].contains("•"));

        // Nested level
        let lines2 = render_list_item(
            2,
            &ListBullet::Dash,
            "Level 2",
            80,
            "",
            &default_style(),
            &mut state,
        );
        // Second level should use different bullet or more indent
        assert!(lines2[0].contains("Level 2"));
        // Check it has more leading spaces
        let indent1 = lines1[0]
            .chars()
            .take_while(|c| *c == ' ' || *c == '\t')
            .count();
        let indent2 = lines2[0]
            .chars()
            .take_while(|c| *c == ' ' || *c == '\t')
            .count();
        assert!(indent2 > indent1 || lines2[0].contains("◦")); // Either more indent or different bullet
    }

    #[test]
    fn test_render_long_item_wraps() {
        let mut state = ListState::new();
        let long_content = "This is a very long list item that should definitely wrap to multiple lines when rendered";
        let lines = render_list_item(
            0,
            &ListBullet::Dash,
            long_content,
            40,
            "",
            &default_style(),
            &mut state,
        );

        // Should wrap to multiple lines
        assert!(lines.len() > 1);
    }

    #[test]
    fn test_bullet_cycling() {
        // Different bullet styles for different levels
        assert_eq!(BULLETS[0], "•");
        assert_eq!(BULLETS[1], "◦");
        assert_eq!(BULLETS[2], "▪");
    }

    #[test]
    fn test_list_item_with_ansi_margin() {
        // BUG: left_margin.len() uses byte length instead of visible length.
        // When left_margin contains ANSI codes (e.g., blockquote border),
        // the content_width calculation is wrong, causing text to wrap too early.
        use streamdown_ansi::utils::visible_length;
        use streamdown_parser::ListBullet;

        let mut state = ListState::new();

        // Simulate blockquote border: "│ " with ANSI color codes
        // The visible width is 2 (│ + space), but byte length is much larger
        let margin = "\x1b[38;5;245m│\x1b[0m "; // colored │ + space
        let margin_visible_len = visible_length(margin);
        let margin_byte_len = margin.len();

        assert_eq!(margin_visible_len, 2, "Visible length should be 2");
        assert!(margin_byte_len > 10, "Byte length should be much larger due to ANSI codes");

        // Short content that should fit on one line
        // With correct calculation: content fits in (40 - 2 - 4) = 34 chars
        // With buggy calculation: content fits in (40 - 15 - 4) = 21 chars
        let content = "This content is 24 chars"; // 24 chars (fits in 34, not in 21)

        let lines = render_list_item(
            0,
            &ListBullet::Dash,
            content,
            40,
            margin,
            &default_style(),
            &mut state,
        );

        // With correct calculation, 30 chars should fit in 34 char width (1 line)
        // With buggy calculation, 30 chars won't fit in 21 char width (multiple lines)
        assert_eq!(
            lines.len(), 1,
            "Content should fit on one line with correct margin calculation, got {} lines: {:?}",
            lines.len(), lines
        );
    }
}
