# Buffered Grid Table Rendering Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the streaming table renderer with a buffered two-pass approach that produces fully-bordered grid tables with content-based column widths.

**Architecture:** Buffer all table rows during parsing events (`TableHeader`, `TableRow`), then at `TableEnd` compute content-based column widths and emit a complete grid table with box-drawing borders (`┌┬┐├┼┤└┴┘─│`). The old `render_table_row` and `render_table_separator` functions are replaced by a single `render_buffered_table` entry point.

**Tech Stack:** Rust, Unicode box-drawing characters, `streamdown-ansi` for ANSI codes/visible length, `insta` for snapshot tests.

**Spec:** `docs/superpowers/specs/2026-03-16-buffered-grid-table-rendering-design.md`

---

## Chunk 1: Core Implementation

### Task 1: Add buffer fields to `TableState` and update `reset()`/`new()`

**Files:**
- Modify: `crates/streamdown-render/src/table.rs:15-96`

- [ ] **Step 1: Add `header_cells` and `body_rows` fields to `TableState`**

In `crates/streamdown-render/src/table.rs`, update the `TableState` struct to add two new fields:

```rust
#[derive(Debug, Clone)]
pub struct TableState {
    /// Whether we're in the header
    pub is_header: bool,
    /// Column widths (calculated to fill available width)
    pub column_widths: Vec<usize>,
    /// Number of columns
    pub num_columns: usize,
    /// Available width for the table
    pub available_width: usize,
    /// Buffered header cells (for deferred rendering)
    pub header_cells: Option<Vec<String>>,
    /// Buffered body rows (for deferred rendering)
    pub body_rows: Vec<Vec<String>>,
}
```

- [ ] **Step 2: Update `new()` to initialize the new fields**

```rust
pub fn new() -> Self {
    Self {
        is_header: true,
        column_widths: Vec::new(),
        num_columns: 0,
        available_width: 80,
        header_cells: None,
        body_rows: Vec::new(),
    }
}
```

- [ ] **Step 3: Update `reset()` to clear the new fields**

```rust
pub fn reset(&mut self) {
    self.is_header = true;
    self.column_widths.clear();
    self.num_columns = 0;
    self.header_cells = None;
    self.body_rows.clear();
}
```

- [ ] **Step 4: Run `cargo build` to verify it compiles**

