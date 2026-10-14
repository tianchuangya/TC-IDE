//! Detached terminal: standalone ConPTY window, layout, painting and selection.

use super::panel::handle_terminal_control;
use super::*;

pub(super) unsafe fn draw_terminal_text(
    dc: Hdc,
    value: &str,
    x: i32,
    y: i32,
    right: i32,
    color: Dword,
) {
    draw_terminal_line(
        dc,
        value,
        Rect {
            left: x,
            top: y,
            right,
            bottom: y + 24,
        },
        DT_SINGLELINE | DT_NOPREFIX,
        color,
    );
}

pub(super) unsafe fn draw_terminal_line(
    dc: Hdc,
    value: &str,
    area: Rect,
    format: Uint,
    color: Dword,
) {
    let wide = value.encode_utf16().collect::<Vec<_>>();
    let mut area = area;
    unsafe {
        SetTextColor(dc, color);
        DrawTextW(dc, wide.as_ptr(), wide.len() as i32, &mut area, format);
    }
}

/// Monospace font of the detached terminal, rebuilt when the window DPI changes.
pub(super) unsafe fn detached_font(window: Hwnd) -> Hfont {
    let dpi = platform::dpi_for_window(window);
    let (current, current_dpi) = DETACHED.with(|cell| {
        let state = cell.borrow();
        (state.font, state.font_dpi)
    });
    if !current.is_null() && current_dpi == dpi {
        return current;
    }
    let scale = dpi.max(96) as f32 / 96.0;
    let mut log_font: LogFontW = unsafe { std::mem::zeroed() };
    log_font.height = -((DETACHED_FONT_HEIGHT * scale).round() as i32).max(9);
    log_font.weight = FW_NORMAL;
    log_font.char_set = DEFAULT_CHARSET;
    log_font.out_precision = OUT_TT_PRECIS;
    log_font.clip_precision = CLIP_DEFAULT_PRECIS;
    log_font.quality = CLEARTYPE_QUALITY;
    log_font.pitch_and_family = FF_MODERN | FIXED_PITCH;
    for (index, unit) in DETACHED_FONT_FACE.encode_utf16().take(31).enumerate() {
        log_font.face_name[index] = unit;
    }
    let font = unsafe { CreateFontIndirectW(&log_font) };
    DETACHED.with(|cell| {
        let mut state = cell.borrow_mut();
        let previous = state.font;
        state.font = font;
        state.font_dpi = dpi;
        if !previous.is_null() {
            unsafe { DeleteObject(previous) };
        }
    });
    font
}

/// Glyph advance and line height measured from the detached terminal font.
pub(super) fn detached_cell_metrics(window: Hwnd) -> (f32, f32) {
    let dpi = platform::dpi_for_window(window);
    let cached = DETACHED.with(|cell| {
        let state = cell.borrow();
        if state.metrics_dpi == dpi {
            state.metrics
        } else {
            None
        }
    });
    if let Some(metrics) = cached {
        return metrics;
    }
    let metrics = measure_detached_cells(window);
    DETACHED.with(|cell| {
        let mut state = cell.borrow_mut();
        state.metrics = Some(metrics);
        state.metrics_dpi = dpi;
    });
    metrics
}

