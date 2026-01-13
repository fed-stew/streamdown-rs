//! Streamdown Core
//!
//! This crate provides core types, traits, and error definitions
//! for the streamdown markdown renderer.
//!
//! # Overview
//!
//! The core crate contains:
//! - [`ParseState`] - The main state machine for streaming markdown parsing
//! - [`Code`], [`ListType`], [`TableState`], [`BlockType`], [`EmitFlag`] - State enums
//! - [`StreamdownError`] - Error types
//! - [`Position`], [`Span`] - Source location types

pub mod enums;
pub mod error;
pub mod state;
pub mod types;

pub use enums::{BlockType, Code, EmitFlag, ListType, TableState};
pub use error::{Result, StreamdownError};
pub use state::{InlineState, ParseState};
pub use types::{Position, Span};
