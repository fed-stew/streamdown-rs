//! Streamdown Plugin System
//!
//! This crate provides the plugin architecture for extending streamdown
//! with custom content processors. Plugins can intercept lines of input
//! and transform them before normal markdown processing.
//!
//! # Plugin Behavior
//!
//! - If a plugin returns `None`, it's not interested in the line
//! - If a plugin returns `Some(ProcessResult::Lines(vec))`, those lines are emitted
//! - If a plugin returns `Some(ProcessResult::Continue)`, it's buffering input
//! - A plugin that returns non-None gets priority until it returns None
//!
//! # Example
//!
//! ```
//! use streamdown_plugin::{Plugin, ProcessResult, PluginManager};
//! use streamdown_core::state::ParseState;
//! use streamdown_config::ComputedStyle;
//!
//! struct EchoPlugin;
//!
//! impl Plugin for EchoPlugin {
//!     fn name(&self) -> &str { "echo" }
//!
//!     fn process_line(
//!         &mut self,
//!         line: &str,
//!         _state: &ParseState,
//!         _style: &ComputedStyle,
//!     ) -> Option<ProcessResult> {
//!         if line.starts_with("!echo ") {
//!             Some(ProcessResult::Lines(vec![line[6..].to_string()]))
//!         } else {
//!             None
//!         }
//!     }
//!
//!     fn flush(&mut self) -> Option<Vec<String>> { None }
//!     fn reset(&mut self) {}
//! }
//! ```

pub mod builtin;
pub mod latex;

use streamdown_config::ComputedStyle;
use streamdown_core::state::ParseState;

/// Result of plugin processing.
#[derive(Debug, Clone, PartialEq)]
pub enum ProcessResult {
    /// Emit these formatted lines instead of normal processing
    Lines(Vec<String>),
    /// Plugin is buffering, continue without further processing
    Continue,
}

impl ProcessResult {
    /// Create a result with a single line.
    pub fn line(s: impl Into<String>) -> Self {
        Self::Lines(vec![s.into()])
    }

    /// Create a result with multiple lines.
    pub fn lines(lines: Vec<String>) -> Self {
        Self::Lines(lines)
    }

    /// Create a continue result.
    pub fn cont() -> Self {
        Self::Continue
    }
}

/// Plugin trait for custom content processors.
///
/// Plugins intercept input lines and can:
/// - Transform them into different output
/// - Buffer multiple lines before emitting
/// - Pass through to normal processing
pub trait Plugin: Send + Sync {
    /// Plugin name for identification and logging.
    fn name(&self) -> &str;

    /// Process a line of input.
    ///
    /// # Returns
    /// - `None`: Plugin not interested, continue normal processing
    /// - `Some(ProcessResult::Lines(vec))`: Emit these lines instead
    /// - `Some(ProcessResult::Continue)`: Plugin consumed input, keep buffering
    fn process_line(
        &mut self,
        line: &str,
        state: &ParseState,
        style: &ComputedStyle,
    ) -> Option<ProcessResult>;

    /// Called when stream ends to flush any buffered content.
    ///
    /// # Returns
    /// - `None`: Nothing to flush
    /// - `Some(vec)`: Remaining buffered lines to emit
    fn flush(&mut self) -> Option<Vec<String>>;

    /// Reset plugin state.
    ///
    /// Called when starting a new document or clearing state.
    fn reset(&mut self);

    /// Plugin priority (lower = higher priority).
    ///
    /// Default is 0. Plugins with lower priority numbers are called first.
    fn priority(&self) -> i32 {
        0
    }

    /// Whether this plugin is currently active (buffering).
    ///
    /// Active plugins get priority for subsequent lines.
    fn is_active(&self) -> bool {
        false
    }
}

/// Plugin manager for registering and coordinating plugins.
///
/// The manager handles:
/// - Plugin registration with priority sorting
/// - Active plugin priority (a plugin processing multi-line content)
/// - Flushing all plugins at end of stream
#[derive(Default)]
pub struct PluginManager {
    /// Registered plugins (sorted by priority)
    plugins: Vec<Box<dyn Plugin>>,
    /// Index of the currently active plugin (has priority)
    active_plugin: Option<usize>,
}

impl PluginManager {
    /// Create a new empty plugin manager.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a plugin manager with built-in plugins.
    pub fn with_builtins() -> Self {
        let mut manager = Self::new();
        manager.register(Box::new(latex::LatexPlugin::new()));
        manager
    }

