//! Built-in plugins and plugin discovery.
//!
//! This module provides:
//! - A list of all built-in plugins
//! - Plugin discovery from configuration directory
//! - Plugin factory functions

use crate::{Plugin, latex::LatexPlugin};
use std::path::Path;

/// Get all built-in plugins.
///
/// Returns a vector of boxed plugins ready for registration.
pub fn builtin_plugins() -> Vec<Box<dyn Plugin>> {
    vec![Box::new(LatexPlugin::new())]
}

/// Plugin metadata.
#[derive(Debug, Clone)]
pub struct PluginInfo {
    /// Plugin name
    pub name: &'static str,
    /// Short description
    pub description: &'static str,
    /// Whether it's enabled by default
    pub default_enabled: bool,
    /// Priority (lower = higher priority)
    pub priority: i32,
}

/// Get information about all built-in plugins.
pub fn builtin_plugin_info() -> Vec<PluginInfo> {
    vec![PluginInfo {
        name: "latex",
        description: "Converts LaTeX math expressions ($$ or $) to Unicode",
        default_enabled: true,
        priority: 10,
    }]
}

/// Create a plugin by name.
///
/// # Returns
/// - `Some(plugin)` if the name matches a built-in plugin
/// - `None` if the name is not recognized
pub fn create_plugin(name: &str) -> Option<Box<dyn Plugin>> {
    match name {
        "latex" => Some(Box::new(LatexPlugin::new())),
        _ => None,
    }
}

/// Discover plugins from a directory.
///
/// Currently this is a placeholder for future dynamic plugin loading.
/// In the future, this could load:
/// - WASM plugins
/// - Shared library plugins
/// - Script-based plugins
///
/// # Arguments
/// * `_config_dir` - Path to the configuration directory
///
/// # Returns
/// Vector of discovered plugins (currently empty)
pub fn discover_plugins(_config_dir: &Path) -> Vec<Box<dyn Plugin>> {
    // TODO: Implement dynamic plugin loading
    // For now, return empty - all plugins must be built-in
    vec![]
}

/// Plugin filter for selective loading.
#[derive(Debug, Clone, Default)]
pub struct PluginFilter {
    /// Plugins to include (if empty, include all)
    pub include: Vec<String>,
    /// Plugins to exclude
    pub exclude: Vec<String>,
}

impl PluginFilter {
    /// Create a filter that includes all plugins.
    pub fn all() -> Self {
        Self::default()
    }

    /// Create a filter that includes no plugins.
    pub fn none() -> Self {
        Self {
            include: vec![],
            exclude: vec!["*".to_string()],
        }
    }

    /// Create a filter that only includes specific plugins.
    pub fn only(names: Vec<String>) -> Self {
        Self {
            include: names,
            exclude: vec![],
        }
    }

    /// Check if a plugin should be loaded.
    pub fn should_load(&self, name: &str) -> bool {
        // Check exclude list first
        if self.exclude.contains(&"*".to_string()) {
            return self.include.iter().any(|n| n == name);
        }
        if self.exclude.iter().any(|n| n == name) {
            return false;
        }

        // Check include list
        if self.include.is_empty() {
            return true;
        }
        self.include.iter().any(|n| n == name)
    }
}

/// Load built-in plugins with a filter.
pub fn load_builtin_plugins(filter: &PluginFilter) -> Vec<Box<dyn Plugin>> {
    builtin_plugin_info()
        .iter()
        .filter(|info| filter.should_load(info.name))
        .filter_map(|info| create_plugin(info.name))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_plugins() {
        let plugins = builtin_plugins();
        assert!(!plugins.is_empty());

        // Should have latex plugin
        let names: Vec<_> = plugins.iter().map(|p| p.name()).collect();
        assert!(names.contains(&"latex"));
    }

    #[test]
    fn test_builtin_plugin_info() {
        let info = builtin_plugin_info();
        assert!(!info.is_empty());

        let latex_info = info.iter().find(|i| i.name == "latex");
        assert!(latex_info.is_some());
        assert!(latex_info.unwrap().default_enabled);
    }

    #[test]
    fn test_create_plugin() {
        let latex = create_plugin("latex");
        assert!(latex.is_some());
        assert_eq!(latex.unwrap().name(), "latex");

        let unknown = create_plugin("unknown");
        assert!(unknown.is_none());
    }

    #[test]
    fn test_discover_plugins() {
        // Currently returns empty
        let plugins = discover_plugins(Path::new("/tmp"));
        assert!(plugins.is_empty());
    }

    #[test]
    fn test_plugin_filter_all() {
        let filter = PluginFilter::all();
        assert!(filter.should_load("latex"));
        assert!(filter.should_load("any"));
    }

    #[test]
    fn test_plugin_filter_none() {
        let filter = PluginFilter::none();
        assert!(!filter.should_load("latex"));
        assert!(!filter.should_load("any"));
    }

    #[test]
    fn test_plugin_filter_only() {
        let filter = PluginFilter::only(vec!["latex".to_string()]);
        assert!(filter.should_load("latex"));
        assert!(!filter.should_load("other"));
    }

    #[test]
    fn test_plugin_filter_exclude() {
        let mut filter = PluginFilter::all();
        filter.exclude.push("latex".to_string());
        assert!(!filter.should_load("latex"));
        assert!(filter.should_load("other"));
    }

    #[test]
    fn test_load_builtin_plugins() {
        let filter = PluginFilter::all();
        let plugins = load_builtin_plugins(&filter);
        assert!(!plugins.is_empty());

        let filter = PluginFilter::none();
        let plugins = load_builtin_plugins(&filter);
        assert!(plugins.is_empty());
    }
}
