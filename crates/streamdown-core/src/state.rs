//! Parse state for streaming markdown processing.
//!
//! The [`ParseState`] struct maintains all state needed to process
//! streaming markdown input incrementally.

use crate::enums::{BlockType, Code, EmitFlag, ListType};
use regex::Regex;
use serde::{Deserialize, Serialize};

/// Background reset ANSI code (used as default background).
pub const BGRESET: &str = "\x1b[49m";

/// Snapshot of current inline formatting states.
///
/// This is returned by [`ParseState::current()`] to capture
/// the current inline formatting context.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct InlineState {
    /// Whether inline code formatting is active
    pub inline_code: bool,
    /// Whether bold formatting is active
    pub in_bold: bool,
    /// Whether italic formatting is active
    pub in_italic: bool,
    /// Whether underline formatting is active
    pub in_underline: bool,
    /// Whether strikeout formatting is active
    pub in_strikeout: bool,
}

/// Main parse state for streaming markdown processing.
///
/// This struct maintains all the state needed to incrementally parse
/// markdown as it streams in. It tracks formatting states, list contexts,
/// code blocks, tables, and various configuration options.
///
/// # Example
///
/// ```
/// use streamdown_core::ParseState;
///
/// let mut state = ParseState::new();
/// state.set_width(80);
/// ```
#[derive(Debug, Clone)]
pub struct ParseState {
    // === Input buffer state ===
    /// Raw byte buffer for incomplete UTF-8 sequences
    pub buffer: Vec<u8>,
    /// Current line being processed
    pub current_line: String,
    /// Whether this is the first line of input
    pub first_line: bool,
    /// Whether the last line was empty
    pub last_line_empty: bool,

    // === Terminal/execution context ===
    /// Whether input is from a PTY
    pub is_pty: bool,
    /// Whether in execution mode
    pub is_exec: bool,
    /// Whether we might be at a shell prompt
    pub maybe_prompt: bool,
    /// Compiled regex for prompt detection
    pub prompt_regex: Option<Regex>,
    /// Current emit flag for special output handling
    pub emit_flag: Option<EmitFlag>,
    /// Scrape buffer for content extraction
    pub scrape: Option<String>,
    /// Current index in scrape buffer
    pub scrape_ix: usize,
    /// Terminal reference (placeholder for terminal handle)
    pub terminal: Option<String>,

    // === Width configuration ===
    /// User-specified width argument
    pub width_arg: Option<usize>,
    /// Full terminal width
    pub width_full: Option<usize>,
    /// Whether to wrap text
    pub width_wrap: bool,

    // === Indentation state ===
    /// First line indentation level
    pub first_indent: Option<usize>,
    /// Whether current content has a newline
    pub has_newline: bool,
    /// Current background color code
    pub bg: String,

    // === Code block state ===
    /// Buffer for code block content (with highlighting)
    pub code_buffer: String,
    /// Raw code buffer (without highlighting)
    pub code_buffer_raw: String,
    /// Generation counter for code blocks
    pub code_gen: usize,
    /// Language of current code block
    pub code_language: Option<String>,
    /// Whether on first line of code block
    pub code_first_line: bool,
    /// Indentation level of code block
    pub code_indent: usize,
    /// Current line in code block
    pub code_line: String,

    // === List state ===
    /// Stack of ordered list numbers for nested lists
    pub ordered_list_numbers: Vec<usize>,
    /// Stack of (indent, type) for nested lists
    pub list_item_stack: Vec<(usize, ListType)>,
    /// Text indentation for list content
    pub list_indent_text: usize,

    // === Block/inline state flags ===
    /// Whether currently in a list
    pub in_list: bool,
    /// Current code block type (None if not in code)
    pub in_code: Option<Code>,
    /// Whether inline code is active
    pub inline_code: bool,
    /// Whether bold formatting is active
    pub in_bold: bool,
    /// Whether italic formatting is active
    pub in_italic: bool,
    /// Current table state (None if not in table)
    pub in_table: Option<Code>,
    /// Whether underline formatting is active
    pub in_underline: bool,
    /// Whether strikeout formatting is active
    pub in_strikeout: bool,
    /// Current block quote depth
    pub block_depth: usize,
    /// Type of current block element
    pub block_type: Option<BlockType>,

