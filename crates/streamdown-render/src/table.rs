//! Table rendering.
//!
//! Renders markdown tables with full-width columns and styled borders.

use crate::RenderStyle;
use crate::text::text_wrap;
use crate::{bg_color, fg_color};
use streamdown_ansi::codes::RESET;
use streamdown_ansi::utils::visible_length;
use streamdown_core::ColumnAlignment;
use streamdown_parser::inline::format_line;

/// Minimum column width (characters)
const MIN_COL_WIDTH: usize = 8;

/// Table rendering state.
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
    /// Buffered header cells
    pub header_cells: Option<Vec<String>>,
    /// Buffered body rows
    pub body_rows: Vec<Vec<String>>,
    /// Column alignments parsed from separator row
    pub column_alignments: Vec<ColumnAlignment>,
}

impl TableState {
    /// Create a new table state.
    pub fn new() -> Self {
        Self {
            is_header: true,
            column_widths: Vec::new(),
            num_columns: 0,
            available_width: 80,
            header_cells: None,
            body_rows: Vec::new(),
            column_alignments: Vec::new(),
        }
    }

    /// Get total table width including borders and padding
    pub fn total_width(&self) -> usize {
        let content: usize = self.column_widths.iter().sum();
        let borders = self.num_columns + 1;
        let padding = self.num_columns * 2;
        content + borders + padding
    }

    /// Mark that we've passed the separator row.
    pub fn end_header(&mut self) {
        self.is_header = false;
    }

    /// Reset for a new table.
    pub fn reset(&mut self) {
        self.is_header = true;
        self.column_widths.clear();
        self.num_columns = 0;
        self.header_cells = None;
        self.body_rows.clear();
        self.column_alignments.clear();
    }
}

impl Default for TableState {
    fn default() -> Self {
        Self::new()
    }
}

/// Render a horizontal border line for the buffered grid table.
fn render_horizontal_border(
    state: &TableState,
    left_margin: &str,
    border_fg: &str,
    left: &str,
    middle: &str,
    right: &str,
) -> String {
    let mut parts = Vec::with_capacity(state.num_columns);
    for i in 0..state.num_columns {
        let col_width = state.column_widths.get(i).copied().unwrap_or(MIN_COL_WIDTH);
        // Each column cell area is col_width + 2 (for padding spaces)
        parts.push("─".repeat(col_width + 2));
    }
    format!(
        "{}{}{}{}{}{}",
        left_margin,
        border_fg,
        left,
        parts.join(middle),
        right,
        RESET
    )
}

/// Render a content row (header or body) for the buffered grid table.
/// Accepts pre-formatted cells (already processed by `format_line`).
fn render_content_row(
    formatted_cells: &[String],
    state: &TableState,
    left_margin: &str,
    border_fg: &str,
    bg: &str,
    alignments: &[ColumnAlignment],
) -> Vec<String> {
    let num_cols = state.num_columns;

    // Wrap each pre-formatted cell's content to fit column width
    let mut wrapped_cells: Vec<Vec<String>> = Vec::with_capacity(num_cols);
    let mut max_height = 1;

    for i in 0..num_cols {
        let cell = formatted_cells.get(i).cloned().unwrap_or_default();
        let col_width = state.column_widths.get(i).copied().unwrap_or(MIN_COL_WIDTH);
        let wrapped = text_wrap(&cell, col_width, 0, "", "", true, true);

        let cell_lines = if wrapped.is_empty() {
            vec![String::new()]
        } else {
            wrapped.lines
        };

        max_height = max_height.max(cell_lines.len());
        wrapped_cells.push(cell_lines);
    }

    // Render each line of the row
    let mut result = Vec::with_capacity(max_height);

    for row_idx in 0..max_height {
        let mut line = String::new();
        line.push_str(left_margin);

        for col_idx in 0..num_cols {
            let col_width = state
                .column_widths
                .get(col_idx)
                .copied()
                .unwrap_or(MIN_COL_WIDTH);
            let content = wrapped_cells
                .get(col_idx)
                .and_then(|lines| lines.get(row_idx))
                .cloned()
                .unwrap_or_default();
            let content_len = visible_length(&content);
            let padding = col_width.saturating_sub(content_len);
            let alignment = alignments.get(col_idx).copied().unwrap_or(ColumnAlignment::Left);

            // Border then cell content with alignment
            match alignment {
                ColumnAlignment::Left => {
                    line.push_str(&format!(
                        "{}│{} {}{}{}",
                        border_fg, bg, content, " ".repeat(padding + 1), RESET
                    ));
                }
                ColumnAlignment::Right => {
                    line.push_str(&format!(
                        "{}│{}{}{}{}{}",
                        border_fg, bg, " ".repeat(padding + 1), content, " ", RESET
                    ));
                }
                ColumnAlignment::Center => {
                    let left_pad = padding / 2;
                    let right_pad = padding - left_pad;
                    line.push_str(&format!(
                        "{}│{}{}{}{}{}",
                        border_fg, bg, " ".repeat(left_pad + 1), content, " ".repeat(right_pad + 1), RESET
                    ));
                }
            }
        }

        // Closing border
        line.push_str(&format!("{}│{}", border_fg, RESET));

        result.push(line);
    }

    result
}