    /// Register a plugin.
    ///
    /// Plugins are sorted by priority after registration.
    pub fn register(&mut self, plugin: Box<dyn Plugin>) {
        self.plugins.push(plugin);
        self.plugins.sort_by_key(|p| p.priority());
    }

    /// Get the number of registered plugins.
    pub fn plugin_count(&self) -> usize {
        self.plugins.len()
    }

    /// Get plugin names.
    pub fn plugin_names(&self) -> Vec<&str> {
        self.plugins.iter().map(|p| p.name()).collect()
    }

    /// Process a line through registered plugins.
    ///
    /// # Returns
    /// - `None`: No plugin handled the line, continue normal processing
    /// - `Some(vec)`: Plugin produced these lines, skip normal processing
    pub fn process_line(
        &mut self,
        line: &str,
        state: &ParseState,
        style: &ComputedStyle,
    ) -> Option<Vec<String>> {
        // If there's an active plugin, give it priority
        if let Some(idx) = self.active_plugin {
            let plugin = &mut self.plugins[idx];
            match plugin.process_line(line, state, style) {
                Some(ProcessResult::Lines(lines)) => {
                    // Plugin finished, clear active
                    self.active_plugin = None;
                    return Some(lines);
                }
                Some(ProcessResult::Continue) => {
                    // Plugin still active
                    return Some(vec![]);
                }
                None => {
                    // Plugin released priority
                    self.active_plugin = None;
                }
            }
        }

        // Try each plugin in priority order
        for (idx, plugin) in self.plugins.iter_mut().enumerate() {
            match plugin.process_line(line, state, style) {
                Some(ProcessResult::Lines(lines)) => {
                    return Some(lines);
                }
                Some(ProcessResult::Continue) => {
                    // This plugin is now active
                    self.active_plugin = Some(idx);
                    return Some(vec![]);
                }
                None => continue,
            }
        }

        None
    }

    /// Flush all plugins at end of stream.
    ///
    /// Returns all remaining buffered content from all plugins.
    pub fn flush(&mut self) -> Vec<String> {
        let mut result = Vec::new();

        for plugin in &mut self.plugins {
            if let Some(lines) = plugin.flush() {
                result.extend(lines);
            }
        }

        self.active_plugin = None;
        result
    }

    /// Reset all plugins.
    pub fn reset(&mut self) {
        for plugin in &mut self.plugins {
            plugin.reset();
        }
        self.active_plugin = None;
    }

    /// Check if any plugin is currently active.
    pub fn has_active_plugin(&self) -> bool {
        self.active_plugin.is_some()
    }

