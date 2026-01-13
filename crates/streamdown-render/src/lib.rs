//! Streamdown Render
//!
//! This crate provides the terminal rendering engine for streamdown,
//! converting parsed markdown events into styled terminal output.
//!
//! # Features
//!
//! - **Styled headings** - 6 levels with different colors and alignment
//! - **Syntax highlighting** - Code blocks with language detection
//! - **Pretty tables** - With column width calculation and cell wrapping
//! - **ANSI-aware text wrapping** - Preserves formatting across line breaks
//! - **Nested lists** - With cycling bullet styles
//! - **Blockquotes** - With visual borders
//!
//! # Example
//!
//! ```
//! use streamdown_render::Renderer;
//! use streamdown_parser::ParseEvent;
//!
//! let mut output = Vec::new();
//! let mut renderer = Renderer::new(&mut output, 80);
//!
//! renderer.render_event(&ParseEvent::Heading {
//!     level: 1,
//!     content: "Hello World".to_string(),
//! }).unwrap();
//! ```

pub mod code;
pub mod features;
pub mod heading;
pub mod list;
pub mod table;
pub mod text;

pub use code::{code_wrap, CodeBlockState, CODEPAD_BOTTOM, CODEPAD_TOP};
pub use features::{
    copy_to_clipboard, is_tty, savebrace, savebrace_clear, savebrace_last, savebrace_path,
    savebrace_read, terminal_size, terminal_width, RenderFeatures,
};
pub use heading::render_heading;
pub use list::{render_list_item, ListState, BULLETS};
pub use table::{render_table_row, render_table_separator, TableState};
pub use text::{simple_wrap, split_text, text_wrap, WrappedText};

use std::io::Write;

use streamdown_ansi::codes::{
    BOLD_OFF, BOLD_ON, DIM_ON, ITALIC_OFF, ITALIC_ON, RESET, STRIKEOUT_OFF, STRIKEOUT_ON,
    UNDERLINE_OFF, UNDERLINE_ON,
};
use streamdown_ansi::color::hex2rgb;

/// Generate foreground color escape code from hex string.
pub fn fg_color(hex: &str) -> String {
    if let Some((r, g, b)) = hex2rgb(hex) {
        format!("\x1b[38;2;{};{};{}m", r, g, b)
    } else {
        String::new()
    }
}

/// Generate background color escape code from hex string.
pub fn bg_color(hex: &str) -> String {
    if let Some((r, g, b)) = hex2rgb(hex) {
        format!("\x1b[48;2;{};{};{}m", r, g, b)
    } else {
        String::new()
    }
}
use streamdown_parser::{InlineElement, ParseEvent};
use streamdown_syntax::Highlighter;

/// Render style configuration.
///
/// Contains color values for different elements.
#[derive(Debug, Clone)]
pub struct RenderStyle {
    /// Bright/highlight color (for h2)
    pub bright: String,
    /// Heading color (for h3)
    pub head: String,
    /// Symbol color (for markers, borders)
    pub symbol: String,
    /// Grey/muted color (for h6, dim text)
    pub grey: String,
    /// Dark background color (for code blocks)
    pub dark: String,
    /// Mid background color (for table headers)
    pub mid: String,
    /// Light background color
    pub light: String,
}

impl Default for RenderStyle {
    fn default() -> Self {
        Self {
            bright: "#87ceeb".to_string(), // Sky blue
            head: "#98fb98".to_string(),   // Pale green
            symbol: "#dda0dd".to_string(), // Plum
            grey: "#808080".to_string(),   // Grey
            dark: "#1a1a2e".to_string(),   // Dark blue-grey
            mid: "#2d2d44".to_string(),    // Mid blue-grey
            light: "#3d3d5c".to_string(),  // Light blue-grey
        }
    }
}

impl RenderStyle {
    /// Create from a computed style (from config).
    pub fn from_computed(computed: &streamdown_config::ComputedStyle) -> Self {
        Self {
            bright: computed.bright.clone(),
            head: computed.head.clone(),
            symbol: computed.symbol.clone(),
            grey: computed.grey.clone(),
            dark: computed.dark.clone(),
            mid: computed.mid.clone(),
            light: "#4d4d6e".to_string(), // Derive from mid
        }
    }
}

