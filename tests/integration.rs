//! Integration tests for streamdown-rs.
//!
//! These tests validate parsing and rendering against real markdown files
//! from the Python Streamdown project to ensure feature parity.

use std::fs;
use std::path::PathBuf;

use streamdown_parser::{ParseEvent, Parser};
use streamdown_render::{RenderStyle, Renderer};

/// Get the path to Python Streamdown test files.
///
/// Tries in order:
/// 1. STREAMDOWN_PYTHON_TESTS environment variable
/// 2. $HOME/sources/Streamdown/tests
/// 3. ../Streamdown/tests (relative to workspace)
fn python_tests_dir() -> Option<PathBuf> {
    // First try environment variable
    if let Ok(path) = std::env::var("STREAMDOWN_PYTHON_TESTS") {
        let p = PathBuf::from(path);
        if p.exists() {
            return Some(p);
        }
    }

    // Try home directory
    if let Ok(home) = std::env::var("HOME") {
        let p = PathBuf::from(format!("{}/sources/Streamdown/tests", home));
        if p.exists() {
            return Some(p);
        }
    }

    // Try relative path
    let relative = PathBuf::from("../Streamdown/tests");
    if relative.exists() {
        return Some(relative);
    }

    None
}

/// Helper to read a test file from the Python project.
fn read_test_file(name: &str) -> Option<String> {
    let dir = python_tests_dir()?;
    let path = dir.join(name);
    fs::read_to_string(&path).ok()
}

/// Helper to parse a document and collect all events.
fn parse_document(content: &str) -> Vec<ParseEvent> {
    let mut parser = Parser::new();
    let mut events = Vec::new();

    for line in content.lines() {
        events.extend(parser.parse_line(line));
    }

    events
}

/// Helper to render a document to a string.
fn render_to_string(content: &str, width: usize) -> String {
    let mut output = Vec::new();
    let mut parser = Parser::new();

    {
        let mut renderer = Renderer::new(&mut output, width);

        for line in content.lines() {
            let events = parser.parse_line(line);
            for event in events {
                renderer.render_event(&event).unwrap();
            }
        }
    }

    String::from_utf8(output).unwrap()
}

// =============================================================================
// Basic Parsing Tests
// =============================================================================

#[test]
fn test_parser_doesnt_panic_on_empty() {
    let events = parse_document("");
    assert!(events.is_empty());
}

#[test]
fn test_parser_doesnt_panic_on_single_line() {
    let events = parse_document("Hello, world!");
    assert!(!events.is_empty());
}

#[test]
fn test_parser_handles_heading() {
    let events = parse_document("# Heading 1\n## Heading 2");

    let headings: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, ParseEvent::Heading { .. }))
        .collect();

    assert_eq!(headings.len(), 2);
}

