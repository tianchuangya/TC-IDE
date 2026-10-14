//! Shared terminal geometry, scrollback and selection core.
//!
//! The Windows backend owns the real ConPTY session, but most terminal logic is
//! plain arithmetic: turning a window size into a column/row grid, keeping a
//! scrollback offset inside its bounds, mapping a mouse position to a buffer
//! line and turning a drag into copied text. Keeping that arithmetic here means
//! `cargo test` on Linux covers logic that would otherwise only be validated by
//! a remote Windows build.
#![allow(dead_code)] // The Linux and macOS backends adopt these helpers next.

/// Smallest column count handed to a pseudo console.
pub const MIN_COLUMNS: i16 = 20;
/// Smallest row count handed to a pseudo console.
pub const MIN_ROWS: i16 = 4;

/// Height of the terminal panel header strip, in logical pixels.
pub const PANEL_HEADER_HEIGHT: f32 = 28.0;
/// Extra panel height added when the terminal is fully open, in logical pixels.
pub const PANEL_BODY_HEIGHT: f32 = 214.0;
/// Distance from the top of the panel to the first output row, in logical pixels.
pub const PANEL_OUTPUT_TOP: f32 = 38.0;
/// Width of the terminal list rail on the right edge, in logical pixels.
pub const PANEL_LIST_WIDTH: f32 = 168.0;
/// Left inset of terminal text, in logical pixels.
pub const PANEL_TEXT_LEFT: f32 = 22.0;
/// Right inset between terminal text and the list rail, in logical pixels.
pub const PANEL_TEXT_RIGHT: f32 = 12.0;
/// Space reserved below the output area for the input line, in logical pixels.
pub const PANEL_INPUT_HEIGHT: f32 = 34.0;
/// Row height of terminal output, in logical pixels.
pub const PANEL_ROW_HEIGHT: f32 = 24.0;
/// Output rows the panel keeps on screen at full height.
pub const PANEL_VISIBLE_ROWS: usize = 6;

/// Laid out geometry of the bottom terminal panel, in device pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TerminalPanel {
    /// Total panel height including the header strip.
    pub height: f32,
    /// Top edge of the scrollback area.
    pub body_top: f32,
    /// Bottom edge of the scrollback area, above the input line.
    pub body_bottom: f32,
    /// Left edge of terminal text.
    pub body_left: f32,
    /// Right edge of terminal text, left of the list rail.
    pub body_right: f32,
    /// Left edge of the terminal list rail.
    pub list_left: f32,
    /// Height of one output row.
    pub row_height: f32,
    /// Output rows that currently fit in the scrollback area.
    pub visible_rows: usize,
}

/// Compute the terminal panel geometry for a client area and DPI scale.
pub fn panel_metrics(width: f32, height: f32, scale: f32, progress: f32) -> TerminalPanel {
    let scale = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    };
    let progress = if progress.is_finite() {
        progress.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let width = if width.is_finite() {
        width.max(1.0)
    } else {
        1.0
    };
    let height = if height.is_finite() {
        height.max(1.0)
    } else {
        1.0
    };
    let height_total = (PANEL_HEADER_HEIGHT + PANEL_BODY_HEIGHT * progress) * scale;
    let row_height = PANEL_ROW_HEIGHT * scale;
    let body_top = height - height_total + PANEL_OUTPUT_TOP * scale;
    let body_bottom = (height - PANEL_INPUT_HEIGHT * scale).max(body_top);
    let list_left = (width - PANEL_LIST_WIDTH * scale).max(0.0);
    let body_left = (PANEL_TEXT_LEFT * scale).min(list_left);
    let body_right = (list_left - PANEL_TEXT_RIGHT * scale).max(body_left);
    let visible_rows = if row_height > 0.0 {
        (((body_bottom - body_top) / row_height).floor().max(0.0) as usize).max(1)
    } else {
        PANEL_VISIBLE_ROWS
    };
    TerminalPanel {
        height: height_total,
        body_top,
        body_bottom,
        body_left,
        body_right,
        list_left,
        row_height,
        visible_rows,
    }
}

/// Character advance of the editor code font for a zoom step, in logical pixels.
pub fn code_cell_width(code_zoom: i32) -> f32 {
    8.9 + code_zoom.clamp(-2, 8) as f32 * 0.6
}

/// Column/row grid handed to a pseudo console.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GridSize {
    pub columns: i16,
    pub rows: i16,
}

