//! Window events: message loop plumbing, hit testing and command dispatch.

use super::detached::open_detached_terminal;
use super::keys::handle_key_down;
use super::panel::sync_terminal_grid;
use super::pointer::{
    handle_left_button_down, handle_left_button_up, handle_mouse_move, handle_mouse_wheel,
    handle_right_button_down,
};
use super::scene::render_window;
use super::workspace::save_session;
use super::*;

pub(super) fn editor_position_at(window: Hwnd, x: i32, y: i32) -> Option<TextPosition> {
    let mut area = unsafe { std::mem::zeroed::<Rect>() };
    unsafe { GetClientRect(window, &mut area) };
    let width = area.right.max(1) as f32;
    let scale = platform::dpi_for_window(window) as f32 / 96.0;
    let tree_w = (width * 0.19).max(220.0 * scale);
    let graph_w = (width * 0.27).max(300.0 * scale);
    let editor_right = width - graph_w;
    let code_top = (48.0 + 58.0) * scale;
    if x as f32 <= tree_w + 42.0 * scale || x as f32 >= editor_right || y as f32 <= code_top {
        return None;
    }
    let line = ((y as f32 - code_top) / (25.0 * scale)).floor() as usize;
    APP.with(|cell| {
        let app = cell.borrow();
        let value = app.lines.get(line)?;
        let code_cell = 8.9 + app.code_zoom.clamp(-2, 8) as f32 * 0.6;
        let target = ((x as f32 - tree_w - 52.0 * scale) / (code_cell * scale)).max(0.0);
        let mut cells = 0.0;
        for (column, ch) in value.chars().enumerate() {
            let width = character_cells(ch) as f32;
            if target < cells + width * 0.5 {
                return Some(TextPosition { line, column });
            }
            cells += width;
        }
        Some(TextPosition {
            line,
            column: value.chars().count(),
        })
    })
}

pub(super) fn comment_bubble_at(window: Hwnd, x: i32, y: i32) -> Option<(usize, bool)> {
    let mut area = unsafe { std::mem::zeroed::<Rect>() };
    unsafe { GetClientRect(window, &mut area) };
    let width = area.right.max(1) as f32;
    let height = area.bottom.max(1) as f32;
    let scale = platform::dpi_for_window(window) as f32 / 96.0;
    let tree_w = (width * 0.19).max(220.0 * scale);
    let graph_w = (width * 0.27).max(300.0 * scale);
    let editor_right = width - graph_w;
    if x as f32 <= tree_w || x as f32 >= editor_right {
        return None;
    }
    APP.with(|cell| {
        let app = cell.borrow();
        let terminal_h = (28.0 + 214.0 * app.terminal_motion.clamp(0.0, 1.0)) * scale;
        let body_bottom = height - terminal_h;
        let code_top = 106.0 * scale;
        let line_step = 25.0 * scale;
        let bubble_right = editor_right - 14.0 * scale;
        let bubble_left = (bubble_right - 286.0 * scale).max(tree_w + 130.0 * scale);
        for (line, value) in app.lines.iter().enumerate() {
            if editor_core::comment_bounds(value, &app.file_name).is_none() {
                continue;
            }
            let top = code_top + line as f32 * line_step - 5.0 * scale;
            if top >= body_bottom {
                break;
            }
            let height = if app.expanded_comments.contains(&line) {
                112.0
            } else {
                34.0
            } * scale;
            if x as f32 >= bubble_left
                && x as f32 <= bubble_right
                && y as f32 >= top
                && y as f32 <= top + height
            {
                return Some((line, x as f32 <= bubble_left + 76.0 * scale));
            }
        }
        None
    })
}

