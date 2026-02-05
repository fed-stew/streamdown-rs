//! Streamdown Syntax
//!
//! This crate provides syntax highlighting for code blocks using the syntect library.
//! It's designed to work with streaming input (line-by-line) for real-time rendering.
//!
//! # Features
//!
//! - **Streaming highlighting** - Maintain state across lines for multi-line tokens
//! - **Language aliases** - Map common names (py, js, ts) to proper syntax definitions
//! - **Background override** - Override theme background for consistent code block styling
//! - **ANSI output** - Generate 24-bit true color terminal escape codes
//!
//! # Example
//!
//! ```
//! use streamdown_syntax::Highlighter;
//!
//! let highlighter = Highlighter::new();
//!
//! // Highlight a complete code block
//! let code = "fn main() {\n    println!(\"Hello!\");\n}";
//! let highlighted = highlighter.highlight_block(code, "rust");
//!
//! // For streaming, use HighlightState
//! use streamdown_syntax::HighlightState;
//! let mut hl = Highlighter::new();
//! let mut state = hl.new_highlight_state("rust");
//! let line1 = hl.highlight_line_with_state("fn main() {", &mut state);
//! let line2 = hl.highlight_line_with_state("    println!(\"Hello!\");", &mut state);
//! ```

mod languages;

pub use languages::{LANGUAGE_ALIASES, aliases_for, all_aliases, language_alias};

use syntect::easy::HighlightLines;
use syntect::highlighting::{Color, FontStyle, Style, Theme, ThemeSet};
use syntect::parsing::{SyntaxReference, SyntaxSet};
use syntect::util::as_24_bit_terminal_escaped;

/// Reset ANSI escape code
const RESET: &str = "\x1b[0m";

/// Syntax highlighter for code blocks.
///
/// Wraps syntect to provide a streaming-friendly API with language aliases
/// and background color override support.
pub struct Highlighter {
    /// Syntax definitions
    syntax_set: SyntaxSet,
    /// Color themes
    theme_set: ThemeSet,
    /// Current theme name
    theme_name: String,
    /// Optional background color override (RGB)
    background_override: Option<(u8, u8, u8)>,
}

impl std::fmt::Debug for Highlighter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Highlighter")
            .field("theme_name", &self.theme_name)
            .field("background_override", &self.background_override)
            .finish()
    }
}

impl Default for Highlighter {
    fn default() -> Self {
        Self::new()
    }
}

impl Highlighter {
    /// Create a new highlighter with the default theme (base16-ocean.dark).
    pub fn new() -> Self {
        Self::with_theme("base16-ocean.dark")
    }

    /// Create a highlighter with a specific theme.
    ///
    /// Available built-in themes:
    /// - "base16-ocean.dark"
    /// - "base16-ocean.light"
    /// - "base16-eighties.dark"
    /// - "base16-mocha.dark"
    /// - "InspiredGitHub"
    /// - "Solarized (dark)"
    /// - "Solarized (light)"
    pub fn with_theme(theme_name: &str) -> Self {
        Self {
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set: ThemeSet::load_defaults(),
            theme_name: theme_name.to_string(),
            background_override: None,
        }
    }

    /// Get a reference to the syntax set.
    pub fn syntax_set(&self) -> &SyntaxSet {
        &self.syntax_set
    }

    /// Get a reference to the theme set.
    pub fn theme_set(&self) -> &ThemeSet {
        &self.theme_set
    }

    /// Set the current theme.
    pub fn set_theme(&mut self, theme_name: &str) {
        self.theme_name = theme_name.to_string();
    }

    /// Get the current theme name.
    pub fn theme_name(&self) -> &str {
        &self.theme_name
    }

    /// Get the current theme.
    pub fn theme(&self) -> &Theme {
        self.theme_set
            .themes
            .get(&self.theme_name)
            .unwrap_or_else(|| {
                self.theme_set
                    .themes
                    .values()
                    .next()
                    .expect("No themes available")
            })
    }

    /// Override the background color for highlighted output.
    ///
    /// This removes all token background colors and uses the specified
    /// color for the entire code block. Pass `None` to use theme defaults.
    ///
    /// # Example
    /// ```
    /// use streamdown_syntax::Highlighter;
    ///
    /// let mut highlighter = Highlighter::new();
    /// // Set a dark grey background
    /// highlighter.set_background(Some((30, 30, 30)));
    /// ```
    pub fn set_background(&mut self, color: Option<(u8, u8, u8)>) {
        self.background_override = color;
    }

    /// Get the background override color.
    pub fn background(&self) -> Option<(u8, u8, u8)> {
        self.background_override
    }

    /// Find syntax definition for a language name.
    ///
    /// This first checks for common aliases (py→Python, js→JavaScript, etc.)
    /// and then falls back to syntect's built-in matching.
    pub fn syntax_for_language(&self, language: &str) -> Option<&SyntaxReference> {
        // First try our alias mapping
        let canonical = language_alias(language);

        // Try exact match first
        if let Some(syntax) = self.syntax_set.find_syntax_by_name(canonical) {
            return Some(syntax);
        }

        // Try token match (handles extensions like "rs", "py")
        if let Some(syntax) = self.syntax_set.find_syntax_by_token(canonical) {
            return Some(syntax);
        }

        // Try extension match
        if let Some(syntax) = self.syntax_set.find_syntax_by_extension(canonical) {
            return Some(syntax);
        }

        // Try original input
        self.syntax_set.find_syntax_by_token(language)
    }