impl GridSize {
    /// Fit a grid of `cell_width` x `cell_height` cells into a pixel area.
    pub fn from_metrics(width: f32, height: f32, cell_width: f32, cell_height: f32) -> Self {
        Self {
            columns: fit_axis(width, cell_width, MIN_COLUMNS),
            rows: fit_axis(height, cell_height, MIN_ROWS),
        }
    }
}

fn fit_axis(available: f32, cell: f32, minimum: i16) -> i16 {
    if !available.is_finite() || !cell.is_finite() || cell <= 0.0 {
        return minimum;
    }
    let count = (available / cell).floor();
    if !count.is_finite() || count < 1.0 {
        return minimum;
    }
    (count.min(i16::MAX as f32) as i16).max(minimum)
}

/// Scrollback window over the buffered terminal lines.
///
/// `scroll` counts rows above the newest output, so `0` means "stuck to the
/// bottom" and larger values move the window into history.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct TerminalViewport {
    pub total_lines: usize,
    pub visible_rows: usize,
    pub scroll: usize,
}

impl TerminalViewport {
    pub fn max_scroll(&self) -> usize {
        self.total_lines.saturating_sub(self.visible_rows)
    }

    pub fn clamped_scroll(&self) -> usize {
        self.scroll.min(self.max_scroll())
    }

    /// Buffer index of the first row currently on screen.
    pub fn first_line(&self) -> usize {
        self.total_lines
            .saturating_sub(self.visible_rows.saturating_add(self.clamped_scroll()))
    }

    /// New scroll offset after a wheel or key scroll of `rows`.
    pub fn scroll_by(&self, rows: i32) -> usize {
        let maximum = self.max_scroll();
        if rows >= 0 {
            self.scroll.saturating_add(rows as usize).min(maximum)
        } else {
            self.scroll.saturating_sub(rows.saturating_neg() as usize)
        }
    }

    /// New scroll offset that returns to the newest output.
    pub fn scrolled_to_bottom(&self) -> usize {
        0
    }

    /// Buffer line shown at a viewport row.
    pub fn line_at_row(&self, row: usize) -> Option<usize> {
        if row >= self.visible_rows {
            return None;
        }
        let line = self.first_line().saturating_add(row);
        (line < self.total_lines).then_some(line)
    }

    /// Viewport row that shows a buffer line.
    pub fn row_of_line(&self, line: usize) -> Option<usize> {
        let first = self.first_line();
        if line < first
            || line >= first.saturating_add(self.visible_rows)
            || line >= self.total_lines
        {
            return None;
        }
        Some(line - first)
    }

    /// Whether the viewport is showing the newest output.
    pub fn is_at_bottom(&self) -> bool {
        self.clamped_scroll() == 0
    }
}

/// Rows currently on screen, in buffer order.
pub fn visible_lines<'a>(lines: &'a [String], viewport: &TerminalViewport) -> &'a [String] {
    let first = viewport.first_line();
    let last = first.saturating_add(viewport.visible_rows).min(lines.len());
    if first >= last {
        &[]
    } else {
        &lines[first..last]
    }
}

/// A character position inside the terminal buffer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct TerminalPoint {
    pub line: usize,
    pub column: usize,
}

/// Mouse drag selection over the terminal buffer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TerminalSelection {
    pub anchor: TerminalPoint,
    pub focus: TerminalPoint,
}

impl TerminalSelection {
    pub fn new(anchor: TerminalPoint, focus: TerminalPoint) -> Self {
        Self { anchor, focus }
    }

    /// Selection corners in reading order, so dragging backwards still works.
    pub fn ordered(&self) -> (TerminalPoint, TerminalPoint) {
        if self.anchor <= self.focus {
            (self.anchor, self.focus)
        } else {
            (self.focus, self.anchor)
        }
    }

    pub fn is_empty(&self) -> bool {
        self.anchor == self.focus
    }

    /// Selection extended to a new focus point.
    pub fn extended_to(&self, focus: TerminalPoint) -> Self {
        Self {
            anchor: self.anchor,
            focus,
        }
    }

    /// Whether a buffer position falls inside the selection.
    pub fn contains(&self, point: TerminalPoint) -> bool {
        let (start, end) = self.ordered();
        point >= start && point < end
    }
}

/// Build the selection covering a whole buffer line.
pub fn line_selection(line: usize, length: usize) -> TerminalSelection {
    TerminalSelection::new(
        TerminalPoint { line, column: 0 },
        TerminalPoint {
            line,
            column: length,
        },
    )
}