pub(super) fn perform_command(window: Hwnd, command: Wparam) {
    if let Some(item) = platform::menu_item_for_command(command) {
        perform_menu_item(window, item);
        invalidate(window);
        return;
    }
    APP.with(|cell| {
        let mut app = cell.borrow_mut();
        match command {
            platform::COMMAND_OPEN_FILE => {
                if let Some(path) = platform::choose_file(window) {
                    app.open_file(path);
                }
            }
            platform::COMMAND_COPY_DIAGNOSTIC => {
                let selected = app
                    .selected_text()
                    .unwrap_or_else(|| app.lines.get(app.caret_line).cloned().unwrap_or_default());
                match platform::write_clipboard(window, &selected) {
                    Ok(()) => app.status = "Copied Unicode text".into(),
                    Err(error) => app.status = format!("Clipboard write failed: {error}"),
                }
            }
            platform::COMMAND_PASTE => match platform::read_clipboard(window) {
                Ok(value) => app.insert_text(&value),
                Err(error) => app.status = format!("Clipboard read failed: {error}"),
            },
            platform::COMMAND_SELECT_ALL => {
                if let Some((line, start, end)) = app.comment_edit_bounds() {
                    app.selection_anchor = Some(TextPosition {
                        line,
                        column: start,
                    });
                    app.caret_line = line;
                    app.caret_column = end;
                } else {
                    app.select_all();
                }
            }
            platform::COMMAND_CUT => {
                if let Some(value) = app.cut_selection() {
                    match platform::write_clipboard(window, &value) {
                        Ok(()) => app.status = "Cut selection".into(),
                        Err(error) => app.status = format!("Clipboard write failed: {error}"),
                    }
                }
            }
            platform::COMMAND_UNDO => app.undo(),
            platform::COMMAND_REDO => app.redo(),
            platform::COMMAND_THEME_DARK => {
                app.light_theme = false;
                app.status = "Dark theme enabled".into();
            }
            platform::COMMAND_THEME_LIGHT => {
                app.light_theme = true;
                app.status = "Light theme enabled".into();
            }
            platform::COMMAND_LANGUAGE_TOGGLE => {
                app.ui_chinese = !app.ui_chinese;
                app.status = if app.ui_chinese {
                    "已切换为中文界面".into()
                } else {
                    "English interface enabled".into()
                };
            }
            platform::COMMAND_FONT_INCREASE => app.code_zoom = (app.code_zoom + 1).min(8),
            platform::COMMAND_FONT_DECREASE => app.code_zoom = (app.code_zoom - 1).max(-2),
            platform::COMMAND_NEW_PROJECT => {
                if let Some(location) = platform::choose_folder(window) {
                    app.begin_create(CreateKind::Project);
                    app.create_location = location;
                }
            }
            platform::COMMAND_NEW_FILE => app.begin_create(CreateKind::File),
            platform::COMMAND_NEW_FOLDER => app.begin_create(CreateKind::Folder),
            platform::COMMAND_WORKSPACE_SEARCH => {
                app.workspace_search_focused = true;
                app.create_kind = None;
            }
            platform::COMMAND_REFRESH_PROJECT => {
                app.refresh_project();
                app.status = "Project tree refreshed".into();
            }
            platform::COMMAND_SETTINGS => {
                app.settings_open = !app.settings_open;
                app.extensions_open = false;
            }
            platform::COMMAND_EXTENSIONS => {
                app.extensions_open = !app.extensions_open;
                app.settings_open = false;
            }
            platform::COMMAND_PACKAGE => {
                app.package_open = !app.package_open;
                app.settings_open = false;
                app.extensions_open = false;
            }
            platform::COMMAND_OPEN_FOLDER => {
                if let Some(path) = platform::choose_folder(window) {
                    app.open_project(path);
                }
            }
            platform::COMMAND_REVEAL_FOLDER => {
                if platform::open_external_terminal(window, &app.project_root) {
                    app.status = format!(
                        "Opened an external terminal in {}",
                        app.project_root.display()
                    );
                } else if platform::open_in_explorer(window, &app.project_root) {
                    app.status = format!("Opened Explorer at {}", app.project_root.display());
                } else {
                    app.status = "Could not open the current project in Explorer".into();
                }
            }
            platform::COMMAND_OPEN_TERMINAL => app.toggle_terminal(),
            _ => {}
        }
        app.save_preferences();
    });
    if matches!(
        command,
        platform::COMMAND_FONT_INCREASE | platform::COMMAND_FONT_DECREASE
    ) {
        RENDERER.with(|cell| *cell.borrow_mut() = None);
    }
    invalidate(window);
}

