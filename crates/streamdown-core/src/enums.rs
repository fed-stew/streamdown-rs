//! Core enums for streamdown parsing state.
//!
//! These enums represent the various states that the parser can be in
//! while processing streaming markdown input.

use serde::{Deserialize, Serialize};

/// Represents the type of code block or code-related state.
///
/// This enum tracks whether we're in a fenced code block (backticks),
/// an indented code block (spaces), or processing table sections.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Code {
    /// Code block defined by leading spaces (indented code)
    Spaces,
    /// Code block defined by backtick fence (```)
    Backtick,
    /// Table header row
    Header,
    /// Table body rows
    Body,
    /// Flush/reset state
    Flush,
}

impl std::fmt::Display for Code {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Code::Spaces => write!(f, "spaces"),
            Code::Backtick => write!(f, "backtick"),
            Code::Header => write!(f, "header"),
            Code::Body => write!(f, "body"),
            Code::Flush => write!(f, "flush"),
        }
    }
}

/// Represents the type of list being processed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ListType {
    /// Unordered list with bullets (*, -, +)
    Bullet,
    /// Ordered list with numbers (1., 2., etc.)
    Ordered,
}

impl std::fmt::Display for ListType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ListType::Bullet => write!(f, "bullet"),
            ListType::Ordered => write!(f, "ordered"),
        }
    }
}

/// Represents the current section of a table being processed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TableState {
    /// Processing table header row
    Header,
    /// Processing table body rows
    Body,
}

impl std::fmt::Display for TableState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TableState::Header => write!(f, "header"),
            TableState::Body => write!(f, "body"),
        }
    }
}

/// Represents the alignment of a table column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ColumnAlignment {
    Left,
    Center,
    Right,
}

impl Default for ColumnAlignment {
    fn default() -> Self { Self::Left }
}

impl std::fmt::Display for ColumnAlignment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ColumnAlignment::Left => write!(f, "left"),
            ColumnAlignment::Center => write!(f, "center"),
            ColumnAlignment::Right => write!(f, "right"),
        }
    }
}

/// Represents the type of block-level element being processed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BlockType {
    /// Block quote (> prefix)
    Quote,
    /// "Think" block (special AI thinking sections)
    Think,
}

impl std::fmt::Display for BlockType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BlockType::Quote => write!(f, "quote"),
            BlockType::Think => write!(f, "think"),
        }
    }
}

/// Flags for special emit behavior.
///
/// These flags signal to the renderer that special handling is needed
/// for the current output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EmitFlag {
    /// Emit as level 1 header
    Header1,
    /// Emit as level 2 header
    Header2,
    /// Flush output immediately
    Flush,
}

impl std::fmt::Display for EmitFlag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EmitFlag::Header1 => write!(f, "header1"),
            EmitFlag::Header2 => write!(f, "header2"),
            EmitFlag::Flush => write!(f, "flush"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_code_display() {
        assert_eq!(Code::Spaces.to_string(), "spaces");
        assert_eq!(Code::Backtick.to_string(), "backtick");
        assert_eq!(Code::Header.to_string(), "header");
        assert_eq!(Code::Body.to_string(), "body");
        assert_eq!(Code::Flush.to_string(), "flush");
    }

    #[test]
    fn test_list_type_display() {
        assert_eq!(ListType::Bullet.to_string(), "bullet");
        assert_eq!(ListType::Ordered.to_string(), "ordered");
    }

    #[test]
    fn test_table_state_display() {
        assert_eq!(TableState::Header.to_string(), "header");
        assert_eq!(TableState::Body.to_string(), "body");
    }

    #[test]
    fn test_column_alignment_display() {
        assert_eq!(ColumnAlignment::Left.to_string(), "left");
        assert_eq!(ColumnAlignment::Center.to_string(), "center");
        assert_eq!(ColumnAlignment::Right.to_string(), "right");
    }

    #[test]
    fn test_column_alignment_default() {
        assert_eq!(ColumnAlignment::default(), ColumnAlignment::Left);
    }

    #[test]
    fn test_block_type_display() {
        assert_eq!(BlockType::Quote.to_string(), "quote");
        assert_eq!(BlockType::Think.to_string(), "think");
    }

    #[test]
    fn test_emit_flag_display() {
        assert_eq!(EmitFlag::Header1.to_string(), "header1");
        assert_eq!(EmitFlag::Header2.to_string(), "header2");
        assert_eq!(EmitFlag::Flush.to_string(), "flush");
    }
}