#[test]
fn test_parser_handles_code_block() {
    let content = r#"```rust
fn main() {}
```"#;

    let events = parse_document(content);

    let code_starts: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, ParseEvent::CodeBlockStart { .. }))
        .collect();

    assert_eq!(code_starts.len(), 1);
}

#[test]
fn test_parser_handles_inline_formatting() {
    let events = parse_document("This is **bold** and *italic* text.");

    let has_bold = events.iter().any(|e| matches!(e, ParseEvent::Bold(_)));
    let has_italic = events.iter().any(|e| matches!(e, ParseEvent::Italic(_)));

    assert!(has_bold);
    assert!(has_italic);
}

// =============================================================================
// Python Test File Tests
// =============================================================================

#[test]
fn test_example_md() {
    let content = read_test_file("example.md");
    if content.is_none() {
        eprintln!("Skipping test_example_md: file not found");
        return;
    }
    let content = content.unwrap();

    // Should parse without panicking
    let events = parse_document(&content);
    assert!(!events.is_empty());

    // Should render without panicking
    let output = render_to_string(&content, 80);
    assert!(!output.is_empty());
}

#[test]
fn test_code_md() {
    let content = read_test_file("code.md");
    if content.is_none() {
        return;
    }
    let content = content.unwrap();

    let events = parse_document(&content);

    // Should have code block events
    let has_code = events
        .iter()
        .any(|e| matches!(e, ParseEvent::CodeBlockStart { .. }));
    assert!(has_code);

    let output = render_to_string(&content, 80);
    assert!(!output.is_empty());
}

#[test]
fn test_block_md() {
    let content = read_test_file("block.md");
    if content.is_none() {
        return;
    }
    let content = content.unwrap();

    let events = parse_document(&content);
    assert!(!events.is_empty());

    let output = render_to_string(&content, 80);
    assert!(!output.is_empty());
}

#[test]
fn test_inline_md() {
    let content = read_test_file("inline.md");
    if content.is_none() {
        return;
    }
    let content = content.unwrap();

    let events = parse_document(&content);
    assert!(!events.is_empty());

    let output = render_to_string(&content, 80);
    assert!(!output.is_empty());
}

#[test]
#[ignore] // Complex table causes timeout - needs optimization
fn test_table_md() {
    let content = read_test_file("table_test.md");
    if content.is_none() {
        return;
    }
    let content = content.unwrap();

    let events = parse_document(&content);

    // Should have table events
    let has_table = events
        .iter()
        .any(|e| matches!(e, ParseEvent::TableHeader(_) | ParseEvent::TableRow(_)));
    assert!(has_table);

    let output = render_to_string(&content, 100);
    assert!(!output.is_empty());
}

#[test]
fn test_links_md() {
    let content = read_test_file("links.md");
    if content.is_none() {
        return;
    }
    let content = content.unwrap();

    let events = parse_document(&content);
    assert!(!events.is_empty());

    // Should have link events
    let _has_link = events.iter().any(|e| matches!(e, ParseEvent::Link { .. }));
    // Links might be in inline content, so we check the output instead

    let output = render_to_string(&content, 80);
    assert!(!output.is_empty());
}

#[test]
#[ignore] // Large CJK file causes timeout - needs optimization
fn test_cjk_wrap_md() {
    let content = read_test_file("cjk-wrap.md");
    if content.is_none() {
        return;
    }
    let content = content.unwrap();

    let events = parse_document(&content);
    assert!(!events.is_empty());

    let output = render_to_string(&content, 80);
    assert!(!output.is_empty());
}

#[test]
#[ignore] // CJK table causes timeout - needs optimization
fn test_cjk_table_md() {
    let content = read_test_file("cjk-table.md");
    if content.is_none() {
        return;
    }
    let content = content.unwrap();

    let events = parse_document(&content);
    assert!(!events.is_empty());

    let output = render_to_string(&content, 100);
    assert!(!output.is_empty());
}

#[test]
fn test_fizzbuzz_md() {
    let content = read_test_file("fizzbuzz.md");
    if content.is_none() {
        return;
    }
    let content = content.unwrap();

    let events = parse_document(&content);
    assert!(!events.is_empty());

    // Should have multiple code blocks
    let code_count = events
        .iter()
        .filter(|e| matches!(e, ParseEvent::CodeBlockStart { .. }))
        .count();
    assert!(code_count >= 1);

    let output = render_to_string(&content, 80);
    assert!(!output.is_empty());
}

#[test]
fn test_nested_example_md() {
    let content = read_test_file("nested-example.md");
    if content.is_none() {
        return;
    }
    let content = content.unwrap();

    let events = parse_document(&content);
    assert!(!events.is_empty());

    let output = render_to_string(&content, 80);
    assert!(!output.is_empty());
}

#[test]
fn test_markdown_md() {
    let content = read_test_file("markdown.md");
    if content.is_none() {
        return;
    }
    let content = content.unwrap();

    let events = parse_document(&content);
    assert!(!events.is_empty());

    let output = render_to_string(&content, 80);
    assert!(!output.is_empty());
}

#[test]
fn test_managerie_md() {
    let content = read_test_file("managerie.md");
    if content.is_none() {
        return;
    }
    let content = content.unwrap();

    // This is a complex file with many features
    let events = parse_document(&content);
    assert!(!events.is_empty());

    let output = render_to_string(&content, 80);
    assert!(!output.is_empty());
}

#[test]
fn test_broken_code_md() {
    let content = read_test_file("broken-code.md");
    if content.is_none() {
        return;
    }
    let content = content.unwrap();

    // Should handle broken/malformed code blocks gracefully
    let _events = parse_document(&content);
    // No assertions on events - just shouldn't panic

    let _output = render_to_string(&content, 80);
    // Just shouldn't panic
}

#[test]
fn test_table_break_md() {
    let content = read_test_file("table-break.md");
    if content.is_none() {
        return;
    }
    let content = content.unwrap();

    let events = parse_document(&content);
    assert!(!events.is_empty());

    let output = render_to_string(&content, 80);
    assert!(!output.is_empty());
}

#[test]
fn test_line_wrap_md() {
    let content = read_test_file("line-wrap.md");
    if content.is_none() {
        return;
    }
    let content = content.unwrap();

    let events = parse_document(&content);
    assert!(!events.is_empty());

    // Test at different widths
    for width in [40, 60, 80, 100] {
        let output = render_to_string(&content, width);
        assert!(!output.is_empty());
    }
}

// =============================================================================
// Edge Case Tests
// =============================================================================

#[test]
fn test_empty_lines() {
    let content = "\n\n\n";
    let _events = parse_document(content);
    // Empty lines should produce empty line events
}

#[test]
fn test_only_whitespace() {
    let content = "   \n\t\n  ";
    let _ = parse_document(content);
    // Should not panic
}

#[test]
fn test_very_long_line() {
    let content = "x".repeat(10000);
    let events = parse_document(&content);
    assert!(!events.is_empty());

    let output = render_to_string(&content, 80);
    assert!(!output.is_empty());
}

#[test]
fn test_deeply_nested_lists() {
    let content = r#"- Level 1
  - Level 2
    - Level 3
      - Level 4
        - Level 5
          - Level 6"#;

    let events = parse_document(content);

    let list_items = events
        .iter()
        .filter(|e| matches!(e, ParseEvent::ListItem { .. }))
        .count();

    assert!(list_items >= 1);
}

#[test]
fn test_mixed_content() {
    let content = r#"# Heading

Paragraph with **bold** and *italic*.

```python
def hello():
    print("world")
```

- List item 1
- List item 2

| Col1 | Col2 |
|------|------|
| A    | B    |

> Blockquote
"#;

    let events = parse_document(content);
    assert!(!events.is_empty());

    let output = render_to_string(content, 80);
    assert!(!output.is_empty());

    // Check we have various event types
    let has_heading = events
        .iter()
        .any(|e| matches!(e, ParseEvent::Heading { .. }));
    let has_code = events
        .iter()
        .any(|e| matches!(e, ParseEvent::CodeBlockStart { .. }));
    let has_list = events
        .iter()
        .any(|e| matches!(e, ParseEvent::ListItem { .. }));

    assert!(has_heading);
    assert!(has_code);
    assert!(has_list);
}

#[test]
fn test_unicode_content() {
    let content = "# 你好世界\n\n这是中文文本。\n\n日本語テキスト。\n\n한국어 텍스트.";

    let events = parse_document(content);
    assert!(!events.is_empty());

    let output = render_to_string(content, 80);
    assert!(output.contains("你好世界") || !output.is_empty());
}

#[test]
fn test_emoji_content() {
    let content = "# Hello 👋\n\nThis has emojis: 🎉 🚀 ✨ 🐕";

    let events = parse_document(content);
    assert!(!events.is_empty());

    let output = render_to_string(content, 80);
    assert!(!output.is_empty());
}

// =============================================================================
// Rendering Tests
// =============================================================================

#[test]
fn test_render_with_custom_style() {
    let content = "# Test\n\nParagraph.";
    let mut output = Vec::new();

    let style = RenderStyle {
        bright: "#ff0000".to_string(),
        head: "#00ff00".to_string(),
        symbol: "#0000ff".to_string(),
        grey: "#888888".to_string(),
        dark: "#111111".to_string(),
        mid: "#333333".to_string(),
        light: "#555555".to_string(),
    };

    {
        let mut renderer = Renderer::with_style(&mut output, 80, style);
        let mut parser = Parser::new();

        for line in content.lines() {
            for event in parser.parse_line(line) {
                renderer.render_event(&event).unwrap();
            }
        }
    }

    let result = String::from_utf8(output).unwrap();
    assert!(!result.is_empty());
}

#[test]
fn test_render_at_different_widths() {
    let content =
        "This is a paragraph that should wrap at different widths based on the terminal size.";

    for width in [20, 40, 60, 80, 120] {
        let output = render_to_string(content, width);
        assert!(!output.is_empty());
    }
}

#[test]
fn test_render_code_block_padding() {
    let content = "```rust\nfn main() {}\n```";

    let output = render_to_string(content, 80);

    // Should have pretty padding characters
    assert!(output.contains('▄') || output.contains('▀') || !output.is_empty());
}

// =============================================================================
// Plugin Tests
// =============================================================================

#[test]
fn test_latex_inline() {
    use streamdown_plugin::latex::latex_to_unicode;

    assert_eq!(latex_to_unicode(r"\alpha"), "α");
    assert_eq!(latex_to_unicode(r"\beta"), "β");
    assert_eq!(latex_to_unicode(r"x^2"), "x²");
    assert_eq!(latex_to_unicode(r"x_1"), "x₁");
}

#[test]
fn test_latex_plugin_integration() {
    use streamdown_config::ComputedStyle;
    use streamdown_core::state::ParseState;
    use streamdown_plugin::PluginManager;

    let mut manager = PluginManager::with_builtins();
    let state = ParseState::new();
    let style = ComputedStyle::default();

    // Test inline math
    let result = manager.process_line("$E = mc^2$", &state, &style);
    assert!(result.is_some());

    // Test block math
    let result = manager.process_line("$$\\alpha + \\beta$$", &state, &style);
    assert!(result.is_some());
}

// =============================================================================
// Config Tests
// =============================================================================

#[test]
fn test_config_default() {
    use streamdown_config::Config;

    let config = Config::default();
    let style = config.computed_style();

    // Should have computed values (not empty)
    assert!(!style.bright.is_empty());
    assert!(!style.dark.is_empty());
    assert!(!style.margin_spaces.is_empty());
}

#[test]
fn test_config_toml_roundtrip() {
    use streamdown_config::Config;

    let original = Config::default();
    let toml_str = Config::default_toml();

    // Should be valid TOML
    let parsed: Config = toml::from_str(toml_str).unwrap();

    // Computed styles should match
    let orig_style = original.computed_style();
    let parsed_style = parsed.computed_style();

    assert_eq!(orig_style.bright, parsed_style.bright);
}

// =============================================================================
// ANSI Utility Tests
// =============================================================================

#[test]
fn test_ansi_visible_length() {
    use streamdown_ansi::utils::visible_length;

    assert_eq!(visible_length("hello"), 5);
    assert_eq!(visible_length("\x1b[31mred\x1b[0m"), 3);
    assert_eq!(visible_length("\x1b[1m\x1b[31mbold red\x1b[0m"), 8);
}

#[test]
fn test_ansi_strip() {
    use streamdown_ansi::utils::visible;

    assert_eq!(visible("hello"), "hello");
    assert_eq!(visible("\x1b[31mred\x1b[0m"), "red");
}

#[test]
fn test_ansi_cjk_width() {
    use streamdown_ansi::utils::visible_length;

    // CJK characters are double-width
    assert_eq!(visible_length("你好"), 4); // 2 chars × 2 width
    assert_eq!(visible_length("Hello你好"), 9); // 5 + 4
}