/// Execute one shared menu item using the same state transitions as keyboard
/// shortcuts and visible controls. Unsupported items are disabled by the
/// native menu builder, so every item that reaches here has an observable
/// action rather than being a decorative row.
fn perform_menu_item(window: Hwnd, item: crate::menu_core::Item) {
    use crate::menu_core::Item;

    let legacy = match item {
        Item::Undo => Some(platform::COMMAND_UNDO),
        Item::Redo => Some(platform::COMMAND_REDO),
        Item::OpenFile => Some(platform::COMMAND_OPEN_FILE),
        Item::OpenFolder => Some(platform::COMMAND_OPEN_FOLDER),
        Item::SelectAll => Some(platform::COMMAND_SELECT_ALL),
        Item::Copy => Some(platform::COMMAND_COPY_DIAGNOSTIC),
        Item::Paste => Some(platform::COMMAND_PASTE),
        Item::FindInProject => Some(platform::COMMAND_WORKSPACE_SEARCH),
        Item::ToggleTerminal => Some(platform::COMMAND_OPEN_TERMINAL),
        Item::LargerCode => Some(platform::COMMAND_FONT_INCREASE),
        Item::SmallerCode => Some(platform::COMMAND_FONT_DECREASE),
        Item::Package => Some(platform::COMMAND_PACKAGE),
        Item::ShowExtensions => Some(platform::COMMAND_EXTENSIONS),
        Item::ShowSettings => Some(platform::COMMAND_SETTINGS),
        _ => None,
    };
    if let Some(command) = legacy {
        perform_command(window, command);
        return;
    }

    match item {
        Item::Save => APP.with(|cell| cell.borrow_mut().save()),
        Item::NewWindow => match std::env::current_exe()
            .and_then(|executable| std::process::Command::new(executable).spawn())
        {
            Ok(_) => APP.with(|cell| cell.borrow_mut().status = "Opened a new TC-IDE window".into()),
            Err(error) => APP.with(|cell| {
                cell.borrow_mut().status = format!("Could not open a new window: {error}")
            }),
        },
        Item::CloseWindow => unsafe {
            DestroyWindow(window);
        },
        Item::ToggleComments => APP.with(|cell| {
            let mut app = cell.borrow_mut();
            if app.expanded_comments.is_empty() {
                app.expanded_comments = app
                    .lines
                    .iter()
                    .enumerate()
                    .filter_map(|(line, text)| {
                        editor_core::comment_bounds(text, &app.file_name).map(|_| line)
                    })
                    .collect();
                app.status = "Comment bubbles expanded".into();
            } else {
                app.expanded_comments.clear();
                app.status = "Comment bubbles collapsed".into();
            }
        }),
        Item::ToggleDebugInfo => APP.with(|cell| cell.borrow_mut().toggle_problems()),
        Item::Build => APP.with(|cell| cell.borrow_mut().run_task(true)),
        Item::Run => APP.with(|cell| cell.borrow_mut().run_task(false)),
        Item::StopRun => APP.with(|cell| {
            let mut app = cell.borrow_mut();
            if let Some(terminal) = app.terminals.get(app.active_terminal) {
                let _ = terminal.session.write("\u{3}");
                app.status = "Interrupt sent to the active task".into();
            } else {
                app.status = "No running terminal task".into();
            }
        }),
        Item::StartDebug => APP.with(|cell| cell.borrow_mut().start_or_continue_debugging()),
        Item::StepOver => APP.with(|cell| {
            cell.borrow_mut()
                .step_debugging(dap_core::PendingRequest::Next)
        }),
        Item::StepInto => APP.with(|cell| {
            cell.borrow_mut()
                .step_debugging(dap_core::PendingRequest::StepIn)
        }),
        Item::StepOut => APP.with(|cell| {
            cell.borrow_mut()
                .step_debugging(dap_core::PendingRequest::StepOut)
        }),
        Item::ToggleBreakpoint => APP.with(|cell| cell.borrow_mut().toggle_breakpoint()),
        Item::StopDebug => APP.with(|cell| cell.borrow_mut().stop_debugging()),
        Item::ShowAiPanel => APP.with(|cell| {
            let mut app = cell.borrow_mut();
            app.chat_open = !app.chat_open;
            app.chat_input_focused = app.chat_open;
            app.status = if app.chat_open {
                "AI panel opened".into()
            } else {
                "AI panel closed".into()
            };
        }),
        Item::Minimize => unsafe {
            ShowWindow(window, SW_MINIMIZE);
        },
        Item::Zoom => unsafe {
            ShowWindow(window, SW_MAXIMIZE);
        },
        Item::NewTerminal => APP.with(|cell| {
            let mut app = cell.borrow_mut();
            app.terminal_open = true;
            app.add_terminal();
        }),
        Item::DetachTerminal => {
            let opened = open_detached_terminal();
            APP.with(|cell| {
                cell.borrow_mut().status = if opened {
                    "Detached terminal opened".into()
                } else {
                    "Could not open detached terminal".into()
                }
            });
        }
        Item::ShowShortcuts => platform::show_message(
            window,
            "TC-IDE keyboard shortcuts",
            "Ctrl+S Save\nCtrl+O Open file\nCtrl+P Search project\nCtrl+` Terminal\nF5 Run / debug\nF9 Breakpoint\nF10 Step over\nF11 Step into",
        ),
        Item::ShowAbout => platform::show_message(
            window,
            "About TC-IDE",
            "TC-IDE 0.3.0\nNative semantic workspace\nhttps://github.com/tianchuangya/TC-IDE",
        ),
        Item::OpenProjectFolder => APP.with(|cell| {
            let mut app = cell.borrow_mut();
            let root = app.project_root.clone();
            app.status = if platform::open_in_explorer(window, &root) {
                format!("Opened Explorer at {}", root.display())
            } else {
                "Could not open the project folder".into()
            };
        }),
        // These are disabled in the native menu until their Windows state and
        // layout exist; retaining the arms makes the exhaustiveness explicit.
        Item::Find
        | Item::Replace
        | Item::GoToLine
        | Item::ToggleProjectTree
        | Item::ToggleGraph
        | Item::FoldAll
        | Item::ToggleFullScreen => {}
        // Already handled through the legacy command table above.
        Item::Undo
        | Item::Redo
        | Item::OpenFile
        | Item::OpenFolder
        | Item::SelectAll
        | Item::Copy
        | Item::Paste
        | Item::FindInProject
        | Item::ToggleTerminal
        | Item::LargerCode
        | Item::SmallerCode
        | Item::Package
        | Item::ShowExtensions
        | Item::ShowSettings => unreachable!(),
    }
}