Run: `cargo build 2>&1 | head -20`
Expected: Compiles successfully (existing code still references old functions, that's fine).

---

### Task 2: Implement `render_buffered_table` and helper functions

**Files:**
- Modify: `crates/streamdown-render/src/table.rs`

This is the core rendering function. It replaces `render_table_row` and `render_table_separator`.

- [ ] **Step 1: Add content-based `calculate_content_widths` method to `TableState`**

This new method computes column widths from actual cell content rather than distributing evenly. Add it as a method on `TableState`:

```rust
/// Calculate column widths based on content, falling back to even distribution if too wide.
pub fn calculate_content_widths(&mut self, available_width: usize) {
    let num_cols = self.num_columns;
    if num_cols == 0 {
        return;
    }
    self.available_width = available_width;

    // Measure max visible content length per column
    let mut max_widths = vec![0usize; num_cols];

    if let Some(ref header) = self.header_cells {
        for (i, cell) in header.iter().enumerate() {
            if i < num_cols {
                let formatted = format_line(cell, true, true);
                let len = visible_length(&formatted);
                max_widths[i] = max_widths[i].max(len);
            }
        }
    }

    for row in &self.body_rows {
        for (i, cell) in row.iter().enumerate() {
            if i < num_cols {
                let formatted = format_line(cell, true, true);
                let len = visible_length(&formatted);
                max_widths[i] = max_widths[i].max(len);
            }
        }
    }

    // Apply minimum width
    for w in &mut max_widths {
        *w = (*w).max(MIN_COL_WIDTH);
    }

    // Check if total fits: each column = content_width + 2 (padding), plus num_cols + 1 borders
    let borders = num_cols + 1;
    let padding = num_cols * 2;
    let content_total: usize = max_widths.iter().sum();
    let total = content_total + padding + borders;

    if total <= available_width {
        // Content-based sizing fits
        self.column_widths = max_widths;
    } else {
        // Fall back to proportional distribution
        let budget = available_width.saturating_sub(borders + padding);
        let base = (budget / num_cols).max(MIN_COL_WIDTH);
        let remainder = budget % num_cols;
        self.column_widths = (0..num_cols)
            .map(|i| if i < remainder { base + 1 } else { base })
            .collect();
    }
}
```

- [ ] **Step 2: Update `total_width` to account for outer borders**

```rust
/// Get total table width including borders, separators, and padding.
pub fn total_width(&self) -> usize {
    let content: usize = self.column_widths.iter().sum();
    let borders = self.num_columns + 1; // left wall + right wall + internal separators
    let padding = self.num_columns * 2;
    content + borders + padding
}
```

- [ ] **Step 3: Write the `render_buffered_table` function**

Add this function after the existing functions (before `#[cfg(test)]`):

```rust
/// Render a complete buffered table with full grid borders.
///
/// Called at `TableEnd` after all rows have been buffered in `state`.
pub fn render_buffered_table(
    state: &mut TableState,
    width: usize,
    left_margin: &str,
    style: &RenderStyle,
) -> Vec<String> {
    // Need at least a header to render
    let header = match &state.header_cells {
        Some(h) => h.clone(),
        None => return Vec::new(),
    };

    let num_cols = header.len();
    if num_cols == 0 {
        return Vec::new();
    }

    state.num_columns = num_cols;
    state.calculate_content_widths(width);

    let border_fg = fg_color(&style.table_border);
    let header_bg = bg_color(&style.table_header_bg);
    let body_bg = bg_color(&style.table_body_bg);

    let mut lines = Vec::new();

    // 1. Top border: ┌────┬────┐
    lines.push(render_horizontal_border(
        state, left_margin, &border_fg, '┌', '┬', '┐',
    ));

    // 2. Header row
    let header_lines = render_content_row(&header, state, left_margin, &border_fg, &header_bg);
    lines.extend(header_lines);

    if state.body_rows.is_empty() {
        // Header-only table: skip separator, go to bottom border
    } else {
        // 3. Header separator: ├────┼────┤
        lines.push(render_horizontal_border(
            state, left_margin, &border_fg, '├', '┼', '┤',
        ));

        // 4. Body rows with separators between them
        let body_rows = state.body_rows.clone();
        for (i, row) in body_rows.iter().enumerate() {
            let row_lines = render_content_row(row, state, left_margin, &border_fg, &body_bg);
            lines.extend(row_lines);

            // Separator between body rows (not after last)
            if i < body_rows.len() - 1 {
                lines.push(render_horizontal_border(
                    state, left_margin, &border_fg, '├', '┼', '┤',
                ));
            }
        }
    }

    // 6. Bottom border: └────┴────┘
    lines.push(render_horizontal_border(
        state, left_margin, &border_fg, '└', '┴', '┘',
    ));

    lines
}

/// Render a horizontal border line (top, separator, or bottom).
fn render_horizontal_border(
    state: &TableState,
    left_margin: &str,
    border_fg: &str,
    left: char,
    middle: char,
    right: char,
) -> String {
    let mut line = format!("{}{}{}", left_margin, border_fg, left);

    for (i, col_width) in state.column_widths.iter().enumerate() {
        // col_width is content width; add 2 for padding
        line.push_str(&"─".repeat(col_width + 2));
        if i < state.column_widths.len() - 1 {
            line.push(middle);
        }
    }

    line.push(right);
    line.push_str(RESET);
    line
}

/// Render a content row (header or body) with outer borders.
fn render_content_row(
    cells: &[String],
    state: &TableState,
    left_margin: &str,
    border_fg: &str,
    bg: &str,
) -> Vec<String> {
    let num_cols = state.num_columns;

    // Format and wrap each cell
    let mut wrapped_cells: Vec<Vec<String>> = Vec::with_capacity(num_cols);
    let mut max_height = 1;

    for i in 0..num_cols {
        let cell_text = cells.get(i).map(|s| s.as_str()).unwrap_or("");
        let col_width = state.column_widths.get(i).copied().unwrap_or(MIN_COL_WIDTH);
        let formatted = format_line(cell_text, true, true);
        let wrapped = text_wrap(&formatted, col_width, 0, "", "", true, true);

        let cell_lines = if wrapped.is_empty() {
            vec![String::new()]
        } else {
            wrapped.lines
        };

        max_height = max_height.max(cell_lines.len());
        wrapped_cells.push(cell_lines);
    }

    // Render each visual line of the row
    let mut result = Vec::with_capacity(max_height);

    for row_idx in 0..max_height {
        let mut line = format!("{}{}{}", left_margin, border_fg, "│");

        for (col_idx, cell_lines) in wrapped_cells.iter().enumerate() {
            let col_width = state.column_widths.get(col_idx).copied().unwrap_or(MIN_COL_WIDTH);
            let content = cell_lines.get(row_idx).cloned().unwrap_or_default();
            let content_len = visible_length(&content);
            let padding = col_width.saturating_sub(content_len);

            // bg + space + content + padding + space
            line.push_str(&format!(
                "{} {}{}{}{}│",
                bg, content, " ".repeat(padding + 1), RESET, border_fg
            ));
        }

        line.push_str(RESET);
        result.push(line);
    }

    result
}
```

- [ ] **Step 4: Run `cargo build` to verify it compiles**

Run: `cargo build 2>&1 | head -20`
Expected: Compiles successfully.

---

### Task 3: Update event handlers in `lib.rs` to buffer and render at `TableEnd`

**Files:**
- Modify: `crates/streamdown-render/src/lib.rs:46,531-569`

- [ ] **Step 1: Update the public export in `lib.rs`**

At line 46, change the table exports:

```rust
pub use table::{TableState, render_buffered_table};
```

(Remove `render_table_row` and `render_table_separator` from the public exports since they are no longer needed externally.)

- [ ] **Step 2: Update `TableHeader` handler to buffer instead of render**

Replace lines 531-543:

```rust
ParseEvent::TableHeader(cells) => {
    self.table_state.reset();
    self.table_state.is_header = true;
    self.table_state.header_cells = Some(cells.clone());
}
```

- [ ] **Step 3: Update `TableSeparator` handler to just flip the flag**

Replace lines 556-565:

```rust
ParseEvent::TableSeparator => {
    self.table_state.end_header();
}
```

- [ ] **Step 4: Update `TableRow` handler to buffer**

Replace lines 545-554:

```rust
ParseEvent::TableRow(cells) => {
    self.table_state.body_rows.push(cells.clone());
}
```

- [ ] **Step 5: Update `TableEnd` handler to render the complete table**

Replace lines 567-569:

```rust
ParseEvent::TableEnd => {
    let width = self.current_width();
    let margin = self.left_margin();
    let style = self.style.clone();
    let lines = render_buffered_table(
        &mut self.table_state,
        width,
        &margin,
        &style,
    );
    for line in lines {
        self.writeln(&line)?;
    }
    self.table_state.reset();
}
```

- [ ] **Step 6: Run `cargo build` to verify it compiles**

Run: `cargo build 2>&1 | head -20`
Expected: Compiles successfully.

---

### Task 4: Fix snapshot tests to call `finalize()` and update snapshots

**Files:**
- Modify: `tests/snapshots.rs:10-27`
- Update: `tests/snapshots/snapshots__snapshot_simple_table.snap`
- Update: `tests/snapshots/snapshots__snapshot_wide_table.snap`
- Update: `tests/snapshots/snapshots__snapshot_complex_document.snap`

**Critical context:** The current snapshot test `render()` function does NOT call `parser.finalize()` after processing all lines. This means `TableEnd` is never emitted for tables at the end of input. With the buffered approach, no table output appears without `TableEnd`. This must be fixed.

- [ ] **Step 1: Add `finalize()` call to the snapshot test `render()` function**

In `tests/snapshots.rs`, update the `render` function (lines 10-27):

```rust
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
```

- [ ] **Step 2: Update snapshot files**

Run: `cargo test --test snapshots -- --ignored 2>&1; cargo insta review 2>&1 || cargo test --test snapshots 2>&1 | head -50`

If using `insta`, run `cargo insta test --test snapshots` followed by `cargo insta accept` to accept the new snapshots.

If `insta` CLI is not installed, delete the existing snapshot files for table tests and re-run:

```bash
rm tests/snapshots/snapshots__snapshot_simple_table.snap
rm tests/snapshots/snapshots__snapshot_wide_table.snap
rm tests/snapshots/snapshots__snapshot_complex_document.snap
cargo test --test snapshots 2>&1 | head -30
```

Then verify the new `.snap.new` files contain the expected grid table output and rename them.

- [ ] **Step 3: Verify all snapshot tests pass**

Run: `cargo test --test snapshots 2>&1`
Expected: All tests pass.

---

### Task 5: Update unit tests in `table.rs` and `lib.rs`

**Files:**
- Modify: `crates/streamdown-render/src/table.rs:196-239` (unit tests)
- Modify: `crates/streamdown-render/src/lib.rs:819-841` (table integration test)

- [ ] **Step 1: Update unit tests in `table.rs`**

Replace the existing `#[cfg(test)] mod tests` block at the bottom of `table.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn default_style() -> RenderStyle {
        RenderStyle::default()
    }

    #[test]
    fn test_table_state_new() {
        let state = TableState::new();
        assert!(state.is_header);
        assert!(state.column_widths.is_empty());
        assert!(state.header_cells.is_none());
        assert!(state.body_rows.is_empty());
    }

    #[test]
    fn test_table_state_reset_clears_buffers() {
        let mut state = TableState::new();
        state.header_cells = Some(vec!["A".to_string()]);
        state.body_rows.push(vec!["1".to_string()]);
        state.reset();
        assert!(state.header_cells.is_none());
        assert!(state.body_rows.is_empty());
    }

    #[test]
    fn test_calculate_content_widths_fits() {
        let mut state = TableState::new();
        state.num_columns = 3;
        state.header_cells = Some(vec![
            "Language".to_string(),
            "Typing".to_string(),
            "Speed".to_string(),
        ]);
        state.body_rows = vec![
            vec!["Rust".to_string(), "Static".to_string(), "Fast".to_string()],
            vec!["JavaScript".to_string(), "Dynamic".to_string(), "Fast".to_string()],
        ];
        state.calculate_content_widths(80);

        // "JavaScript" is 10 chars, should be the widest in col 0
        assert_eq!(state.column_widths[0], 10);
        // "Dynamic" is 7 chars, but MIN_COL_WIDTH is 8
        assert_eq!(state.column_widths[1], 8);
        // "Speed" is 5 chars, but MIN_COL_WIDTH is 8
        assert_eq!(state.column_widths[2], 8);
    }

    #[test]
    fn test_calculate_content_widths_fallback() {
        let mut state = TableState::new();
        state.num_columns = 3;
        state.header_cells = Some(vec![
            "A".to_string(),
            "B".to_string(),
            "C".to_string(),
        ]);
        state.body_rows = vec![];
        // Very narrow width forces fallback
        state.calculate_content_widths(20);

        // Should still have 3 columns, each at MIN_COL_WIDTH
        assert_eq!(state.column_widths.len(), 3);
        for w in &state.column_widths {
            assert!(*w >= MIN_COL_WIDTH);
        }
    }

    #[test]
    fn test_total_width_with_borders() {
        let mut state = TableState::new();
        state.num_columns = 3;
        state.column_widths = vec![10, 8, 8];
        // total = 10+8+8 (content) + 4 (borders: 3+1) + 6 (padding: 3*2) = 36
        assert_eq!(state.total_width(), 36);
    }

    #[test]
    fn test_render_buffered_table_basic() {
        let mut state = TableState::new();
        state.header_cells = Some(vec!["A".to_string(), "B".to_string()]);
        state.body_rows = vec![
            vec!["1".to_string(), "2".to_string()],
        ];
        state.num_columns = 2;

        let lines = render_buffered_table(&mut state, 80, "", &default_style());

        assert!(!lines.is_empty());
        // Check for grid border characters
        let joined = lines.join("\n");
        assert!(joined.contains('┌'));
        assert!(joined.contains('┐'));
        assert!(joined.contains('└'));
        assert!(joined.contains('┘'));
        assert!(joined.contains('├'));
        assert!(joined.contains('┤'));
        assert!(joined.contains('┼'));
        assert!(joined.contains('┬'));
        assert!(joined.contains('┴'));
        assert!(joined.contains('│'));
        assert!(joined.contains('─'));
    }

    #[test]
    fn test_render_buffered_table_header_only() {
        let mut state = TableState::new();
        state.header_cells = Some(vec!["A".to_string(), "B".to_string()]);
        state.body_rows = vec![];
        state.num_columns = 2;

        let lines = render_buffered_table(&mut state, 80, "", &default_style());

        let joined = lines.join("\n");
        // Should have top border, header row, bottom border — no ├┼┤ separator
        assert!(joined.contains('┌'));
        assert!(joined.contains('└'));
        assert!(!joined.contains('├'));
        assert!(!joined.contains('┼'));
    }

    #[test]
    fn test_render_buffered_table_no_header() {
        let mut state = TableState::new();
        // No header buffered
        state.header_cells = None;
        let lines = render_buffered_table(&mut state, 80, "", &default_style());
        assert!(lines.is_empty());
    }
}
```

- [ ] **Step 2: Update the table integration test in `lib.rs`**

The existing test at `lib.rs:819-841` sends events in sequence but doesn't test the final output well since with buffering, output only appears at `TableEnd`. Update it:

```rust
#[test]
fn test_render_table() {
    let mut output = Vec::new();
    let mut renderer = Renderer::new(&mut output, 80);

    renderer
        .render_event(&ParseEvent::TableHeader(vec![
            "A".to_string(),
            "B".to_string(),
        ]))
        .unwrap();
    renderer.render_event(&ParseEvent::TableSeparator).unwrap();
    renderer
        .render_event(&ParseEvent::TableRow(vec![
            "1".to_string(),
            "2".to_string(),
        ]))
        .unwrap();
    renderer.render_event(&ParseEvent::TableEnd).unwrap();

    let result = String::from_utf8(output).unwrap();
    assert!(result.contains("A"));
    assert!(result.contains("1"));
    // Verify grid border characters are present
    assert!(result.contains("┌"));
    assert!(result.contains("└"));
    assert!(result.contains("│"));
}
```

- [ ] **Step 3: Run all tests**

Run: `cargo test 2>&1 | tail -20`
Expected: All tests pass.

---

### Task 6: Remove dead code

**Files:**
- Modify: `crates/streamdown-render/src/table.rs`

- [ ] **Step 1: Remove old `render_table_row` and `render_table_separator` functions**

After verifying all tests pass, remove the old `render_table_row` function (lines 98-169) and `render_table_separator` function (lines 171-194) from `table.rs`. Also remove the old `calculate_widths` method from `TableState` if it's no longer called.

- [ ] **Step 2: Check for any remaining references to removed functions**

Run: `grep -r "render_table_row\|render_table_separator\|calculate_widths" crates/ tests/ --include="*.rs"`

Fix any remaining references.

- [ ] **Step 3: Run full test suite**

Run: `cargo test 2>&1 | tail -20`
Expected: All tests pass, no compile errors.

- [ ] **Step 4: Commit all changes**

```bash
git add -A
git commit -m "feat: buffered grid table rendering with content-based column widths"
```