    // === Execution state ===
    /// Subprocess handle placeholder
    pub exec_sub: Option<String>,
    /// Master PTY fd placeholder
    pub exec_master: Option<i32>,
    /// Slave PTY fd placeholder
    pub exec_slave: Option<i32>,
    /// Keyboard input counter for exec mode
    pub exec_kb: usize,

    // === Exit/debug state ===
    /// Exit code
    pub exit: i32,
    /// Debug: where input came from
    pub where_from: Option<String>,

    // === Feature flags from config ===
    /// Enable link rendering
    pub links: bool,
    /// Enable image rendering
    pub images: bool,
    /// Enable space-indented code blocks
    pub code_spaces: bool,
    /// Enable clipboard integration
    pub clipboard: bool,
    /// Enable logging
    pub logging: bool,
    /// Timeout for streaming operations (seconds)
    pub timeout: f64,
    /// Save brace matching state
    pub savebrace: bool,
}

impl Default for ParseState {
    fn default() -> Self {
        Self::new()
    }
}

impl ParseState {
    /// Create a new ParseState with default values.
    ///
    /// # Example
    ///
    /// ```
    /// use streamdown_core::ParseState;
    /// let state = ParseState::new();
    /// assert!(state.first_line);
    /// ```
    pub fn new() -> Self {
        Self {
            // Input buffer state
            buffer: Vec::new(),
            current_line: String::new(),
            first_line: true,
            last_line_empty: true,

            // Terminal/execution context
            is_pty: false,
            is_exec: false,
            maybe_prompt: false,
            prompt_regex: None,
            emit_flag: None,
            scrape: None,
            scrape_ix: 0,
            terminal: None,

            // Width configuration
            width_arg: None,
            width_full: None,
            width_wrap: false,

            // Indentation state
            first_indent: None,
            has_newline: false,
            bg: BGRESET.to_string(),

            // Code block state
            code_buffer: String::new(),
            code_buffer_raw: String::new(),
            code_gen: 0,
            code_language: None,
            code_first_line: false,
            code_indent: 0,
            code_line: String::new(),

            // List state
            ordered_list_numbers: Vec::new(),
            list_item_stack: Vec::new(),
            list_indent_text: 0,

            // Block/inline state flags
            in_list: false,
            in_code: None,
            inline_code: false,
            in_bold: false,
            in_italic: false,
            in_table: None,
            in_underline: false,
            in_strikeout: false,
            block_depth: 0,
            block_type: None,

            // Execution state
            exec_sub: None,
            exec_master: None,
            exec_slave: None,
            exec_kb: 0,

            // Exit/debug state
            exit: 0,
            where_from: None,

            // Feature flags from config
            links: true,
            images: true,
            code_spaces: false,
            clipboard: true,
            logging: false,
            timeout: 0.1,
            savebrace: true,
        }
    }

    /// Returns a snapshot of current inline formatting states.
    ///
    /// This captures whether bold, italic, underline, strikeout,
    /// and inline code are currently active.
    ///
    /// # Example
    ///
    /// ```
    /// use streamdown_core::ParseState;
    /// let mut state = ParseState::new();
    /// state.in_bold = true;
    /// let inline = state.current();
    /// assert!(inline.in_bold);
    /// ```
    pub fn current(&self) -> InlineState {
        InlineState {
            inline_code: self.inline_code,
            in_bold: self.in_bold,
            in_italic: self.in_italic,
            in_underline: self.in_underline,
            in_strikeout: self.in_strikeout,
        }
    }

    /// Reset all inline formatting states to false.
    ///
    /// This is typically called when exiting a block that should
    /// not carry inline formatting across boundaries.
    ///
    /// # Example
    ///
    /// ```
    /// use streamdown_core::ParseState;
    /// let mut state = ParseState::new();
    /// state.in_bold = true;
    /// state.in_italic = true;
    /// state.reset_inline();
    /// assert!(!state.in_bold);
    /// assert!(!state.in_italic);
    /// ```
    pub fn reset_inline(&mut self) {
        self.inline_code = false;
        self.in_bold = false;
        self.in_italic = false;
        self.in_underline = false;
        self.in_strikeout = false;
    }

    /// Set the terminal width.
    ///
    /// # Arguments
    ///
    /// * `width` - The terminal width in columns
    pub fn set_width(&mut self, width: usize) {
        self.width_full = Some(width);
    }

