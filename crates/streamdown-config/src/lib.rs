//! Streamdown Config
//!
//! This crate handles configuration loading and management
//! for streamdown, supporting TOML configuration files.
//!
//! # Overview
//!
//! Configuration is loaded from platform-specific locations:
//! - Linux: `~/.config/streamdown/config.toml`
//! - macOS: `~/Library/Application Support/streamdown/config.toml`
//! - Windows: `%APPDATA%\streamdown\config.toml`
//!
//! # Example
//!
//! ```no_run
//! use streamdown_config::Config;
//!
//! // Load config with defaults
//! let config = Config::load().unwrap();
//!
//! // Or load with an override file
//! let config = Config::load_with_override(Some("./custom.toml".as_ref())).unwrap();
//! ```

mod computed;
mod features;
mod style;

pub use computed::ComputedStyle;
pub use features::FeaturesConfig;
pub use style::{HsvMultiplier, StyleConfig};

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use streamdown_core::{Result, StreamdownError};

/// Default TOML configuration string.
///
/// This matches the Python implementation's default_toml exactly.
const DEFAULT_TOML: &str = r#"[features]
CodeSpaces = false
Clipboard  = true
Logging    = false
Timeout    = 0.1
Savebrace  = true
Images     = true
Links      = true

[style]
Margin          = 2
ListIndent      = 2
PrettyPad       = true
PrettyBroken    = true
Width           = 0
HSV     = [0.8, 0.5, 0.5]
Dark    = { H = 1.00, S = 1.50, V = 0.25 }
Mid     = { H = 1.00, S = 1.00, V = 0.50 }
Symbol  = { H = 1.00, S = 1.00, V = 1.50 }
Head    = { H = 1.00, S = 1.00, V = 1.75 }
Grey    = { H = 1.00, S = 0.25, V = 1.37 }
Bright  = { H = 1.00, S = 0.60, V = 2.00 }
Syntax  = "native"
"#;

/// Main configuration structure.
///
/// Contains all configuration sections for streamdown.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Feature flags configuration
    #[serde(default)]
    pub features: FeaturesConfig,

    /// Style configuration
    #[serde(default)]
    pub style: StyleConfig,
}

impl Default for Config {
    fn default() -> Self {
        // Parse the default TOML to ensure consistency
        toml::from_str(DEFAULT_TOML).expect("Default TOML should be valid")
    }
}

