//! Style pairs for toggleable ANSI formatting.
//!
//! Each style is represented as a tuple of (on_code, off_code)
//! for easy toggling of formatting states.

use crate::codes;
use crossterm::style::{Attribute, Color};

/// A style pair consisting of (enable_code, disable_code).
///
/// This makes it easy to toggle formatting:
/// ```
/// use streamdown_ansi::style::BOLD;
/// let text = format!("{}bold text{}", BOLD.0, BOLD.1);
/// ```
pub type StylePair = (&'static str, &'static str);

/// Bold formatting pair.
pub const BOLD: StylePair = (codes::BOLD_ON, codes::BOLD_OFF);

/// Underline formatting pair.
pub const UNDERLINE: StylePair = (codes::UNDERLINE_ON, codes::UNDERLINE_OFF);

/// Italic formatting pair.
pub const ITALIC: StylePair = (codes::ITALIC_ON, codes::ITALIC_OFF);

/// Strikeout formatting pair.
pub const STRIKEOUT: StylePair = (codes::STRIKEOUT_ON, codes::STRIKEOUT_OFF);

/// Dim/faint formatting pair.
pub const DIM: StylePair = (codes::DIM_ON, codes::DIM_OFF);

/// Reverse video formatting pair.
pub const REVERSE: StylePair = (codes::REVERSE_ON, codes::REVERSE_OFF);

/// Link formatting pair (OSC 8 hyperlinks).
/// Note: The URL must be inserted between LINK.0 and the closing escape.
pub const LINK: StylePair = (codes::LINK_START, codes::LINK_END);

/// Represents a complete text style with colors and attributes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Style {
    /// Foreground color
    pub fg: Option<Color>,
    /// Background color
    pub bg: Option<Color>,
    /// Text attributes (bold, italic, etc.)
    pub attributes: Vec<Attribute>,
}

impl Style {
    /// Create a new empty style.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the foreground color.
    pub fn fg(mut self, color: Color) -> Self {
        self.fg = Some(color);
        self
    }

    /// Set the background color.
    pub fn bg(mut self, color: Color) -> Self {
        self.bg = Some(color);
        self
    }

    /// Add an attribute.
    pub fn attr(mut self, attr: Attribute) -> Self {
        self.attributes.push(attr);
        self
    }

    /// Make the text bold.
    pub fn bold(self) -> Self {
        self.attr(Attribute::Bold)
    }

    /// Make the text italic.
    pub fn italic(self) -> Self {
        self.attr(Attribute::Italic)
    }

    /// Make the text underlined.
    pub fn underline(self) -> Self {
        self.attr(Attribute::Underlined)
    }

    /// Make the text dim.
    pub fn dim(self) -> Self {
        self.attr(Attribute::Dim)
    }

    /// Apply strikethrough.
    pub fn strikethrough(self) -> Self {
        self.attr(Attribute::CrossedOut)
    }

    /// Convert to ANSI escape sequence.
    pub fn to_ansi(&self) -> String {
        let mut codes = Vec::new();

        // Add attributes
        for attr in &self.attributes {
            let code = match attr {
                Attribute::Bold => "1",
                Attribute::Dim => "2",
                Attribute::Italic => "3",
                Attribute::Underlined => "4",
                Attribute::CrossedOut => "9",
                Attribute::Reverse => "7",
                _ => continue,
            };
            codes.push(code.to_string());
        }

        // Add foreground color
        if let Some(Color::Rgb { r, g, b }) = self.fg {
            codes.push(format!("38;2;{};{};{}", r, g, b));
        }

        // Add background color
        if let Some(Color::Rgb { r, g, b }) = self.bg {
            codes.push(format!("48;2;{};{};{}", r, g, b));
        }

        if codes.is_empty() {
            String::new()
        } else {
            format!("\x1b[{}m", codes.join(";"))
        }
    }
}

/// Builder for creating formatted text with ANSI codes.
#[derive(Debug, Clone, Default)]
pub struct StyledText {
    /// The text content
    pub text: String,
    /// Applied styles
    pub styles: Vec<(usize, usize, Style)>,
}

impl StyledText {
    /// Create a new styled text builder.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            styles: Vec::new(),
        }
    }

    /// Apply a style to a range of the text.
    pub fn style_range(mut self, start: usize, end: usize, style: Style) -> Self {
        self.styles.push((start, end, style));
        self
    }

    /// Render the styled text with ANSI codes.
    pub fn render(&self) -> String {
        // For simple cases, just wrap the whole text
        // TODO: Implement proper range-based styling
        self.text.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_style_pairs() {
        assert_eq!(BOLD.0, "\x1b[1m");
        assert_eq!(BOLD.1, "\x1b[22m");
        assert_eq!(ITALIC.0, "\x1b[3m");
        assert_eq!(ITALIC.1, "\x1b[23m");
    }

    #[test]
    fn test_style_builder() {
        let style = Style::new().bold().fg(Color::Rgb { r: 255, g: 0, b: 0 });

        let ansi = style.to_ansi();
        assert!(ansi.contains("1")); // bold
        assert!(ansi.contains("38;2;255;0;0")); // red foreground
    }
}
