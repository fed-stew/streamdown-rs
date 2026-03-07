//! Custom style example: Render with custom colors.
//!
//! Run with: `cargo run --example custom_style`

use streamdown_parser::Parser;
use streamdown_render::{RenderStyle, Renderer};

fn main() {
    let markdown = r#"# Custom Styled Output

This example shows how to customize the colors and styling.

## Code Block

```python
def greet(name):
    return f"Hello, {name}!"

print(greet("World"))
```

## Features

- **Bold text** stands out
- *Italic text* is emphasized
- `inline code` is highlighted

> A quote with custom colors!
"#;

    // Create a custom style with different colors
    let custom_style = RenderStyle {
        // Headings: green gradient
        h1: "0;255;128".to_string(),
        h2: "0;220;128".to_string(),
        h3: "0;200;128".to_string(),
        h4: "0;180;128".to_string(),
        h5: "0;160;128".to_string(),
        h6: "0;140;128".to_string(),
        // Code blocks: dark blue background, cyan labels
        code_bg: "20;20;60".to_string(),
        code_fg: String::new(),
        code_dim: true,
        code_pad: true,
        code_label: "0;255;255".to_string(),
        // Lists: yellow bullets
        bullet: "255;255;0".to_string(),
        // Tables: purple tones
        table_header_bg: "80;60;120".to_string(),
        table_body_bg: "20;20;60".to_string(),
        table_border: "180;160;220".to_string(),
        // Borders and decorations
        blockquote_border: "0;255;255".to_string(),
        think_border: "128;128;128".to_string(),
        hr: "128;128;128".to_string(),
        // Links and references
        link_url: "0;255;255".to_string(),
        image_marker: "255;255;0".to_string(),
        footnote: "180;160;220".to_string(),
        // Left-align headings instead of centering
        heading_centered: false,
    };

    // Create output buffer
    let mut output = Vec::new();

    // Create parser
    let mut parser = Parser::new();

    // Get terminal width
    let width = terminal_size::terminal_size()
        .map(|(w, _)| w.0 as usize)
        .unwrap_or(80);

    {
        // Create renderer with custom style
        let mut renderer = Renderer::with_style(&mut output, width, custom_style);
        // Disable thick block borders (▄▄▄/▀▀▀)
        renderer.set_pretty_pad(false);

        // Parse and render
        for line in markdown.lines() {
            let events = parser.parse_line(line);
            for event in events {
                renderer.render_event(&event).unwrap();
            }
        }
    }

    // Print the styled output
    print!("{}", String::from_utf8(output).unwrap());
}
