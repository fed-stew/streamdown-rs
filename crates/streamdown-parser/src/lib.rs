//! Streamdown Parser
//!
//! A streaming markdown parser designed for real-time rendering of markdown
//! content as it arrives. This is the core parsing engine for streamdown.
//!
//! # Overview
//!
//! The parser is designed to handle byte-by-byte input for streaming scenarios
//! (like LLM output) while also working efficiently with complete documents.
//!
//! # Example
//!
//! ```
//! use streamdown_parser::{Parser, ParseEvent};
//!
//! let mut parser = Parser::new();
//!
//! // Feed lines and get events
//! for event in parser.parse_line("# Hello World") {
//!     match event {
//!         ParseEvent::Heading { level, content } => {
//!             println!("H{}: {}", level, content);
//!         }
//!         _ => {}
//!     }
//! }
//! ```

pub mod entities;
pub mod inline;
pub mod tokenizer;

pub use entities::decode_html_entities;
pub use inline::{InlineElement, InlineParser, format_line};
pub use tokenizer::{Token, Tokenizer, cjk_count, is_cjk, not_text};

use regex::Regex;
use std::sync::LazyLock;
use streamdown_core::{BlockType, Code, ListType, ParseState};

// =============================================================================
// Regex patterns
// =============================================================================

/// Regex for code fence: ``` or ~~~ or <pre>
static CODE_FENCE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*(```+|~~~+|<pre>)\s*([^\s]*)\s*$").unwrap());

/// Regex for code fence end (also matches </pre>)
static CODE_FENCE_END_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*(```+|~~~+|</pre>)\s*$").unwrap());

/// Regex for space-indented code (4+ spaces, not starting with * for lists)
static SPACE_CODE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^    \s*[^\s*]").unwrap());

/// Regex for headings
static HEADING_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(#{1,6})\s+(.*)$").unwrap());

/// Regex for list items: handles -, *, +, +---, and 1. style
static LIST_ITEM_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\s*)([+*-]|\+-+|\d+\.)\s+(.*)$").unwrap());

/// Regex for blockquotes and think blocks (including unicode variants)
static BLOCK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*((>\s*)+|[◁<].?think[>▷]|</?.?think[>▷]?)(.*)$").unwrap());

/// Regex for horizontal rules
static HR_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(---+|\*\*\*+|___+)\s*$").unwrap());

/// Regex for table rows
static TABLE_ROW_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*\|(.+)\|\s*$").unwrap());

/// Regex for table separator (only contains |, -, :, spaces)
static TABLE_SEP_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[\s|:-]+$").unwrap());

// =============================================================================
// Types
// =============================================================================

/// List bullet type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListBullet {
    /// Dash bullet: -
    Dash,
    /// Asterisk bullet: *
    Asterisk,
    /// Plus bullet: +
    Plus,
    /// Expandable plus: +---
    PlusExpand,
    /// Ordered number
    Ordered(usize),
}

impl ListBullet {
    /// Parse a bullet string.
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        if s.starts_with("+") && s.len() > 1 && s.chars().skip(1).all(|c| c == '-') {
            return Some(ListBullet::PlusExpand);
        }
        match s {
            "-" => Some(ListBullet::Dash),
            "*" => Some(ListBullet::Asterisk),
            "+" => Some(ListBullet::Plus),
            s if s.ends_with('.') => {
                let num = s.trim_end_matches('.').parse().ok()?;
                Some(ListBullet::Ordered(num))
            }
            _ => None,
        }
    }

    /// Check if this is an ordered bullet.
    pub fn is_ordered(&self) -> bool {
        matches!(self, ListBullet::Ordered(_))
    }
}

/// Table parsing state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableState {
    /// Parsing header row
    Header,
    /// Saw separator, now in body
    Body,
}

/// Events emitted by the parser.
#[derive(Debug, Clone, PartialEq)]
pub enum ParseEvent {
    // === Inline elements ===
    Text(String),
    InlineCode(String),
    Bold(String),
    Italic(String),
    Underline(String),
    Strikeout(String),
    BoldItalic(String),
    Link {
        text: String,
        url: String,
    },
    Image {
        alt: String,
        url: String,
    },
    Footnote(String),

