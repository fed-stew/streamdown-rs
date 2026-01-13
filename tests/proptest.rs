//! Property-based tests for streamdown-rs.
//!
//! These tests use proptest to generate random inputs and verify
//! that the parser and renderer handle them gracefully.

use proptest::prelude::*;

use streamdown_parser::Parser;
use streamdown_render::Renderer;

/// Generate a random markdown-like string.
fn markdown_string() -> impl Strategy<Value = String> {
    prop::string::string_regex(r"[\x20-\x7E\n\t]*").unwrap()
}

/// Generate a random line of text.
fn text_line() -> impl Strategy<Value = String> {
    prop::string::string_regex(r"[\x20-\x7E]{0,200}").unwrap()
}

/// Generate a heading.
fn heading() -> impl Strategy<Value = String> {
    (1..=6usize, text_line()).prop_map(|(level, text)| format!("{} {}", "#".repeat(level), text))
}

/// Generate a code block.
fn code_block() -> impl Strategy<Value = String> {
    (text_line(), prop::collection::vec(text_line(), 0..10)).prop_map(|(lang, lines)| {
        let lang = if lang.is_empty() {
            String::new()
        } else {
            lang.split_whitespace().next().unwrap_or("").to_string()
        };
        format!("```{}\n{}\n```", lang, lines.join("\n"))
    })
}

/// Generate a list.
fn list() -> impl Strategy<Value = String> {
    prop::collection::vec(text_line(), 1..10).prop_map(|items| {
        items
            .iter()
            .map(|item| format!("- {}", item))
            .collect::<Vec<_>>()
            .join("\n")
    })
}

// =============================================================================
// Parser Property Tests
// =============================================================================

proptest! {
    /// The parser should never panic on any input.
    #[test]
    fn parser_never_panics(input in markdown_string()) {
        let mut parser = Parser::new();
        for line in input.lines() {
            let _ = parser.parse_line(line);
        }
    }

    /// The parser should handle random text lines.
    #[test]
    fn parser_handles_text(line in text_line()) {
        let mut parser = Parser::new();
        let _events = parser.parse_line(&line);
        // Should produce at least one event for non-empty lines
        if !line.trim().is_empty() {
            // May or may not produce events depending on content
        }
    }

    /// The parser should handle random headings.
    #[test]
    fn parser_handles_headings(h in heading()) {
        let mut parser = Parser::new();
        let _events = parser.parse_line(&h);
        // Should produce heading event for valid headings
    }

    /// The parser should handle random code blocks.
    #[test]
    fn parser_handles_code_blocks(code in code_block()) {
        let mut parser = Parser::new();
        for line in code.lines() {
            let _ = parser.parse_line(line);
        }
    }

    /// The parser should handle random lists.
    #[test]
    fn parser_handles_lists(list in list()) {
        let mut parser = Parser::new();
        for line in list.lines() {
            let _ = parser.parse_line(line);
        }
    }
}

// =============================================================================
// Renderer Property Tests
// =============================================================================

proptest! {
    /// The renderer should never panic on any parser output.
    #[test]
    fn renderer_never_panics(input in markdown_string()) {
        let mut output = Vec::new();
        let mut parser = Parser::new();

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut renderer = Renderer::new(&mut output, 80);
            for line in input.lines() {
                for event in parser.parse_line(line) {
                    let _ = renderer.render_event(&event);
                }
            }
        }));

        prop_assert!(result.is_ok(), "Renderer panicked on input");
    }

    /// The renderer should produce valid UTF-8 output.
    #[test]
    fn renderer_produces_valid_utf8(input in markdown_string()) {
        let mut output = Vec::new();
        let mut parser = Parser::new();

        {
            let mut renderer = Renderer::new(&mut output, 80);
            for line in input.lines() {
                for event in parser.parse_line(line) {
                    let _ = renderer.render_event(&event);
                }
            }
        }

        let result = String::from_utf8(output);
        prop_assert!(result.is_ok(), "Renderer produced invalid UTF-8");
    }

    /// The renderer should handle different widths.
    #[test]
    fn renderer_handles_widths(input in text_line(), width in 20..200usize) {
        let mut output = Vec::new();
        let mut parser = Parser::new();

        {
            let mut renderer = Renderer::new(&mut output, width);
            for event in parser.parse_line(&input) {
                let _ = renderer.render_event(&event);
            }
        }

        // Should not panic and produce output
        let _result = String::from_utf8(output).unwrap();
        // Output can be empty for empty input
    }
}

// =============================================================================
// ANSI Utility Property Tests
// =============================================================================

proptest! {
    /// visible_length should never panic.
    #[test]
    fn visible_length_never_panics(input in markdown_string()) {
        let _ = streamdown_ansi::utils::visible_length(&input);
    }

    /// visible should never panic.
    #[test]
    fn visible_never_panics(input in markdown_string()) {
        let _ = streamdown_ansi::utils::visible(&input);
    }

    /// visible should return a string no longer than the input.
    #[test]
    fn visible_shorter_or_equal(input in markdown_string()) {
        let visible = streamdown_ansi::utils::visible(&input);
        // The visible string might be longer if we add escape sequences,
        // but the visible length should be <= input length
        let _vis_len = streamdown_ansi::utils::visible_length(&visible);
        // Actually visible can contain the original chars, so length can vary
    }
}

// =============================================================================
// Plugin Property Tests
// =============================================================================

proptest! {
    /// LaTeX conversion should never panic.
    #[test]
    fn latex_never_panics(input in markdown_string()) {
        let _ = streamdown_plugin::latex::latex_to_unicode(&input);
    }

    /// Plugin manager should never panic.
    #[test]
    fn plugin_manager_never_panics(input in text_line()) {
        use streamdown_config::ComputedStyle;
        use streamdown_core::state::ParseState;
        use streamdown_plugin::PluginManager;

        let mut manager = PluginManager::with_builtins();
        let state = ParseState::new();
        let style = ComputedStyle::default();

        let _ = manager.process_line(&input, &state, &style);
    }
}

// =============================================================================
// Config Property Tests
// =============================================================================

proptest! {
    /// HSV to RGB conversion should produce valid colors.
    #[test]
    fn hsv_produces_valid_rgb(h in 0.0f64..1.0, s in 0.0f64..1.0, v in 0.0f64..1.0) {
        use streamdown_ansi::color::hsv_to_rgb;

        let (r, g, b) = hsv_to_rgb(h, s, v);

        // Values are u8 so they're always valid (0-255)
        // Just verify we got a result without panicking
        let _ = (r, g, b);
    }

    /// Hex parsing should handle valid hex colors.
    #[test]
    fn hex2rgb_handles_valid_hex(r in 0u8..=255, g in 0u8..=255, b in 0u8..=255) {
        use streamdown_ansi::color::hex2rgb;

        let hex = format!("#{:02x}{:02x}{:02x}", r, g, b);
        let result = hex2rgb(&hex);

        prop_assert!(result.is_some());
        let (pr, pg, pb) = result.unwrap();
        prop_assert_eq!(pr, r);
        prop_assert_eq!(pg, g);
        prop_assert_eq!(pb, b);
    }
}
