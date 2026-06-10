//! Heading rendering.
//!
//! Renders markdown headings (h1-h6) with different styles:
//! - h1: Bold, h1 color, centered
//! - h2: Bold, h2 color, centered
//! - h3: Bold, h3 color
//! - h4: Bold, h4 color
//! - h5: h5 color (no bold)
//! - h6: h6 color (muted)

use crate::RenderStyle;
use crate::fg_color;
use crate::text::simple_wrap;
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
        let rendered = match level {
            1 => {
                let fg = fg_color(&style.h1);
                if style.heading_centered {
                    let line_width = visible_length(&line);
                    let spaces_to_center = (width.saturating_sub(line_width)) / 2;
                    let center_pad = " ".repeat(spaces_to_center);
                    format!(
                        "{}\n{}{}{}{}{}{}{}",
                        left_margin, left_margin, BOLD_ON, fg, center_pad, line, BOLD_OFF, RESET
                    )
                } else {
                    format!(
                        "{}\n{}{}{}{}{}{}",
                        left_margin, left_margin, BOLD_ON, fg, line, BOLD_OFF, RESET
                    )
                }
            }
            2 => {
                let fg = fg_color(&style.h2);
                if style.heading_centered {
                    let line_width = visible_length(&line);
                    let spaces_to_center = (width.saturating_sub(line_width)) / 2;
                    let center_pad = " ".repeat(spaces_to_center);
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
                } else {
                    format!(
                        "{}\n{}{}{}{}{}{}",
                        left_margin, left_margin, BOLD_ON, fg, line, BOLD_OFF, RESET
                    )
                }
            }
            3 => {
                // h3: Bold, colored
                let fg = fg_color(&style.h3);
                format!(
                    "{}{}{}{}{}{}",
                    left_margin, BOLD_ON, fg, line, BOLD_OFF, RESET
                )
            }
            4 => {
                // h4: Bold, colored
                let fg = fg_color(&style.h4);
                format!(
                    "{}{}{}{}{}{}",
                    left_margin, BOLD_ON, fg, line, BOLD_OFF, RESET
                )
            }
            5 => {
                // h5: Colored (no bold)
                let fg = fg_color(&style.h5);
                format!("{}{}{}{}", left_margin, fg, line, RESET)
            }
            _ => {
                // h6 and beyond: Colored (muted)
                let fg = fg_color(&style.h6);
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

    #[test]
    fn test_h1_left_aligned() {
        let mut style = default_style();
        style.heading_centered = false;
        let lines = render_heading(1, "Title", 80, "", &style);
        assert!(!lines.is_empty());
        // Should not have center padding before "Title"
        assert!(lines[0].contains(BOLD_ON));
        // The line after the newline should start with bold+color then text directly
        let after_newline = lines[0].split('\n').nth(1).unwrap();
        assert!(!after_newline.starts_with(' '));
    }
}
