//! Table rendering.
//!
//! Renders markdown tables with full-width columns and styled borders.

use crate::text::text_wrap;
use crate::RenderStyle;
use crate::{bg_color, fg_color};
use streamdown_ansi::codes::RESET;
use streamdown_ansi::utils::visible_length;
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
}

impl TableState {
    /// Create a new table state.
    pub fn new() -> Self {
        Self {
            is_header: true,
            column_widths: Vec::new(),
            num_columns: 0,
            available_width: 80,
        }
    }

    /// Calculate column widths to fill the available width evenly.
    pub fn calculate_widths(&mut self, num_cols: usize, available_width: usize) {
        self.num_columns = num_cols;
        self.available_width = available_width;

        if num_cols == 0 {
            self.column_widths = Vec::new();
            return;
        }

        // Account for separators and padding
        // Each column has: " content " (2 chars padding)
        // Between columns: "│" (1 char)
        let separator_width = num_cols.saturating_sub(1);
        let padding_width = num_cols * 2;
        let content_width = available_width.saturating_sub(separator_width + padding_width);

        // Distribute evenly with remainder going to leftmost columns
        let base_width = (content_width / num_cols).max(MIN_COL_WIDTH);
        let remainder = content_width % num_cols;

        self.column_widths = (0..num_cols)
            .map(|i| {
                if i < remainder {
                    base_width + 1
                } else {
                    base_width
                }
            })
            .collect();
    }

    /// Get total table width including separators and padding
    pub fn total_width(&self) -> usize {
        let content: usize = self.column_widths.iter().sum();
        let separators = self.num_columns.saturating_sub(1);
        let padding = self.num_columns * 2;
        content + separators + padding
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
    }
}

impl Default for TableState {
    fn default() -> Self {
        Self::new()
    }
}

/// Render a table row with full-width columns.
pub fn render_table_row(
    cells: &[String],
    state: &mut TableState,
    width: usize,
    left_margin: &str,
    style: &RenderStyle,
    _is_last_row: bool,
) -> Vec<String> {
    let num_cols = cells.len();

    // Calculate column widths if not already done
    if state.column_widths.is_empty() || state.num_columns != num_cols {
        state.calculate_widths(num_cols, width);
    }

    // Choose background color based on header state
    let bg = if state.is_header {
        bg_color(&style.mid)
    } else {
        bg_color(&style.dark)
    };

    // Wrap each cell's content to fit column width
    let mut wrapped_cells: Vec<Vec<String>> = Vec::with_capacity(num_cols);
    let mut max_height = 1;

    for (i, cell) in cells.iter().enumerate() {
        let col_width = state.column_widths.get(i).copied().unwrap_or(MIN_COL_WIDTH);
        // Process inline markdown (bold, italic, code, etc.) before wrapping
        let formatted_cell = format_line(cell, true, true);
        let wrapped = text_wrap(&formatted_cell, col_width, 0, "", "", true, true);

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
    let separator_fg = fg_color(&style.symbol);

    for row_idx in 0..max_height {
        let mut line_parts = Vec::with_capacity(num_cols);

        for (col_idx, cell_lines) in wrapped_cells.iter().enumerate() {
            let col_width = state
                .column_widths
                .get(col_idx)
                .copied()
                .unwrap_or(MIN_COL_WIDTH);
            let content = cell_lines.get(row_idx).cloned().unwrap_or_default();
            let content_len = visible_length(&content);
            let padding = col_width.saturating_sub(content_len);

            // Format: bg + " " + content + padding + " "
            line_parts.push(format!("{} {}{}", bg, content, " ".repeat(padding + 1)));
        }

        // Join with separator
        let joined = line_parts.join(&format!("{}│{}", RESET, separator_fg));

        result.push(format!("{}{}{}{}", left_margin, joined, RESET, RESET));
    }

    result
}

/// Render a table separator row (the --- line).
pub fn render_table_separator(
    state: &TableState,
    width: usize,
    left_margin: &str,
    style: &RenderStyle,
) -> String {
    let fg = fg_color(&style.grey);

    // Use full width for separator
    let separator_width = if state.column_widths.is_empty() {
        width
    } else {
        state.total_width()
    };

    format!(
        "{}{}{}{}",
        left_margin,
        fg,
        "─".repeat(separator_width),
        RESET
    )
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
    }

    #[test]
    fn test_calculate_widths_full_width() {
        let mut state = TableState::new();
        state.calculate_widths(3, 80);

        assert_eq!(state.num_columns, 3);
        assert_eq!(state.column_widths.len(), 3);

        // Total should use most of the available width
        let total = state.total_width();
        assert!(
            total >= 70,
            "Table should use most of available width, got {}",
            total
        );
    }

    #[test]
    fn test_render_table_row() {
        let mut state = TableState::new();
        let cells = vec!["A".to_string(), "B".to_string(), "C".to_string()];
        let lines = render_table_row(&cells, &mut state, 80, "", &default_style(), false);

        assert!(!lines.is_empty());
        assert!(lines[0].contains("A"));
        assert!(lines[0].contains("B"));
        assert!(lines[0].contains("C"));
    }
}
