//! HSV/RGB color manipulation utilities.
//!
//! This module provides functions for color space conversions
//! and style multiplier application.

use std::collections::HashMap;

/// Parse an ANSI color code and convert to hex string.
///
/// Expects a color code in the format "r;g;b" or full ANSI like "\x1b[38;2;r;g;bm".
///
/// # Arguments
///
/// * `ansi_code` - An ANSI color code string
///
/// # Returns
///
/// A hex color string like "#ff0000" for red.
///
/// # Example
///
/// ```
/// use streamdown_ansi::color::ansi2hex;
/// assert_eq!(ansi2hex("255;128;0"), Some("#ff8000".to_string()));
/// ```
pub fn ansi2hex(ansi_code: &str) -> Option<String> {
    // Strip ANSI escape prefix/suffix if present
    let code = ansi_code
        .trim_start_matches("\x1b[")
        .trim_start_matches("38;2;")
        .trim_start_matches("48;2;")
        .trim_end_matches('m');

    let parts: Vec<&str> = code.split(';').collect();
    if parts.len() >= 3 {
        let r: u8 = parts[0].parse().ok()?;
        let g: u8 = parts[1].parse().ok()?;
        let b: u8 = parts[2].parse().ok()?;
        Some(format!("#{:02x}{:02x}{:02x}", r, g, b))
    } else {
        None
    }
}

/// Parse a hex color string to RGB components.
///
/// # Arguments
///
/// * `hex` - A hex color string like "#ff0000" or "ff0000"
///
/// # Returns
///
/// RGB tuple (r, g, b) with values 0-255.
///
/// # Example
///
/// ```
/// use streamdown_ansi::color::hex2rgb;
/// assert_eq!(hex2rgb("#ff8000"), Some((255, 128, 0)));
/// ```
pub fn hex2rgb(hex: &str) -> Option<(u8, u8, u8)> {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }

    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;

    Some((r, g, b))
}

/// Convert RGB to HSV color space.
///
/// # Arguments
///
/// * `r` - Red component (0-255)
/// * `g` - Green component (0-255)
/// * `b` - Blue component (0-255)
///
/// # Returns
///
/// HSV tuple (h, s, v) with h in 0.0..360.0 and s, v in 0.0..1.0.
///
/// # Example
///
/// ```
/// use streamdown_ansi::color::rgb_to_hsv;
/// let (h, s, v) = rgb_to_hsv(255, 0, 0);
/// assert!((h - 0.0).abs() < 0.01); // Red has hue 0
/// assert!((s - 1.0).abs() < 0.01); // Full saturation
/// assert!((v - 1.0).abs() < 0.01); // Full value
/// ```
pub fn rgb_to_hsv(r: u8, g: u8, b: u8) -> (f64, f64, f64) {
    let r = r as f64 / 255.0;
    let g = g as f64 / 255.0;
    let b = b as f64 / 255.0;

    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;

    let v = max;

    let s = if max == 0.0 { 0.0 } else { delta / max };

    let h = if delta == 0.0 {
        0.0
    } else if max == r {
        60.0 * (((g - b) / delta) % 6.0)
    } else if max == g {
        60.0 * (((b - r) / delta) + 2.0)
    } else {
        60.0 * (((r - g) / delta) + 4.0)
    };

    let h = if h < 0.0 { h + 360.0 } else { h };

    (h, s, v)
}

/// Convert HSV to RGB color space.
///
/// # Arguments
///
/// * `h` - Hue (0.0..360.0)
/// * `s` - Saturation (0.0..1.0)
/// * `v` - Value (0.0..1.0)
///
/// # Returns
///
/// RGB tuple (r, g, b) with values 0-255.
///
/// # Example
///
/// ```
/// use streamdown_ansi::color::hsv_to_rgb;
/// let (r, g, b) = hsv_to_rgb(0.0, 1.0, 1.0);
/// assert_eq!((r, g, b), (255, 0, 0)); // Red
/// ```
pub fn hsv_to_rgb(h: f64, s: f64, v: f64) -> (u8, u8, u8) {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;

    let (r1, g1, b1) = if h < 60.0 {
        (c, x, 0.0)
    } else if h < 120.0 {
        (x, c, 0.0)
    } else if h < 180.0 {
        (0.0, c, x)
    } else if h < 240.0 {
        (0.0, x, c)
    } else if h < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };

    let r = ((r1 + m) * 255.0).round() as u8;
    let g = ((g1 + m) * 255.0).round() as u8;
    let b = ((b1 + m) * 255.0).round() as u8;

    (r, g, b)
}

/// Style multipliers for HSV adjustment.
#[derive(Debug, Clone, Default)]
pub struct HsvMultiplier {
    /// Hue multiplier
    pub h: f64,
    /// Saturation multiplier
    pub s: f64,
    /// Value multiplier
    pub v: f64,
}

