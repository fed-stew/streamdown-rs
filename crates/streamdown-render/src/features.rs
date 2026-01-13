//! Special features: clipboard integration, savebrace, terminal utilities.
//!
//! This module provides advanced features for streamdown rendering:
//!
//! - **Clipboard (OSC 52)**: Copy code blocks to system clipboard via terminal
//! - **Savebrace**: Save code blocks to a temp file for shell access
//! - **Terminal size**: Dynamic terminal width detection

use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::PathBuf;

/// OSC 52 clipboard operation.
///
/// Sends code to the terminal's clipboard using the OSC 52 escape sequence.
/// This works in many modern terminals (kitty, iTerm2, tmux, etc.)
///
/// # Arguments
/// * `code` - The code/text to copy to clipboard
/// * `writer` - Output writer (usually stdout)
///
/// # Example
/// ```ignore
/// use std::io::stdout;
/// use streamdown_render::features::copy_to_clipboard;
///
/// copy_to_clipboard("fn main() {}", &mut stdout()).unwrap();
/// ```
/// Maximum size for OSC 52 clipboard (50KB - terminal limit).
const MAX_CLIPBOARD_SIZE: usize = 50_000;

pub fn copy_to_clipboard<W: Write>(code: &str, writer: &mut W) -> io::Result<()> {
    use base64::{engine::general_purpose::STANDARD, Engine};

    // Check size limit to avoid terminal issues
    if code.len() > MAX_CLIPBOARD_SIZE {
        // Silently skip - code block too large for clipboard
        return Ok(());
    }

    let encoded = STANDARD.encode(code.as_bytes());

    // OSC 52: \033]52;c;<base64>\a
    // The 'c' means clipboard (as opposed to 'p' for primary selection)
    write!(writer, "\x1b]52;c;{}\x07", encoded)?;
    writer.flush()
}

/// Check if we're running in an interactive terminal.
///
/// Returns true if stdout is a TTY, which means we can use
/// terminal features like OSC 52 and dynamic width.
pub fn is_tty() -> bool {
    use std::io::IsTerminal;
    std::io::stdout().is_terminal()
}

/// Get the terminal size.
///
/// Returns (columns, rows) or a default of (80, 24) if detection fails.
pub fn terminal_size() -> (u16, u16) {
    crossterm::terminal::size().unwrap_or((80, 24))
}

/// Get the terminal width.
///
/// Returns the number of columns, or 80 as a fallback.
pub fn terminal_width() -> usize {
    let (cols, _) = terminal_size();
    cols as usize
}

/// Maximum savebrace file size (10MB).
const MAX_SAVEBRACE_SIZE: u64 = 10 * 1024 * 1024;

/// Savebrace: save code blocks to a temp file.
///
/// This allows shell scripts to access the most recent code block
/// via a well-known path. Each code block is separated by a null byte.
///
/// The file is automatically cleared when it exceeds 10MB to prevent
/// unbounded growth.
///
/// # Arguments
/// * `code` - The code to save
///
/// # Returns
/// The path to the savebrace file
pub fn savebrace(code: &str) -> io::Result<PathBuf> {
    let path = savebrace_path();

    // Security: Check for symlink attack before writing
    if path.exists() {
        let meta = std::fs::symlink_metadata(&path)?;
        if meta.file_type().is_symlink() {
            return Err(io::Error::other("Refusing to write to symlink"));
        }

        // Clear file if it's too large to prevent unbounded growth
        if meta.len() > MAX_SAVEBRACE_SIZE {
            let _ = std::fs::remove_file(&path);
        }
    }

    let mut file = OpenOptions::new().create(true).append(true).open(&path)?;

    // Write code followed by null byte separator
    file.write_all(code.as_bytes())?;
    file.write_all(b"\0")?;
    file.flush()?;

    Ok(path)
}

/// Get the path to the savebrace file.
pub fn savebrace_path() -> PathBuf {
    std::env::temp_dir().join("savebrace")
}

/// Clear the savebrace file.
pub fn savebrace_clear() -> io::Result<()> {
    let path = savebrace_path();
    if path.exists() {
        std::fs::remove_file(&path)?;
    }
    Ok(())
}

/// Read all code blocks from the savebrace file.
///
/// Returns a vector of code blocks, or empty if the file doesn't exist.
pub fn savebrace_read() -> io::Result<Vec<String>> {
    let path = savebrace_path();
    if !path.exists() {
        return Ok(Vec::new());
    }

    let content = std::fs::read_to_string(&path)?;
    let blocks: Vec<String> = content
        .split('\0')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();

    Ok(blocks)
}