    // === Block-level elements ===
    Heading {
        level: u8,
        content: String,
    },
    CodeBlockStart {
        language: Option<String>,
        indent: usize,
    },
    CodeBlockLine(String),
    CodeBlockEnd,
    ListItem {
        indent: usize,
        bullet: ListBullet,
        content: String,
    },
    ListEnd,
    TableHeader(Vec<String>),
    TableRow(Vec<String>),
    TableSeparator,
    TableEnd,
    BlockquoteStart {
        depth: usize,
    },
    BlockquoteLine(String),
    BlockquoteEnd,
    ThinkBlockStart,
    ThinkBlockLine(String),
    ThinkBlockEnd,
    HorizontalRule,
    EmptyLine,
    Newline,
    Prompt(String),
    InlineElements(Vec<InlineElement>),
}

impl ParseEvent {
    pub fn is_block(&self) -> bool {
        !self.is_inline()
    }

    pub fn is_inline(&self) -> bool {
        matches!(
            self,
            ParseEvent::Text(_)
                | ParseEvent::InlineCode(_)
                | ParseEvent::Bold(_)
                | ParseEvent::Italic(_)
                | ParseEvent::Underline(_)
                | ParseEvent::Strikeout(_)
                | ParseEvent::BoldItalic(_)
                | ParseEvent::Link { .. }
                | ParseEvent::Image { .. }
                | ParseEvent::Footnote(_)
        )
    }
}

// =============================================================================
// Parser
// =============================================================================

/// Streaming markdown parser.
#[derive(Debug)]
pub struct Parser {
    state: ParseState,
    inline_parser: InlineParser,
    code_fence: Option<String>,
    table_state: Option<TableState>,
    events: Vec<ParseEvent>,
    /// Track previous empty line for collapsing
    prev_was_empty: bool,
}

impl Default for Parser {
    fn default() -> Self {
        Self::new()
    }
}

impl Parser {
    /// Create a new parser with default settings.
    pub fn new() -> Self {
        Self {
            state: ParseState::new(),
            inline_parser: InlineParser::new(),
            code_fence: None,
            table_state: None,
            events: Vec::new(),
            prev_was_empty: false,
        }
    }

    /// Create a parser with a custom ParseState.
    pub fn with_state(state: ParseState) -> Self {
        let inline_parser = InlineParser::with_settings(state.links, state.images);
        Self {
            state,
            inline_parser,
            code_fence: None,
            table_state: None,
            events: Vec::new(),
            prev_was_empty: false,
        }
    }

    pub fn state(&self) -> &ParseState {
        &self.state
    }
    pub fn state_mut(&mut self) -> &mut ParseState {
        &mut self.state
    }

    pub fn set_process_links(&mut self, enabled: bool) {
        self.state.links = enabled;
        self.inline_parser.process_links = enabled;
    }

    pub fn set_process_images(&mut self, enabled: bool) {
        self.state.images = enabled;
        self.inline_parser.process_images = enabled;
    }

    /// Enable space-indented code blocks (4 spaces = code).
    pub fn set_code_spaces(&mut self, enabled: bool) {
        self.state.code_spaces = enabled;
    }

    /// Parse a single line and return events.
    pub fn parse_line(&mut self, line: &str) -> Vec<ParseEvent> {
        self.events.clear();

        // Handle code blocks first (they consume everything)
        if self.state.is_in_code() {
            self.parse_in_code_block(line);
            return std::mem::take(&mut self.events);
        }

        // Handle think blocks
        if self.state.block_type == Some(BlockType::Think) {
            self.parse_in_think_block(line);
            return std::mem::take(&mut self.events);
        }

        // Check for empty line (with collapsing) - BEFORE indent stripping
        if line.trim().is_empty() {
            return self.handle_empty_line();
        }

        // Track that previous line wasn't empty
        let was_prev_empty = self.prev_was_empty;
        self.prev_was_empty = false;
        self.state.last_line_empty = false;

        // Check for space-indented code BEFORE first-indent stripping
        // (so we don't accidentally strip the 4-space indent)
        if self.try_parse_space_code(line, was_prev_empty) {
            return self.take_events();
        }

        // Now apply first-indent stripping for other constructs
        let line = self.strip_first_indent(line);

        // Try block-level constructs in order
        if self.try_parse_code_fence(&line) {
            return self.take_events();
        }
        if self.try_parse_block(&line) {
            return self.take_events();
        }
        if self.try_parse_heading(&line) {
            return self.take_events();
        }
        if self.try_parse_hr(&line) {
            return self.take_events();
        }
        if self.try_parse_list_item(&line) {
            return self.take_events();
        }
        if self.try_parse_table(&line) {
            return self.take_events();
        }

        // Exit special contexts for plain text
        self.exit_block_contexts();

        // Parse as inline content
        self.parse_inline_content(&line);
        self.take_events()
    }

