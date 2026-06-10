//! Snapshot tests for streamdown-rs output.
//!
//! These tests capture the rendered output and compare against stored snapshots.
//! Run with `cargo insta review` to update snapshots.

use streamdown_parser::Parser;
use streamdown_render::Renderer;

/// Helper to render markdown to a string.
fn render(input: &str, width: usize) -> String {
    let mut output = Vec::new();
    let mut parser = Parser::new();

    {
        let mut renderer = Renderer::new(&mut output, width);

        for line in input.lines() {
            for event in parser.parse_line(line) {
                renderer.render_event(&event).unwrap();
            }
        }

        // Flush any remaining state (e.g., buffered table)
        for event in parser.finalize() {
            renderer.render_event(&event).unwrap();
        }
    }

    // Strip ANSI codes for cleaner snapshots
    let raw = String::from_utf8(output).unwrap();
    streamdown_ansi::utils::visible(&raw)
}

// =============================================================================
// Heading Snapshots
// =============================================================================

#[test]
fn test_snapshot_heading_h1() {
    let output = render("# Hello World", 80);
    insta::assert_snapshot!(output);
}

#[test]
fn test_snapshot_heading_h2() {
    let output = render("## Section Title", 80);
    insta::assert_snapshot!(output);
}

#[test]
fn test_snapshot_heading_all_levels() {
    let input = r#"# H1
## H2
### H3
#### H4
##### H5
###### H6"#;
    let output = render(input, 80);
    insta::assert_snapshot!(output);
}

// =============================================================================
// Code Block Snapshots
// =============================================================================

#[test]
fn test_snapshot_code_block_rust() {
    let input = r#"```rust
fn main() {
    println!("Hello, world!");
}
```"#;
    let output = render(input, 80);
    insta::assert_snapshot!(output);
}

#[test]
fn test_snapshot_code_block_python() {
    let input = r#"```python
def hello():
    print("Hello, world!")
```"#;
    let output = render(input, 80);
    insta::assert_snapshot!(output);
}

#[test]
fn test_snapshot_code_block_no_language() {
    let input = r#"```
plain text
code block
```"#;
    let output = render(input, 80);
    insta::assert_snapshot!(output);
}

// =============================================================================
// List Snapshots
// =============================================================================

#[test]
fn test_snapshot_unordered_list() {
    let input = r#"- Item 1
- Item 2
- Item 3"#;
    let output = render(input, 80);
    insta::assert_snapshot!(output);
}

#[test]
fn test_snapshot_ordered_list() {
    let input = r#"1. First
2. Second
3. Third"#;
    let output = render(input, 80);
    insta::assert_snapshot!(output);
}

#[test]
fn test_snapshot_nested_list() {
    let input = r#"- Level 1
  - Level 2
    - Level 3
  - Back to 2
- Back to 1"#;
    let output = render(input, 80);
    insta::assert_snapshot!(output);
}

// =============================================================================
// Table Snapshots
// =============================================================================

#[test]
fn test_snapshot_simple_table() {
    let input = r#"| Name | Age |
|------|-----|
| Alice | 30 |
| Bob | 25 |"#;
    let output = render(input, 80);
    insta::assert_snapshot!(output);
}

#[test]
fn test_snapshot_wide_table() {
    let input = r#"| Column 1 | Column 2 | Column 3 | Column 4 |
|----------|----------|----------|----------|
| A | B | C | D |
| E | F | G | H |"#;
    let output = render(input, 100);
    insta::assert_snapshot!(output);
}

// =============================================================================
// Inline Formatting Snapshots
// =============================================================================

#[test]
fn test_snapshot_bold() {
    let output = render("This is **bold** text.", 80);
    insta::assert_snapshot!(output);
}

#[test]
fn test_snapshot_italic() {
    let output = render("This is *italic* text.", 80);
    insta::assert_snapshot!(output);
}

#[test]
fn test_snapshot_inline_code() {
    let output = render("Use `code` inline.", 80);
    insta::assert_snapshot!(output);
}

#[test]
fn test_snapshot_mixed_inline() {
    let output = render("**Bold**, *italic*, and `code` together.", 80);
    insta::assert_snapshot!(output);
}

// =============================================================================
// Block Quote Snapshots
// =============================================================================

#[test]
fn test_snapshot_blockquote() {
    let input = r#"> This is a quote.
> It spans multiple lines."#;
    let output = render(input, 80);
    insta::assert_snapshot!(output);
}

#[test]
fn test_snapshot_nested_blockquote() {
    let input = r#"> Level 1
>> Level 2
>>> Level 3"#;
    let output = render(input, 80);
    insta::assert_snapshot!(output);
}

// =============================================================================
// Think Block Snapshots
// =============================================================================

#[test]
fn test_snapshot_think_block() {
    let input = r#"<think>
This is internal reasoning.
It should be styled differently.
</think>"#;
    let output = render(input, 80);
    insta::assert_snapshot!(output);
}

// =============================================================================
// Horizontal Rule Snapshots
// =============================================================================

#[test]
fn test_snapshot_horizontal_rule() {
    let input = "---";
    let output = render(input, 80);
    insta::assert_snapshot!(output);
}

// =============================================================================
// Complex Document Snapshots
// =============================================================================

#[test]
fn test_snapshot_complex_document() {
    let input = r#"# Welcome

This is a **complex** document with *various* formatting.

## Code Example

```python
def greet(name):
    return f"Hello, {name}!"
```

## List of Features

- Headings
- Code blocks
- Lists
  - Nested items
- Tables

| Feature | Status |
|---------|--------|
| Parser  | Done   |
| Render  | Done   |

> A wise quote.

---

The end."#;
    let output = render(input, 80);
    insta::assert_snapshot!(output);
}

#[test]
fn test_snapshot_narrow_width() {
    let input = "This is a long paragraph that should wrap at a narrow width to test the text wrapping functionality.";
    let output = render(input, 40);
    insta::assert_snapshot!(output);
}

#[test]
fn test_snapshot_cjk_content() {
    let input = r#"# 你好世界

这是一段中文文本。

- 列表项 1
- 列表项 2"#;
    let output = render(input, 80);
    insta::assert_snapshot!(output);
}