/// Measure the detached terminal font once per window DPI.
pub(super) fn measure_detached_cells(window: Hwnd) -> (f32, f32) {
    unsafe {
        let font = detached_font(window);
        let dc = GetDC(window);
        if dc.is_null() {
            return (
                terminal_core::code_cell_width(0),
                DETACHED_FONT_HEIGHT * 1.35,
            );
        }
        let previous = if font.is_null() {
            ptr::null_mut()
        } else {
            SelectObject(dc, font)
        };
        let sample = "M".repeat(64);
        let sample_wide = sample.encode_utf16().collect::<Vec<_>>();
        let mut sample_area = Rect {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        DrawTextW(
            dc,
            sample_wide.as_ptr(),
            sample_wide.len() as i32,
            &mut sample_area,
            DT_CALCRECT | DT_SINGLELINE | DT_NOPREFIX,
        );
        let width = ((sample_area.right - sample_area.left).max(1) as f32) / 64.0;
        let glyph = wide("M");
        let mut glyph_area = Rect {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        DrawTextW(
            dc,
            glyph.as_ptr(),
            1,
            &mut glyph_area,
            DT_CALCRECT | DT_SINGLELINE | DT_NOPREFIX,
        );
        let height = (glyph_area.bottom - glyph_area.top).max(1) as f32;
        if !previous.is_null() {
            SelectObject(dc, previous);
        }
        ReleaseDC(window, dc);
        (width, height)
    }
}

/// Lay out header, scrollback area and input line of the detached window.
pub(super) fn detached_layout(area: &Rect, cell_width: f32, cell_height: f32) -> DetachedLayout {
    let width = (area.right - area.left).max(1) as f32;
    let height = (area.bottom - area.top).max(1) as f32;
    let cell_width = if cell_width.is_finite() && cell_width > 0.0 {
        cell_width
    } else {
        8.0
    };
    let cell_height = if cell_height.is_finite() && cell_height > 0.0 {
        cell_height
    } else {
        16.0
    };
    let left = DETACHED_TEXT_LEFT;
    let right = (width - DETACHED_TEXT_RIGHT).max(left + cell_width);
    let top = DETACHED_HEADER_HEIGHT;
    let input_top = (height - cell_height - DETACHED_INPUT_GAP).max(top + cell_height);
    let bottom = (input_top - DETACHED_INPUT_GAP).max(top + 1.0);
    let visible_rows = (((bottom - top) / cell_height).floor().max(1.0)) as usize;
    let grid =
        terminal_core::GridSize::from_metrics(right - left, bottom - top, cell_width, cell_height);
    DetachedLayout {
        left,
        right,
        top,
        input_top,
        row_height: cell_height,
        cell_width,
        visible_rows,
        columns: grid.columns,
        rows: grid.rows,
    }
}

/// Buffered output of the terminal the detached window shares with the IDE.
pub(super) fn detached_lines() -> Vec<String> {
    APP.with(|cell| {
        let app = cell.borrow();
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
    })
}

/// Lines, scrollback window and layout used by painting and hit testing.
pub(super) fn detached_context(window: Hwnd) -> DetachedContext {
    let mut area: Rect = unsafe { std::mem::zeroed() };
    unsafe { GetClientRect(window, &mut area) };
    let (cell_width, cell_height) = detached_cell_metrics(window);
    let layout = detached_layout(&area, cell_width, cell_height);
    let lines = detached_lines();
    let scroll = DETACHED.with(|cell| cell.borrow().scroll);
    let viewport = terminal_core::TerminalViewport {
        total_lines: lines.len(),
        visible_rows: layout.visible_rows,
        scroll,
    };
    DetachedContext {
        lines,
        viewport,
        layout,
    }
}

/// Buffer position under a window point, optionally clamped into the viewport.
pub(super) fn detached_point_at(
    context: &DetachedContext,
    x: i32,
    y: i32,
    clamp: bool,
) -> Option<terminal_core::TerminalPoint> {
    let row = (y as f32 - context.layout.top) / context.layout.row_height;
    let row = if row < 0.0 {
        if clamp {
            0.0
        } else {
            return None;
        }
    } else if row >= context.viewport.visible_rows as f32 {
        if clamp {
            (context.viewport.visible_rows - 1) as f32
        } else {
            return None;
        }
    } else {
        row
    };
    let line = context.viewport.line_at_row(row.floor() as usize)?;
    let cells = ((x as f32 - context.layout.left) / context.layout.cell_width).max(0.0) as usize;
    let value = context.lines.get(line)?;
    Some(terminal_core::TerminalPoint {
        line,
        column: terminal_core::column_for_cells(value, cells),
    })
}

/// Push the detached window size into the ConPTY grid it shows.
pub(super) fn sync_detached_terminal_grid(window: Hwnd) {
    let mut area: Rect = unsafe { std::mem::zeroed() };
    unsafe { GetClientRect(window, &mut area) };
    let (cell_width, cell_height) = detached_cell_metrics(window);
    let layout = detached_layout(&area, cell_width, cell_height);
    let (active, count) = APP.with(|cell| {
        let app = cell.borrow();
        (app.active_terminal, app.terminals.len())
    });
    let signature = (
        (area.right - area.left).max(0) as u32,
        (area.bottom - area.top).max(0) as u32,
        cell_width.to_bits(),
        cell_height.to_bits(),
        active,
    );
    let changed = DETACHED.with(|cell| {
        let mut state = cell.borrow_mut();
        if state.grid_signature == signature {
            return false;
        }
        state.grid_signature = signature;
        true
    });
    if !changed || count == 0 {
        return;
    }
    APP.with(|cell| {
        let app = cell.borrow();
        if let Some(tab) = app.terminals.get(active) {
            tab.session.resize(layout.columns, layout.rows);
        }
    });
}

/// Copy the detached window selection into the clipboard.
pub(super) fn copy_detached_selection(window: Hwnd) -> bool {
    let Some(selection) = DETACHED.with(|cell| cell.borrow().selection) else {
        return false;
    };
    let lines = detached_lines();
    let Some(text) = terminal_core::selected_text(&lines, &selection) else {
        return false;
    };
    let copied = platform::write_clipboard(window, &text).is_ok();
    let notice = if copied {
        format!("Copied {} characters", text.chars().count())
    } else {
        "Clipboard unavailable".to_string()
    };
    DETACHED.with(|cell| {
        let mut state = cell.borrow_mut();
        state.notice = Some(notice);
        state.notice_started = Some(Instant::now());
    });
    copied
}

/// Scroll the detached viewport while keeping it inside the buffered history.
pub(super) fn scroll_detached(window: Hwnd, rows: i32) {
    let context = detached_context(window);
    let scroll = context.viewport.scroll_by(rows);
    DETACHED.with(|cell| cell.borrow_mut().scroll = scroll);
    invalidate(window);
}

/// Return the detached viewport to the newest output.
pub(super) fn scroll_detached_to_bottom() {
    DETACHED.with(|cell| cell.borrow_mut().scroll = 0);
}

pub(super) unsafe extern "system" fn detached_terminal_proc(
    window: Hwnd,
    message: Uint,
    w_param: Wparam,
    l_param: Lparam,
) -> Lresult {
    match message {
        WM_PAINT => {
            let mut paint: PaintStruct = unsafe { std::mem::zeroed() };
            let dc = unsafe { BeginPaint(window, &mut paint) };
            let mut area: Rect = unsafe { std::mem::zeroed() };
            unsafe { GetClientRect(window, &mut area) };
            let context = detached_context(window);
            let layout = &context.layout;
            let (name, input) = APP.with(|cell| {
                let app = cell.borrow();
                let tab = app.terminals.get(app.active_terminal);
                (
                    tab.map(|tab| tab.name.clone())
                        .unwrap_or_else(|| "Terminal".to_string()),
                    tab.map(|tab| tab.input.clone()).unwrap_or_default(),
                )
            });
            let (selection, notice, composition) =
                DETACHED.with(|cell| {
                    let state = cell.borrow();
                    let notice = state.notice.as_ref().zip(state.notice_started).and_then(
                        |(text, started)| {
                            (started.elapsed().as_secs_f32() < NOTICE_SECONDS).then(|| text.clone())
                        },
                    );
                    (state.selection, notice, state.composition.clone())
                });
            let background = unsafe { CreateSolidBrush(0x001b1917) };
            let highlight = unsafe { CreateSolidBrush(0x00513419) };
            unsafe {
                FillRect(dc, &area, background);
                SetBkMode(dc, TRANSPARENT_MODE);
            }
            let font = detached_font(window);
            let previous_font = if font.is_null() {
                ptr::null_mut()
            } else {
                unsafe { SelectObject(dc, font) }
            };
            let hint = if context.viewport.clamped_scroll() > 0 {
                format!(
                    "history ↑{} of {} · wheel scrolls",
                    context.viewport.clamped_scroll(),
                    context.viewport.max_scroll()
                )
            } else {
                "wheel scrolls · drag selects · Ctrl+Shift+C copies · Ctrl+C interrupts".to_string()
            };
            unsafe {
                draw_terminal_text(
                    dc,
                    "TianChuangYa",
                    DETACHED_TEXT_LEFT as i32,
                    14,
                    area.right - 20,
                    0x00e6bc71,
                );
                draw_terminal_line(
                    dc,
                    &hint,
                    Rect {
                        left: (layout.right - 420.0).max(layout.left) as i32,
                        top: 14,
                        right: layout.right as i32,
                        bottom: 40,
                    },
                    DT_SINGLELINE | DT_NOPREFIX | DT_RIGHT,
                    0x008e8073,
                );
                draw_terminal_text(
                    dc,
                    &format!("TC-IDE · {name} · shared ConPTY session"),
                    DETACHED_TEXT_LEFT as i32,
                    42,
                    area.right - 20,
                    0x009d9690,
                );
                if let Some(notice) = notice.as_deref() {
                    draw_terminal_line(
                        dc,
                        notice,
                        Rect {
                            left: (layout.right - 300.0).max(layout.left) as i32,
                            top: 42,
                            right: layout.right as i32,
                            bottom: 68,
                        },
                        DT_SINGLELINE | DT_NOPREFIX | DT_RIGHT,
                        0x00dfb56b,
                    );
                }
            }
            let (selection_start, selection_end) = match selection.map(|it| it.ordered()) {
                Some((start, end)) => (Some(start), Some(end)),
                None => (None, None),
            };
            for (index, value) in terminal_core::visible_lines(&context.lines, &context.viewport)
                .iter()
                .enumerate()
            {
                let row_top = layout.top + index as f32 * layout.row_height;
                if let (Some(start), Some(end)) = (selection_start, selection_end) {
                    if let Some(line) = context.viewport.line_at_row(index) {
                        if line >= start.line && line <= end.line {
                            let length = value.chars().count();
                            let from = if line == start.line {
                                terminal_core::visual_cells(value, start.column)
                            } else {
                                0
                            };
                            let to = if line == end.line {
                                terminal_core::visual_cells(value, end.column)
                            } else {
                                terminal_core::visual_cells(value, length)
                            };
                            if to > from {
                                let band = Rect {
                                    left: (layout.left + from as f32 * layout.cell_width) as i32,
                                    top: row_top as i32,
                                    right: (layout.left + to as f32 * layout.cell_width).ceil()
                                        as i32,
                                    bottom: (row_top + layout.row_height).ceil() as i32,
                                };
                                unsafe { FillRect(dc, &band, highlight) };
                            }
                        }
                    }
                }
                unsafe {
                    draw_terminal_line(
                        dc,
                        value,
                        Rect {
                            left: layout.left as i32,
                            top: row_top as i32,
                            right: layout.right as i32,
                            bottom: (row_top + layout.row_height + 2.0) as i32,
                        },
                        DT_SINGLELINE | DT_NOPREFIX,
                        0x00e8e8e8,
                    );
                }
            }
            unsafe {
                draw_terminal_line(
                    dc,
                    &format!("> {input}{composition}▌"),
                    Rect {
                        left: layout.left as i32,
                        top: layout.input_top as i32,
                        right: layout.right as i32,
                        bottom: (layout.input_top + layout.row_height + 3.0) as i32,
                    },
                    DT_SINGLELINE | DT_NOPREFIX,
                    0x00dfb56b,
                );
                if !previous_font.is_null() {
                    SelectObject(dc, previous_font);
                }
                DeleteObject(background);
                DeleteObject(highlight);
                EndPaint(window, &paint);
            }
            0
        }
        WM_CHAR => {
            scroll_detached_to_bottom();
            if let Some(value) = char::from_u32(w_param as u32) {
                APP.with(|cell| {
                    let mut app = cell.borrow_mut();
                    let active = app.active_terminal;
                    if let Some(tab) = app.terminals.get_mut(active) {
                        match value {
                            '\r' | '\n' => {
                                let _ = tab.session.write("\r");
                                tab.input.clear();
                            }
                            '\u{8}' => {
                                tab.input.pop();
                                let _ = tab.session.write("\u{7f}");
                            }
                            ch if !ch.is_control() => {
                                tab.input.push(ch);
                                let _ = tab.session.write(&ch.to_string());
                            }
                            _ => {}
                        }
                    }
                });
            }
            invalidate(window);
            0
        }
        WM_KEYDOWN => {
            scroll_detached_to_bottom();
            if platform::control_pressed() {
                let key = w_param as u32;
                if key == 0x43 && platform::shift_pressed() {
                    copy_detached_selection(window);
                    invalidate(window);
                    return 0;
                }
                if handle_terminal_control(window, key) {
                    invalidate(window);
                    return 0;
                }
            }
            let sequence = match w_param as u32 {
                VK_UP => Some("\u{1b}[A"),
                VK_DOWN => Some("\u{1b}[B"),
                VK_RIGHT => Some("\u{1b}[C"),
                VK_LEFT => Some("\u{1b}[D"),
                VK_HOME => Some("\u{1b}[H"),
                VK_END => Some("\u{1b}[F"),
                VK_DELETE => Some("\u{1b}[3~"),
                VK_TAB => Some("\t"),
                _ => None,
            };
            if let Some(sequence) = sequence {
                APP.with(|cell| {
                    let app = cell.borrow();
                    if let Some(tab) = app.terminals.get(app.active_terminal) {
                        let _ = tab.session.write(sequence);
                    }
                });
            }
            0
        }
        WM_IME_STARTCOMPOSITION => {
            DETACHED.with(|cell| cell.borrow_mut().composition.clear());
            invalidate(window);
            0
        }
        WM_IME_COMPOSITION => {
            if l_param & GCS_RESULTSTR != 0 {
                let result = ime_string(window, GCS_RESULTSTR as Dword);
                DETACHED.with(|cell| cell.borrow_mut().composition.clear());
                if !result.is_empty() {
                    APP.with(|cell| {
                        let mut app = cell.borrow_mut();
                        let active = app.active_terminal;
                        if let Some(tab) = app.terminals.get_mut(active) {
                            tab.input.push_str(&result);
                            let _ = tab.session.write(&result);
                        }
                    });
                }
            } else if l_param & GCS_COMPSTR != 0 {
                let composition = ime_string(window, GCS_COMPSTR as Dword);
                DETACHED.with(|cell| cell.borrow_mut().composition = composition);
            }
            invalidate(window);
            0
        }
        WM_IME_ENDCOMPOSITION => {
            DETACHED.with(|cell| cell.borrow_mut().composition.clear());
            invalidate(window);
            0
        }
        WM_MOUSEWHEEL => {
            let delta = ((w_param >> 16) as u16 as i16) as i32;
            if delta > 0 {
                scroll_detached(window, WHEEL_SCROLL_LINES);
            } else if delta < 0 {
                scroll_detached(window, -WHEEL_SCROLL_LINES);
            }
            0
        }
        WM_LBUTTONDOWN => {
            let x = (l_param as u16 as i16) as i32;
            let y = ((l_param >> 16) as u16 as i16) as i32;
            let context = detached_context(window);
            if let Some(point) = detached_point_at(&context, x, y, false) {
                DETACHED.with(|cell| {
                    let mut state = cell.borrow_mut();
                    state.selection = Some(terminal_core::TerminalSelection::new(point, point));
                    state.dragging = true;
                });
                unsafe { SetCapture(window) };
                invalidate(window);
            }
            0
        }
        WM_MOUSEMOVE => {
            let dragging = DETACHED.with(|cell| cell.borrow().dragging);
            if dragging && w_param & 1 != 0 {
                let x = (l_param as u16 as i16) as i32;
                let y = ((l_param >> 16) as u16 as i16) as i32;
                let context = detached_context(window);
                if let Some(point) = detached_point_at(&context, x, y, true) {
                    let changed = DETACHED.with(|cell| {
                        let mut state = cell.borrow_mut();
                        let Some(selection) = state.selection else {
                            return false;
                        };
                        if selection.focus == point {
                            return false;
                        }
                        state.selection = Some(selection.extended_to(point));
                        true
                    });
                    if changed {
                        invalidate(window);
                    }
                }
            }
            0
        }
        WM_LBUTTONUP => {
            let dragging = DETACHED.with(|cell| {
                let mut state = cell.borrow_mut();
                let dragging = state.dragging;
                state.dragging = false;
                dragging
            });
            if dragging {
                unsafe { ReleaseCapture() };
            }
            let cleared = DETACHED.with(|cell| {
                let mut state = cell.borrow_mut();
                if state.selection.map(|it| it.is_empty()).unwrap_or(false) {
                    state.selection = None;
                    true
                } else {
                    false
                }
            });
            if cleared || dragging {
                invalidate(window);
            }
            0
        }
        WM_TIMER | WM_SIZE => {
            sync_detached_terminal_grid(window);
            invalidate(window);
            0
        }
        WM_CLOSE => {
            unsafe { DestroyWindow(window) };
            0
        }
        WM_DESTROY => {
            unsafe { ReleaseCapture() };
            let font = DETACHED.with(|cell| {
                let mut state = cell.borrow_mut();
                let font = state.font;
                *state = DetachedTerminalState::default();
                font
            });
            if !font.is_null() {
                unsafe { DeleteObject(font) };
            }
            APP.with(|cell| cell.borrow_mut().detached_terminal = ptr::null_mut());
            0
        }
        _ => unsafe { DefWindowProcW(window, message, w_param, l_param) },
    }
}

pub(super) fn open_detached_terminal() -> bool {
    unsafe {
        let existing = APP.with(|cell| cell.borrow().detached_terminal);
        if !existing.is_null() {
            SetForegroundWindow(existing);
            return true;
        }
        APP.with(|cell| cell.borrow_mut().ensure_terminal());
        let instance = GetModuleHandleW(ptr::null());
        let class_name = wide("TC_IDE_DETACHED_TERMINAL");
        let class = WindowClass {
            style: CS_HREDRAW | CS_VREDRAW,
            window_proc: Some(detached_terminal_proc),
            class_extra: 0,
            window_extra: 0,
            instance,
            icon: ptr::null_mut(),
            cursor: LoadCursorW(ptr::null_mut(), IDC_ARROW),
            background: ptr::null_mut(),
            menu_name: ptr::null(),
            class_name: class_name.as_ptr(),
        };
        RegisterClassW(&class);
        let title = wide("TC-IDE — Detached ConPTY Terminal");
        let window = CreateWindowExW(
            0,
            class_name.as_ptr(),
            title.as_ptr(),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            1080,
            680,
            ptr::null_mut(),
            ptr::null_mut(),
            instance,
            ptr::null_mut(),
        );
        if window.is_null() {
            return false;
        }
        APP.with(|cell| cell.borrow_mut().detached_terminal = window);
        SetTimer(window, 2, 33, ptr::null());
        ShowWindow(window, SW_SHOW);
        sync_detached_terminal_grid(window);
        UpdateWindow(window);
        true
    }
}