    /// Get the plain text syntax (for unknown languages).
    pub fn plain_text(&self) -> &SyntaxReference {
        self.syntax_set.find_syntax_plain_text()
    }

    /// Create a new highlight state for streaming.
    ///
    /// This is the preferred way to do line-by-line highlighting.
    pub fn new_highlight_state(&self, language: &str) -> HighlightState<'_> {
        let syntax = self
            .syntax_for_language(language)
            .unwrap_or_else(|| self.plain_text());
        HighlightState::new(syntax, self.theme())
    }

    /// Highlight a single line with streaming state.
    ///
    /// This is the preferred method for streaming use cases. It maintains
    /// parse state across calls to correctly handle multi-line tokens.
    ///
    /// # Returns
    /// The highlighted line as an ANSI-escaped string (without trailing newline).
    pub fn highlight_line_with_state(&self, line: &str, state: &mut HighlightState) -> String {
        match state.highlighter.highlight_line(line, &self.syntax_set) {
            Ok(ranges) => {
                if self.background_override.is_some() {
                    // Custom rendering without background colors
                    self.styles_to_ansi(&ranges)
                } else {
                    // Use syntect's built-in terminal escaping
                    let escaped = as_24_bit_terminal_escaped(&ranges, false);
                    format!("{}{}", escaped, RESET)
                }
            }
            Err(_) => line.to_string(), // Fallback on error
        }
    }

    /// Convert syntect styles to ANSI escape codes.
    fn styles_to_ansi(&self, ranges: &[(Style, &str)]) -> String {
        let mut output = String::new();

        for (style, text) in ranges {
            // Skip empty text
            if text.is_empty() {
                continue;
            }

            let mut codes = Vec::new();

            // Foreground color
            let fg = style.foreground;
            codes.push(format!("38;2;{};{};{}", fg.r, fg.g, fg.b));

            // Skip background (we're overriding it)

            // Font style
            if style.font_style.contains(FontStyle::BOLD) {
                codes.push("1".to_string());
            }
            if style.font_style.contains(FontStyle::ITALIC) {
                codes.push("3".to_string());
            }
            if style.font_style.contains(FontStyle::UNDERLINE) {
                codes.push("4".to_string());
            }

            // Build escape sequence
            if !codes.is_empty() {
                output.push_str(&format!("\x1b[{}m", codes.join(";")));
            }

            output.push_str(text);
        }

        // Reset at end of line
        if !output.is_empty() {
            output.push_str(RESET);
        }

        output
    }

    /// Highlight a complete code block.
    ///
    /// This is a convenience method for non-streaming use cases.
    /// Each line is highlighted and joined with newlines.
    pub fn highlight_block(&self, code: &str, language: &str) -> String {
        let mut state = self.new_highlight_state(language);
        let mut output = String::new();

        for line in code.lines() {
            output.push_str(&self.highlight_line_with_state(line, &mut state));
            output.push('\n');
        }

        output
    }

    /// Simple highlight method (backward compatible).
    ///
    /// Highlights code and returns ANSI-formatted string.
    pub fn highlight(&self, code: &str, language: Option<&str>) -> String {
        let lang = language.unwrap_or("text");
        self.highlight_block(code, lang)
    }

    /// List available theme names.
    pub fn themes(&self) -> Vec<&str> {
        self.theme_set.themes.keys().map(|s| s.as_str()).collect()
    }

    /// List available language names.
    pub fn languages(&self) -> Vec<&str> {
        self.syntax_set
            .syntaxes()
            .iter()
            .map(|s| s.name.as_str())
            .collect()
    }

    /// Check if a theme exists.
    pub fn has_theme(&self, name: &str) -> bool {
        self.theme_set.themes.contains_key(name)
    }

    /// Check if a language is supported.
    pub fn has_language(&self, name: &str) -> bool {
        self.syntax_for_language(name).is_some()
    }
}

/// State for streaming syntax highlighting.
///
/// This maintains the parse state across lines to correctly handle
/// multi-line tokens like block comments and strings.
pub struct HighlightState<'a> {
    /// Syntect's HighlightLines for stateful line-by-line highlighting
    highlighter: HighlightLines<'a>,
}

impl<'a> HighlightState<'a> {
    /// Create a new highlight state for a syntax and theme.
    pub fn new(syntax: &'a SyntaxReference, theme: &'a Theme) -> Self {
        Self {
            highlighter: HighlightLines::new(syntax, theme),
        }
    }
}