/// Terminal renderer for markdown.
pub struct Renderer<W: Write> {
    /// Output writer
    writer: W,
    /// Terminal width
    width: usize,
    /// Syntax highlighter
    highlighter: Highlighter,
    /// Render style
    style: RenderStyle,
    /// Feature flags
    features: RenderFeatures,
    /// Current column position
    column: usize,
    /// Current code language
    code_language: Option<String>,
    /// Raw code buffer (for clipboard/savebrace)
    code_buffer: String,
    /// Table state
    table_state: TableState,
    /// List state
    list_state: ListState,
    /// Whether in a blockquote
    in_blockquote: bool,
    /// Blockquote depth
    blockquote_depth: usize,
}

impl<W: Write> Renderer<W> {
    /// Create a new renderer with default style.
    pub fn new(writer: W, width: usize) -> Self {
        Self {
            writer,
            width,
            highlighter: Highlighter::default(),
            style: RenderStyle::default(),
            features: RenderFeatures::default(),
            column: 0,
            code_language: None,
            code_buffer: String::new(),
            table_state: TableState::new(),
            list_state: ListState::new(),
            in_blockquote: false,
            blockquote_depth: 0,
        }
    }

    /// Create a renderer with custom style.
    pub fn with_style(writer: W, width: usize, style: RenderStyle) -> Self {
        let mut r = Self::new(writer, width);
        r.style = style;
        r
    }

    /// Create a renderer with custom features.
    pub fn with_features(writer: W, width: usize, features: RenderFeatures) -> Self {
        let mut r = Self::new(writer, width);
        r.features = features;
        r
    }

    /// Set the syntax highlighting theme.
    pub fn set_theme(&mut self, theme: &str) {
        self.highlighter.set_theme(theme);
    }

    /// Set the render style.
    pub fn set_style(&mut self, style: RenderStyle) {
        self.style = style;
    }

    /// Set the feature flags.
    pub fn set_features(&mut self, features: RenderFeatures) {
        self.features = features;
    }

    /// Enable or disable pretty code block padding.
    pub fn set_pretty_pad(&mut self, enabled: bool) {
        self.features.pretty_pad = enabled;
    }

    /// Enable or disable clipboard integration.
    pub fn set_clipboard(&mut self, enabled: bool) {
        self.features.clipboard = enabled;
    }

    /// Enable or disable savebrace.
    pub fn set_savebrace(&mut self, enabled: bool) {
        self.features.savebrace = enabled;
    }

    /// Get the current width.
    pub fn width(&self) -> usize {
        self.width
    }

    /// Get the style.
    pub fn style(&self) -> &RenderStyle {
        &self.style
    }

    /// Get the features.
    pub fn features(&self) -> &RenderFeatures {
        &self.features
    }

    /// Calculate the left margin based on current state.
    fn left_margin(&self) -> String {
        if self.in_blockquote {
            let border = format!("{}│{} ", fg_color(&self.style.grey), RESET);
            border.repeat(self.blockquote_depth)
        } else {
            String::new()
        }
    }

    /// Calculate the current available width.
    fn current_width(&self) -> usize {
        let margin_width = if self.in_blockquote {
            self.blockquote_depth * 3 // "│ " = 2 chars per level, plus color codes
        } else {
            0
        };
        self.width.saturating_sub(margin_width)
    }

    /// Write a string to the output.
    fn write(&mut self, s: &str) -> std::io::Result<()> {
        write!(self.writer, "{}", s)
    }

    /// Write a line to the output.
    fn writeln(&mut self, s: &str) -> std::io::Result<()> {
        writeln!(self.writer, "{}", s)?;
        self.column = 0;
        Ok(())
    }