    /// Get the name of the active plugin, if any.
    pub fn active_plugin_name(&self) -> Option<&str> {
        self.active_plugin.map(|idx| self.plugins[idx].name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Simple test plugin that echoes lines starting with "!echo "
    struct EchoPlugin;

    impl Plugin for EchoPlugin {
        fn name(&self) -> &str {
            "echo"
        }

        fn process_line(
            &mut self,
            line: &str,
            _state: &ParseState,
            _style: &ComputedStyle,
        ) -> Option<ProcessResult> {
            line.strip_prefix("!echo ")
                .map(|stripped| ProcessResult::Lines(vec![stripped.to_string()]))
        }

        fn flush(&mut self) -> Option<Vec<String>> {
            None
        }

        fn reset(&mut self) {}
    }

    /// Test plugin that buffers lines until "!end"
    struct BufferPlugin {
        buffer: Vec<String>,
        active: bool,
    }

    impl BufferPlugin {
        fn new() -> Self {
            Self {
                buffer: Vec::new(),
                active: false,
            }
        }
    }

    impl Plugin for BufferPlugin {
        fn name(&self) -> &str {
            "buffer"
        }

        fn process_line(
            &mut self,
            line: &str,
            _state: &ParseState,
            _style: &ComputedStyle,
        ) -> Option<ProcessResult> {
            if line == "!start" {
                self.active = true;
                self.buffer.clear();
                return Some(ProcessResult::Continue);
            }

            if !self.active {
                return None;
            }

            if line == "!end" {
                self.active = false;
                let result = std::mem::take(&mut self.buffer);
                return Some(ProcessResult::Lines(result));
            }

            self.buffer.push(line.to_string());
            Some(ProcessResult::Continue)
        }

        fn flush(&mut self) -> Option<Vec<String>> {
            if self.buffer.is_empty() {
                None
            } else {
                Some(std::mem::take(&mut self.buffer))
            }
        }

        fn reset(&mut self) {
            self.buffer.clear();
            self.active = false;
        }

        fn is_active(&self) -> bool {
            self.active
        }
    }

    fn default_state() -> ParseState {
        ParseState::new()
    }

    fn default_style() -> ComputedStyle {
        ComputedStyle::default()
    }

    #[test]
    fn test_process_result_constructors() {
        let r1 = ProcessResult::line("hello");
        assert_eq!(r1, ProcessResult::Lines(vec!["hello".to_string()]));

        let r2 = ProcessResult::lines(vec!["a".to_string(), "b".to_string()]);
        assert_eq!(
            r2,
            ProcessResult::Lines(vec!["a".to_string(), "b".to_string()])
        );

        let r3 = ProcessResult::cont();
        assert_eq!(r3, ProcessResult::Continue);
    }

    #[test]
    fn test_plugin_manager_new() {
        let manager = PluginManager::new();
        assert_eq!(manager.plugin_count(), 0);
        assert!(!manager.has_active_plugin());
    }

    #[test]
    fn test_plugin_manager_register() {
        let mut manager = PluginManager::new();
        manager.register(Box::new(EchoPlugin));
        assert_eq!(manager.plugin_count(), 1);
        assert_eq!(manager.plugin_names(), vec!["echo"]);
    }

    #[test]
    fn test_plugin_manager_with_builtins() {
        let manager = PluginManager::with_builtins();
        assert!(manager.plugin_count() >= 1);
        assert!(manager.plugin_names().contains(&"latex"));
    }

    #[test]
    fn test_echo_plugin() {
        let mut manager = PluginManager::new();
        manager.register(Box::new(EchoPlugin));

        let state = default_state();
        let style = default_style();

        // Echo plugin should handle this
        let result = manager.process_line("!echo hello world", &state, &style);
        assert_eq!(result, Some(vec!["hello world".to_string()]));

        // Echo plugin should not handle this
        let result = manager.process_line("normal line", &state, &style);
        assert_eq!(result, None);
    }

    #[test]
    fn test_buffer_plugin() {
        let mut manager = PluginManager::new();
        manager.register(Box::new(BufferPlugin::new()));

        let state = default_state();
        let style = default_style();

        // Start buffering
        let result = manager.process_line("!start", &state, &style);
        assert_eq!(result, Some(vec![]));
        assert!(manager.has_active_plugin());

        // Buffer lines
        let result = manager.process_line("line 1", &state, &style);
        assert_eq!(result, Some(vec![]));

        let result = manager.process_line("line 2", &state, &style);
        assert_eq!(result, Some(vec![]));

        // End buffering
        let result = manager.process_line("!end", &state, &style);
        assert_eq!(
            result,
            Some(vec!["line 1".to_string(), "line 2".to_string()])
        );
        assert!(!manager.has_active_plugin());
    }

    #[test]
    fn test_buffer_plugin_flush() {
        let mut manager = PluginManager::new();
        manager.register(Box::new(BufferPlugin::new()));

        let state = default_state();
        let style = default_style();

        // Start buffering without ending
        manager.process_line("!start", &state, &style);
        manager.process_line("line 1", &state, &style);
        manager.process_line("line 2", &state, &style);

        // Flush should return buffered content
        let result = manager.flush();
        assert_eq!(result, vec!["line 1".to_string(), "line 2".to_string()]);
    }

    #[test]
    fn test_plugin_reset() {
        let mut manager = PluginManager::new();
        manager.register(Box::new(BufferPlugin::new()));

        let state = default_state();
        let style = default_style();

        // Start buffering
        manager.process_line("!start", &state, &style);
        manager.process_line("line 1", &state, &style);
        assert!(manager.has_active_plugin());

        // Reset
        manager.reset();
        assert!(!manager.has_active_plugin());

        // Flush should return nothing after reset
        let result = manager.flush();
        assert!(result.is_empty());
    }

    #[test]
    fn test_active_plugin_name() {
        let mut manager = PluginManager::new();
        manager.register(Box::new(BufferPlugin::new()));

        let state = default_state();
        let style = default_style();

        assert_eq!(manager.active_plugin_name(), None);

        manager.process_line("!start", &state, &style);
        assert_eq!(manager.active_plugin_name(), Some("buffer"));

        manager.process_line("!end", &state, &style);
        assert_eq!(manager.active_plugin_name(), None);
    }
}
