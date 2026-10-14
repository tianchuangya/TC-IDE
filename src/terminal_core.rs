//! Shared terminal geometry, scrollback and selection core.
//!
//! The Windows backend owns the real ConPTY session, but most terminal logic is
//! plain arithmetic: turning a window size into a column/row grid, keeping a
//! scrollback offset inside its bounds, mapping a mouse position to a buffer
//! line and turning a drag into copied text. Keeping that arithmetic here means
//! `cargo test` on Linux covers logic that would otherwise only be validated by
//! a remote Windows build.
#![allow(dead_code)] // The Linux and macOS backends adopt these helpers next.

use std::sync::{Arc, Mutex};

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

/// Grid cells a single character occupies in a monospace grid.
pub fn character_cells(value: char) -> usize {
    match value as u32 {
        0x1100..=0x115f
        | 0x2329..=0x232a
        | 0x2e80..=0xa4cf
        | 0xac00..=0xd7a3
        | 0xf900..=0xfaff
        | 0xfe10..=0xfe6f
        | 0xff00..=0xff60
        | 0xffe0..=0xffe6
        | 0x1f300..=0x1faff
        | 0x20000..=0x3fffd => 2,
        _ => 1,
    }
}

/// Grid cells the first `columns` characters of a line occupy.
pub fn visual_cells(value: &str, columns: usize) -> usize {
    value.chars().take(columns).map(character_cells).sum()
}