impl HsvMultiplier {
    /// Create a new HSV multiplier with default values (1.0).
    pub fn new() -> Self {
        Self {
            h: 1.0,
            s: 1.0,
            v: 1.0,
        }
    }

    /// Create with specific multipliers.
    pub fn with_values(h: f64, s: f64, v: f64) -> Self {
        Self { h, s, v }
    }
}

/// Apply HSV multipliers to create an ANSI color code suffix.
///
/// This function takes HSV values, applies multipliers from a style map,
/// and returns the RGB values formatted for ANSI escape sequences.
///
/// # Arguments
///
/// * `style` - HashMap of style names to HSV multipliers
/// * `name` - The style name to look up
/// * `h` - Base hue (0.0..360.0)
/// * `s` - Base saturation (0.0..1.0)
/// * `v` - Base value (0.0..1.0)
///
/// # Returns
///
/// A string like "255;128;64m" ready to append to ANSI FG/BG prefix.
///
/// # Example
///
/// ```
/// use streamdown_ansi::color::{apply_multipliers, HsvMultiplier};
/// use std::collections::HashMap;
///
/// let mut styles = HashMap::new();
/// styles.insert("highlight".to_string(), HsvMultiplier::with_values(1.0, 1.2, 0.9));
///
/// let result = apply_multipliers(&styles, "highlight", 0.0, 1.0, 1.0);
/// assert!(result.ends_with('m'));
/// ```
pub fn apply_multipliers(
    style: &HashMap<String, HsvMultiplier>,
    name: &str,
    h: f64,
    s: f64,
    v: f64,
) -> String {
    let multiplier = style.get(name).cloned().unwrap_or_else(HsvMultiplier::new);

    // Apply multipliers, clamping to valid ranges
    let new_h = (h * multiplier.h) % 360.0;
    let new_s = (s * multiplier.s).min(1.0);
    let new_v = (v * multiplier.v).min(1.0);

    let (r, g, b) = hsv_to_rgb(new_h, new_s, new_v);

    format!("{};{};{}m", r, g, b)
}

/// Create an ANSI foreground color from HSV with multipliers.
pub fn fg_from_hsv(
    style: &HashMap<String, HsvMultiplier>,
    name: &str,
    h: f64,
    s: f64,
    v: f64,
) -> String {
    format!("\x1b[38;2;{}", apply_multipliers(style, name, h, s, v))
}

/// Create an ANSI background color from HSV with multipliers.
pub fn bg_from_hsv(
    style: &HashMap<String, HsvMultiplier>,
    name: &str,
    h: f64,
    s: f64,
    v: f64,
) -> String {
    format!("\x1b[48;2;{}", apply_multipliers(style, name, h, s, v))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ansi2hex() {
        assert_eq!(ansi2hex("255;0;0"), Some("#ff0000".to_string()));
        assert_eq!(ansi2hex("0;255;0"), Some("#00ff00".to_string()));
        assert_eq!(ansi2hex("0;0;255"), Some("#0000ff".to_string()));
        assert_eq!(ansi2hex("255;128;64"), Some("#ff8040".to_string()));
    }

    #[test]
    fn test_ansi2hex_with_escape() {
        assert_eq!(ansi2hex("\x1b[38;2;255;0;0m"), Some("#ff0000".to_string()));
    }

    #[test]
    fn test_hex2rgb() {
        assert_eq!(hex2rgb("#ff0000"), Some((255, 0, 0)));
        assert_eq!(hex2rgb("00ff00"), Some((0, 255, 0)));
        assert_eq!(hex2rgb("#0000ff"), Some((0, 0, 255)));
    }

    #[test]
    fn test_rgb_hsv_roundtrip() {
        // Test pure red
        let (h, s, v) = rgb_to_hsv(255, 0, 0);
        let (r, g, b) = hsv_to_rgb(h, s, v);
        assert_eq!((r, g, b), (255, 0, 0));

        // Test pure green
        let (h, s, v) = rgb_to_hsv(0, 255, 0);
        let (r, g, b) = hsv_to_rgb(h, s, v);
        assert_eq!((r, g, b), (0, 255, 0));

        // Test pure blue
        let (h, s, v) = rgb_to_hsv(0, 0, 255);
        let (r, g, b) = hsv_to_rgb(h, s, v);
        assert_eq!((r, g, b), (0, 0, 255));
    }

    #[test]
    fn test_apply_multipliers() {
        let mut styles = HashMap::new();
        styles.insert(
            "test".to_string(),
            HsvMultiplier::with_values(1.0, 1.0, 1.0),
        );

        let result = apply_multipliers(&styles, "test", 0.0, 1.0, 1.0);
        assert_eq!(result, "255;0;0m"); // Pure red
    }
}
