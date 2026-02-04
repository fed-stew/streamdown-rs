//! Colodore color palette and resolution.
//!
//! Provides named color presets based on the Colodore palette
//! (Commodore 64/128 inspired colors by Pepto).

use std::collections::HashMap;
use std::sync::LazyLock;

/// Colodore color palette - Commodore 64/128 inspired colors.
///
/// Source: <https://lospec.com/palette-list/colodore>
pub static COLODORE: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    let mut m = HashMap::new();
    m.insert("black", "#000000");
    m.insert("dark_grey", "#4a4a4a");
    m.insert("grey", "#7b7b7b");
    m.insert("light_grey", "#b2b2b2");
    m.insert("white", "#ffffff");
    m.insert("dark_red", "#813338");
    m.insert("red", "#c46c71");
    m.insert("brown", "#553800");
    m.insert("orange", "#8e5029");
    m.insert("yellow", "#edf171");
    m.insert("light_green", "#a9ff9f");
    m.insert("green", "#56ac4d");
    m.insert("cyan", "#75cec8");
    m.insert("light_blue", "#706deb");
    m.insert("blue", "#2e2c9b");
    m.insert("purple", "#8e3c97");
    m
});

/// Resolve a color string to hex.
///
/// If input matches a Colodore preset name, returns the preset hex value.
/// Otherwise, returns the input unchanged (treated as hex passthrough).
pub fn resolve_color(color: &str) -> &str {
    COLODORE.get(color).copied().unwrap_or(color)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_preset() {
        assert_eq!(resolve_color("yellow"), "#edf171");
        assert_eq!(resolve_color("cyan"), "#75cec8");
        assert_eq!(resolve_color("black"), "#000000");
    }

    #[test]
    fn test_resolve_hex_passthrough() {
        assert_eq!(resolve_color("#ff0000"), "#ff0000");
        assert_eq!(resolve_color("#123456"), "#123456");
    }

    #[test]
    fn test_resolve_unknown() {
        assert_eq!(resolve_color("not_a_color"), "not_a_color");
    }

    #[test]
    fn test_all_colodore_colors() {
        assert_eq!(COLODORE.len(), 16);
        assert!(COLODORE.contains_key("black"));
        assert!(COLODORE.contains_key("white"));
        assert!(COLODORE.contains_key("purple"));
    }
}