/// Character index whose grid cell range contains a cell offset.
pub fn column_for_cells(value: &str, cells: usize) -> usize {
    let mut used = 0;
    for (index, ch) in value.chars().enumerate() {
        if used >= cells {
            return index;
        }
        used += character_cells(ch);
    }
    value.chars().count()
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
    for (line, value) in lines.iter().enumerate().take(end_line + 1).skip(start_line) {
        let chars = value.chars().collect::<Vec<_>>();
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

/// Environment block of a Windows child process: `KEY=VALUE` entries, sorted
/// case insensitively and terminated by an extra NUL, as `CreateProcessW` wants.
pub fn environment_block(entries: &[(String, String)]) -> Vec<u16> {
    let mut sorted = entries.to_vec();
    sorted.sort_by(|left, right| {
        left.0
            .to_ascii_uppercase()
            .cmp(&right.0.to_ascii_uppercase())
    });
    sorted.dedup_by(|left, right| left.0.eq_ignore_ascii_case(&right.0));
    let mut block = Vec::new();
    for (key, value) in sorted {
        if key.is_empty() || key.contains('=') || key.contains('\0') {
            continue;
        }
        for unit in key.encode_utf16() {
            block.push(unit);
        }
        block.push('=' as u16);
        for unit in value.encode_utf16() {
            block.push(unit);
        }
        block.push(0);
    }
    block.push(0);
    block
}

/// Output lines a session keeps before dropping the oldest ones./// Output lines a session keeps before dropping the oldest ones.
pub const MAX_SCROLLBACK_LINES: usize = 800;

/// Incremental terminal scrollback shared by every backend.
#[derive(Default)]
pub struct TerminalView {
    pub lines: Vec<String>,
    pub current: String,
}

enum EscapeState {
    Text,
    Escape,
    Csi(String),
    Osc,
    OscEscape,
}

/// Incremental VT parser: text, CSI cursor/erase and OSC skipping.
pub struct TerminalParser {
    lines: Vec<String>,
    current: Vec<char>,
    cursor: usize,
    state: EscapeState,
}

impl TerminalParser {
    pub fn new() -> Self {
        Self {
            lines: vec![
                "TianChuangYa  /  TC-IDE native ConPTY".into(),
                "GitHub: https://github.com/tianchuangya/TC-IDE".into(),
            ],
            current: Vec::new(),
            cursor: 0,
            state: EscapeState::Text,
        }
    }

    pub fn feed(&mut self, value: &str) {
        for ch in value.chars() {
            let state = std::mem::replace(&mut self.state, EscapeState::Text);
            self.state = match state {
                EscapeState::Text => self.feed_text(ch),
                EscapeState::Escape if ch == '[' => EscapeState::Csi(String::new()),
                EscapeState::Escape if ch == ']' => EscapeState::Osc,
                EscapeState::Escape => EscapeState::Text,
                EscapeState::Csi(mut sequence) => {
                    if ('@'..='~').contains(&ch) {
                        self.apply_csi(&sequence, ch);
                        EscapeState::Text
                    } else {
                        sequence.push(ch);
                        EscapeState::Csi(sequence)
                    }
                }
                EscapeState::Osc if ch == '\u{7}' => EscapeState::Text,
                EscapeState::Osc if ch == '\u{1b}' => EscapeState::OscEscape,
                EscapeState::Osc => EscapeState::Osc,
                EscapeState::OscEscape if ch == '\\' => EscapeState::Text,
                EscapeState::OscEscape => EscapeState::Osc,
            };
        }
    }

    fn feed_text(&mut self, ch: char) -> EscapeState {
        match ch {
            '\u{1b}' => return EscapeState::Escape,
            '\r' => self.cursor = 0,
            '\n' => {
                self.lines.push(self.current.iter().collect());
                self.current.clear();
                self.cursor = 0;
                if self.lines.len() > MAX_SCROLLBACK_LINES {
                    let remove = self.lines.len() - MAX_SCROLLBACK_LINES;
                    self.lines.drain(..remove);
                }
            }
            '\u{8}' => self.cursor = self.cursor.saturating_sub(1),
            value if !value.is_control() => {
                while self.current.len() < self.cursor {
                    self.current.push(' ');
                }
                if self.cursor < self.current.len() {
                    self.current[self.cursor] = value;
                } else {
                    self.current.push(value);
                }
                self.cursor += 1;
            }
            _ => {}
        }
        EscapeState::Text
    }

    fn apply_csi(&mut self, sequence: &str, command: char) {
        let params = sequence
            .trim_start_matches('?')
            .split(';')
            .map(|value| value.parse::<usize>().unwrap_or(0))
            .collect::<Vec<_>>();
        let first = params.first().copied().unwrap_or(0);
        match command {
            'C' => self.cursor = self.cursor.saturating_add(first.max(1)),
            'D' => self.cursor = self.cursor.saturating_sub(first.max(1)),
            'G' => self.cursor = first.max(1) - 1,
            'K' => match first {
                1 => {
                    for value in self.current.iter_mut().take(self.cursor.saturating_add(1)) {
                        *value = ' ';
                    }
                }
                2 => {
                    self.current.clear();
                    self.cursor = 0;
                }
                _ => self.current.truncate(self.cursor),
            },
            'J' if first == 2 || first == 3 => {
                self.lines.clear();
                self.current.clear();
                self.cursor = 0;
            }
            _ => {}
        }
    }

    pub fn publish(&self, target: &Arc<Mutex<TerminalView>>) {
        if let Ok(mut view) = target.lock() {
            view.lines.clone_from(&self.lines);
            view.current = self.current.iter().collect();
        }
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
        assert!(selection.contains(TerminalPoint { line: 0, column: 0 }));
        assert!(selection.contains(TerminalPoint { line: 0, column: 6 }));
        assert!(!selection.contains(TerminalPoint { line: 0, column: 7 }));
        assert_eq!(
            selected_text(&buffer, &selection).as_deref(),
            Some("one two")
        );
        let dragged = selection.extended_to(TerminalPoint { line: 1, column: 0 });
        assert_eq!(dragged.anchor, selection.anchor);
        assert!(dragged.contains(TerminalPoint { line: 0, column: 3 }));
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

    #[test]
    fn environment_blocks_are_sorted_and_double_terminated() {
        let block = environment_block(&[
            ("Path".into(), "C:\\bin".into()),
            ("TC_IDE_AI_KEY".into(), "secret".into()),
            ("path".into(), "ignored".into()),
            ("BAD=KEY".into(), "x".into()),
            (String::new(), "y".into()),
        ]);
        assert_eq!(*block.last().unwrap(), 0);
        assert_eq!(block[block.len() - 2], 0);
        let text = String::from_utf16_lossy(&block);
        let entries = text
            .split('\0')
            .filter(|entry| !entry.is_empty())
            .collect::<Vec<_>>();
        assert_eq!(entries, ["Path=C:\\bin", "TC_IDE_AI_KEY=secret"]);
    }

    #[test]
    fn parses_virtual_terminal_sequences_and_live_prompt() {
        let mut parser = TerminalParser::new();
        parser.feed("\u{1b}[31mred\u{1b}[0m\r\nPS C:\\repo> ");
        assert_eq!(parser.lines.last().map(String::as_str), Some("red"));
        assert_eq!(parser.current.iter().collect::<String>(), "PS C:\\repo> ");
    }

    #[test]
    fn carriage_return_overwrites_the_current_line() {
        let mut parser = TerminalParser::new();
        parser.feed("progress 10%\rprogress 90%\u{1b}[K");
        assert_eq!(parser.current.iter().collect::<String>(), "progress 90%");
    }

    #[test]
    fn osc_titles_never_reach_the_buffer() {
        let mut parser = TerminalParser::new();
        parser.feed("\u{1b}]0;window title\u{7}ready");
        parser.feed("\u{1b}]0;second\u{1b}\\done");
        assert_eq!(parser.current.iter().collect::<String>(), "readydone");
        assert!(!parser
            .lines
            .iter()
            .any(|line| line.contains("window title")));
    }

    #[test]
    fn csi_cursor_and_erase_sequences_edit_the_line() {
        let mut parser = TerminalParser::new();
        parser.feed("abcdef\u{1b}[3DXY");
        assert_eq!(parser.current.iter().collect::<String>(), "abcXYf");
        parser.feed("\u{1b}[K");
        assert_eq!(parser.current.iter().collect::<String>(), "abcXY");
        // CSI G moves to a column, so the gap is padded like a real terminal.
        parser.feed("\u{1b}[8Gz");
        assert_eq!(parser.current.iter().collect::<String>(), "abcXY  z");
        parser.feed("\u{1b}[2K");
        assert!(parser.current.is_empty());
    }

    #[test]
    fn backspace_rewrites_in_place_and_clear_screen_empties_the_buffer() {
        let mut parser = TerminalParser::new();
        let banner = parser.lines.len();
        parser.feed("one\ntwo\nthree");
        assert_eq!(parser.lines.len(), banner + 2);
        assert_eq!(parser.current.iter().collect::<String>(), "three");
        parser.feed("\u{8}\u{8}XY");
        assert_eq!(parser.current.iter().collect::<String>(), "thrXY");
        parser.feed("\u{1b}[2J");
        assert!(parser.lines.is_empty());
        assert!(parser.current.is_empty());
    }

    #[test]
    fn scrollback_is_capped_and_published_to_views() {
        let mut parser = TerminalParser::new();
        let mut text = String::new();
        for index in 0..(MAX_SCROLLBACK_LINES + 40) {
            text.push_str(&format!("line {index}\n"));
        }
        parser.feed(&text);
        assert_eq!(parser.lines.len(), MAX_SCROLLBACK_LINES);
        assert!(parser.lines[0].starts_with("line 40"));

        let view = Arc::new(Mutex::new(TerminalView::default()));
        parser.feed("prompt> ");
        parser.publish(&view);
        let view = view.lock().expect("view lock");
        assert_eq!(view.lines.len(), MAX_SCROLLBACK_LINES);
        assert_eq!(view.current, "prompt> ");
    }

    #[test]
    fn viewport_reads_the_published_scrollback() {
        let mut parser = TerminalParser::new();
        parser.feed("alpha\nbeta\ngamma\n");
        let view = Arc::new(Mutex::new(TerminalView::default()));
        parser.publish(&view);
        let view = view.lock().expect("view lock");
        let viewport = TerminalViewport {
            total_lines: view.lines.len(),
            visible_rows: 2,
            scroll: 1,
        };
        assert_eq!(
            visible_lines(&view.lines, &viewport)
                .last()
                .map(String::as_str),
            Some("beta")
        );
    }

    #[test]
    fn wide_characters_occupy_two_grid_cells() {
        assert_eq!(character_cells('a'), 1);
        assert_eq!(character_cells('你'), 2);
        assert_eq!(visual_cells("你好ab", 4), 6);
        assert_eq!(column_for_cells("你好ab", 0), 0);
        assert_eq!(column_for_cells("你好ab", 2), 1);
        assert_eq!(column_for_cells("你好ab", 4), 2);
        assert_eq!(column_for_cells("你好ab", 99), 4);
        assert_eq!(column_for_cells("", 3), 0);
    }
}
