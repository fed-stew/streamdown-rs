//! Terminal output sanitization utilities.
//!
//! This module provides functions to sanitize strings for safe terminal output,
//! preventing escape sequence injection attacks and validating URLs for hyperlinks.

/// Sanitize a string for safe terminal output.
///
/// Removes control characters except newline and tab.
/// This prevents escape sequence injection attacks where malicious
/// content could manipulate the terminal.
///
/// # Arguments
/// * `s` - The string to sanitize
///
/// # Returns
/// A new string with dangerous control characters removed.
///
/// # Example
/// ```
/// use streamdown_ansi::sanitize::sanitize_for_terminal;
///
/// let safe = sanitize_for_terminal("Hello\x1b[31mWorld");
/// assert_eq!(safe, "Hello[31mWorld"); // ESC removed
/// ```
pub fn sanitize_for_terminal(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
        .collect()
}

/// Sanitize a string while preserving valid ANSI SGR (color/style) sequences.
///
/// This is for content we've formatted ourselves but that includes user text.
/// It allows our ANSI sequences (\x1b[...m) but strips other escape sequences
/// and control characters.
///
/// # Arguments
/// * `s` - The string to sanitize
///
/// # Returns
/// A new string with dangerous sequences removed but SGR codes preserved.
///
/// # Example
/// ```
/// use streamdown_ansi::sanitize::sanitize_preserving_ansi;
///
/// // SGR (color) sequences are preserved
/// let s = sanitize_preserving_ansi("\x1b[31mRed\x1b[0m");
/// assert!(s.contains("\x1b[31m"));
///
/// // Other escape sequences are stripped
/// let s = sanitize_preserving_ansi("\x1b]0;title\x07"); // OSC title
/// assert!(!s.contains("\x1b"));
/// ```
pub fn sanitize_preserving_ansi(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // Check if this is a valid SGR sequence (\x1b[...m)
            if chars.peek() == Some(&'[') {
                let mut seq = String::from(c);
                seq.push(chars.next().unwrap()); // consume '['

                // Collect the sequence until we hit a letter
                let mut valid_sgr = true;
                while let Some(&next) = chars.peek() {
                    seq.push(chars.next().unwrap());
                    if next.is_ascii_alphabetic() {
                        // Only allow 'm' (SGR) sequences for safety
                        if next != 'm' {
                            valid_sgr = false;
                        }
                        break;
                    }
                    // SGR parameters should only contain digits and semicolons
                    if !next.is_ascii_digit() && next != ';' {
                        valid_sgr = false;
                    }
                }

                if valid_sgr {
                    result.push_str(&seq);
                }
                // Otherwise, skip the whole sequence
            }
            // Skip bare escape characters
        } else if c.is_control() && c != '\n' && c != '\t' {
            // Skip other control characters
        } else {
            result.push(c);
        }
    }
    result
}

/// Check if a URL is safe for OSC 8 hyperlinks.
///
/// A safe URL:
/// - Starts with a known safe scheme (http, https, mailto, file)
/// - Does not contain control characters or escape sequences
///
/// # Arguments
/// * `url` - The URL to validate
///
/// # Returns
/// `true` if the URL is safe for use in terminal hyperlinks.
///
/// # Example
/// ```
/// use streamdown_ansi::sanitize::is_safe_url;
///
/// assert!(is_safe_url("https://example.com"));
/// assert!(is_safe_url("mailto:user@example.com"));
/// assert!(!is_safe_url("javascript:alert(1)"));
/// assert!(!is_safe_url("https://evil.com\x1b]0;pwned\x07"));
/// ```
pub fn is_safe_url(url: &str) -> bool {
    // Must start with a safe scheme
    let lower = url.to_lowercase();
    let safe_scheme = lower.starts_with("http://")
        || lower.starts_with("https://")
        || lower.starts_with("mailto:")
        || lower.starts_with("file://");

    if !safe_scheme {
        return false;
    }

    // Must not contain escape sequences or control characters
    !url.chars().any(|c| c.is_control())
}

/// Sanitize a URL for OSC 8 output, returning None if unsafe.
///
/// # Arguments
/// * `url` - The URL to sanitize
///
/// # Returns
/// `Some(url)` if the URL is safe, `None` otherwise.
///
/// # Example
/// ```
/// use streamdown_ansi::sanitize::sanitize_url;
///
/// assert_eq!(sanitize_url("https://example.com"), Some("https://example.com".to_string()));
/// assert_eq!(sanitize_url("javascript:alert(1)"), None);
/// ```
pub fn sanitize_url(url: &str) -> Option<String> {
    if is_safe_url(url) {
        Some(url.to_string())
    } else {
        None
    }
}