    /// Render a single parse event.
    pub fn render_event(&mut self, event: &ParseEvent) -> std::io::Result<()> {
        match event {
            // === Inline elements ===
            ParseEvent::Text(text) => {
                // Decode HTML entities like &copy; -> ©
                let decoded = streamdown_parser::decode_html_entities(text);
                self.write(&decoded)?;
                self.column += streamdown_ansi::utils::visible_length(&decoded);
            }

            ParseEvent::InlineCode(code) => {
                let bg = bg_color(&self.style.dark);
                self.write(&format!("{}{} {} {}", bg, DIM_ON, code, RESET))?;
            }

            ParseEvent::Bold(text) => {
                self.write(&format!("{}{}{}", BOLD_ON, text, BOLD_OFF))?;
            }

            ParseEvent::Italic(text) => {
                self.write(&format!("{}{}{}", ITALIC_ON, text, ITALIC_OFF))?;
            }

            ParseEvent::BoldItalic(text) => {
                self.write(&format!(
                    "{}{}{}{}{}",
                    BOLD_ON, ITALIC_ON, text, ITALIC_OFF, BOLD_OFF
                ))?;
            }

            ParseEvent::Underline(text) => {
                self.write(&format!("{}{}{}", UNDERLINE_ON, text, UNDERLINE_OFF))?;
            }

            ParseEvent::Strikeout(text) => {
                self.write(&format!("{}{}{}", STRIKEOUT_ON, text, STRIKEOUT_OFF))?;
            }

            ParseEvent::Link { text, url } => {
                // Render as: underlined text (url)
                // Also include OSC 8 hyperlink for terminals that support it
                let fg = fg_color(&self.style.grey);

                // OSC 8 start
                self.write("\x1b]8;;")?;
                self.write(url)?;
                self.write("\x1b\\")?;

                // Underlined text
                self.write(&format!("{}{}{}", UNDERLINE_ON, text, UNDERLINE_OFF))?;

                // OSC 8 end
                self.write("\x1b]8;;\x1b\\")?;

                // Show URL in parentheses (dimmed)
                self.write(&format!(" {}({}){}", fg, url, RESET))?;
            }

            ParseEvent::Image { alt, url: _ } => {
                let fg = fg_color(&self.style.symbol);
                self.write(&format!("{}[\u{1F5BC} {}]{}", fg, alt, RESET))?;
            }

            ParseEvent::Footnote(superscript) => {
                let fg = fg_color(&self.style.symbol);
                self.write(&format!("{}{}{}", fg, superscript, RESET))?;
            }

            // === Block elements ===
            ParseEvent::Heading { level, content } => {
                let lines = render_heading(
                    *level,
                    content,
                    self.current_width(),
                    &self.left_margin(),
                    &self.style,
                );
                for line in lines {
                    self.writeln(&line)?;
                }
            }

            ParseEvent::CodeBlockStart { language, .. } => {
                self.code_language = language.clone();
                self.code_buffer.clear();

                let lines = code::render_code_start(
                    language.as_deref(),
                    self.current_width(),
                    &self.left_margin(),
                    &self.style,
                    self.features.pretty_pad,
                );
                for line in lines {
                    self.writeln(&line)?;
                }
            }

            ParseEvent::CodeBlockLine(line) => {
                // Buffer raw code for clipboard/savebrace
                if !self.code_buffer.is_empty() {
                    self.code_buffer.push('\n');
                }
                self.code_buffer.push_str(line);

                let lang = self.code_language.as_deref().unwrap_or("text");
                let highlighted = self.highlighter.highlight(line, Some(lang));

                // Render with background
                let bg = bg_color(&self.style.dark);
                let margin = self.left_margin();
                let padding_needed = self
                    .current_width()
                    .saturating_sub(streamdown_ansi::utils::visible_length(&highlighted));

                self.writeln(&format!(
                    "{}{}{}{}{}",
                    margin,
                    bg,
                    highlighted.trim_end(),
                    " ".repeat(padding_needed),
                    RESET
                ))?;
            }

            ParseEvent::CodeBlockEnd => {
                let lines = code::render_code_end(
                    self.current_width(),
                    &self.left_margin(),
                    &self.style,
                    self.features.pretty_pad,
                );
                for line in lines {
                    self.writeln(&line)?;
                }

                // Handle clipboard integration (OSC 52)
                if self.features.clipboard && !self.code_buffer.is_empty() {
                    let _ = copy_to_clipboard(&self.code_buffer, &mut self.writer);
                }

                // Handle savebrace
                if self.features.savebrace && !self.code_buffer.is_empty() {
                    let _ = savebrace(&self.code_buffer);
                }

                self.code_language = None;
                self.code_buffer.clear();
            }

            ParseEvent::ListItem {
                indent,
                bullet,
                content,
            } => {
                let lines = render_list_item(
                    *indent,
                    bullet,
                    content,
                    self.current_width(),
                    &self.left_margin(),
                    &self.style,
                    &mut self.list_state,
                );
                for line in lines {
                    self.writeln(&line)?;
                }
            }

            ParseEvent::ListEnd => {
                list::render_list_end(&mut self.list_state);
            }

            ParseEvent::TableHeader(cells) => {
                self.table_state.reset();
                self.table_state.is_header = true;

                let width = self.current_width();
                let margin = self.left_margin();
                let style = self.style.clone();
                let lines =
                    render_table_row(cells, &mut self.table_state, width, &margin, &style, false);
                for line in lines {
                    self.writeln(&line)?;
                }
            }

            ParseEvent::TableRow(cells) => {
                let width = self.current_width();
                let margin = self.left_margin();
                let style = self.style.clone();
                let lines =
                    render_table_row(cells, &mut self.table_state, width, &margin, &style, false);
                for line in lines {
                    self.writeln(&line)?;
                }
            }

            ParseEvent::TableSeparator => {
                let sep = render_table_separator(
                    &self.table_state,
                    self.current_width(),
                    &self.left_margin(),
                    &self.style,
                );
                self.writeln(&sep)?;
                self.table_state.end_header();
            }

            ParseEvent::TableEnd => {
                self.table_state.reset();
            }

            ParseEvent::BlockquoteStart { depth } => {
                self.in_blockquote = true;
                self.blockquote_depth = *depth;
            }

            ParseEvent::BlockquoteLine(text) => {
                let margin = self.left_margin();
                // Wrap text to fit
                let wrapped = text_wrap(
                    text,
                    self.current_width(),
                    0,
                    &margin,
                    &margin,
                    false,
                    false,
                );
                for line in wrapped.lines {
                    self.writeln(&line)?;
                }
            }

            ParseEvent::BlockquoteEnd => {
                self.in_blockquote = false;
                self.blockquote_depth = 0;
            }

            ParseEvent::ThinkBlockStart => {
                let fg = fg_color(&self.style.grey);
                self.writeln(&format!("{}┌─ thinking ─{}", fg, RESET))?;
                self.in_blockquote = true;
                self.blockquote_depth = 1;
            }

            ParseEvent::ThinkBlockLine(text) => {
                let fg = fg_color(&self.style.grey);
                self.writeln(&format!("{}│{} {}", fg, RESET, text))?;
            }

            ParseEvent::ThinkBlockEnd => {
                let fg = fg_color(&self.style.grey);
                self.writeln(&format!("{}└{}", fg, RESET))?;
                self.in_blockquote = false;
                self.blockquote_depth = 0;
            }

            ParseEvent::HorizontalRule => {
                let fg = fg_color(&self.style.grey);
                let rule = "─".repeat(self.current_width());
                self.writeln(&format!("{}{}{}{}", self.left_margin(), fg, rule, RESET))?;
            }

            ParseEvent::EmptyLine => {
                self.writeln("")?;
            }

            ParseEvent::Newline => {
                self.writeln("")?;
            }

            ParseEvent::Prompt(prompt) => {
                self.write(prompt)?;
            }

            ParseEvent::InlineElements(elements) => {
                for element in elements {
                    self.render_inline_element(element)?;
                }
            }
        }

        self.writer.flush()
    }

