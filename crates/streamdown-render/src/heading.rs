//! Heading rendering.
//!
//! Renders markdown headings (h1-h6) with different styles:
//! - h1: Bold, centered
//! - h2: Bold, colored (bright), centered
//! - h3: Bold, head color
//! - h4: Bold, symbol color
//! - h5: Plain text
//! - h6: Grey/dimmed

use crate::fg_color;
use crate::text::simple_wrap;
use crate::RenderStyle;
use streamdown_ansi::codes::{BOLD_OFF, BOLD_ON, RESET};
use streamdown_ansi::utils::visible_length;

/// Render a heading with appropriate styling.
///
/// # Arguments
/// * `level` - Heading level (1-6)
/// * `text` - The heading text (already inline-formatted)
/// * `width` - Available width for rendering
/// * `left_margin` - Left margin/padding
/// * `style` - Render style configuration
///
/// # Returns
/// A vector of rendered lines
pub fn render_heading(
    level: u8,
    text: &str,
    width: usize,
    left_margin: &str,
    style: &RenderStyle,
) -> Vec<String> {
    // Wrap text if needed
    let lines = simple_wrap(text, width);
    let mut result = Vec::new();

    for line in lines {
        let line_width = visible_length(&line);
        let spaces_to_center = (width.saturating_sub(line_width)) / 2;
        let center_pad = " ".repeat(spaces_to_center);

        let rendered = match level {
            1 => {
                // h1: Bold, centered
                format!(
                    "{}\n{}{}{}{}{}",
                    left_margin, left_margin, BOLD_ON, center_pad, line, BOLD_OFF
                )
            }
            2 => {
                // h2: Bold, bright color, centered
                let fg = fg_color(&style.bright);
                let spaces_right = width
                    .saturating_sub(line_width)
                    .saturating_sub(spaces_to_center);
                format!(
                    "{}\n{}{}{}{}{}{}{}{}",
                    left_margin,
                    left_margin,
                    BOLD_ON,
                    fg,
                    center_pad,
                    line,
                    " ".repeat(spaces_right),
                    BOLD_OFF,
                    RESET
                )
            }
            3 => {
                // h3: Bold, head color
                let fg = fg_color(&style.head);
                format!(
                    "{}{}{}{}{}{}",
                    left_margin, BOLD_ON, fg, line, BOLD_OFF, RESET
                )
            }
            4 => {
                // h4: Bold, symbol color
                let fg = fg_color(&style.symbol);
                format!(
                    "{}{}{}{}{}{}",
                    left_margin, BOLD_ON, fg, line, BOLD_OFF, RESET
                )
            }
            5 => {
                // h5: Plain text
                format!("{}{}{}", left_margin, line, RESET)
            }
            _ => {
                // h6 and beyond: Grey/dimmed
                let fg = fg_color(&style.grey);
                format!("{}{}{}{}", left_margin, fg, line, RESET)
            }
        };

        result.push(rendered);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_style() -> RenderStyle {
        RenderStyle::default()
    }

    #[test]
    fn test_h1_centered() {
        let lines = render_heading(1, "Title", 80, "", &default_style());
        assert!(!lines.is_empty());
        // Should contain bold codes
        assert!(lines[0].contains(BOLD_ON));
    }

    #[test]
    fn test_h2_colored() {
        let lines = render_heading(2, "Subtitle", 80, "", &default_style());
        assert!(!lines.is_empty());
        // Should contain color codes
        assert!(lines[0].contains("\x1b[38;2;"));
    }

    #[test]
    fn test_h3_head_color() {
        let lines = render_heading(3, "Section", 80, "", &default_style());
        assert!(!lines.is_empty());
        assert!(lines[0].contains(BOLD_ON));
    }

    #[test]
    fn test_h6_grey() {
        let lines = render_heading(6, "Minor", 80, "", &default_style());
        assert!(!lines.is_empty());
        // Should contain grey color
        assert!(lines[0].contains("\x1b[38;2;"));
    }

    #[test]
    fn test_heading_with_margin() {
        let lines = render_heading(1, "Title", 80, "  ", &default_style());
        assert!(!lines.is_empty());
        assert!(lines[0].starts_with("  "));
    }

    #[test]
    fn test_long_heading_wraps() {
        let long_text = "This is a very long heading that should wrap to multiple lines";
        let lines = render_heading(1, long_text, 20, "", &default_style());
        assert!(!lines.is_empty());
    }
}