pub(super) fn handle_character(window: Hwnd, value: char) {
    APP.with(|cell| {
        let mut app = cell.borrow_mut();
        if app.create_kind.is_some() {
            if !value.is_control() {
                app.create_name.push(value);
                app.create_error.clear();
            }
        } else if app.workspace_search_focused {
            if !value.is_control() {
                app.workspace_search.push(value);
                app.project_scroll = 0;
            }
        } else if app.font_search_focused {
            if !value.is_control() {
                app.font_search.push(value);
            }
        } else if app.chat_input_focused {
            if !value.is_control() {
                app.chat_input.push(value);
            }
        } else if app.rename_prompt.is_some() {
            if !value.is_control() {
                if let Some(prompt) = app.rename_prompt.as_mut() {
                    prompt.push(value);
                }
            }
        } else if app.breakpoint_prompt.is_some() {
            if !value.is_control() {
                if let Some(prompt) = app.breakpoint_prompt.as_mut() {
                    prompt.push(value);
                }
            }
        } else if app.ai_prompt_focused {
            if !value.is_control() {
                app.ai_prompt.push(value);
            }
        } else if let Some(field) = app.ai_field_focused {
            if !value.is_control() {
                match field {
                    0 => app.ai_endpoint.push(value),
                    1 => app.ai_model.push(value),
                    _ => app.ai_key.push(value),
                }
            }
        } else if app.extension_search_focused {
            if !value.is_control() {
                app.extension_search.push(value);
                app.extension_scroll = 0;
            }
        } else if app.settings_open || app.extensions_open || app.package_open {
            // Overlay shortcuts must never leak their printable WM_CHAR into the editor.
        } else if app.graph_search_focused {
            if !value.is_control() {
                app.graph_search.push(value);
                app.graph_scroll = 0;
            }
        } else if app.terminal_open {
            app.terminal_scroll = 0;
            app.terminal_selection = None;
            let active = app.active_terminal;
            let Some(tab) = app.terminals.get_mut(active) else {
                return;
            };
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
        } else if app.editing_comment.is_some() {
            if !value.is_control() {
                app.insert_char(value);
            }
        } else if value == '\r' || value == '\n' {
            app.newline();
        } else if !value.is_control() {
            app.insert_char(value);
        }
    });
    invalidate(window);
}