/// Read the last code block from the savebrace file.
pub fn savebrace_last() -> io::Result<Option<String>> {
    let blocks = savebrace_read()?;
    Ok(blocks.into_iter().last())
}

/// Feature flags for rendering.
#[derive(Debug, Clone)]
pub struct RenderFeatures {
    /// Use ▄/▀ characters for code block borders (pretty but may not copy well)
    pub pretty_pad: bool,

    /// Wrap long code lines visually (pretty but breaks copy-paste)
    pub pretty_broken: bool,

    /// Enable clipboard integration (OSC 52)
    pub clipboard: bool,

    /// Enable savebrace (save code to temp file)
    pub savebrace: bool,

    /// Allow terminal to handle line wrapping
    pub width_wrap: bool,

    /// Fixed width (None = auto-detect from terminal)
    pub fixed_width: Option<usize>,

    /// Margin on each side
    pub margin: usize,
}

impl Default for RenderFeatures {
    fn default() -> Self {
        Self {
            pretty_pad: true,
            pretty_broken: false,
            clipboard: false,
            savebrace: false,
            width_wrap: true,
            fixed_width: None,
            margin: 1,
        }
    }
}

impl RenderFeatures {
    /// Create features optimized for visual appearance.
    pub fn pretty() -> Self {
        Self {
            pretty_pad: true,
            pretty_broken: true,
            ..Default::default()
        }
    }

    /// Create features optimized for copy-paste.
    pub fn copyable() -> Self {
        Self {
            pretty_pad: false,
            pretty_broken: false,
            ..Default::default()
        }
    }

    /// Calculate the effective width.
    pub fn effective_width(&self) -> usize {
        let base = self.fixed_width.unwrap_or_else(terminal_width);
        base.saturating_sub(self.margin * 2)
    }

    /// Calculate the full width (before margin).
    pub fn full_width(&self) -> usize {
        self.fixed_width.unwrap_or_else(terminal_width)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terminal_size() {
        let (cols, rows) = terminal_size();
        // Should return something reasonable
        assert!(cols > 0);
        assert!(rows > 0);
    }

    #[test]
    fn test_terminal_width() {
        let width = terminal_width();
        assert!(width > 0);
    }

    #[test]
    fn test_is_tty() {
        // In test environment, this might be false
        let _ = is_tty();
    }

    #[test]
    fn test_render_features_default() {
        let features = RenderFeatures::default();
        assert!(features.pretty_pad);
        assert!(!features.pretty_broken);
        assert!(!features.clipboard);
        assert!(!features.savebrace);
    }

    #[test]
    fn test_render_features_pretty() {
        let features = RenderFeatures::pretty();
        assert!(features.pretty_pad);
        assert!(features.pretty_broken);
    }

    #[test]
    fn test_render_features_copyable() {
        let features = RenderFeatures::copyable();
        assert!(!features.pretty_pad);
        assert!(!features.pretty_broken);
    }

    #[test]
    fn test_effective_width() {
        let features = RenderFeatures {
            fixed_width: Some(80),
            margin: 2,
            ..Default::default()
        };
        assert_eq!(features.effective_width(), 76); // 80 - 2*2
    }

    #[test]
    fn test_savebrace_path() {
        let path = savebrace_path();
        assert!(path.ends_with("savebrace"));
    }

    #[test]
    fn test_savebrace_write_read() {
        // Clean up first
        let _ = savebrace_clear();

        // Write some code
        savebrace("fn main() {}").unwrap();
        savebrace("let x = 1;").unwrap();

        // Read back
        let blocks = savebrace_read().unwrap();
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0], "fn main() {}");
        assert_eq!(blocks[1], "let x = 1;");

        // Get last
        let last = savebrace_last().unwrap();
        assert_eq!(last, Some("let x = 1;".to_string()));

        // Clean up
        savebrace_clear().unwrap();
    }

    #[test]
    fn test_copy_to_clipboard() {
        let mut output = Vec::new();
        copy_to_clipboard("test code", &mut output).unwrap();

        let result = String::from_utf8(output).unwrap();
        // Should contain OSC 52 sequence
        assert!(result.starts_with("\x1b]52;c;"));
        assert!(result.ends_with("\x07"));
        // Should contain base64 encoded "test code"
        assert!(result.contains("dGVzdCBjb2Rl")); // base64("test code")
    }
}
