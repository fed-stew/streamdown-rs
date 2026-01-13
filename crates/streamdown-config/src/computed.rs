//! Computed style values.
//!
//! This module contains `ComputedStyle` which holds pre-computed
//! ANSI color codes derived from the style configuration.

use crate::style::{HsvMultiplier, StyleConfig};
use streamdown_ansi::color::hsv_to_rgb;

/// Pre-computed ANSI color strings.
///
/// These values are computed from `StyleConfig` by applying HSV multipliers
/// to the base color. Each string is formatted for ANSI escape sequences,
/// e.g., "48;23;15m" for background colors or "255;128;64m" for foreground.
#[derive(Debug, Clone, Default)]
pub struct ComputedStyle {
    /// Dark color (for backgrounds, code block bg).
    /// Format: "r;g;bm"
    pub dark: String,

    /// Mid color (for secondary elements).
    /// Format: "r;g;bm"
    pub mid: String,

    /// Symbol color (for special characters, bullets).
    /// Format: "r;g;bm"
    pub symbol: String,

    /// Head color (for headers).
    /// Format: "r;g;bm"
    pub head: String,

    /// Grey color (for muted text, blockquote bars).
    /// Format: "r;g;bm"
    pub grey: String,

    /// Bright color (for emphasis, links).
    /// Format: "r;g;bm"
    pub bright: String,

    /// Margin spaces string (e.g., "  " for margin=2).
    pub margin_spaces: String,

    /// Block quote prefix string with ANSI styling.
    pub blockquote: String,

    /// Code block background ANSI sequence.
    pub codebg: String,

    /// Link color ANSI sequence.
    pub link: String,

    /// Code block padding characters (left, right).
    /// Used for pretty padding around code blocks.
    pub codepad: (String, String),

    /// List indent string.
    pub list_indent: String,

    /// Full ANSI foreground escape for dark.
    pub dark_fg: String,

    /// Full ANSI background escape for dark.
    pub dark_bg: String,

    /// Full ANSI foreground escape for mid.
    pub mid_fg: String,

    /// Full ANSI foreground escape for symbol.
    pub symbol_fg: String,

    /// Full ANSI foreground escape for head.
    pub head_fg: String,

    /// Full ANSI foreground escape for grey.
    pub grey_fg: String,

    /// Full ANSI foreground escape for bright.
    pub bright_fg: String,
}

impl ComputedStyle {
    /// Compute style values from a StyleConfig.
    ///
    /// This applies the HSV multipliers to the base color to generate
    /// all derived colors, then formats them as ANSI escape sequences.
    ///
    /// # Arguments
    ///
    /// * `config` - The style configuration to compute from
    ///
    /// # Example
    ///
    /// ```
    /// use streamdown_config::{StyleConfig, ComputedStyle};
    ///
    /// let config = StyleConfig::default();
    /// let computed = ComputedStyle::from_config(&config);
    ///
    /// // Use the computed dark background
    /// let bg_escape = format!("\x1b[48;2;{}", computed.dark);
    /// ```
    pub fn from_config(config: &StyleConfig) -> Self {
        let (base_h, base_s, base_v) = config.base_hsv();

        // Compute all colors by applying multipliers
        let dark = apply_hsv_multiplier(base_h, base_s, base_v, &config.dark);
        let mid = apply_hsv_multiplier(base_h, base_s, base_v, &config.mid);
        let symbol = apply_hsv_multiplier(base_h, base_s, base_v, &config.symbol);
        let head = apply_hsv_multiplier(base_h, base_s, base_v, &config.head);
        let grey = apply_hsv_multiplier(base_h, base_s, base_v, &config.grey);
        let bright = apply_hsv_multiplier(base_h, base_s, base_v, &config.bright);

        // Pre-compute full ANSI sequences
        let dark_fg = format!("\x1b[38;2;{}", dark);
        let dark_bg = format!("\x1b[48;2;{}", dark);
        let mid_fg = format!("\x1b[38;2;{}", mid);
        let symbol_fg = format!("\x1b[38;2;{}", symbol);
        let head_fg = format!("\x1b[38;2;{}", head);
        let grey_fg = format!("\x1b[38;2;{}", grey);
        let bright_fg = format!("\x1b[38;2;{}", bright);

        // Margin spaces
        let margin_spaces = " ".repeat(config.margin);

        // List indent
        let list_indent = " ".repeat(config.list_indent);

        // Block quote with grey bar
        let blockquote = format!("{}│\x1b[0m ", grey_fg);

        // Code background
        let codebg = dark_bg.clone();

        // Link color (using bright)
        let link = bright_fg.clone();

        // Code padding characters
        let codepad = if config.pretty_pad {
            // Use box drawing characters for pretty padding
            (
                format!("{}▌\x1b[0m", grey_fg), // Left half block
                format!("{}▐\x1b[0m", grey_fg), // Right half block
            )
        } else {
            (String::new(), String::new())
        };

        Self {
            dark,
            mid,
            symbol,
            head,
            grey,
            bright,
            margin_spaces,
            blockquote,
            codebg,
            link,
            codepad,
            list_indent,
            dark_fg,
            dark_bg,
            mid_fg,
            symbol_fg,
            head_fg,
            grey_fg,
            bright_fg,
        }
    }

