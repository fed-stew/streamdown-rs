# Buffered Grid Table Rendering

## Summary

Replace the current streaming table renderer with a buffered two-pass approach that produces fully-bordered grid tables with content-based column widths.

### Current behavior

- Tables render row-by-row as events arrive
- Only vertical `│` separators between columns and a single `─` line between header and body
- No outer border
- Columns stretch to fill terminal width evenly

### Target behavior

```
┌────────────┬─────────┬──────────┐
│  Language  │ Typing  │  Speed   │
├────────────┼─────────┼──────────┤
│ Rust       │ Static  │ Fast     │
├────────────┼─────────┼──────────┤
│ Python     │ Dynamic │ Moderate │
├────────────┼─────────┼──────────┤
│ JavaScript │ Dynamic │ Fast     │
└────────────┴─────────┴──────────┘
```

## Box-drawing characters

| Position | Character | Unicode |
|----------|-----------|---------|
| Top-left corner | `┌` | U+250C |
| Top-right corner | `┐` | U+2510 |
| Bottom-left corner | `└` | U+2514 |
| Bottom-right corner | `┘` | U+2518 |
| Top T-intersection | `┬` | U+252C |
| Bottom T-intersection | `┴` | U+2534 |
| Left T-intersection | `├` | U+251C |
| Right T-intersection | `┤` | U+2524 |
| Cross intersection | `┼` | U+253C |
| Horizontal | `─` | U+2500 |
| Vertical | `│` | U+2502 |

## Rendering model

### Buffered approach

Events are buffered rather than rendered immediately:

1. `TableHeader(cells)` — Reset state, store header cells in `TableState.header_cells`
2. `TableSeparator` — Mark header complete (flip `is_header` flag), no output
3. `TableRow(cells)` — Append to `TableState.body_rows`
4. `TableEnd` — Render complete table, then reset state

**Streaming tradeoff:** No table content appears until `TableEnd` arrives. This is an intentional tradeoff — content-based column sizing requires knowing all cells. Tables in markdown are typically small and complete quickly.

### Column width calculation

At `TableEnd`, compute column widths:

1. For each column, find the max visible length across header + all body rows (after inline markdown formatting)
2. Apply minimum width: `max(content_width, MIN_COL_WIDTH)`
3. Each column's rendered width = content_width + 2 (1 space padding each side)
4. Total table width = sum(column_rendered_widths) + num_columns + 1 (border characters: `num_columns - 1` internal separators + 2 outer walls)
5. If total table width exceeds available terminal width, fall back to proportional distribution that fits within terminal width, with each column clamped to at least `MIN_COL_WIDTH`

### Rendering sequence

At `TableEnd`, emit lines in this order:

1. **Top border:** `left_margin` + border_fg + `┌` + (`─` * col_width) + [`┬` + (`─` * col_width)]... + `┐` + RESET
2. **Header row:** For each wrapped line: `left_margin` + `│` + header_bg + ` content padding ` + [`│` + header_bg + ` content padding `]... + `│` + RESET
3. **Header separator:** `left_margin` + border_fg + `├` + (`─` * col_width) + [`┼` + (`─` * col_width)]... + `┤` + RESET
4. **For each body row (except last):**
   - Row content lines (same format as header but with body_bg)
   - Row separator: same format as header separator (`├...┼...┤`)
5. **Last body row:** Row content lines only (no separator after)
6. **Bottom border:** `left_margin` + border_fg + `└` + (`─` * col_width) + [`┴` + (`─` * col_width)]... + `┘` + RESET

### Edge case: header-only table (no body rows)

When `body_rows` is empty, skip the header separator (step 3) and go directly to the bottom border:

```
┌────┬────┐
│ A  │ B  │
└────┴────┘
```

### Cell content rendering

Each cell is formatted as: border_fg `│` + bg + ` ` + content + padding_spaces + ` `

- Content is processed through `format_line()` once and cached — the cached result is used for both width measurement and rendering to avoid double-formatting
- Measured with `visible_length()` for padding calculation
- Multi-line wrapping applies when content exceeds column width
- All cells in a row are padded to the same height (existing behavior preserved)

## Changes to `TableState`

Add two new fields:

```rust
pub struct TableState {
    pub is_header: bool,
    pub column_widths: Vec<usize>,
    pub num_columns: usize,
    pub available_width: usize,
    // New fields:
    pub header_cells: Option<Vec<String>>,
    pub body_rows: Vec<Vec<String>>,
}
```

- `header_cells` stores the header row when `TableHeader` fires
- `body_rows` accumulates rows from `TableRow` events
- Both are cleared on `reset()`:
  ```rust
  pub fn reset(&mut self) {
      self.is_header = true;
      self.column_widths.clear();
      self.num_columns = 0;
      self.header_cells = None;
      self.body_rows.clear();
  }
  ```

## Changes to `calculate_widths`

Replace even-distribution logic with content-based sizing:

1. Iterate all rows (header + body), format each cell, measure visible length
2. Track max width per column
3. Apply `MIN_COL_WIDTH` floor
4. Check if total fits in available width; if not, scale down proportionally

## Changes to event handlers in `lib.rs`

- `TableHeader` — Buffer cells, no output
- `TableSeparator` — Flip header flag, no output
- `TableRow` — Buffer cells, no output
- `TableEnd` — Call new `render_buffered_table()` function, emit all lines, reset state

## Styling

- All border characters use `table_border` color (existing field)
- Header cells use `table_header_bg` (existing field)
- Body cells use `table_body_bg` (existing field)
- Defaults remain unchanged
- Users who set different header/body colors still see the distinction

## Files to modify

1. `crates/streamdown-render/src/table.rs` — Core changes: add buffer fields to `TableState`, new `render_buffered_table()` function, new helper functions for border lines
2. `crates/streamdown-render/src/lib.rs` — Change event handlers to buffer instead of render immediately
3. `tests/snapshots/*.snap` — Update snapshot files to match new output format