    /// Calculate the full available width with an optional offset.
    ///
    /// Returns the full terminal width minus the offset. If no width
    /// is configured, returns a default of 80.
    ///
    /// # Arguments
    ///
    /// * `offset` - Number of columns to subtract from full width
    ///
    /// # Example
    ///
    /// ```
    /// use streamdown_core::ParseState;
    /// let mut state = ParseState::new();
    /// state.set_width(100);
    /// assert_eq!(state.full_width(10), 90);
    /// ```
    pub fn full_width(&self, offset: usize) -> usize {
        let base = self.width_full.unwrap_or(80);
        base.saturating_sub(offset)
    }

    /// Calculate the current usable width for content.
    ///
    /// This takes into account:
    /// - Base terminal width
    /// - Block quote depth (each level uses 2 columns)
    /// - List indentation (if `listwidth` is true)
    ///
    /// # Arguments
    ///
    /// * `listwidth` - Whether to account for list indentation
    ///
    /// # Example
    ///
    /// ```
    /// use streamdown_core::ParseState;
    /// let mut state = ParseState::new();
    /// state.set_width(80);
    /// state.block_depth = 2;
    /// assert_eq!(state.current_width(false), 76); // 80 - (2 * 2)
    /// ```
    pub fn current_width(&self, listwidth: bool) -> usize {
        let base = self.width_full.unwrap_or(80);

        // Subtract block quote indentation (2 chars per level)
        let block_offset = self.block_depth * 2;

        // Subtract list indentation if requested
        let list_offset = if listwidth { self.list_indent_text } else { 0 };

        base.saturating_sub(block_offset + list_offset)
    }

    /// Generate the left spacing/margin string for current context.
    ///
    /// This creates the appropriate leading whitespace and block quote
    /// markers for the current nesting level.
    ///
    /// # Arguments
    ///
    /// * `listwidth` - Whether to include list indentation
    ///
    /// # Example
    ///
    /// ```
    /// use streamdown_core::ParseState;
    /// let mut state = ParseState::new();
    /// state.block_depth = 1;
    /// let margin = state.space_left(false);
    /// assert_eq!(margin, "│ ");
    /// ```
    pub fn space_left(&self, listwidth: bool) -> String {
        let mut result = String::new();

        // Add block quote markers
        for _ in 0..self.block_depth {
            result.push_str("│ ");
        }

        // Add list indentation if requested
        if listwidth && self.list_indent_text > 0 {
            result.push_str(&" ".repeat(self.list_indent_text));
        }

        result
    }

    /// Check if currently inside any code block.
    pub fn is_in_code(&self) -> bool {
        self.in_code.is_some()
    }

    /// Check if currently inside a table.
    pub fn is_in_table(&self) -> bool {
        self.in_table.is_some()
    }

    /// Check if any inline formatting is active.
    pub fn has_inline_formatting(&self) -> bool {
        self.inline_code || self.in_bold || self.in_italic || self.in_underline || self.in_strikeout
    }

    /// Push a new list level onto the stack.
    ///
    /// # Arguments
    ///
    /// * `indent` - Indentation level of the list
    /// * `list_type` - Type of list (Bullet or Ordered)
    pub fn push_list(&mut self, indent: usize, list_type: ListType) {
        self.list_item_stack.push((indent, list_type));
        if list_type == ListType::Ordered {
            self.ordered_list_numbers.push(1);
        }
        self.in_list = true;
    }

    /// Pop the current list level from the stack.
    ///
    /// Returns the popped (indent, type) tuple if the stack was non-empty.
    pub fn pop_list(&mut self) -> Option<(usize, ListType)> {
        let result = self.list_item_stack.pop();
        if let Some((_, ListType::Ordered)) = result {
            self.ordered_list_numbers.pop();
        }
        self.in_list = !self.list_item_stack.is_empty();
        result
    }

    /// Get the current list depth (nesting level).
    pub fn list_depth(&self) -> usize {
        self.list_item_stack.len()
    }

    /// Get and increment the current ordered list number.
    ///
    /// Returns the current number before incrementing.
    pub fn next_list_number(&mut self) -> Option<usize> {
        self.ordered_list_numbers.last_mut().map(|n| {
            let current = *n;
            *n += 1;
            current
        })
    }

    /// Enter a code block.
    ///
    /// # Arguments
    ///
    /// * `code_type` - The type of code block (Backtick or Spaces)
    /// * `language` - Optional language identifier
    pub fn enter_code_block(&mut self, code_type: Code, language: Option<String>) {
        self.in_code = Some(code_type);
        self.code_language = language;
        self.code_first_line = true;
        self.code_buffer.clear();
        self.code_buffer_raw.clear();
        self.code_gen += 1;
    }