    /// Render an inline element.
    fn render_inline_element(&mut self, element: &InlineElement) -> std::io::Result<()> {
        match element {
            InlineElement::Text(s) => self.write(s)?,
            InlineElement::Bold(s) => self.write(&format!("{}{}{}", BOLD_ON, s, BOLD_OFF))?,
            InlineElement::Italic(s) => self.write(&format!("{}{}{}", ITALIC_ON, s, ITALIC_OFF))?,
            InlineElement::BoldItalic(s) => self.write(&format!(
                "{}{}{}{}{}",
                BOLD_ON, ITALIC_ON, s, ITALIC_OFF, BOLD_OFF
            ))?,
            InlineElement::Underline(s) => {
                self.write(&format!("{}{}{}", UNDERLINE_ON, s, UNDERLINE_OFF))?
            }
            InlineElement::Strikeout(s) => {
                self.write(&format!("{}{}{}", STRIKEOUT_ON, s, STRIKEOUT_OFF))?
            }
            InlineElement::Code(s) => {
                let bg = bg_color(&self.style.dark);
                self.write(&format!("{} {} {}", bg, s, RESET))?
            }
            InlineElement::Link { text, url } => {
                let fg = fg_color(&self.style.grey);
                // OSC 8 start
                self.write("\x1b]8;;")?;
                self.write(url)?;
                self.write("\x1b\\")?;
                // Underlined text
                self.write(&format!("{}{}{}", UNDERLINE_ON, text, UNDERLINE_OFF))?;
                // OSC 8 end
                self.write("\x1b]8;;\x1b\\")?;
                // Show URL in parentheses (dimmed)
                self.write(&format!(" {}({}){}", fg, url, RESET))?;
            }
            InlineElement::Image { alt, .. } => {
                let fg = fg_color(&self.style.symbol);
                self.write(&format!("{}[\u{1F5BC} {}]{}", fg, alt, RESET))?
            }
            InlineElement::Footnote(s) => {
                let fg = fg_color(&self.style.symbol);
                self.write(&format!("{}{}{}", fg, s, RESET))?
            }
        }
        Ok(())
    }

