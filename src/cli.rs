//! Command-line interface for Streamdown.
//!
//! Provides argument parsing with full feature parity to the Python version.

use clap::Parser;
use std::path::PathBuf;

/// Streamdown - A streaming markdown renderer for modern terminals.
///
/// Renders markdown with syntax highlighting, tables, and special formatting
/// optimized for streaming output from LLMs and other sources.
#[derive(Parser, Debug)]
#[command(
    name = "sd",
    author = "Streamdown Contributors",
    version,
    about = "A streaming markdown renderer for modern terminals",
    after_help = "Repository: https://github.com/fed-stew/streamdown-rs\n\n\
                  Examples:\n  \
                  cat README.md | sd\n  \
                  sd document.md\n  \
                  sd -w 100 -c theme.toml input.md\n  \
                  sd --exec 'ollama run llama3'"
)]
pub struct Cli {
    /// Input files to process (reads from stdin if not provided)
    #[arg(value_name = "FILE")]
    pub files: Vec<PathBuf>,

    /// Set the logging level (trace, debug, info, warn, error)
    #[arg(short = 'l', long = "loglevel", default_value = "warn")]
    pub log_level: String,

    /// Set the HSV base color: h,s,v (e.g., "0.6,0.5,0.5")
    #[arg(short = 'b', long = "base")]
    pub base: Option<String>,

    /// Use a custom config file or inline TOML
    #[arg(short = 'c', long = "config")]
    pub config: Option<String>,

    /// Set the output width (0 = auto-detect from terminal)
    #[arg(short = 'w', long = "width", default_value = "0")]
    pub width: u16,

    /// Wrap a program for proper streaming I/O handling
    #[arg(short = 'e', long = "exec", value_name = "CMD")]
    pub exec_cmd: Option<String>,

    /// PCRE regex for prompt detection (with --exec)
    #[arg(short = 'p', long = "prompt", default_value = r"^.*>\s+$")]
    pub prompt: String,

    /// Scrape code snippets to a directory
    #[arg(short = 's', long = "scrape", value_name = "DIR")]
    pub scrape: Option<PathBuf>,

    /// Disable syntax highlighting
    #[arg(long = "no-highlight")]
    pub no_highlight: bool,

    /// Disable pretty code block borders (use spaces instead)
    #[arg(long = "no-pretty-pad")]
    pub no_pretty_pad: bool,

    /// Enable code line wrapping (breaks copy-paste)
    #[arg(long = "pretty-broken")]
    pub pretty_broken: bool,

    /// Enable clipboard integration (OSC 52)
    #[arg(long = "clipboard")]
    pub clipboard: bool,

    /// Enable savebrace (save code to /tmp/savebrace)
    #[arg(long = "savebrace")]
    pub savebrace: bool,

    /// Show configuration paths and exit
    #[arg(long = "paths")]
    pub show_paths: bool,

    /// Syntax highlighting theme
    #[arg(long = "theme", default_value = "base16-ocean.dark")]
    pub theme: String,
}

impl Cli {
    /// Get the effective width (0 means auto-detect).
    pub fn effective_width(&self) -> usize {
        if self.width == 0 {
            // Auto-detect from terminal
            crossterm::terminal::size()
                .map(|(cols, _)| cols as usize)
                .unwrap_or(80)
        } else {
            self.width as usize
        }
    }

    /// Check if we should read from stdin.
    pub fn should_read_stdin(&self) -> bool {
        self.files.is_empty() && self.exec_cmd.is_none()
    }

    /// Parse HSV base color if provided.
    pub fn parse_base(&self) -> Option<(f32, f32, f32)> {
        self.base.as_ref().and_then(|b| {
            let parts: Vec<&str> = b.split(',').collect();
            if parts.len() == 3 {
                let h = parts[0].parse().ok()?;
                let s = parts[1].parse().ok()?;
                let v = parts[2].parse().ok()?;
                Some((h, s, v))
            } else {
                None
            }
        })
    }
}

/// Show paths information.
pub fn show_paths() {
    use streamdown_config::Config;

    let config_path = Config::config_path()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "(not found)".to_string());
    let log_dir = std::env::temp_dir()
        .join("sd")
        .join(std::env::var("UID").unwrap_or_else(|_| "unknown".to_string()));

    println!("paths:");
    println!("  config                {}", config_path);
    println!("  logs                  {}", log_dir.display());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_parse_default() {
        let cli = Cli::parse_from(["sd"]);
        assert!(cli.files.is_empty());
        assert_eq!(cli.width, 0);
        assert_eq!(cli.log_level, "warn");
        assert!(!cli.clipboard);
    }

    #[test]
    fn test_cli_parse_with_file() {
        let cli = Cli::parse_from(["sd", "test.md"]);
        assert_eq!(cli.files.len(), 1);
        assert_eq!(cli.files[0], PathBuf::from("test.md"));
    }

    #[test]
    fn test_cli_parse_with_options() {
        let cli = Cli::parse_from([
            "sd",
            "-w",
            "100",
            "-l",
            "debug",
            "--clipboard",
            "--savebrace",
            "file.md",
        ]);
        assert_eq!(cli.width, 100);
        assert_eq!(cli.log_level, "debug");
        assert!(cli.clipboard);
        assert!(cli.savebrace);
    }

    #[test]
    fn test_cli_parse_exec() {
        let cli = Cli::parse_from(["sd", "-e", "ollama run llama3", "-p", ">>> "]);
        assert_eq!(cli.exec_cmd, Some("ollama run llama3".to_string()));
        assert_eq!(cli.prompt, ">>> ");
    }

    #[test]
    fn test_cli_parse_base() {
        let cli = Cli::parse_from(["sd", "-b", "0.6,0.5,0.5"]);
        let base = cli.parse_base();
        assert!(base.is_some());
        let (h, s, v) = base.unwrap();
        assert!((h - 0.6).abs() < 0.01);
        assert!((s - 0.5).abs() < 0.01);
        assert!((v - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_should_read_stdin() {
        let cli = Cli::parse_from(["sd"]);
        assert!(cli.should_read_stdin());

        let cli = Cli::parse_from(["sd", "file.md"]);
        assert!(!cli.should_read_stdin());

        let cli = Cli::parse_from(["sd", "-e", "cmd"]);
        assert!(!cli.should_read_stdin());
    }
}