/// Sanitize a file extension to prevent path traversal attacks.
///
/// - Only allows alphanumeric characters
/// - Limits length to 10 characters
/// - Converts to lowercase
///
/// # Arguments
/// * `ext` - The extension to sanitize
///
/// # Returns
/// A safe extension string.
///
/// # Example
/// ```
/// use streamdown_ansi::sanitize::sanitize_extension;
///
/// assert_eq!(sanitize_extension("rs"), "rs");
/// assert_eq!(sanitize_extension("../../../etc/passwd"), "etcpasswd");
/// assert_eq!(sanitize_extension("PYTHON"), "python");
/// ```
pub fn sanitize_extension(ext: &str) -> String {
    ext.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .take(10)
        .collect::<String>()
        .to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_for_terminal_removes_escape() {
        assert_eq!(
            sanitize_for_terminal("Hello\x1b[31mWorld"),
            "Hello[31mWorld"
        );
    }

    #[test]
    fn test_sanitize_for_terminal_preserves_newline() {
        assert_eq!(sanitize_for_terminal("Hello\nWorld"), "Hello\nWorld");
    }

    #[test]
    fn test_sanitize_for_terminal_preserves_tab() {
        assert_eq!(sanitize_for_terminal("Hello\tWorld"), "Hello\tWorld");
    }

    #[test]
    fn test_sanitize_for_terminal_removes_bell() {
        assert_eq!(sanitize_for_terminal("Hello\x07World"), "HelloWorld");
    }

    #[test]
    fn test_sanitize_preserving_ansi_keeps_sgr() {
        let input = "\x1b[31mRed\x1b[0m";
        let output = sanitize_preserving_ansi(input);
        assert_eq!(output, input);
    }

    #[test]
    fn test_sanitize_preserving_ansi_strips_osc() {
        // OSC title change sequence
        let input = "\x1b]0;evil title\x07";
        let output = sanitize_preserving_ansi(input);
        assert!(!output.contains("\x1b"));
        assert!(!output.contains("\x07"));
    }

    #[test]
    fn test_sanitize_preserving_ansi_strips_cursor_movement() {
        // Cursor movement (\x1b[H) should be stripped
        let input = "Hello\x1b[HWorld";
        let output = sanitize_preserving_ansi(input);
        assert!(!output.contains("\x1b"));
        assert!(output.contains("Hello"));
        assert!(output.contains("World"));
    }

    #[test]
    fn test_is_safe_url_https() {
        assert!(is_safe_url("https://example.com"));
        assert!(is_safe_url("https://example.com/path?query=1"));
    }

    #[test]
    fn test_is_safe_url_http() {
        assert!(is_safe_url("http://example.com"));
    }

    #[test]
    fn test_is_safe_url_mailto() {
        assert!(is_safe_url("mailto:user@example.com"));
    }

    #[test]
    fn test_is_safe_url_file() {
        assert!(is_safe_url("file:///path/to/file"));
    }

    #[test]
    fn test_is_safe_url_rejects_javascript() {
        assert!(!is_safe_url("javascript:alert(1)"));
    }

    #[test]
    fn test_is_safe_url_rejects_data() {
        assert!(!is_safe_url("data:text/html,<script>alert(1)</script>"));
    }

    #[test]
    fn test_is_safe_url_rejects_control_chars() {
        assert!(!is_safe_url("https://evil.com\x1b]0;pwned\x07"));
        assert!(!is_safe_url("https://evil.com\nHeader: injected"));
    }

    #[test]
    fn test_sanitize_url() {
        assert_eq!(
            sanitize_url("https://example.com"),
            Some("https://example.com".to_string())
        );
        assert_eq!(sanitize_url("javascript:alert(1)"), None);
    }

    #[test]
    fn test_sanitize_extension_normal() {
        assert_eq!(sanitize_extension("rs"), "rs");
        assert_eq!(sanitize_extension("py"), "py");
        assert_eq!(sanitize_extension("js"), "js");
    }

    #[test]
    fn test_sanitize_extension_path_traversal() {
        assert_eq!(sanitize_extension("../../../etc/passwd"), "etcpasswd");
        assert_eq!(sanitize_extension("..\\..\\windows"), "windows");
    }

    #[test]
    fn test_sanitize_extension_lowercase() {
        assert_eq!(sanitize_extension("PYTHON"), "python");
        assert_eq!(sanitize_extension("RuSt"), "rust");
    }

    #[test]
    fn test_sanitize_extension_length_limit() {
        assert_eq!(sanitize_extension("verylongextension"), "verylongex");
    }

    #[test]
    fn test_sanitize_extension_special_chars() {
        assert_eq!(sanitize_extension("a.b.c"), "abc");
        assert_eq!(sanitize_extension("test!@#$%"), "test");
    }
}