/// Text covered by a selection, with Windows line endings between lines.
pub fn selected_text(lines: &[String], selection: &TerminalSelection) -> Option<String> {
    if selection.is_empty() || lines.is_empty() {
        return None;
    }
    let last_line = lines.len() - 1;
    let (start, end) = selection.ordered();
    let start_line = start.line.min(last_line);
    let end_line = end.line.min(last_line).max(start_line);
    let mut result = String::new();
    for line in start_line..=end_line {
        let chars = lines[line].chars().collect::<Vec<_>>();
        let from = if line == start_line { start.column } else { 0 }.min(chars.len());
        let to = if line == end_line {
            end.column
        } else {
            chars.len()
        }
        .min(chars.len())
        .max(from);
        result.extend(chars[from..to].iter());
        if line != end_line {
            result.push_str("\r\n");
        }
    }
    if result.is_empty() {
        None
    } else {
        Some(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    #[test]
    fn grid_size_fits_cells_and_respects_minimums() {
        let grid = GridSize::from_metrics(800.0, 480.0, 8.0, 16.0);
        assert_eq!(grid.columns, 100);
        assert_eq!(grid.rows, 30);

        let tiny = GridSize::from_metrics(32.0, 8.0, 8.0, 16.0);
        assert_eq!(tiny.columns, MIN_COLUMNS);
        assert_eq!(tiny.rows, MIN_ROWS);

        let broken = GridSize::from_metrics(f32::NAN, 100.0, 0.0, 0.0);
        assert_eq!(broken.columns, MIN_COLUMNS);
        assert_eq!(broken.rows, MIN_ROWS);

        let mixed = GridSize::from_metrics(f32::NAN, 100.0, 0.0, 16.0);
        assert_eq!(mixed.columns, MIN_COLUMNS);
        assert_eq!(mixed.rows, 6);
    }

    #[test]
    fn grid_size_scales_with_dpi() {
        let logical = GridSize::from_metrics(800.0, 480.0, 8.0, 16.0);
        let hidpi = GridSize::from_metrics(1600.0, 960.0, 16.0, 32.0);
        assert_eq!(logical, hidpi);
    }

    #[test]
    fn panel_metrics_track_open_and_closed_states() {
        let closed = panel_metrics(1440.0, 900.0, 1.0, 0.0);
        assert_eq!(closed.height, PANEL_HEADER_HEIGHT);
        let open = panel_metrics(1440.0, 900.0, 1.0, 1.0);
        assert_eq!(open.height, PANEL_HEADER_HEIGHT + PANEL_BODY_HEIGHT);
        assert_eq!(open.body_top, 900.0 - open.height + PANEL_OUTPUT_TOP);
        assert_eq!(open.body_bottom, 900.0 - PANEL_INPUT_HEIGHT);
        assert_eq!(open.list_left, 1440.0 - PANEL_LIST_WIDTH);
        assert_eq!(open.body_right, open.list_left - PANEL_TEXT_RIGHT);
        assert_eq!(open.row_height, PANEL_ROW_HEIGHT);
        assert_eq!(open.visible_rows, 7);

        let hidpi = panel_metrics(2880.0, 1800.0, 2.0, 1.0);
        assert_eq!(hidpi.visible_rows, open.visible_rows);
    }

    #[test]
    fn panel_metrics_survive_tiny_and_invalid_windows() {
        let tiny = panel_metrics(40.0, 30.0, 1.0, 1.0);
        assert!(tiny.visible_rows >= 1);
        assert!(tiny.body_right >= tiny.body_left);
        let invalid = panel_metrics(f32::NAN, f32::NAN, 0.0, f32::NAN);
        assert_eq!(invalid.row_height, PANEL_ROW_HEIGHT);
        assert!(invalid.body_bottom >= invalid.body_top);
    }

    #[test]
    fn scrollback_clamps_inside_buffered_history() {
        let viewport = TerminalViewport {
            total_lines: 100,
            visible_rows: 20,
            scroll: 0,
        };
        assert_eq!(viewport.max_scroll(), 80);
        assert_eq!(viewport.first_line(), 80);
        assert!(viewport.is_at_bottom());

        let scrolled = TerminalViewport {
            scroll: viewport.scroll_by(10),
            ..viewport
        };
        assert_eq!(scrolled.scroll, 10);
        assert_eq!(scrolled.first_line(), 70);
        assert!(!scrolled.is_at_bottom());

        let beyond = TerminalViewport {
            scroll: viewport.scroll_by(500),
            ..viewport
        };
        assert_eq!(beyond.clamped_scroll(), 80);
        assert_eq!(beyond.first_line(), 0);
        assert_eq!(beyond.line_at_row(0), Some(0));
        assert_eq!(beyond.line_at_row(20), None);

        let back = TerminalViewport {
            scroll: scrolled.scroll_by(-4),
            ..viewport
        };
        assert_eq!(back.scroll, 6);
        let floor = TerminalViewport {
            scroll: scrolled.scroll_by(-40),
            ..viewport
        };
        assert_eq!(floor.scrolled_to_bottom(), 0);
    }

    #[test]
    fn short_buffers_do_not_produce_negative_ranges() {
        let viewport = TerminalViewport {
            total_lines: 3,
            visible_rows: 20,
            scroll: 9,
        };
        assert_eq!(viewport.max_scroll(), 0);
        assert_eq!(viewport.first_line(), 0);
        let buffer = lines(&["one", "two", "three"]);
        assert_eq!(visible_lines(&buffer, &viewport), buffer.as_slice());
        assert_eq!(viewport.row_of_line(2), Some(2));
        assert_eq!(viewport.row_of_line(3), None);
    }

    #[test]
    fn visible_lines_slice_follows_the_scroll_offset() {
        let buffer = lines(&["a", "b", "c", "d", "e"]);
        let viewport = TerminalViewport {
            total_lines: 5,
            visible_rows: 2,
            scroll: 0,
        };
        assert_eq!(visible_lines(&buffer, &viewport), &buffer[3..5]);
        let scrolled = TerminalViewport {
            scroll: 2,
            ..viewport
        };
        assert_eq!(visible_lines(&buffer, &scrolled), &buffer[1..3]);
    }

    #[test]
    fn selection_copies_forward_and_backward_drags() {
        let buffer = lines(&["alpha beta", "gamma", "delta"]);
        let forward = TerminalSelection::new(
            TerminalPoint { line: 0, column: 6 },
            TerminalPoint { line: 2, column: 3 },
        );
        assert_eq!(
            selected_text(&buffer, &forward).as_deref(),
            Some("beta\r\ngamma\r\ndel")
        );

        let backward = TerminalSelection::new(
            TerminalPoint { line: 2, column: 3 },
            TerminalPoint { line: 0, column: 6 },
        );
        assert_eq!(
            selected_text(&buffer, &backward),
            selected_text(&buffer, &forward)
        );
    }

    #[test]
    fn selection_stays_inside_unicode_lines() {
        let buffer = lines(&["你好 world"]);
        let selection = TerminalSelection::new(
            TerminalPoint { line: 0, column: 2 },
            TerminalPoint {
                line: 0,
                column: 99,
            },
        );
        assert_eq!(
            selected_text(&buffer, &selection).as_deref(),
            Some(" world")
        );

        let empty = TerminalSelection::new(
            TerminalPoint { line: 0, column: 4 },
            TerminalPoint { line: 0, column: 4 },
        );
        assert!(empty.is_empty());
        assert_eq!(selected_text(&buffer, &empty), None);
    }

    #[test]
    fn line_selection_and_containment_are_consistent() {
        let buffer = lines(&["one two"]);
        let selection = line_selection(0, buffer[0].chars().count());
        assert_eq!(
            selection.contains(TerminalPoint { line: 0, column: 0 }),
            true
        );
        assert_eq!(
            selection.contains(TerminalPoint { line: 0, column: 6 }),
            true
        );
        assert_eq!(
            selection.contains(TerminalPoint { line: 0, column: 7 }),
            false
        );
        assert_eq!(
            selected_text(&buffer, &selection).as_deref(),
            Some("one two")
        );
        let dragged = selection.extended_to(TerminalPoint { line: 1, column: 0 });
        assert_eq!(dragged.anchor, selection.anchor);
        assert_eq!(dragged.contains(TerminalPoint { line: 0, column: 3 }), true);
    }

    #[test]
    fn viewport_maps_points_for_hit_testing() {
        let viewport = TerminalViewport {
            total_lines: 40,
            visible_rows: 5,
            scroll: 3,
        };
        assert_eq!(viewport.first_line(), 32);
        assert_eq!(viewport.line_at_row(0), Some(32));
        assert_eq!(viewport.row_of_line(35), Some(3));
        assert_eq!(viewport.row_of_line(31), None);
    }

    #[test]
    fn code_cell_width_follows_zoom_clamping() {
        assert_eq!(code_cell_width(0), 8.9);
        assert_eq!(code_cell_width(-5), code_cell_width(-2));
        assert_eq!(code_cell_width(20), code_cell_width(8));
    }
}