    fn take_events(&mut self) -> Vec<ParseEvent> {
        std::mem::take(&mut self.events)
    }

    /// Strip first-indent from line if configured.
    /// This handles markdown that's indented in the input stream.
    fn strip_first_indent(&mut self, line: &str) -> String {
        // Set first_indent from the very first non-empty line
        // Use character count, not byte count, to handle multi-byte whitespace
        if self.state.first_indent.is_none() && !line.trim().is_empty() {
            let indent = line.chars().take_while(|c| c.is_whitespace()).count();
            self.state.first_indent = Some(indent);
        }

        // Only strip if first_indent is > 0
        if let Some(first_indent) = self.state.first_indent
            && first_indent > 0
        {
            let current_indent = line.chars().take_while(|c| c.is_whitespace()).count();
            if current_indent >= first_indent {
                // Skip first_indent characters (not bytes) to avoid UTF-8 boundary issues
                return line.chars().skip(first_indent).collect();
            }
        }

        line.to_string()
    }

    /// Handle empty line with collapsing.
    fn handle_empty_line(&mut self) -> Vec<ParseEvent> {
        // Collapse consecutive empty lines
        if self.prev_was_empty {
            return vec![]; // Skip this empty line
        }

        self.prev_was_empty = true;
        self.state.last_line_empty = true;

        // End blockquote if in one
        if self.state.block_depth > 0 && self.state.block_type == Some(BlockType::Quote) {
            while self.state.block_depth > 0 {
                self.state.exit_block();
            }
            self.events.push(ParseEvent::BlockquoteEnd);
        }

        // End list if in one
        if self.state.in_list {
            self.exit_list_context();
        }

        // End table if in one
        if self.table_state.is_some() {
            self.table_state = None;
            self.state.in_table = None;
            self.events.push(ParseEvent::TableEnd);
        }

        self.events.push(ParseEvent::EmptyLine);
        self.take_events()
    }

    /// Exit block contexts when encountering plain text.
    fn exit_block_contexts(&mut self) {
        if self.state.in_list {
            self.exit_list_context();
        }
        if self.table_state.is_some() {
            self.table_state = None;
            self.state.in_table = None;
            self.events.push(ParseEvent::TableEnd);
        }
    }

    // =========================================================================
    // Code block parsing
    // =========================================================================

    fn parse_in_code_block(&mut self, line: &str) {
        // Check for closing fence
        if let Some(ref fence) = self.code_fence.clone()
            && let Some(caps) = CODE_FENCE_END_RE.captures(line)
        {
            let end_fence = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            // Match fence type: ``` with ```, </pre> with <pre>
            let matches = (fence.starts_with('`') && end_fence.starts_with('`'))
                || (fence.starts_with('~') && end_fence.starts_with('~'))
                || (fence == "<pre>" && end_fence == "</pre>");

            if matches {
                self.events.push(ParseEvent::CodeBlockEnd);
                self.state.exit_code_block();
                self.code_fence = None;
                return;
            }
        }

        // For space-indented code, check if we've dedented
        if self.state.in_code == Some(Code::Spaces) {
            let indent = line.chars().take_while(|c| c.is_whitespace()).count();
            if indent < 4 && !line.trim().is_empty() {
                self.events.push(ParseEvent::CodeBlockEnd);
                self.state.exit_code_block();
                // Re-parse this line - need to do it after we return
                // For now, just parse inline content
                self.parse_inline_content(line);
                return;
            }
        }

        // Emit code line (strip indent for space-indented code)
        let code_line = if self.state.in_code == Some(Code::Spaces) {
            line.chars().skip(4).collect()
        } else {
            line.to_string()
        };

        self.events.push(ParseEvent::CodeBlockLine(code_line));
    }

    fn try_parse_code_fence(&mut self, line: &str) -> bool {
        if let Some(caps) = CODE_FENCE_RE.captures(line) {
            let fence = caps.get(1).map(|m| m.as_str()).unwrap_or("```");
            let lang = caps.get(2).map(|m| m.as_str()).filter(|s| !s.is_empty());
            let indent = line.chars().take_while(|c| c.is_whitespace()).count();

            self.code_fence = Some(fence.to_string());
            self.state.code_indent = indent;
            self.state.enter_code_block(
                Code::Backtick,
                lang.map(|s| s.to_string())
                    .or_else(|| Some("text".to_string())),
            );

            self.events.push(ParseEvent::CodeBlockStart {
                language: lang.map(|s| s.to_string()),
                indent,
            });
            true
        } else {
            false
        }
    }

