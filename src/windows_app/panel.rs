//! Terminal panel: bottom terminal panel, grid sync, selection and copy.

use super::*;

impl WindowsAppState {
    pub(super) fn ensure_terminal(&mut self) {
        if !self.terminals.is_empty() {
            return;
        }
        for name in ["Build", "Run", "Output"] {
            self.add_named_terminal(name.to_string());
        }
        self.active_terminal = 0;
    }

    pub(super) fn add_terminal(&mut self) {
        self.add_named_terminal(format!("Terminal {}", self.terminals.len() + 1));
    }

    pub(super) fn add_named_terminal(&mut self, name: String) {
        let cwd = self.project_root.clone();
        let grid = self.terminal_grid;
        match unsafe { ConPtySession::spawn(&cwd, grid.columns, grid.rows) } {
            Ok(session) => {
                self.terminals.push(TerminalTab {
                    name,
                    session,
                    input: String::new(),
                });
                self.active_terminal = self.terminals.len() - 1;
                self.status = format!("ConPTY started in {}", cwd.display());
            }
            Err(error) => self.status = format!("ConPTY start failed: {error}"),
        }
    }

    pub(super) fn toggle_terminal(&mut self) {
        self.terminal_open = !self.terminal_open;
        if self.terminal_open {
            self.ensure_terminal();
        }
    }
}

/// Buffered output of the terminal the panel and the IDE share.
pub(super) fn terminal_buffer(app: &WindowsAppState) -> Vec<String> {
    app.terminals
        .get(app.active_terminal)
        .and_then(|tab| {
            tab.session.view.lock().ok().map(|view| {
                let mut lines = view.lines.clone();
                if !view.current.is_empty() {
                    lines.push(view.current.clone());
                }
                lines
            })
        })
        .unwrap_or_default()
}

/// Buffer position under a point inside the bottom terminal panel.
pub(super) fn terminal_point_at(
    app: &WindowsAppState,
    panel: &terminal_core::TerminalPanel,
    scale: f32,
    x: i32,
    y: i32,
    clamp: bool,
) -> Option<terminal_core::TerminalPoint> {
    let row = (y as f32 - panel.body_top) / panel.row_height.max(1.0);
    let row = if row < 0.0 {
        if clamp {
            0.0
        } else {
            return None;
        }
    } else if row >= panel.visible_rows as f32 {
        if clamp {
            (panel.visible_rows - 1) as f32
        } else {
            return None;
        }
    } else {
        row
    };
    let lines = terminal_buffer(app);
    let viewport = terminal_core::TerminalViewport {
        total_lines: lines.len(),
        visible_rows: panel.visible_rows,
        scroll: app.terminal_scroll,
    };
    let line = viewport.line_at_row(row.floor() as usize)?;
    let cell_width = (terminal_core::code_cell_width(app.code_zoom) * scale).max(1.0);
    let cells = ((x as f32 - panel.body_left) / cell_width).max(0.0) as usize;
    let value = lines.get(line)?;
    Some(terminal_core::TerminalPoint {
        line,
        column: terminal_core::column_for_cells(value, cells),
    })
}

/// Copy the current bottom panel selection into the clipboard.
pub(super) fn copy_terminal_selection(window: Hwnd) -> bool {
    let selection = APP.with(|cell| cell.borrow().terminal_selection);
    let Some(selection) = selection else {
        return false;
    };
    let lines = APP.with(|cell| terminal_buffer(&cell.borrow()));
    let Some(text) = terminal_core::selected_text(&lines, &selection) else {
        return false;
    };
    platform::write_clipboard(window, &text).is_ok()
}

pub(super) fn handle_terminal_control(window: Hwnd, key: u32) -> bool {
    if key == VK_TAB {
        APP.with(|cell| {
            let mut app = cell.borrow_mut();
            if !app.terminals.is_empty() {
                app.active_terminal = if platform::shift_pressed() {
                    app.active_terminal
                        .checked_sub(1)
                        .unwrap_or(app.terminals.len() - 1)
                } else {
                    (app.active_terminal + 1) % app.terminals.len()
                };
                app.status = format!(
                    "Active terminal: {}",
                    app.terminals[app.active_terminal].name
                );
            }
        });
        return true;
    }
    let paste = if key == 0x56 {
        platform::read_clipboard(window).ok()
    } else {
        None
    };
    let sequence = match key {
        0x41 => Some("\u{1}"),
        0x43 => Some("\u{3}"),
        0x4c => Some("\u{c}"),
        0x56 => paste.as_deref(),
        _ => return false,
    };
    if let Some(sequence) = sequence {
        APP.with(|cell| {
            let mut app = cell.borrow_mut();
            let active = app.active_terminal;
            if let Some(tab) = app.terminals.get_mut(active) {
                let _ = tab.session.write(sequence);
                if key == 0x56 {
                    tab.input.push_str(sequence);
                }
            }
        });
    }
    true
}

/// Grid the bottom terminal panel shows at the current client size./// Grid the bottom terminal panel shows at the current client size.
pub(super) fn panel_grid(window: Hwnd, code_zoom: i32) -> terminal_core::GridSize {
    let mut area = unsafe { std::mem::zeroed::<Rect>() };
    unsafe { GetClientRect(window, &mut area) };
    let scale = platform::dpi_for_window(window) as f32 / 96.0;
    let panel = terminal_core::panel_metrics(
        (area.right - area.left).max(1) as f32,
        (area.bottom - area.top).max(1) as f32,
        scale,
        1.0,
    );
    grid_for_panel(&panel, scale, code_zoom)
}

/// Fit a column/row grid into a laid out terminal panel body.
pub(super) fn grid_for_panel(
    panel: &terminal_core::TerminalPanel,
    scale: f32,
    code_zoom: i32,
) -> terminal_core::GridSize {
    terminal_core::GridSize::from_metrics(
        (panel.body_right - panel.body_left).max(1.0),
        (panel.body_bottom - panel.body_top).max(1.0),
        terminal_core::code_cell_width(code_zoom) * scale,
        panel.row_height,
    )
}

/// Resize every ConPTY session when the client area, DPI, zoom or session set changed.
///
/// While a detached terminal window is open it owns the grid of the session it
/// shows, so the IDE keeps resizing the other sessions only.
pub(super) fn sync_terminal_grid(window: Hwnd) {
    let mut area: Rect = unsafe { std::mem::zeroed::<Rect>() };
    unsafe { GetClientRect(window, &mut area) };
    let dpi = platform::dpi_for_window(window);
    let (code_zoom, count, active, detached) = APP.with(|cell| {
        let app = cell.borrow();
        (
            app.code_zoom,
            app.terminals.len(),
            app.active_terminal,
            !app.detached_terminal.is_null(),
        )
    });
    let signature = (
        (area.right - area.left).max(0) as u32,
        (area.bottom - area.top).max(0) as u32,
        dpi,
        code_zoom,
        count,
        active,
        detached,
    );
    let changed = APP.with(|cell| {
        let mut app = cell.borrow_mut();
        if app.terminal_grid_signature == signature {
            return false;
        }
        app.terminal_grid_signature = signature;
        true
    });
    if !changed {
        return;
    }
    let grid = panel_grid(window, code_zoom);
    APP.with(|cell| {
        let mut app = cell.borrow_mut();
        app.terminal_grid = grid;
        let detached = !app.detached_terminal.is_null();
        let active = app.active_terminal;
        for (index, terminal) in app.terminals.iter().enumerate() {
            if detached && index == active {
                continue;
            }
            terminal.session.resize(grid.columns, grid.rows);
        }
    });
}
