//! Keys: window messages handled outside the dispatcher.

use super::events::perform_command;
use super::panel::{copy_terminal_selection, handle_terminal_control};
use super::scene::{build_graph, graph_visible_rows};
use super::workspace::save_session;
use super::*;

/// Keyboard handling: panel shortcuts, editor keys and terminal keys.
pub(super) fn handle_key_down(window: Hwnd, w_param: Wparam) -> Lresult {
    if platform::control_pressed() && w_param as u32 == 0x44 && platform::shift_pressed() {
        // Ctrl+Shift+D opens the problems and debug panel.
        APP.with(|cell| cell.borrow_mut().toggle_problems());
        invalidate(window);
        return 0;
    }
    if platform::control_pressed() && w_param as u32 == 0x41 && platform::shift_pressed() {
        APP.with(|cell| {
            let mut app = cell.borrow_mut();
            app.chat_open = !app.chat_open;
            app.chat_input_focused = app.chat_open;
            app.status = if app.chat_open {
                "AI panel: type a prompt and press Enter, Esc cancels a running request".into()
            } else {
                "AI panel closed".into()
            };
        });
        invalidate(window);
        return 0;
    }
    if platform::control_pressed() {
        let terminal_open = APP.with(|cell| cell.borrow().terminal_open);
        if terminal_open
            && w_param as u32 == 0x43
            && platform::shift_pressed()
            && copy_terminal_selection(window)
        {
            APP.with(|cell| {
                cell.borrow_mut().status =
                    "Terminal selection copied; Ctrl+C still interrupts the shell".into()
            });
            invalidate(window);
            return 0;
        }
        if terminal_open && handle_terminal_control(window, w_param as u32) {
            invalidate(window);
            return 0;
        }
        let command = match w_param as u32 {
            0x41 => platform::COMMAND_SELECT_ALL,
            0x43 => platform::COMMAND_COPY_DIAGNOSTIC,
            0x4f => platform::COMMAND_OPEN_FILE,
            0x50 => platform::COMMAND_WORKSPACE_SEARCH,
            0x54 => platform::COMMAND_OPEN_TERMINAL,
            0x56 => platform::COMMAND_PASTE,
            0x58 => platform::COMMAND_CUT,
            0x59 => platform::COMMAND_REDO,
            0x5a => platform::COMMAND_UNDO,
            0xbb | 0x6b => platform::COMMAND_FONT_INCREASE,
            0xbd | 0x6d => platform::COMMAND_FONT_DECREASE,
            0x53 => {
                APP.with(|cell| cell.borrow_mut().save());
                invalidate(window);
                return 0;
            }
            0x49 => {
                APP.with(|cell| {
                    cell.borrow_mut()
                        .request_language_feature(LspRequest::Hover)
                });
                invalidate(window);
                return 0;
            }
            0x52 => {
                // Ctrl+R lists references, then cycles through them.
                APP.with(|cell| {
                    let mut app = cell.borrow_mut();
                    if app.lsp_locations.is_empty() {
                        app.request_language_feature(LspRequest::References);
                    } else {
                        app.jump_to_next_reference();
                    }
                });
                invalidate(window);
                return 0;
            }
            0x57 => {
                // Ctrl+W closes the active document.
                APP.with(|cell| {
                    let mut app = cell.borrow_mut();
                    let index = app.session.active();
                    app.close_document(index, window);
                    save_session(&app.session);
                });
                invalidate(window);
                return 0;
            }
            _ => 0,
        };
        if command != 0 {
            perform_command(window, command);
            return 0;
        }
    }
    if w_param as u32 == 0x78 {
        // F9 toggles the breakpoint, Ctrl+F9 conditions it in place.
        APP.with(|cell| {
            let mut app = cell.borrow_mut();
            if platform::control_pressed() {
                app.begin_breakpoint_condition();
            } else {
                app.toggle_breakpoint();
            }
        });
        invalidate(window);
        return 0;
    }
    if w_param as u32 == 0x74 {
        APP.with(|cell| {
            let mut app = cell.borrow_mut();
            if platform::shift_pressed() {
                app.stop_debugging();
            } else {
                app.start_or_continue_debugging();
            }
        });
        invalidate(window);
        return 0;
    }
    if w_param as u32 == 0x79 {
        APP.with(|cell| {
            cell.borrow_mut()
                .step_debugging(dap_core::PendingRequest::Next)
        });
        invalidate(window);
        return 0;
    }
    if w_param as u32 == 0x7a {
        APP.with(|cell| {
            let request = if platform::shift_pressed() {
                dap_core::PendingRequest::StepOut
            } else {
                dap_core::PendingRequest::StepIn
            };
            cell.borrow_mut().step_debugging(request)
        });
        invalidate(window);
        return 0;
    }
    if w_param as u32 == 0x7b {
        APP.with(|cell| {
            cell.borrow_mut()
                .request_language_feature(LspRequest::Definition)
        });
        invalidate(window);
        return 0;
    }
    // An inline prompt owns the keyboard until Enter or Esc.
    if APP.with(|cell| cell.borrow().rename_prompt.is_some()) {
        let handled = APP.with(|cell| {
            let mut app = cell.borrow_mut();
            match w_param as u32 {
                0x1b => {
                    app.rename_prompt = None;
                    app.status = "Rename cancelled".into();
                }
                VK_BACK => {
                    if let Some(prompt) = app.rename_prompt.as_mut() {
                        prompt.pop();
                    }
                }
                0x0d => {
                    // The prompt is the new name; the request reads it back.
                    if app
                        .rename_prompt
                        .as_deref()
                        .map(str::trim)
                        .unwrap_or_default()
                        .is_empty()
                    {
                        app.status = "Rename needs a new name".into();
                    } else {
                        app.request_language_feature(LspRequest::Rename);
                        app.rename_prompt = None;
                    }
                }
                _ => return false,
            }
            true
        });
        if handled {
            invalidate(window);
            return 0;
        }
    }
    if APP.with(|cell| cell.borrow().breakpoint_prompt.is_some()) {
        let handled = APP.with(|cell| {
            let mut app = cell.borrow_mut();
            match w_param as u32 {
                0x1b => {
                    app.breakpoint_prompt = None;
                    app.status = "Breakpoint condition cancelled".into();
                }
                VK_BACK => {
                    if let Some(prompt) = app.breakpoint_prompt.as_mut() {
                        prompt.pop();
                    }
                }
                0x0d => app.apply_breakpoint_condition(),
                _ => return false,
            }
            true
        });
        if handled {
            invalidate(window);
            return 0;
        }
    }
    if w_param as u32 == 0x71 {
        // F2 asks for a new name and then for the rename edits.
        APP.with(|cell| {
            let mut app = cell.borrow_mut();
            app.rename_prompt = Some(String::new());
            app.graph_search_focused = false;
            app.status = "Rename: type the new name and press Enter".into();
        });
        invalidate(window);
        return 0;
    }
    APP.with(|cell| {
        let mut app = cell.borrow_mut();
        if app.package_open {
            match w_param as u32 {
                0x1b => app.package_open = false,
                0x31 => app.package_targets[0] = !app.package_targets[0],
                0x32 => app.package_targets[1] = !app.package_targets[1],
                0x33 => app.package_targets[2] = !app.package_targets[2],
                0x0d => app.start_package(),
                _ => return,
            }
            app.save_package_targets();
            return;
        }
        if app.create_kind.is_some() {
            match w_param as u32 {
                0x0d => app.create_requested_item(),
                0x1b => {
                    app.create_kind = None;
                    app.create_error.clear();
                }
                VK_BACK => {
                    app.create_name.pop();
                    app.create_error.clear();
                }
                VK_UP => {
                    app.project_language = app.project_language.saturating_sub(1);
                    app.project_environment = 0;
                }
                VK_DOWN => {
                    app.project_language =
                        (app.project_language + 1) % WindowsAppState::PROJECT_LANGUAGES.len();
                    app.project_environment = 0;
                }
                VK_LEFT => {
                    app.project_environment = app.project_environment.saturating_sub(1);
                }
                VK_RIGHT => {
                    let environment_count = app.project_environments().len();
                    app.project_environment = (app.project_environment + 1) % environment_count;
                }
                _ => return,
            }
            app.save_preferences();
            return;
        }
        if app.settings_open {
            if app.font_search_focused {
                match w_param as u32 {
                    0x1b | 0x0d => app.font_search_focused = false,
                    VK_BACK => {
                        app.font_search.pop();
                    }
                    _ => return,
                }
                return;
            }
            // Tab walks the controls the keyboard can actually use, Enter uses
            // the one it is on, and Esc leaves the controls before it closes the
            // panel; the shared rule decides what is reachable.
            let controls = background_core::background_controls(
                app.background_path.is_some(),
                app.background_opacity,
                app.background_zoom as f64,
            );
            let reachable = background_core::SettingsRow::focusable(controls);
            if w_param as u32 == VK_TAB {
                let forward = !platform::shift_pressed();
                app.settings_focus =
                    theme_core::step_focus(&reachable, app.settings_focus, forward);
                if let Some(row) = app.settings_focus {
                    app.status = format!("Settings control: {row:?}");
                }
                return;
            }
            if matches!(w_param as u32, 0x0d | 0x20) && app.settings_focus.is_some() {
                let row = app.settings_focus;
                app.settings_focus = None;
                match row {
                    Some(background_core::SettingsRow::Close) => app.settings_open = false,
                    Some(background_core::SettingsRow::ChooseBackground) => {
                        if let Some(path) = platform::choose_background(window) {
                            app.background_path = Some(path);
                            app.status = "Background image loaded with Windows WIC".into();
                        }
                    }
                    Some(background_core::SettingsRow::AdjustBackground) => {
                        // Enter on the adjustment row steps the zoom, which is
                        // the one value with a natural next state.
                        app.background_zoom =
                            background_core::zoomed(app.background_zoom as f64, 1) as f32;
                        app.status = format!("Zoom {:.1}x", app.background_zoom);
                    }
                    Some(background_core::SettingsRow::ClearBackground) => {
                        app.background_path = None;
                        app.status = "Background cleared".into();
                    }
                    Some(_) => {}
                    None => {}
                }
                app.save_preferences();
                return;
            }
            if w_param as u32 == 0x1b && app.settings_focus.is_some() {
                app.settings_focus = None;
                app.status = "Left the settings controls".into();
                return;
            }
            match w_param as u32 {
                0x1b => {
                    app.settings_open = false;
                    app.settings_focus = None;
                }
                0x4c => app.ui_chinese = !app.ui_chinese,
                0x54 => app.light_theme = !app.light_theme,
                0x42 => {
                    if let Some(path) = platform::choose_background(window) {
                        app.background_path = Some(path);
                        app.status = "Background image loaded with Windows WIC".into();
                    }
                }
                0x43 => app.background_path = None,
                0x4f => app.background_opacity = (app.background_opacity - 0.05).max(0.05),
                0x50 => app.background_opacity = (app.background_opacity + 0.05).min(1.0),
                0x41 => app.background_offset_x = (app.background_offset_x - 0.05).max(-1.0),
                0x44 => app.background_offset_x = (app.background_offset_x + 0.05).min(1.0),
                0x57 => app.background_offset_y = (app.background_offset_y - 0.05).max(-1.0),
                0x53 => app.background_offset_y = (app.background_offset_y + 0.05).min(1.0),
                0x5a => app.background_zoom = (app.background_zoom - 0.1).max(1.0),
                0x58 => app.background_zoom = (app.background_zoom + 0.1).min(3.0),
                VK_LEFT => app.code_zoom = (app.code_zoom - 1).max(-2),
                VK_RIGHT => app.code_zoom = (app.code_zoom + 1).min(8),
                _ => return,
            }
            app.save_preferences();
            RENDERER.with(|renderer| *renderer.borrow_mut() = None);
            return;
        }
        if app.extensions_open {
            if app.extension_search_focused {
                match w_param as u32 {
                    0x1b | 0x0d => app.extension_search_focused = false,
                    VK_BACK => {
                        app.extension_search.pop();
                        app.extension_scroll = 0;
                    }
                    _ => return,
                }
                return;
            }
            if let Some(field) = app.ai_field_focused {
                match w_param as u32 {
                    0x1b | 0x0d => app.ai_field_focused = None,
                    VK_BACK => match field {
                        0 => {
                            app.ai_endpoint.pop();
                        }
                        1 => {
                            app.ai_model.pop();
                        }
                        _ => {
                            app.ai_key.pop();
                        }
                    },
                    _ => return,
                }
                return;
            }
            match w_param as u32 {
                0x1b => {
                    app.extensions_open = false;
                    app.ai_field_focused = None;
                }
                VK_LEFT => app.extension_category = app.extension_category.saturating_sub(1),
                VK_RIGHT => app.extension_category = (app.extension_category + 1) % 5,
                VK_UP => {
                    app.selected_language = app.selected_language.saturating_sub(1);
                    if app.extension_category == 0 {
                        app.describe_selected_extension();
                    }
                }
                VK_DOWN => {
                    app.selected_language =
                        (app.selected_language + 1) % WindowsAppState::EXTENSION_LANGUAGES.len();
                    if app.extension_category == 0 {
                        app.describe_selected_extension();
                    }
                }
                // U upgrades the selected extension when the catalog is newer,
                // X removes it and drops its runtime cache entry.
                0x55 if app.extension_category == 0 => app.upgrade_selected_extension(),
                0x58 if app.extension_category == 0 => app.uninstall_selected_extension(),
                0x20 if app.extension_category == 0 => {
                    let index = app.selected_language;
                    app.language_installed[index] = !app.language_installed[index];
                }
                0x56 if app.extension_category == 0 => {
                    let index = app.selected_language;
                    app.language_versions[index] = (app.language_versions[index] + 1) % 3;
                }
                _ => return,
            }
            app.save_preferences();
            return;
        }
        if app.problems_open {
            match w_param as u32 {
                0x1b => app.problems_open = false,
                VK_TAB => {
                    app.problem_filter = app.problem_filter.next();
                    app.problems_selection = 0;
                    let count = app.problem_rows().len();
                    app.status = format!(
                        "Problems filter: {} ({count} row(s)); Tab cycles",
                        app.problem_filter.label()
                    );
                }
                VK_UP => app.move_problem_selection(-1),
                VK_DOWN => app.move_problem_selection(1),
                0x0d => app.activate_problem_row(),
                _ => return,
            }
            return;
        }
        if app.chat_input_focused {
            // The AI panel owns the keyboard while it is open.
            match w_param as u32 {
                0x1b => {
                    if app.chat_session.is_some() {
                        app.cancel_chat();
                    } else {
                        app.chat_input_focused = false;
                        app.chat_open = false;
                    }
                }
                0x0d => {
                    if app.chat_session.is_none() {
                        app.send_chat_prompt();
                    } else {
                        app.status = "An AI request is already running".into();
                    }
                }
                VK_BACK => {
                    app.chat_input.pop();
                }
                _ => return,
            }
            return;
        }
        if app.graph_search_focused {
            match w_param as u32 {
                0x1b => {
                    if app.graph_search.is_empty() {
                        app.graph_search_focused = false;
                    } else {
                        app.graph_search.clear();
                        app.graph_scroll = 0;
                    }
                }
                0x0d => app.graph_search_focused = false,
                VK_BACK => {
                    app.graph_search.pop();
                    app.graph_scroll = 0;
                }
                VK_LEFT => {
                    app.graph_scope = graph_core::cycle_scope(app.graph_scope, false);
                    app.graph_scroll = 0;
                }
                VK_RIGHT | VK_TAB => {
                    app.graph_scope = graph_core::cycle_scope(app.graph_scope, true);
                    app.graph_scroll = 0;
                }
                _ => return,
            }
            return;
        }
        if app.workspace_search_focused {
            match w_param as u32 {
                0x1b | 0x0d => app.workspace_search_focused = false,
                VK_BACK => {
                    app.workspace_search.pop();
                    app.project_scroll = 0;
                }
                _ => return,
            }
            return;
        }
        if app.graph_keyboard_active {
            let columns = graph_visible_rows(
                &area_for(window),
                platform::dpi_for_window(window) as f32 / 96.0,
                app.terminal_motion,
            );
            match w_param as u32 {
                VK_UP | VK_DOWN | 0x0d | 0x20 | VK_LEFT | VK_RIGHT => {}
                _ => {
                    app.graph_keyboard_active = false;
                    return;
                }
            }
            if matches!(w_param as u32, VK_LEFT | VK_RIGHT) {
                app.graph_relation = if w_param as u32 == VK_RIGHT {
                    app.graph_relation.next()
                } else {
                    app.graph_relation.previous()
                };
                app.graph_scroll = 0;
                app.graph_selected = None;
                app.status = format!("Graph relation: {}", app.graph_relation.label());
                return;
            }
            let symbols = editor_core::symbol_occurrences(&app.lines, &app.file_name);
            let graph = build_graph(&app, &symbols);
            let current = app
                .graph_selected
                .unwrap_or(0)
                .min(graph.len().saturating_sub(1));
            let index = match w_param as u32 {
                VK_UP => current.saturating_sub(1),
                VK_DOWN => (current + 1).min(graph.len().saturating_sub(1)),
                _ => current,
            };
            app.graph_selected = Some(index);
            let first = graph.clamped_scroll(app.graph_scroll, columns);
            if index < first {
                app.graph_scroll += first - index;
            } else if index >= first + columns {
                app.graph_scroll = app
                    .graph_scroll
                    .saturating_sub(index - (first + columns - 1));
            }
            match w_param as u32 {
                0x0d => {
                    if let Some(target) =
                        graph.nodes.get(index).and_then(|node| node.target.as_ref())
                    {
                        let path = app.project_root.join(&target.file);
                        let line = target.line;
                        let column = target.column;
                        app.open_file(path);
                        app.caret_line = line.min(app.lines.len().saturating_sub(1));
                        app.caret_column = column;
                        app.clamp_caret();
                        app.status = format!("Jumped to relation at {}", app.file_name);
                    }
                }
                0x20 => {
                    if let Some(node) = graph.nodes.get(index) {
                        if node.has_children() {
                            let key = node.key.clone();
                            let label = node.label.clone();
                            if let Some(position) =
                                app.graph_folded.iter().position(|it| *it == key)
                            {
                                app.graph_folded.remove(position);
                                app.status = format!("Expanded {label}");
                            } else {
                                app.graph_folded.push(key);
                                app.status = format!("Collapsed {label}");
                            }
                        }
                    }
                }
                _ => {
                    if let Some(node) = graph.nodes.get(index) {
                        app.status = format!("Graph selection: {}", node.caption());
                    }
                }
            }
            return;
        }
        if !app.terminal_open {
            let old = app.position();
            match w_param as u32 {
                VK_BACK => app.backspace(),
                VK_DELETE => app.delete_forward(),
                VK_TAB => app.accept_completion(),
                VK_HOME => app.caret_column = 0,
                VK_END => {
                    app.caret_column = app.lines[app.caret_line].chars().count();
                }
                VK_LEFT => app.move_left(),
                VK_RIGHT => app.move_right(),
                VK_UP => app.caret_line = app.caret_line.saturating_sub(1),
                VK_DOWN => app.caret_line += 1,
                _ => return,
            }
            app.clamp_caret();
            app.reveal_caret();
            if app.editing_comment.is_some() {
                app.clamp_comment_caret();
            }
            if !matches!(w_param as u32, VK_BACK | VK_DELETE) {
                if platform::shift_pressed() {
                    if app.selection_anchor.is_none() {
                        app.selection_anchor = Some(old);
                    }
                } else {
                    app.selection_anchor = None;
                }
            }
        } else {
            app.terminal_scroll = 0;
            app.terminal_selection = None;
            let active = app.active_terminal;
            let sequence = match w_param as u32 {
                VK_UP => "\u{1b}[A",
                VK_DOWN => "\u{1b}[B",
                VK_RIGHT => "\u{1b}[C",
                VK_LEFT => "\u{1b}[D",
                VK_HOME => "\u{1b}[H",
                VK_END => "\u{1b}[F",
                VK_DELETE => "\u{1b}[3~",
                VK_TAB => "\t",
                _ => return,
            };
            if let Some(tab) = app.terminals.get(active) {
                let _ = tab.session.write(sequence);
            }
        }
    });
    invalidate(window);
    0
}
