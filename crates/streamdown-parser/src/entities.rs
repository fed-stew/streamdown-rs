//! HTML entity decoding

use std::collections::HashMap;
use std::sync::LazyLock;

/// Common HTML entities mapping
static HTML_ENTITIES: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    let mut m = HashMap::new();
    // Copyright, trademark, registered
    m.insert("&copy;", "©");
    m.insert("&trade;", "™");
    m.insert("&reg;", "®");
    // Common symbols
    m.insert("&amp;", "&");
    m.insert("&lt;", "<");
    m.insert("&gt;", ">");
    m.insert("&quot;", "\"");
    m.insert("&apos;", "'");
    m.insert("&nbsp;", " ");
    // Dashes and spaces
    m.insert("&mdash;", "—");
    m.insert("&ndash;", "–");
    m.insert("&hellip;", "…");
    // Arrows
    m.insert("&larr;", "←");
    m.insert("&rarr;", "→");
    m.insert("&uarr;", "↑");
    m.insert("&darr;", "↓");
    // Math
    m.insert("&times;", "×");
    m.insert("&divide;", "÷");
    m.insert("&plusmn;", "±");
    m.insert("&ne;", "≠");
    m.insert("&le;", "≤");
    m.insert("&ge;", "≥");
    m.insert("&infin;", "∞");
    // Currency
    m.insert("&euro;", "€");
    m.insert("&pound;", "£");
    m.insert("&yen;", "¥");
    m.insert("&cent;", "¢");
    // Other common
    m.insert("&deg;", "°");
    m.insert("&para;", "¶");
    m.insert("&sect;", "§");
    m.insert("&bull;", "•");
    m.insert("&middot;", "·");
    m.insert("&laquo;", "«");
    m.insert("&raquo;", "»");
    m.insert("&dagger;", "†");
    m.insert("&Dagger;", "‡");
    m.insert("&permil;", "‰");
    m.insert("&prime;", "′");
    m.insert("&Prime;", "″");
    m
});

/// Decode HTML entities in a string
pub fn decode_html_entities(text: &str) -> String {
    let mut result = text.to_string();

    // Replace named entities
    for (entity, replacement) in HTML_ENTITIES.iter() {
        result = result.replace(entity, replacement);
    }

    // Handle numeric entities like &#169; or &#x00A9;
    // Decimal: &#123;
    while let Some(start) = result.find("&#") {
        if let Some(end) = result[start..].find(';') {
            let entity = &result[start..start + end + 1];
            let num_str = &entity[2..entity.len() - 1];

            let codepoint = if num_str.starts_with('x') || num_str.starts_with('X') {
                // Hex: &#x00A9;
                u32::from_str_radix(&num_str[1..], 16).ok()
            } else {
                // Decimal: &#169;
                num_str.parse::<u32>().ok()
            };

            if let Some(cp) = codepoint
                && let Some(c) = char::from_u32(cp)
            {
                result = result.replace(entity, &c.to_string());
                continue;
            }
        }
        // If we couldn't parse it, break to avoid infinite loop
        break;
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_named_entities() {
        assert_eq!(decode_html_entities("&copy;"), "©");
        assert_eq!(decode_html_entities("&trade;"), "™");
        assert_eq!(decode_html_entities("&reg;"), "®");
        assert_eq!(decode_html_entities("&amp;"), "&");
    }

    #[test]
    fn test_numeric_entities() {
        assert_eq!(decode_html_entities("&#169;"), "©");
        assert_eq!(decode_html_entities("&#x00A9;"), "©");
    }

    #[test]
    fn test_mixed() {
        assert_eq!(
            decode_html_entities("Copyright &copy; 2024"),
            "Copyright © 2024"
        );
    }
}