impl Config {
    /// Returns the default TOML configuration string.
    ///
    /// This can be used to show users the default config or
    /// to write a default config file.
    ///
    /// # Example
    ///
    /// ```
    /// use streamdown_config::Config;
    /// let toml = Config::default_toml();
    /// assert!(toml.contains("[features]"));
    /// assert!(toml.contains("[style]"));
    /// ```
    pub fn default_toml() -> &'static str {
        DEFAULT_TOML
    }

    /// Returns the platform-specific configuration file path.
    ///
    /// # Example
    ///
    /// ```
    /// use streamdown_config::Config;
    /// if let Some(path) = Config::config_path() {
    ///     println!("Config path: {}", path.display());
    /// }
    /// ```
    pub fn config_path() -> Option<PathBuf> {
        directories::ProjectDirs::from("", "", "streamdown")
            .map(|dirs| dirs.config_dir().join("config.toml"))
    }

    /// Returns the platform-specific configuration directory.
    pub fn config_dir() -> Option<PathBuf> {
        directories::ProjectDirs::from("", "", "streamdown")
            .map(|dirs| dirs.config_dir().to_path_buf())
    }

    /// Ensures the config file exists, creating it with defaults if not.
    ///
    /// This mirrors the Python `ensure_config_file` function.
    ///
    /// # Returns
    ///
    /// The path to the config file.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use streamdown_config::Config;
    /// let path = Config::ensure_config_file().unwrap();
    /// assert!(path.exists());
    /// ```
    pub fn ensure_config_file() -> Result<PathBuf> {
        let config_dir = Self::config_dir().ok_or_else(|| {
            StreamdownError::Config("Could not determine config directory".into())
        })?;

        // Create directory if it doesn't exist
        std::fs::create_dir_all(&config_dir)?;

        let config_path = config_dir.join("config.toml");

        // Create default config if file doesn't exist
        if !config_path.exists() {
            std::fs::write(&config_path, DEFAULT_TOML)?;
        }

        Ok(config_path)
    }

    /// Load configuration from the default platform-specific path.
    ///
    /// If no config file exists, returns the default configuration.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use streamdown_config::Config;
    /// let config = Config::load().unwrap();
    /// ```
    pub fn load() -> Result<Self> {
        if let Some(config_path) = Self::config_path()
            && config_path.exists()
        {
            let content = std::fs::read_to_string(&config_path)?;
            return toml::from_str(&content)
                .map_err(|e| StreamdownError::Config(format!("Parse error: {}", e)));
        }

        // Return defaults if no config found
        Ok(Self::default())
    }

    /// Load configuration from a specific path.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the TOML configuration file
    ///
    /// # Example
    ///
    /// ```no_run
    /// use streamdown_config::Config;
    /// use std::path::Path;
    /// let config = Config::load_from(Path::new("./config.toml")).unwrap();
    /// ```
    pub fn load_from(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        toml::from_str(&content).map_err(|e| {
            StreamdownError::Config(format!("Parse error in {}: {}", path.display(), e))
        })
    }

    /// Load configuration with an optional override file or string.
    ///
    /// This mirrors the Python `ensure_config_file` behavior:
    /// 1. Load the base config from the default location
    /// 2. If override_path is provided:
    ///    - If it's a path to an existing file, load and merge it
    ///    - Otherwise, treat it as a TOML string and parse it
    ///
    /// # Arguments
    ///
    /// * `override_config` - Optional path to override file or inline TOML string
    ///
    /// # Example
    ///
    /// ```no_run
    /// use streamdown_config::Config;
    ///
    /// // Load with file override
    /// let config = Config::load_with_override(Some("./custom.toml".as_ref())).unwrap();
    ///
    /// // Load with inline TOML override
    /// let config = Config::load_with_override(Some("[features]\nLinks = false".as_ref())).unwrap();
    /// ```
    pub fn load_with_override(override_config: Option<&str>) -> Result<Self> {
        // Start with base config
        let mut config = Self::load()?;

        // Apply override if provided
        if let Some(override_str) = override_config {
            let override_path = Path::new(override_str);

            let override_toml = if override_path.exists() {
                // It's a file path
                std::fs::read_to_string(override_path)?
            } else {
                // Treat as inline TOML
                override_str.to_string()
            };

            // Parse and merge
            let override_config: Config = toml::from_str(&override_toml)
                .map_err(|e| StreamdownError::Config(format!("Override parse error: {}", e)))?;

            config.merge(&override_config);
        }

        Ok(config)
    }

    /// Merge another config into this one.
    ///
    /// Values from `other` take precedence over values in `self`.
    /// This is used for applying CLI overrides or secondary config files.
    ///
    /// # Arguments
    ///
    /// * `other` - The config to merge from
    ///
    /// # Example
    ///
    /// ```
    /// use streamdown_config::Config;
    ///
    /// let mut base = Config::default();
    /// let override_config: Config = toml::from_str(r#"
    ///     [features]
    ///     Links = false
    /// "#).unwrap();
    ///
    /// base.merge(&override_config);
    /// assert!(!base.features.links);
    /// ```
    pub fn merge(&mut self, other: &Config) {
        self.features.merge(&other.features);
        self.style.merge(&other.style);
    }

    /// Save configuration to a file.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to save the configuration to
    pub fn save_to(&self, path: &Path) -> Result<()> {
        let toml_string = toml::to_string_pretty(self)
            .map_err(|e| StreamdownError::Config(format!("Serialization error: {}", e)))?;
        std::fs::write(path, toml_string)?;
        Ok(())
    }

    /// Compute the style values (ANSI codes) from this config.
    ///
    /// This applies the HSV multipliers to generate actual ANSI color codes.
    ///
    /// # Example
    ///
    /// ```
    /// use streamdown_config::Config;
    /// let config = Config::default();
    /// let computed = config.computed_style();
    /// assert!(!computed.dark.is_empty());
    /// ```
    pub fn computed_style(&self) -> ComputedStyle {
        ComputedStyle::from_config(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert!(config.features.links);
        assert!(config.features.images);
        assert!(!config.features.code_spaces);
        assert_eq!(config.style.margin, 2);
    }

    #[test]
    fn test_default_toml_parses() {
        let config: Config = toml::from_str(DEFAULT_TOML).unwrap();
        assert!(config.features.clipboard);
        assert_eq!(config.style.syntax, "native");
    }

    #[test]
    fn test_merge() {
        let mut base = Config::default();
        assert!(base.features.links);

        let override_toml = r#"
            [features]
            Links = false
            [style]
            Margin = 4
        "#;
        let override_config: Config = toml::from_str(override_toml).unwrap();

        base.merge(&override_config);
        assert!(!base.features.links);
        assert_eq!(base.style.margin, 4);
    }

    #[test]
    fn test_config_path() {
        // Just verify it returns something on most platforms
        let path = Config::config_path();
        // On CI/containers this might be None, so we just check it doesn't panic
        if let Some(p) = path {
            assert!(p.to_string_lossy().contains("streamdown"));
        }
    }

    #[test]
    fn test_computed_style() {
        let config = Config::default();
        let computed = config.computed_style();

        // Verify computed values are non-empty ANSI-like strings
        assert!(computed.dark.contains(';'));
        assert!(computed.mid.contains(';'));
        assert!(computed.margin_spaces.len() == config.style.margin);
    }

    #[test]
    fn test_roundtrip_serialization() {
        let config = Config::default();
        let toml_str = toml::to_string_pretty(&config).unwrap();
        let parsed: Config = toml::from_str(&toml_str).unwrap();

        assert_eq!(config.features.links, parsed.features.links);
        assert_eq!(config.style.margin, parsed.style.margin);
    }
}