/// Render a complete buffered grid table with box-drawing borders.
pub fn render_buffered_table(
    state: &mut TableState,
    width: usize,
    left_margin: &str,
    style: &RenderStyle,
) -> Vec<String> {
    // If no header, nothing to render
    let header = match &state.header_cells {
        Some(h) => h.clone(),
        None => return Vec::new(),
    };

    // Set num_columns from header
    state.num_columns = header.len();
    if state.num_columns == 0 {
        return Vec::new();
    }

    // Pre-format ALL cells once (format-once-and-cache)
    let formatted_header: Vec<String> = header
        .iter()
        .map(|c| format_line(c, true, true))
        .collect();
    let formatted_body: Vec<Vec<String>> = state
        .body_rows
        .iter()
        .map(|row| row.iter().map(|c| format_line(c, true, true)).collect())
        .collect();

    // Compute column widths from pre-formatted strings
    state.available_width = width;
    let num_cols = state.num_columns;
    let mut max_widths = vec![0usize; num_cols];

    for (i, cell) in formatted_header.iter().enumerate() {
        if i < num_cols {
            max_widths[i] = max_widths[i].max(visible_length(cell));
        }
    }
    for row in &formatted_body {
        for (i, cell) in row.iter().enumerate() {
            if i < num_cols {
                max_widths[i] = max_widths[i].max(visible_length(cell));
            }
        }
    }

    // Apply MIN_COL_WIDTH floor
    for w in &mut max_widths {
        if *w < MIN_COL_WIDTH {
            *w = MIN_COL_WIDTH;
        }
    }

    // Check if content-based widths fit
    let overhead = num_cols * 2 + num_cols + 1; // padding + borders
    let content_sum: usize = max_widths.iter().sum();
    let total = content_sum + overhead;

    if total <= width {
        state.column_widths = max_widths;
    } else {
        // Fall back to proportional distribution
        let usable = width.saturating_sub(overhead);
        let base = (usable / num_cols).max(MIN_COL_WIDTH);
        let remainder = usable.saturating_sub(base * num_cols);

        state.column_widths = (0..num_cols)
            .map(|i| if i < remainder { base + 1 } else { base })
            .collect();
    }

    let border_fg = fg_color(&style.table_border);
    let header_bg = bg_color(&style.table_header_bg);
    let body_bg = bg_color(&style.table_body_bg);

    let mut lines = Vec::new();

    // Top border: ┌───┬───┐
    lines.push(render_horizontal_border(
        state,
        left_margin,
        &border_fg,
        "┌",
        "┬",
        "┐",
    ));

    // Header row (using pre-formatted cells)
    let header_lines =
        render_content_row(&formatted_header, state, left_margin, &border_fg, &header_bg, &state.column_alignments);
    lines.extend(header_lines);

    // Header separator (only if there are body rows)
    if !formatted_body.is_empty() {
        lines.push(render_horizontal_border(
            state,
            left_margin,
            &border_fg,
            "├",
            "┼",
            "┤",
        ));
    }

    // Body rows (using pre-formatted cells)
    let num_body_rows = formatted_body.len();
    for (i, row) in formatted_body.iter().enumerate() {
        let row_lines = render_content_row(row, state, left_margin, &border_fg, &body_bg, &state.column_alignments);
        lines.extend(row_lines);

        // Separator between body rows (not after last)
        if i + 1 < num_body_rows {
            lines.push(render_horizontal_border(
                state,
                left_margin,
                &border_fg,
                "├",
                "┼",
                "┤",
            ));
        }
    }

    // Bottom border: └───┴───┘
    lines.push(render_horizontal_border(
        state,
        left_margin,
        &border_fg,
        "└",
        "┴",
        "┘",
    ));

    lines
}

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

        let lines = render_buffered_table(&mut state, 80, "", &default_style());

        assert!(!lines.is_empty());
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

        let lines = render_buffered_table(&mut state, 80, "", &default_style());

        let joined = lines.join("\n");
        assert!(joined.contains('┌'));
        assert!(joined.contains('└'));
        assert!(!joined.contains('├'));
        assert!(!joined.contains('┼'));
    }

    #[test]
    fn test_render_buffered_table_no_header() {
        let mut state = TableState::new();
        state.header_cells = None;
        let lines = render_buffered_table(&mut state, 80, "", &default_style());
        assert!(lines.is_empty());
    }
}