    fn try_parse_space_code(&mut self, line: &str, was_prev_empty: bool) -> bool {
        // Space-indented code only when CodeSpaces is enabled
        if !self.state.code_spaces {
            return false;
        }

        // Only after empty line, and not in a list
        if !was_prev_empty || self.state.in_list {
            return false;
        }

        if SPACE_CODE_RE.is_match(line) {
            self.state
                .enter_code_block(Code::Spaces, Some("text".to_string()));
            self.events.push(ParseEvent::CodeBlockStart {
                language: Some("text".to_string()),
                indent: 4,
            });
            // Also emit the first line (skip 4 chars, not bytes)
            let code_line: String = line.chars().skip(4).collect();
            self.events.push(ParseEvent::CodeBlockLine(code_line));
            true
        } else {
            false
        }
    }

    // =========================================================================
    // Think/blockquote parsing
    // =========================================================================

    fn parse_in_think_block(&mut self, line: &str) {
        // Check for end of think block (various formats)
        if line.trim() == "</think>" || line.trim() == "</think▷" || line.trim() == "◁/think▷"
        {
            self.events.push(ParseEvent::ThinkBlockEnd);
            self.state.exit_block();
        } else {
            self.events
                .push(ParseEvent::ThinkBlockLine(line.to_string()));
        }
    }

    fn try_parse_block(&mut self, line: &str) -> bool {
        if let Some(caps) = BLOCK_RE.captures(line) {
            let marker = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            let content = caps.get(3).map(|m| m.as_str()).unwrap_or("");

            // Check for think block variants
            if marker.contains("think") {
                if marker.contains('/') {
                    // End of think block
                    if self.state.block_type == Some(BlockType::Think) {
                        self.events.push(ParseEvent::ThinkBlockEnd);
                        self.state.exit_block();
                    }
                    return true;
                } else {
                    // Start of think block
                    self.state.enter_block(BlockType::Think);
                    self.events.push(ParseEvent::ThinkBlockStart);
                    if !content.trim().is_empty() {
                        self.events
                            .push(ParseEvent::ThinkBlockLine(content.to_string()));
                    }
                    return true;
                }
            }

            // Regular blockquote
            let depth = marker.matches('>').count();
            if depth > 0 {
                if self.state.block_depth != depth {
                    if depth > self.state.block_depth {
                        for _ in self.state.block_depth..depth {
                            self.state.enter_block(BlockType::Quote);
                        }
                        self.events.push(ParseEvent::BlockquoteStart { depth });
                    } else {
                        for _ in depth..self.state.block_depth {
                            self.state.exit_block();
                        }
                    }
                }
                self.events
                    .push(ParseEvent::BlockquoteLine(content.to_string()));
                return true;
            }
        }

        // End blockquote if we were in one and this line doesn't continue it
        if self.state.block_depth > 0 && self.state.block_type == Some(BlockType::Quote) {
            while self.state.block_depth > 0 {
                self.state.exit_block();
            }
            self.events.push(ParseEvent::BlockquoteEnd);
        }

        false
    }

    // =========================================================================
    // Other block parsing
    // =========================================================================

    fn try_parse_heading(&mut self, line: &str) -> bool {
        if let Some(caps) = HEADING_RE.captures(line) {
            let hashes = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            let content = caps.get(2).map(|m| m.as_str()).unwrap_or("");
            let level = hashes.len().min(6) as u8;

            self.events.push(ParseEvent::Heading {
                level,
                content: content.to_string(),
            });
            true
        } else {
            false
        }
    }

    fn try_parse_hr(&mut self, line: &str) -> bool {
        if HR_RE.is_match(line.trim()) {
            self.events.push(ParseEvent::HorizontalRule);
            true
        } else {
            false
        }
    }