    /// Get the foreground ANSI escape for a specific style.
    pub fn fg(&self, name: &str) -> &str {
        match name {
            "dark" => &self.dark_fg,
            "mid" => &self.mid_fg,
            "symbol" => &self.symbol_fg,
            "head" => &self.head_fg,
            "grey" => &self.grey_fg,
            "bright" => &self.bright_fg,
            _ => "",
        }
    }

    /// Get the background ANSI escape for a specific style.
    pub fn bg(&self, name: &str) -> &str {
        match name {
            "dark" => &self.dark_bg,
            _ => "",
        }
    }

    /// Format text with a specific foreground style.
    pub fn style_fg(&self, name: &str, text: &str) -> String {
        format!("{}{}\x1b[0m", self.fg(name), text)
    }

    /// Create a heading line with the head color.
    pub fn heading(&self, level: u8, text: &str) -> String {
        let prefix = "#".repeat(level as usize);
        format!("{}{} {}\x1b[0m", self.head_fg, prefix, text)
    }

    /// Create a code block start line.
    pub fn code_start(&self, language: Option<&str>, width: usize) -> String {
        let (left, _right) = &self.codepad;
        let lang_display = language.unwrap_or("");
        let inner_width = width.saturating_sub(2); // Account for padding chars

        if !left.is_empty() {
            format!(
                "{}{}─{}{}\x1b[0m",
                left, self.dark_bg, lang_display, "\x1b[0m"
            )
        } else {
            format!("{}{}", self.dark_bg, "─".repeat(inner_width))
        }
    }

    /// Create a blockquote line.
    pub fn quote(&self, text: &str, depth: usize) -> String {
        let prefix = self.blockquote.repeat(depth);
        format!("{}{}", prefix, text)
    }

    /// Create a list bullet.
    pub fn bullet(&self, indent: usize) -> String {
        let spaces = " ".repeat(indent * 2);
        format!("{}{}•\x1b[0m ", spaces, self.symbol_fg)
    }

    /// Create an ordered list number.
    pub fn list_number(&self, indent: usize, num: usize) -> String {
        let spaces = " ".repeat(indent * 2);
        format!("{}{}{}.\x1b[0m ", spaces, self.symbol_fg, num)
    }
}