pub(super) unsafe extern "system" fn window_proc(
    window: Hwnd,
    message: Uint,
    w_param: Wparam,
    l_param: Lparam,
) -> Lresult {
    match message {
        WM_PAINT => {
            let mut paint: PaintStruct = unsafe { std::mem::zeroed() };
            unsafe { BeginPaint(window, &mut paint) };
            render_window(window);
            unsafe { EndPaint(window, &paint) };
            0
        }
        WM_ERASEBKGND => 1,
        WM_COMMAND => {
            perform_command(window, w_param & 0xffff);
            0
        }
        WM_CHAR => {
            if let Some(value) = char::from_u32(w_param as u32) {
                handle_character(window, value);
            }
            0
        }
        WM_IME_STARTCOMPOSITION => {
            APP.with(|cell| cell.borrow_mut().ime_composition.clear());
            invalidate(window);
            0
        }
        WM_IME_COMPOSITION => {
            if l_param & GCS_RESULTSTR != 0 {
                let result = ime_string(window, GCS_RESULTSTR as Dword);
                APP.with(|cell| {
                    let mut app = cell.borrow_mut();
                    app.ime_composition.clear();
                    if !result.is_empty() {
                        app.insert_text(&result);
                    }
                });
            } else if l_param & GCS_COMPSTR != 0 {
                let composition = ime_string(window, GCS_COMPSTR as Dword);
                APP.with(|cell| cell.borrow_mut().ime_composition = composition);
            }
            invalidate(window);
            0
        }
        WM_IME_ENDCOMPOSITION => {
            APP.with(|cell| cell.borrow_mut().ime_composition.clear());
            invalidate(window);
            0
        }
        WM_KEYDOWN => handle_key_down(window, w_param),
        WM_LBUTTONDOWN => handle_left_button_down(window, l_param),
        WM_MOUSEWHEEL => handle_mouse_wheel(window, w_param),
        WM_RBUTTONDOWN => handle_right_button_down(window, l_param),
        WM_MOUSEMOVE => handle_mouse_move(window, w_param, l_param),
        WM_LBUTTONUP => handle_left_button_up(window),
        WM_SIZE => {
            let width = (l_param as u32 & 0xffff).max(1);
            let height = ((l_param as u32 >> 16) & 0xffff).max(1);
            RENDERER.with(|cell| {
                if let Some(renderer) = cell.borrow_mut().as_mut() {
                    unsafe { renderer.resize(width, height, platform::dpi_for_window(window)) };
                }
            });
            APP.with(|cell| {
                let scale = platform::dpi_for_window(window) as f32 / 96.0;
                cell.borrow_mut()
                    .resize_viewport(width as i32, height as i32, scale);
            });
            sync_terminal_grid(window);
            invalidate(window);
            0
        }
        WM_TIMER => {
            let reveal = APP.with(|cell| {
                let mut app = cell.borrow_mut();
                app.update_motion();
                if app.package_reveal_pending {
                    app.package_reveal_pending = false;
                    Some(app.project_root.join(".tc-ide-packages"))
                } else {
                    None
                }
            });
            if let Some(path) = reveal {
                let _ = platform::open_in_explorer(window, &path);
            }
            sync_terminal_grid(window);
            APP.with(|cell| {
                let mut app = cell.borrow_mut();
                app.poll_ai_terminal();
                app.confirm_pending_extension();
                app.poll_project_index();
                app.poll_language_server();
                app.poll_debug_session();
                app.poll_chat_session();
                app.poll_format();
                // Coalesce edits: tell the server after typing pauses.
                if app.lsp_dirty
                    && app
                        .lsp_last_change
                        .map(|started| started.elapsed().as_millis() > 220)
                        .unwrap_or(true)
                {
                    app.lsp_dirty = false;
                    app.lsp_completions.clear();
                    app.notify_language_server();
                }
                if app.language_server.is_some()
                    && app
                        .lsp_completion_asked
                        .map(|asked| asked.elapsed().as_millis() > 400)
                        .unwrap_or(true)
                {
                    app.lsp_completion_asked = Some(Instant::now());
                    app.request_language_feature(LspRequest::Completion);
                }
            });
            invalidate(window);
            0
        }
        WM_DPICHANGED => {
            platform::apply_dpi_change(window, l_param);
            RENDERER.with(|cell| *cell.borrow_mut() = None);
            sync_terminal_grid(window);
            APP.with(|cell| {
                cell.borrow_mut().status =
                    format!("DPI changed to {}", platform::dpi_for_window(window))
            });
            invalidate(window);
            0
        }
        WM_DESTROY => {
            // Order matters on shutdown: stop the detached window first so its
            // timer cannot touch the sessions, then release every ConPTY session
            // (each one closes its pseudo console and ends its shell), then drop
            // the Direct2D/DirectWrite state.
            APP.with(|cell| {
                let detached = cell.borrow().detached_terminal;
                if !detached.is_null() {
                    unsafe { DestroyWindow(detached) };
                }
            });
            APP.with(|cell| {
                let mut app = cell.borrow_mut();
                app.terminals.clear();
                save_session(&app.session);
            });
            RENDERER.with(|cell| *cell.borrow_mut() = None);
            unsafe { PostQuitMessage(0) };
            0
        }
        _ => unsafe { DefWindowProcW(window, message, w_param, l_param) },
    }
}

/// Ask before throwing away unsaved edits. Uses a native Windows message box.
pub(super) fn confirm_discard(window: Hwnd, name: &str) -> bool {
    let text = wide(&format!(
        "{name} has unsaved changes. Close it and discard them?"
    ));
    let caption = wide("TC-IDE - unsaved changes");
    let answer = unsafe {
        MessageBoxW(
            window,
            text.as_ptr(),
            caption.as_ptr(),
            MB_YESNO | MB_ICONWARNING,
        )
    };
    answer == IDYES
}