    fn try_parse_list_item(&mut self, line: &str) -> bool {
        if let Some(caps) = LIST_ITEM_RE.captures(line) {
            let indent_str = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            let bullet_str = caps.get(2).map(|m| m.as_str()).unwrap_or("");
            let content = caps.get(3).map(|m| m.as_str()).unwrap_or("");

            // Use character count, not byte length, for proper multi-byte whitespace handling
            let indent = indent_str.chars().count();
            let bullet = ListBullet::parse(bullet_str).unwrap_or(ListBullet::Dash);

            // Update list_indent_text (width of bullet + space) - use char count
            self.state.list_indent_text = bullet_str.chars().count();

            let list_type = if bullet.is_ordered() {
                ListType::Ordered
            } else {
                ListType::Bullet
            };

            // Pop items with greater or equal indent (for same-level items)
            while let Some((stack_indent, _)) = self.state.list_item_stack.last() {
                if *stack_indent > indent {
                    self.state.pop_list();
                } else {
                    break;
                }
            }

            // Push new level if indented further than current, or if stack is empty
            let need_push = self
                .state
                .list_item_stack
                .last()
                .map(|(i, _)| indent > *i)
                .unwrap_or(true);

            if need_push {
                self.state.push_list(indent, list_type);
            }

            // For ordered lists, get the next number
            let final_bullet = if let ListBullet::Ordered(_) = bullet {
                ListBullet::Ordered(self.state.next_list_number().unwrap_or(1))
            } else {
                bullet
            };

            self.events.push(ParseEvent::ListItem {
                indent,
                bullet: final_bullet,
                content: content.to_string(),
            });
            true
        } else {
            false
        }
    }

    fn exit_list_context(&mut self) {
        while self.state.in_list {
            self.state.pop_list();
        }
        self.events.push(ParseEvent::ListEnd);
    }

    fn try_parse_table(&mut self, line: &str) -> bool {
        if let Some(caps) = TABLE_ROW_RE.captures(line) {
            let inner = caps.get(1).map(|m| m.as_str()).unwrap_or("");

            // Check if this is a separator row
            if TABLE_SEP_RE.is_match(inner) && self.table_state == Some(TableState::Header) {
                self.table_state = Some(TableState::Body);
                self.state.in_table = Some(Code::Body);
                self.events.push(ParseEvent::TableSeparator);
                return true;
            }

            let cells: Vec<String> = inner.split('|').map(|s| s.trim().to_string()).collect();

            match self.table_state {
                None => {
                    // First row is header
                    self.table_state = Some(TableState::Header);
                    self.state.in_table = Some(Code::Header);
                    self.events.push(ParseEvent::TableHeader(cells));
                }
                Some(TableState::Header) => {
                    // If we see another row before separator, it's still header
                    // (some tables have multi-line headers)
                    self.events.push(ParseEvent::TableHeader(cells));
                }
                Some(TableState::Body) => {
                    self.events.push(ParseEvent::TableRow(cells));
                }
            }
            return true;
        }

        // End table if we were in one
        if self.table_state.is_some() {
            self.table_state = None;
            self.state.in_table = None;
            self.events.push(ParseEvent::TableEnd);
        }

        false
    }

    fn parse_inline_content(&mut self, line: &str) {
        let elements = self.inline_parser.parse(line);

        for element in elements {
            let event = match element {
                InlineElement::Text(s) => ParseEvent::Text(s),
                InlineElement::Bold(s) => ParseEvent::Bold(s),
                InlineElement::Italic(s) => ParseEvent::Italic(s),
                InlineElement::BoldItalic(s) => ParseEvent::BoldItalic(s),
                InlineElement::Underline(s) => ParseEvent::Underline(s),
                InlineElement::Strikeout(s) => ParseEvent::Strikeout(s),
                InlineElement::Code(s) => ParseEvent::InlineCode(s),
                InlineElement::Link { text, url } => ParseEvent::Link { text, url },
                InlineElement::Image { alt, url } => ParseEvent::Image { alt, url },
                InlineElement::Footnote(s) => ParseEvent::Footnote(s),
            };
            self.events.push(event);
        }

        self.events.push(ParseEvent::Newline);
    }

    /// Parse a complete document.
    pub fn parse_document(&mut self, content: &str) -> Vec<ParseEvent> {
        let mut all_events = Vec::new();
        for line in content.lines() {
            all_events.extend(self.parse_line(line));
        }
        all_events.extend(self.finalize());
        all_events
    }

    /// Finalize parsing, closing any open blocks.
    pub fn finalize(&mut self) -> Vec<ParseEvent> {
        self.events.clear();

        if self.state.is_in_code() {
            self.events.push(ParseEvent::CodeBlockEnd);
            self.state.exit_code_block();
            self.code_fence = None;
        }

        if self.state.block_type == Some(BlockType::Think) {
            self.events.push(ParseEvent::ThinkBlockEnd);
            self.state.exit_block();
        }

        if self.state.block_depth > 0 {
            self.events.push(ParseEvent::BlockquoteEnd);
            while self.state.block_depth > 0 {
                self.state.exit_block();
            }
        }

        if self.state.in_list {
            self.exit_list_context();
        }

        if self.table_state.is_some() {
            self.table_state = None;
            self.state.in_table = None;
            self.events.push(ParseEvent::TableEnd);
        }

        self.take_events()
    }