    /// Exit the current code block.
    pub fn exit_code_block(&mut self) {
        self.in_code = None;
        self.code_language = None;
        self.code_first_line = false;
    }

    /// Enter a block quote.
    ///
    /// # Arguments
    ///
    /// * `block_type` - The type of block (Quote or Think)
    pub fn enter_block(&mut self, block_type: BlockType) {
        self.block_depth += 1;
        self.block_type = Some(block_type);
    }

    /// Exit one level of block quote.
    pub fn exit_block(&mut self) {
        if self.block_depth > 0 {
            self.block_depth -= 1;
        }
        if self.block_depth == 0 {
            self.block_type = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_state() {
        let state = ParseState::new();
        assert!(state.first_line);
        assert!(state.last_line_empty);
        assert!(!state.in_bold);
        assert!(state.in_code.is_none());
        assert_eq!(state.block_depth, 0);
    }

    #[test]
    fn test_current_inline_state() {
        let mut state = ParseState::new();
        state.in_bold = true;
        state.in_italic = true;

        let inline = state.current();
        assert!(inline.in_bold);
        assert!(inline.in_italic);
        assert!(!inline.inline_code);
    }

    #[test]
    fn test_reset_inline() {
        let mut state = ParseState::new();
        state.in_bold = true;
        state.in_italic = true;
        state.in_underline = true;

        state.reset_inline();

        assert!(!state.in_bold);
        assert!(!state.in_italic);
        assert!(!state.in_underline);
    }

    #[test]
    fn test_full_width() {
        let mut state = ParseState::new();
        state.set_width(100);

        assert_eq!(state.full_width(0), 100);
        assert_eq!(state.full_width(20), 80);
        assert_eq!(state.full_width(150), 0); // saturating_sub
    }

    #[test]
    fn test_current_width_with_blocks() {
        let mut state = ParseState::new();
        state.set_width(80);

        assert_eq!(state.current_width(false), 80);

        state.block_depth = 2;
        assert_eq!(state.current_width(false), 76); // 80 - 4

        state.list_indent_text = 4;
        assert_eq!(state.current_width(true), 72); // 80 - 4 - 4
        assert_eq!(state.current_width(false), 76); // list indent not counted
    }

    #[test]
    fn test_space_left() {
        let mut state = ParseState::new();

        assert_eq!(state.space_left(false), "");

        state.block_depth = 2;
        assert_eq!(state.space_left(false), "│ │ ");

        state.list_indent_text = 3;
        assert_eq!(state.space_left(true), "│ │    ");
    }

    #[test]
    fn test_list_operations() {
        let mut state = ParseState::new();

        state.push_list(0, ListType::Ordered);
        assert!(state.in_list);
        assert_eq!(state.list_depth(), 1);
        assert_eq!(state.next_list_number(), Some(1));
        assert_eq!(state.next_list_number(), Some(2));

        state.push_list(2, ListType::Bullet);
        assert_eq!(state.list_depth(), 2);

        state.pop_list();
        assert_eq!(state.list_depth(), 1);
        assert!(state.in_list);

        state.pop_list();
        assert_eq!(state.list_depth(), 0);
        assert!(!state.in_list);
    }

    #[test]
    fn test_code_block_operations() {
        let mut state = ParseState::new();

        assert!(!state.is_in_code());

        state.enter_code_block(Code::Backtick, Some("rust".to_string()));
        assert!(state.is_in_code());
        assert_eq!(state.code_language, Some("rust".to_string()));
        assert!(state.code_first_line);
        assert_eq!(state.code_gen, 1);

        state.exit_code_block();
        assert!(!state.is_in_code());
        assert!(state.code_language.is_none());
    }

    #[test]
    fn test_block_operations() {
        let mut state = ParseState::new();

        state.enter_block(BlockType::Quote);
        assert_eq!(state.block_depth, 1);
        assert_eq!(state.block_type, Some(BlockType::Quote));

        state.enter_block(BlockType::Quote);
        assert_eq!(state.block_depth, 2);

        state.exit_block();
        assert_eq!(state.block_depth, 1);

        state.exit_block();
        assert_eq!(state.block_depth, 0);
        assert!(state.block_type.is_none());
    }
}