/// Apply HSV multiplier to base HSV values and return ANSI RGB string.
///
/// # Arguments
///
/// * `h` - Base hue (0..360)
/// * `s` - Base saturation (0..1)
/// * `v` - Base value (0..1)
/// * `multiplier` - The HSV multiplier to apply
///
/// # Returns
///
/// String in format "r;g;bm" ready for ANSI escape sequences.
fn apply_hsv_multiplier(h: f64, s: f64, v: f64, multiplier: &HsvMultiplier) -> String {
    // Apply multipliers with clamping
    let new_h = (h * multiplier.h) % 360.0;
    let new_s = (s * multiplier.s).clamp(0.0, 1.0);
    let new_v = (v * multiplier.v).clamp(0.0, 1.0);

    // Convert to RGB
    let (r, g, b) = hsv_to_rgb(new_h, new_s, new_v);

    format!("{};{};{}m", r, g, b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_config_default() {
        let config = StyleConfig::default();
        let computed = ComputedStyle::from_config(&config);

        // Verify format of color strings
        assert!(computed.dark.ends_with('m'));
        assert!(computed.dark.contains(';'));
        assert!(computed.mid.ends_with('m'));
        assert!(computed.bright.ends_with('m'));

        // Verify margin spaces
        assert_eq!(computed.margin_spaces, "  ");

        // Verify list indent
        assert_eq!(computed.list_indent, "  ");

        // Verify codepad is set (pretty_pad is true by default)
        assert!(!computed.codepad.0.is_empty());
        assert!(!computed.codepad.1.is_empty());
    }

    #[test]
    fn test_from_config_no_pretty_pad() {
        let config = StyleConfig {
            pretty_pad: false,
            ..Default::default()
        };
        let computed = ComputedStyle::from_config(&config);

        assert!(computed.codepad.0.is_empty());
        assert!(computed.codepad.1.is_empty());
    }

    #[test]
    fn test_apply_hsv_multiplier() {
        // Base HSV: 288°, 0.5 saturation, 0.5 value (purple)
        let result = apply_hsv_multiplier(288.0, 0.5, 0.5, &HsvMultiplier::new(1.0, 1.0, 1.0));

        // Should be some valid RGB format
        assert!(result.ends_with('m'));
        let parts: Vec<&str> = result.trim_end_matches('m').split(';').collect();
        assert_eq!(parts.len(), 3);

        // All parts should be valid u8 values
        for part in parts {
            let _val: u8 = part.parse().unwrap();
        }
    }

    #[test]
    fn test_dark_is_actually_dark() {
        let config = StyleConfig::default();
        let computed = ComputedStyle::from_config(&config);

        // Parse the RGB values from dark
        let parts: Vec<u8> = computed
            .dark
            .trim_end_matches('m')
            .split(';')
            .map(|s| s.parse().unwrap())
            .collect();

        // With V multiplier of 0.25, values should be low
        let avg = (parts[0] as u32 + parts[1] as u32 + parts[2] as u32) / 3;
        assert!(avg < 100, "Dark should be dark, got avg brightness {}", avg);
    }

    #[test]
    fn test_bright_is_actually_bright() {
        let config = StyleConfig::default();
        let computed = ComputedStyle::from_config(&config);

        // Parse the RGB values from bright
        let parts: Vec<u8> = computed
            .bright
            .trim_end_matches('m')
            .split(';')
            .map(|s| s.parse().unwrap())
            .collect();

        // With V multiplier of 2.0 (clamped to 1.0), at least one value should be high
        let max = parts.iter().max().unwrap();
        assert!(*max > 150, "Bright should be bright, got max {}", max);
    }

    #[test]
    fn test_fg_method() {
        let config = StyleConfig::default();
        let computed = ComputedStyle::from_config(&config);

        assert!(computed.fg("dark").starts_with("\x1b[38;2;"));
        assert!(computed.fg("bright").starts_with("\x1b[38;2;"));
        assert!(computed.fg("unknown").is_empty());
    }

    #[test]
    fn test_style_fg() {
        let config = StyleConfig::default();
        let computed = ComputedStyle::from_config(&config);

        let styled = computed.style_fg("head", "Hello");
        assert!(styled.starts_with("\x1b[38;2;"));
        assert!(styled.contains("Hello"));
        assert!(styled.ends_with("\x1b[0m"));
    }

    #[test]
    fn test_heading() {
        let config = StyleConfig::default();
        let computed = ComputedStyle::from_config(&config);

        let h1 = computed.heading(1, "Title");
        assert!(h1.contains("# Title"));
        assert!(h1.ends_with("\x1b[0m"));

        let h3 = computed.heading(3, "Section");
        assert!(h3.contains("### Section"));
    }

    #[test]
    fn test_bullet() {
        let config = StyleConfig::default();
        let computed = ComputedStyle::from_config(&config);

        let bullet = computed.bullet(0);
        assert!(bullet.contains("•"));

        let indented = computed.bullet(2);
        assert!(indented.starts_with("    ")); // 2 * 2 spaces
    }

    #[test]
    fn test_list_number() {
        let config = StyleConfig::default();
        let computed = ComputedStyle::from_config(&config);

        let num = computed.list_number(0, 1);
        assert!(num.contains("1."));

        let num5 = computed.list_number(1, 5);
        assert!(num5.contains("5."));
        assert!(num5.starts_with("  ")); // 1 * 2 spaces
    }

    #[test]
    fn test_quote() {
        let config = StyleConfig::default();
        let computed = ComputedStyle::from_config(&config);

        let quote = computed.quote("Hello", 1);
        assert!(quote.contains("│"));
        assert!(quote.contains("Hello"));

        let nested = computed.quote("Nested", 2);
        // Should have two quote prefixes
        assert!(nested.matches('│').count() >= 2);
    }
}