    /// Reset the parser to initial state.
    pub fn reset(&mut self) {
        self.state = ParseState::new();
        self.inline_parser.reset();
        self.code_fence = None;
        self.table_state = None;
        self.events.clear();
        self.prev_was_empty = false;
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_heading() {
        let mut parser = Parser::new();
        let events = parser.parse_line("# Hello World");
        assert!(events.iter().any(|e| matches!(
            e, ParseEvent::Heading { level: 1, content } if content == "Hello World"
        )));
    }

    #[test]
    fn test_parse_code_block() {
        let mut parser = Parser::new();
        let e1 = parser.parse_line("```rust");
        assert!(e1.iter().any(
            |e| matches!(e, ParseEvent::CodeBlockStart { language: Some(l), .. } if l == "rust")
        ));
        let e2 = parser.parse_line("let x = 1;");
        assert!(
            e2.iter()
                .any(|e| matches!(e, ParseEvent::CodeBlockLine(s) if s == "let x = 1;"))
        );
        let e3 = parser.parse_line("```");
        assert!(e3.iter().any(|e| matches!(e, ParseEvent::CodeBlockEnd)));
    }

    #[test]
    fn test_parse_pre_tag() {
        let mut parser = Parser::new();
        let e1 = parser.parse_line("<pre>");
        assert!(
            e1.iter()
                .any(|e| matches!(e, ParseEvent::CodeBlockStart { .. }))
        );
        let e2 = parser.parse_line("code");
        assert!(e2.iter().any(|e| matches!(e, ParseEvent::CodeBlockLine(_))));
        let e3 = parser.parse_line("</pre>");
        assert!(e3.iter().any(|e| matches!(e, ParseEvent::CodeBlockEnd)));
    }

    #[test]
    fn test_space_indented_code() {
        let mut parser = Parser::new();
        parser.set_code_spaces(true);
        parser.parse_line(""); // Empty line first
        let events = parser.parse_line("    let x = 1;");
        assert!(
            events
                .iter()
                .any(|e| matches!(e, ParseEvent::CodeBlockStart { .. }))
        );
        assert!(
            events
                .iter()
                .any(|e| matches!(e, ParseEvent::CodeBlockLine(s) if s == "let x = 1;"))
        );
    }

    #[test]
    fn test_empty_line_collapsing() {
        let mut parser = Parser::new();
        let e1 = parser.parse_line("");
        assert!(e1.iter().any(|e| matches!(e, ParseEvent::EmptyLine)));
        let e2 = parser.parse_line("");
        assert!(e2.is_empty()); // Collapsed
        let e3 = parser.parse_line("text");
        assert!(!e3.is_empty());
        let e4 = parser.parse_line("");
        assert!(e4.iter().any(|e| matches!(e, ParseEvent::EmptyLine)));
    }

    #[test]
    fn test_parse_think_block_unicode() {
        let mut parser = Parser::new();
        let e1 = parser.parse_line("◁think▷");
        assert!(e1.iter().any(|e| matches!(e, ParseEvent::ThinkBlockStart)));
    }

    #[test]
    fn test_parse_list() {
        let mut parser = Parser::new();
        let events = parser.parse_line("- Item one");
        assert!(events.iter().any(|e| matches!(
            e, ParseEvent::ListItem { bullet: ListBullet::Dash, content, .. } if content == "Item one"
        )));
    }

    #[test]
    fn test_parse_nested_list() {
        let mut parser = Parser::new();
        parser.parse_line("- Item 1");
        let e2 = parser.parse_line("  - Nested");
        // Nested item should have indent 2
        assert!(
            e2.iter()
                .any(|e| matches!(e, ParseEvent::ListItem { indent: 2, .. }))
        );
    }

    #[test]
    fn test_parse_ordered_list_numbering() {
        let mut parser = Parser::new();
        parser.parse_line("1. First");
        let e2 = parser.parse_line("2. Second");
        // Should auto-number
        assert!(e2.iter().any(|e| matches!(
            e,
            ParseEvent::ListItem {
                bullet: ListBullet::Ordered(2),
                ..
            }
        )));
    }

    #[test]
    fn test_parse_blockquote() {
        let mut parser = Parser::new();
        let events = parser.parse_line("> Quote text");
        assert!(
            events
                .iter()
                .any(|e| matches!(e, ParseEvent::BlockquoteLine(s) if s == "Quote text"))
        );
    }

    #[test]
    fn test_parse_nested_blockquote() {
        let mut parser = Parser::new();
        let events = parser.parse_line(">> Nested quote");
        assert!(
            events
                .iter()
                .any(|e| matches!(e, ParseEvent::BlockquoteStart { depth: 2 }))
        );
    }

    #[test]
    fn test_parse_hr() {
        let mut parser = Parser::new();
        assert!(
            parser
                .parse_line("---")
                .iter()
                .any(|e| matches!(e, ParseEvent::HorizontalRule))
        );
        assert!(
            parser
                .parse_line("***")
                .iter()
                .any(|e| matches!(e, ParseEvent::HorizontalRule))
        );
        assert!(
            parser
                .parse_line("___")
                .iter()
                .any(|e| matches!(e, ParseEvent::HorizontalRule))
        );
    }

    #[test]
    fn test_parse_table() {
        let mut parser = Parser::new();
        let e1 = parser.parse_line("| A | B | C |");
        assert!(e1.iter().any(|e| matches!(e, ParseEvent::TableHeader(_))));
        let e2 = parser.parse_line("|---|---|---|");
        assert!(e2.iter().any(|e| matches!(e, ParseEvent::TableSeparator)));
        let e3 = parser.parse_line("| 1 | 2 | 3 |");
        assert!(e3.iter().any(|e| matches!(e, ParseEvent::TableRow(_))));
    }

    #[test]
    fn test_parse_think_block() {
        let mut parser = Parser::new();
        let e1 = parser.parse_line("<think>");
        assert!(e1.iter().any(|e| matches!(e, ParseEvent::ThinkBlockStart)));
        let e2 = parser.parse_line("Thinking...");
        assert!(
            e2.iter()
                .any(|e| matches!(e, ParseEvent::ThinkBlockLine(s) if s == "Thinking..."))
        );
        let e3 = parser.parse_line("</think>");
        assert!(e3.iter().any(|e| matches!(e, ParseEvent::ThinkBlockEnd)));
    }

    #[test]
    fn test_first_indent_stripping() {
        let mut parser = Parser::new();
        // First line has 4 spaces indent
        let e1 = parser.parse_line("    # Hello");
        // Should strip the 4 spaces and parse as heading
        assert!(
            e1.iter().any(
                |e| matches!(e, ParseEvent::Heading { level: 1, content } if content == "Hello")
            )
        );
    }

    #[test]
    fn test_parse_document() {
        let mut parser = Parser::new();
        let doc = "# Title\n\nSome text.\n\n```\ncode\n```";
        let events = parser.parse_document(doc);
        assert!(
            events
                .iter()
                .any(|e| matches!(e, ParseEvent::Heading { level: 1, .. }))
        );
        assert!(
            events
                .iter()
                .any(|e| matches!(e, ParseEvent::CodeBlockStart { .. }))
        );
        assert!(events.iter().any(|e| matches!(e, ParseEvent::CodeBlockEnd)));
    }

    #[test]
    fn test_finalize_closes_blocks() {
        let mut parser = Parser::new();
        parser.parse_line("```");
        parser.parse_line("code");
        let events = parser.finalize();
        assert!(events.iter().any(|e| matches!(e, ParseEvent::CodeBlockEnd)));
    }

    #[test]
    fn test_is_block_is_inline() {
        assert!(
            ParseEvent::Heading {
                level: 1,
                content: "x".to_string()
            }
            .is_block()
        );
        assert!(
            ParseEvent::CodeBlockStart {
                language: None,
                indent: 0
            }
            .is_block()
        );
        assert!(ParseEvent::Text("x".to_string()).is_inline());
        assert!(ParseEvent::Bold("x".to_string()).is_inline());
    }

    #[test]
    fn test_first_indent_stripping_multibyte_whitespace() {
        // This test reproduces the byte boundary bug in strip_first_indent.
        //
        // Line 1: "  # Hello" = 2 ASCII spaces (2 bytes) + "# Hello"
        // Buggy code calculates first_indent = 2 (bytes)
        //
        // Line 2: "　World" = 1 fullwidth space (3 bytes) + "World"
        // Buggy code checks: current_indent (3 bytes) >= first_indent (2 bytes) ✓
        // Then tries: line[2..] which is INSIDE the fullwidth space!
        // Panic: "byte index 2 is not a char boundary; it is inside '　'"
        let mut parser = Parser::new();

        // First line: 2 ASCII spaces = 2 bytes indent
        let line1 = "  # Hello";
        assert_eq!(line1.len() - line1.trim_start().len(), 2);
        let _ = parser.parse_line(line1);

        // Second line: 1 fullwidth space (3 bytes) - byte 2 is NOT a char boundary
        let line2 = "　World";
        assert!(!line2.is_char_boundary(2)); // Verify byte 2 is invalid

        // This will panic with buggy code: "byte index 2 is not a char boundary"
        let events = parser.parse_line(line2);

        // Should produce valid output without panicking
        assert!(!events.is_empty());
    }

    #[test]
    fn test_space_indented_code_strip_with_fullwidth() {
        // This test reproduces a panic when stripping indent from space-indented code.
        //
        // Scenario:
        // 1. Enter space-indented code block with "    code" (4 ASCII spaces)
        // 2. Continue with "　　more" (2 fullwidth spaces = 6 bytes)
        // 3. Buggy code: line.len() >= 4 is true (6 >= 4), so it tries line[4..]
        // 4. Panic: byte 4 is inside the second fullwidth space (bytes 3..6)
        let mut parser = Parser::new();
        parser.set_code_spaces(true);

        // Empty line required before space-indented code
        parser.parse_line("");

        // First line: 4 ASCII spaces triggers space-indented code block
        let line1 = "    first line of code";
        let events1 = parser.parse_line(line1);
        assert!(
            events1
                .iter()
                .any(|e| matches!(e, ParseEvent::CodeBlockStart { .. }))
        );

        // Second line: 2 fullwidth spaces (6 bytes) - byte 4 is NOT a char boundary
        // This would panic with buggy code: "byte index 4 is not a char boundary"
        let line2 = "　　second line";
        assert!(!line2.is_char_boundary(4)); // Verify byte 4 is invalid

        let events2 = parser.parse_line(line2);

        // Should not panic and produce some output
        assert!(!events2.is_empty());
    }

    #[test]
    fn test_list_item_indent_with_fullwidth_spaces() {
        // BUG: List indent uses byte-based calculation.
        // A list item with 1 fullwidth space (3 bytes) would be treated as
        // having indent 3, which could incorrectly affect nesting level.
        let mut parser = Parser::new();

        // Top-level list item
        let events1 = parser.parse_line("- top level");
        assert!(
            events1
                .iter()
                .any(|e| matches!(e, ParseEvent::ListItem { indent: 0, .. }))
        );

        // List item with 1 fullwidth space indent (3 bytes, 1 char)
        // Should be treated as indent 1 (char-based), not indent 3 (byte-based)
        let line2 = "　- nested item"; // 1 fullwidth space
        let events2 = parser.parse_line(line2);

        // Check that indent is character-based (1), not byte-based (3)
        let list_item = events2
            .iter()
            .find(|e| matches!(e, ParseEvent::ListItem { .. }));
        assert!(list_item.is_some(), "Should have parsed list item");

        if let Some(ParseEvent::ListItem { indent, .. }) = list_item {
            // With byte-based: indent = 3
            // With char-based: indent = 1
            assert_eq!(
                *indent, 1,
                "Indent should be 1 (char-based), not 3 (byte-based)"
            );
        }
    }

    #[test]
    fn test_space_indented_code_dedent_with_fullwidth() {
        // BUG: Dedent detection uses byte-based indent calculation.
        // A line with 2 fullwidth spaces (6 bytes) would NOT trigger dedent
        // because 6 >= 4, but it should because 2 chars < 4 chars.
        let mut parser = Parser::new();
        parser.set_code_spaces(true);

        // Empty line required before space-indented code
        parser.parse_line("");

        // Enter code block with 4 ASCII spaces
        let events1 = parser.parse_line("    code line");
        assert!(
            events1
                .iter()
                .any(|e| matches!(e, ParseEvent::CodeBlockStart { .. }))
        );

        // Line with 2 fullwidth spaces (6 bytes, 2 chars) should EXIT code block
        // because 2 chars < 4 required indent
        let line2 = "　　not code anymore";
        let byte_indent = line2.len() - line2.trim_start().len();
        let char_indent = line2.chars().take_while(|c| c.is_whitespace()).count();
        assert_eq!(byte_indent, 6); // 2 fullwidth spaces = 6 bytes
        assert_eq!(char_indent, 2); // but only 2 characters

        let events2 = parser.parse_line(line2);

        // Should have exited code block (CodeBlockEnd event)
        assert!(
            events2
                .iter()
                .any(|e| matches!(e, ParseEvent::CodeBlockEnd)),
            "Should have exited code block with only 2-char indent"
        );
    }
}