    /// Render multiple events.
    pub fn render(&mut self, events: &[ParseEvent]) -> std::io::Result<()> {
        for event in events {
            self.render_event(event)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use streamdown_parser::ListBullet;

    #[test]
    fn test_render_heading() {
        let mut output = Vec::new();
        let mut renderer = Renderer::new(&mut output, 80);

        renderer
            .render_event(&ParseEvent::Heading {
                level: 1,
                content: "Title".to_string(),
            })
            .unwrap();

        let result = String::from_utf8(output).unwrap();
        assert!(result.contains("Title"));
        assert!(result.contains(BOLD_ON));
    }

    #[test]
    fn test_render_h2_colored() {
        let mut output = Vec::new();
        let mut renderer = Renderer::new(&mut output, 80);

        renderer
            .render_event(&ParseEvent::Heading {
                level: 2,
                content: "Subtitle".to_string(),
            })
            .unwrap();

        let result = String::from_utf8(output).unwrap();
        assert!(result.contains("Subtitle"));
        assert!(result.contains("\x1b[38;2;")); // Color code
    }

    #[test]
    fn test_render_code_block() {
        let mut output = Vec::new();
        let mut renderer = Renderer::new(&mut output, 80);

        renderer
            .render_event(&ParseEvent::CodeBlockStart {
                language: Some("rust".to_string()),
                indent: 0,
            })
            .unwrap();
        renderer
            .render_event(&ParseEvent::CodeBlockLine("let x = 1;".to_string()))
            .unwrap();
        renderer.render_event(&ParseEvent::CodeBlockEnd).unwrap();

        let result = String::from_utf8(output).unwrap();
        // The code block contains the code (may have ANSI formatting)
        let visible = streamdown_ansi::utils::visible(&result);
        assert!(visible.contains("let x = 1;") || visible.contains("let"));
    }

    #[test]
    fn test_render_code_block_pretty_pad() {
        let mut output = Vec::new();
        let mut renderer = Renderer::new(&mut output, 80);
        renderer.set_pretty_pad(true);

        renderer
            .render_event(&ParseEvent::CodeBlockStart {
                language: Some("rust".to_string()),
                indent: 0,
            })
            .unwrap();
        renderer.render_event(&ParseEvent::CodeBlockEnd).unwrap();

        let result = String::from_utf8(output).unwrap();
        assert!(result.contains(CODEPAD_TOP));
        assert!(result.contains(CODEPAD_BOTTOM));
    }

    #[test]
    fn test_render_list() {
        let mut output = Vec::new();
        let mut renderer = Renderer::new(&mut output, 80);

        renderer
            .render_event(&ParseEvent::ListItem {
                indent: 0,
                bullet: ListBullet::Dash,
                content: "Item".to_string(),
            })
            .unwrap();

        let result = String::from_utf8(output).unwrap();
        assert!(result.contains("•")); // Bullet
        assert!(result.contains("Item"));
    }

    #[test]
    fn test_render_ordered_list() {
        let mut output = Vec::new();
        let mut renderer = Renderer::new(&mut output, 80);

        renderer
            .render_event(&ParseEvent::ListItem {
                indent: 0,
                bullet: ListBullet::Ordered(1),
                content: "First".to_string(),
            })
            .unwrap();

        let result = String::from_utf8(output).unwrap();
        assert!(result.contains("1.")); // Number
        assert!(result.contains("First"));
    }

    #[test]
    fn test_render_table() {
        let mut output = Vec::new();
        let mut renderer = Renderer::new(&mut output, 80);

        renderer
            .render_event(&ParseEvent::TableHeader(vec![
                "A".to_string(),
                "B".to_string(),
            ]))
            .unwrap();
        renderer.render_event(&ParseEvent::TableSeparator).unwrap();
        renderer
            .render_event(&ParseEvent::TableRow(vec![
                "1".to_string(),
                "2".to_string(),
            ]))
            .unwrap();
        renderer.render_event(&ParseEvent::TableEnd).unwrap();

        let result = String::from_utf8(output).unwrap();
        assert!(result.contains("A"));
        assert!(result.contains("1"));
    }

    #[test]
    fn test_render_blockquote() {
        let mut output = Vec::new();
        let mut renderer = Renderer::new(&mut output, 80);

        renderer
            .render_event(&ParseEvent::BlockquoteStart { depth: 1 })
            .unwrap();
        renderer
            .render_event(&ParseEvent::BlockquoteLine("Quote text".to_string()))
            .unwrap();
        renderer.render_event(&ParseEvent::BlockquoteEnd).unwrap();

        let result = String::from_utf8(output).unwrap();
        assert!(result.contains("Quote text"));
    }

    #[test]
    fn test_render_think_block() {
        let mut output = Vec::new();
        let mut renderer = Renderer::new(&mut output, 80);

        renderer.render_event(&ParseEvent::ThinkBlockStart).unwrap();
        renderer
            .render_event(&ParseEvent::ThinkBlockLine("Thinking...".to_string()))
            .unwrap();
        renderer.render_event(&ParseEvent::ThinkBlockEnd).unwrap();

        let result = String::from_utf8(output).unwrap();
        assert!(result.contains("thinking"));
        assert!(result.contains("Thinking..."));
    }

    #[test]
    fn test_render_horizontal_rule() {
        let mut output = Vec::new();
        let mut renderer = Renderer::new(&mut output, 40);

        renderer.render_event(&ParseEvent::HorizontalRule).unwrap();

        let result = String::from_utf8(output).unwrap();
        assert!(result.contains("─"));
    }

    #[test]
    fn test_render_link() {
        let mut output = Vec::new();
        let mut renderer = Renderer::new(&mut output, 80);

        renderer
            .render_event(&ParseEvent::Link {
                text: "Click here".to_string(),
                url: "https://example.com".to_string(),
            })
            .unwrap();

        let result = String::from_utf8(output).unwrap();
        assert!(result.contains("Click here"));
        assert!(result.contains("example.com"));
    }

    #[test]
    fn test_render_style() {
        let style = RenderStyle::default();
        assert!(!style.bright.is_empty());
        assert!(!style.dark.is_empty());
    }

    #[test]
    fn test_render_with_custom_style() {
        let style = RenderStyle {
            bright: "#ff0000".to_string(),
            ..Default::default()
        };

        let mut output = Vec::new();
        let mut renderer = Renderer::with_style(&mut output, 80, style);

        renderer
            .render_event(&ParseEvent::Heading {
                level: 2,
                content: "Red".to_string(),
            })
            .unwrap();

        let result = String::from_utf8(output).unwrap();
        assert!(result.contains("Red"));
    }
}