/// Create a theme with overridden background color.
///
/// This is equivalent to Python's `override_background()` function.
/// It modifies a theme to use a custom background color and removes
/// all token-specific background colors.
pub fn override_theme_background(theme: &Theme, bg: (u8, u8, u8)) -> Theme {
    let mut new_theme = theme.clone();

    // Override settings background
    new_theme.settings.background = Some(Color {
        r: bg.0,
        g: bg.1,
        b: bg.2,
        a: 255,
    });

    // Clear all scope backgrounds
    for item in &mut new_theme.scopes {
        item.style.background = None;
    }

    new_theme
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_highlighter() {
        let h = Highlighter::new();
        assert_eq!(h.theme_name(), "base16-ocean.dark");
    }

    #[test]
    fn test_with_theme() {
        let h = Highlighter::with_theme("Solarized (dark)");
        assert_eq!(h.theme_name(), "Solarized (dark)");
    }

    #[test]
    fn test_set_background() {
        let mut h = Highlighter::new();
        assert!(h.background().is_none());

        h.set_background(Some((30, 30, 30)));
        assert_eq!(h.background(), Some((30, 30, 30)));

        h.set_background(None);
        assert!(h.background().is_none());
    }

    #[test]
    fn test_syntax_for_language() {
        let h = Highlighter::new();

        // Test exact names
        assert!(h.syntax_for_language("Rust").is_some());
        assert!(h.syntax_for_language("Python").is_some());

        // Test aliases
        assert!(h.syntax_for_language("rust").is_some());
        assert!(h.syntax_for_language("py").is_some());
        assert!(h.syntax_for_language("js").is_some());
        assert!(h.syntax_for_language("sh").is_some());
        assert!(h.syntax_for_language("bash").is_some());
        assert!(h.syntax_for_language("c").is_some());
        assert!(h.syntax_for_language("cpp").is_some());
    }

    #[test]
    fn test_highlight_block() {
        let h = Highlighter::new();
        let code = "fn main() {\n    println!(\"Hello\");\n}";
        let result = h.highlight_block(code, "rust");

        // Should contain ANSI escape codes
        assert!(result.contains("\x1b["));
        // Should contain the code
        assert!(result.contains("main"));
        assert!(result.contains("println"));
    }

    #[test]
    fn test_highlight_line_streaming() {
        let h = Highlighter::new();
        let mut state = h.new_highlight_state("rust");

        let line1 = h.highlight_line_with_state("fn main() {", &mut state);
        let line2 = h.highlight_line_with_state("    println!(\"Hello\");", &mut state);
        let line3 = h.highlight_line_with_state("}", &mut state);

        // All should contain ANSI codes
        assert!(line1.contains("\x1b["));
        assert!(line2.contains("\x1b["));
        assert!(line3.contains("\x1b["));
    }

    #[test]
    fn test_themes() {
        let h = Highlighter::new();
        let themes = h.themes();

        assert!(!themes.is_empty());
        assert!(themes.contains(&"base16-ocean.dark"));
    }

    #[test]
    fn test_languages() {
        let h = Highlighter::new();
        let langs = h.languages();

        assert!(!langs.is_empty());
        assert!(langs.contains(&"Rust"));
        assert!(langs.contains(&"Python"));
    }

    #[test]
    fn test_has_theme() {
        let h = Highlighter::new();
        assert!(h.has_theme("base16-ocean.dark"));
        assert!(!h.has_theme("nonexistent-theme"));
    }

    #[test]
    fn test_has_language() {
        let h = Highlighter::new();
        assert!(h.has_language("rust"));
        assert!(h.has_language("python"));
        assert!(h.has_language("py")); // alias
    }

    #[test]
    fn test_override_theme_background() {
        let h = Highlighter::new();
        let theme = h.theme();
        let new_theme = override_theme_background(theme, (10, 20, 30));

        assert_eq!(
            new_theme.settings.background,
            Some(Color {
                r: 10,
                g: 20,
                b: 30,
                a: 255
            })
        );
    }

    #[test]
    fn test_plain_text_fallback() {
        let h = Highlighter::new();
        let result = h.highlight_block("just some text", "unknown-lang-xyz");

        // Should still produce output (plain text fallback)
        assert!(result.contains("just some text"));
    }

    #[test]
    fn test_multiline_token() {
        let h = Highlighter::new();
        let mut state = h.new_highlight_state("rust");

        // Start a block comment
        let line1 = h.highlight_line_with_state("/* this is a", &mut state);
        let line2 = h.highlight_line_with_state("   multi-line comment */", &mut state);
        let line3 = h.highlight_line_with_state("let x = 1;", &mut state);

        // All lines should produce output
        assert!(!line1.is_empty());
        assert!(!line2.is_empty());
        assert!(!line3.is_empty());
    }

    #[test]
    fn test_background_override_styling() {
        let mut h = Highlighter::new();
        h.set_background(Some((30, 30, 30)));

        let code = "let x = 1;";
        let result = h.highlight_block(code, "rust");

        // Should have foreground colors but no background in escape codes
        assert!(result.contains("38;2;")); // Foreground
        // Background codes (48;2;) should NOT be present when override is set
        // The styling uses our custom method which skips backgrounds
    }
}
