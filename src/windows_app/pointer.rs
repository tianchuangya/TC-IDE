//! Pointer: window messages handled outside the dispatcher.

use super::detached::open_detached_terminal;
use super::events::{comment_bubble_at, editor_position_at, perform_command};
use super::panel::{terminal_buffer, terminal_point_at};
use super::scene::{
    build_graph, graph_left, graph_scope_rect, graph_search_rect, graph_visible_rows,
};
use super::workspace::save_session;
use super::*;

fn background_preview_rect(area: &Rect, scale: f32) -> background_core::Rect {
    let left = area.right as f32 - 410.0 * scale;
    let top = 58.0 * scale;
    background_core::Rect::new(
        (left + 18.0 * scale) as f64,
        (top + 444.0 * scale) as f64,
        (358.0 * scale) as f64,
        (132.0 * scale) as f64,
    )
}

fn background_view_size(app: &WindowsAppState, area: &Rect, scale: f32) -> (f64, f64) {
    let width = area.right as f32;
    let height = area.bottom as f32;
    let tree = (width * 0.19).max(220.0 * scale);
    let graph = (width * 0.27).max(300.0 * scale);
    let panel = terminal_core::panel_metrics(width, height, scale, app.terminal_motion);
    (
        (width - graph - tree).max(1.0) as f64,
        (height - panel.height - 86.0 * scale).max(1.0) as f64,
    )
}

fn background_crop_on_screen(
    app: &WindowsAppState,
    area: &Rect,
    scale: f32,
    image: (f64, f64),
) -> (background_core::Rect, background_core::Placement) {
    let preview_rect = background_preview_rect(area, scale);
    let preview =
        background_core::preview_placement(image, (preview_rect.width, preview_rect.height));
    let view = background_view_size(app, area, scale);
    let placed = background_core::placement(
        image,
        view,
        app.background_zoom as f64,
        (
            app.background_offset_x as f64,
            app.background_offset_y as f64,
        ),
    );
    let crop = preview.project(&placed.crop(view, image));
    (
        background_core::Rect::new(
            preview_rect.x + crop.x,
            preview_rect.y + crop.y,
            crop.width,
            crop.height,
        ),
        preview,
    )
}

/// Mouse press: overlays, terminal tabs, the graph, the tree and the editor.
pub(super) fn handle_left_button_down(window: Hwnd, l_param: Lparam) -> Lresult {
    // A control shows the pressed look while the button stays down.
    APP.with(|cell| cell.borrow_mut().pointer_pressed = true);
    let x = (l_param as u16 as i16) as i32;
    let y = ((l_param >> 16) as u16 as i16) as i32;
    let mut area = unsafe { std::mem::zeroed::<Rect>() };
    unsafe { GetClientRect(window, &mut area) };
    let scale = platform::dpi_for_window(window) as f32 / 96.0;
    let create_kind = APP.with(|cell| cell.borrow().create_kind);
    if let Some(kind) = create_kind {
        let dialog_w = (520.0 * scale).min(area.right as f32 - 40.0 * scale);
        let dialog_h = if kind == CreateKind::Project {
            350.0
        } else {
            230.0
        } * scale;
        let left = (area.right as f32 - dialog_w) * 0.5;
        let top = (area.bottom as f32 - dialog_h) * 0.42;
        if x as f32 >= left
            && x as f32 <= left + dialog_w
            && y as f32 >= top
            && y as f32 <= top + dialog_h
        {
            let button_top = top + dialog_h - 56.0 * scale;
            if y as f32 >= button_top {
                APP.with(|cell| {
                    let mut app = cell.borrow_mut();
                    if (x as f32) < left + dialog_w * 0.5 {
                        app.create_kind = None;
                        app.create_error.clear();
                    } else {
                        app.create_requested_item();
                    }
                });
            } else if kind == CreateKind::Project
                && y as f32 >= top + 162.0 * scale
                && (y as f32) < top + 202.0 * scale
            {
                APP.with(|cell| {
                    let mut app = cell.borrow_mut();
                    if (x as f32) < left + dialog_w * 0.5 {
                        app.project_language =
                            (app.project_language + 1) % WindowsAppState::PROJECT_LANGUAGES.len();
                        app.project_environment = 0;
                    } else {
                        app.project_environment =
                            (app.project_environment + 1) % app.project_environments().len();
                    }
                    app.save_preferences();
                });
            } else if kind == CreateKind::Project
                && y as f32 >= top + 202.0 * scale
                && (y as f32) < top + 238.0 * scale
            {
                APP.with(|cell| {
                    let mut app = cell.borrow_mut();
                    let language = app.project_language;
                    app.language_versions[language] = (app.language_versions[language] + 1) % 3;
                    app.save_preferences();
                });
            } else if kind == CreateKind::Project
                && y as f32 >= top + 238.0 * scale
                && (y as f32) < top + 282.0 * scale
            {
                if let Some(location) = platform::choose_folder(window) {
                    APP.with(|cell| cell.borrow_mut().create_location = location);
                }
            }
            invalidate(window);
            return 0;
        }
        return 0;
    }
    if y as f32 <= 48.0 * scale && x as f32 >= area.right as f32 - 155.0 * scale {
        APP.with(|cell| {
            let mut app = cell.borrow_mut();
            app.package_open = !app.package_open;
            app.settings_open = false;
            app.extensions_open = false;
        });
        invalidate(window);
        return 0;
    }
    let package_open = APP.with(|cell| cell.borrow().package_open);
    let package_left = area.right as f32 - 326.0 * scale;
    let package_top = 56.0 * scale;
    if package_open
        && x as f32 >= package_left
        && x as f32 <= area.right as f32 - 14.0 * scale
        && y as f32 >= package_top
        && y as f32 <= package_top + 240.0 * scale
    {
        APP.with(|cell| {
            let mut app = cell.borrow_mut();
            let local_y = y as f32 - package_top;
            if (42.0 * scale..174.0 * scale).contains(&local_y) {
                let index = ((local_y / scale - 42.0) / 42.0).floor() as usize;
                if index < app.package_targets.len() {
                    app.package_targets[index] = !app.package_targets[index];
                    app.save_package_targets();
                }
            } else if (166.0 * scale..196.0 * scale).contains(&local_y) {
                if app.signing_certificate.is_some() {
                    app.signing_certificate = None;
                    app.status = "MSIX signing certificate cleared".into();
                } else if let Some(path) = platform::choose_signing_certificate(window) {
                    app.status = format!("MSIX signing certificate: {}", path.display());
                    app.signing_certificate = Some(path);
                } else {
                    app.status = "No signing certificate selected".into();
                }
            } else if local_y >= 200.0 * scale {
                app.start_package();
            }
        });
        invalidate(window);
        return 0;
    }
    let settings_open = APP.with(|cell| cell.borrow().settings_open);
    if settings_open {
        let left = area.right as f32 - 410.0 * scale;
        let top = 58.0 * scale;
        if x as f32 >= left && x as f32 <= area.right as f32 - 20.0 * scale && y as f32 >= top {
            let preview_rect = background_preview_rect(&area, scale);
            if preview_rect.contains((x as f64, y as f64)) {
                let image = RENDERER.with(|renderer| {
                    renderer
                        .borrow()
                        .as_ref()
                        .and_then(DirectEditorRenderer::background_size)
                });
                if let Some(image) = image {
                    let handle = APP.with(|cell| {
                        let app = cell.borrow();
                        let (crop, _) = background_crop_on_screen(&app, &area, scale, image);
                        background_core::handle_at(
                            &crop,
                            (x as f64, y as f64),
                            (12.0 * scale) as f64,
                        )
                    });
                    if let Some(handle) = handle {
                        APP.with(|cell| {
                            cell.borrow_mut().background_drag = Some((handle, x, y));
                        });
                        unsafe { SetCapture(window) };
                    }
                }
                invalidate(window);
                return 0;
            }
            APP.with(|cell| {
                let mut app = cell.borrow_mut();
                let local_y = (y as f32 - top) / scale;
                if (62.0..116.0).contains(&local_y) {
                    app.ui_chinese = !app.ui_chinese;
                } else if (116.0..170.0).contains(&local_y) {
                    app.light_theme = !app.light_theme;
                } else if (170.0..224.0).contains(&local_y) {
                    if x as f32 <= left + 195.0 * scale {
                        app.code_zoom = (app.code_zoom - 1).max(-2);
                    } else {
                        app.code_zoom = (app.code_zoom + 1).min(8);
                    }
                } else if (224.0..278.0).contains(&local_y) {
                    app.font_search_focused = true;
                } else if (278.0..380.0).contains(&local_y) {
                    let visible = ((local_y - 278.0) / 34.0).floor() as usize;
                    if let Some(index) = app.filtered_font_indices().get(visible).copied() {
                        app.code_font_family = index;
                        app.status = format!("Editor font: {}", WindowsAppState::CODE_FONTS[index]);
                    }
                } else if (380.0..434.0).contains(&local_y) {
                    if let Some(path) = platform::choose_background(window) {
                        app.background_path = Some(path);
                    }
                } else if (580.0..634.0).contains(&local_y) {
                    let local_x = (x as f32 - left) / (390.0 * scale);
                    if local_x < 0.25 {
                        app.background_opacity = (app.background_opacity - 0.05).max(0.05);
                    } else if local_x < 0.5 {
                        app.background_opacity = (app.background_opacity + 0.05).min(1.0);
                    } else if local_x < 0.75 {
                        app.background_zoom = (app.background_zoom - 0.1).max(1.0);
                    } else {
                        app.background_zoom = (app.background_zoom + 0.1).min(3.0);
                    }
                } else if (634.0..680.0).contains(&local_y) {
                    app.background_path = None;
                    app.background_drag = None;
                } else if local_y >= 680.0 {
                    app.settings_open = false;
                    app.font_search_focused = false;
                    app.background_drag = None;
                }
                app.save_preferences();
            });
            RENDERER.with(|renderer| *renderer.borrow_mut() = None);
            invalidate(window);
            return 0;
        }
    }
    let extensions_open = APP.with(|cell| cell.borrow().extensions_open);
    if extensions_open {
        let panel_w = (920.0 * scale).min(area.right as f32 - 50.0 * scale);
        let panel_h = (610.0 * scale).min(area.bottom as f32 - 80.0 * scale);
        let left = (area.right as f32 - panel_w) * 0.5;
        let top = 58.0 * scale;
        if x as f32 >= left
            && x as f32 <= left + panel_w
            && y as f32 >= top
            && y as f32 <= top + panel_h
        {
            APP.with(|cell| {
                    let mut app = cell.borrow_mut();
                    if x as f32 <= left + 184.0 * scale {
                        let category = ((y as f32 - top - 58.0 * scale) / (44.0 * scale))
                            .floor()
                            .clamp(0.0, 4.0)
                            as usize;
                        app.extension_category = category;
                        app.extension_search_focused = false;
                    } else if app.extension_category == 0 {
                        let local_y = (y as f32 - top) / scale;
                        if (72.0..116.0).contains(&local_y) {
                            app.extension_search_focused = true;
                        } else if (124.0..508.0).contains(&local_y) {
                            let visible = ((local_y - 124.0) / 48.0).floor() as usize;
                            let position = app.extension_scroll + visible;
                            if let Some(language) =
                                app.filtered_language_indices().get(position).copied()
                            {
                                app.selected_language = language;
                                if x as f32 >= left + 445.0 * scale {
                                    let name =
                                        WindowsAppState::EXTENSION_LANGUAGES[language].to_string();
                                    app.language_installed[language] =
                                        !app.language_installed[language];
                                    if app.project_root.join(".tc-ide-catalog.conf").is_file() {
                                        app.install_language_extension(&name);
                                    } else {
                                        app.status = format!(
                                            "{name} marked as wanted; add .tc-ide-catalog.conf to the project to download it for real"
                                        );
                                    }
                                } else if x as f32 >= left + 315.0 * scale {
                                    app.language_versions[language] =
                                        (app.language_versions[language] + 1) % 3;
                                }
                            }
                        }
                    } else if app.extension_category == 1 {
                        let local_y = (y as f32 - top) / scale;
                        if (190.0..240.0).contains(&local_y) {
                            app.light_theme = false;
                            app.status = "Dark theme applied".into();
                        } else if (240.0..290.0).contains(&local_y) {
                            app.light_theme = true;
                            app.status = "Light theme applied".into();
                        }
                    } else if matches!(app.extension_category, 2 | 3) && (348.0..392.0)
                        .contains(&((y as f32 - top) / scale))
                    {
                        if app.extension_category == 2 {
                            app.start_debug_adapter();
                        } else {
                            app.format_current_file();
                        }
                    } else if matches!(app.extension_category, 2 | 3) {
                        let local_y = (y as f32 - top) / scale;
                        if (190.0..340.0).contains(&local_y) {
                            let item = ((local_y - 190.0) / 50.0).floor() as usize;
                            let (label, command) = if app.extension_category == 2 {
                                [
                                    ("LLDB", "lldb --version"),
                                    ("GDB", "gdb --version"),
                                    ("debugpy", "python -m debugpy --version"),
                                ][item.min(2)]
                            } else {
                                [
                                    ("Biome", "biome --version"),
                                    ("clang-format", "clang-format --version"),
                                    ("Ruff", "ruff --version"),
                                ][item.min(2)]
                            };
                            app.probe_extension_tool(label, command);
                        }
                    } else if app.extension_category == 4 {
                        let content_left = left + 205.0 * scale;
                        for field in 0..3 {
                            let field_top = top + (142.0 + field as f32 * 92.0) * scale;
                            if x as f32 >= content_left
                                && x as f32 <= content_left + 600.0 * scale
                                && y as f32 >= field_top
                                && y as f32 <= field_top + 42.0 * scale
                            {
                                app.ai_field_focused = Some(field);
                            }
                        }
                        let prompt_top = top + 500.0 * scale;
                        if x as f32 >= content_left
                            && x as f32 <= content_left + 600.0 * scale
                            && y as f32 >= prompt_top + 26.0 * scale
                            && y as f32 <= prompt_top + 68.0 * scale
                        {
                            app.ai_prompt_focused = true;
                            app.ai_field_focused = None;
                            return;
                        }
                        if y as f32 >= prompt_top + 78.0 * scale
                            && y as f32 <= prompt_top + 120.0 * scale
                        {
                            if x as f32 >= content_left
                                && x as f32 <= content_left + 210.0 * scale
                            {
                                let prompt = app.ai_prompt.clone();
                                app.ai_prompt_focused = false;
                                app.run_ai_request(&prompt, true);
                            } else if x as f32 >= content_left + 226.0 * scale
                                && x as f32 <= content_left + 466.0 * scale
                            {
                                app.ai_prompt_focused = false;
                                app.run_ai_request("", false);
                            }
                            return;
                        }
                        let save_top = top + 440.0 * scale;
                        if x as f32 >= content_left
                            && x as f32 <= content_left + 210.0 * scale
                            && y as f32 >= save_top
                            && y as f32 <= save_top + 42.0 * scale
                        {
                            if app.ai_endpoint.trim().is_empty()
                                || app.ai_model.trim().is_empty()
                            {
                                app.status = "AI endpoint and model are required".into();
                            } else {
                                app.save_preferences();
                                app.status =
                                    "AI API saved; API key remains in memory for this session"
                                        .into();
                            }
                        }
                    }
                    app.save_preferences();
                });
            invalidate(window);
            return 0;
        }
    }
    // Document tabs sit in the editor header.
    let tab_width = 132.0 * scale;
    let tab_top = 48.0 * scale + 4.0 * scale;
    let tab_bottom = 48.0 * scale + 30.0 * scale;
    let tab_left = (area.right as f32 * 0.19).max(220.0 * scale) + 16.0 * scale;
    let graph_panel_left = graph_left(&area, scale);
    if (y as f32) >= tab_top && (y as f32) <= tab_bottom && (x as f32) >= tab_left {
        let slot = (((x as f32) - tab_left) / tab_width).floor();
        if slot >= 0.0 {
            let index = slot as usize;
            let within = (x as f32) - tab_left - slot * tab_width;
            let closable = within >= tab_width - 30.0 * scale && (x as f32) < graph_panel_left;
            let count = APP.with(|cell| cell.borrow().session.documents().len());
            if index < count {
                APP.with(|cell| {
                    let mut app = cell.borrow_mut();
                    if closable {
                        app.close_document(index, window);
                    } else {
                        app.switch_document(index);
                    }
                    save_session(&app.session);
                });
                invalidate(window);
                return 0;
            }
        }
    }
    let search_rect = graph_search_rect(&area, scale);
    let scope_rect = graph_scope_rect(&area, scale);
    if x as f32 >= graph_left(&area, scale) {
        if inside(&search_rect, x, y) {
            APP.with(|cell| {
                let mut app = cell.borrow_mut();
                app.graph_search_focused = true;
                app.status = "Graph search: type to filter symbols, esc clears".into();
            });
            invalidate(window);
            return 0;
        }
        if inside(&scope_rect, x, y) {
            APP.with(|cell| {
                let mut app = cell.borrow_mut();
                if platform::shift_pressed() {
                    app.graph_relation = app.graph_relation.next();
                    app.status = format!("Graph relation: {}", app.graph_relation.label());
                } else {
                    app.graph_scope = graph_core::cycle_scope(app.graph_scope, true);
                    let label = match app.graph_scope {
                        Some(scope) => scope.label().to_string(),
                        None => "auto".to_string(),
                    };
                    app.status = format!("Graph range: {label}");
                }
                app.graph_scroll = 0;
            });
            invalidate(window);
            return 0;
        }
        let rows = graph_visible_rows(&area, scale, platform::dpi_for_window(window) as f32);
        let row = ((y as f32 - (48.0 * scale + graph_core::FIRST_ROW_TOP * scale))
            / (graph_core::ROW_HEIGHT * scale))
            .floor();
        let (toggled, target) = APP.with(|cell| {
            let mut app = cell.borrow_mut();
            if row < 0.0 {
                return (false, None);
            }
            let symbols = editor_core::symbol_occurrences(&app.lines, &app.file_name);
            let graph = build_graph(&app, &symbols);
            let index = graph.clamped_scroll(app.graph_scroll, rows) + row as usize;
            let Some(node) = graph.nodes.get(index) else {
                return (false, None);
            };
            let panel_left = graph_left(&area, scale);
            let (marker_left, marker_right) =
                graph_core::marker_span(panel_left, node.depth, scale);
            if node.has_children() && (x as f32) >= marker_left && (x as f32) <= marker_right {
                let key = node.key.clone();
                let folded = node.folded;
                let label = node.label.clone();
                if let Some(position) = app.graph_folded.iter().position(|it| *it == key) {
                    app.graph_folded.remove(position);
                } else {
                    app.graph_folded.push(key);
                }
                app.status = if folded {
                    format!("Expanded {label}")
                } else {
                    format!("Collapsed {label}")
                };
                return (true, None);
            }
            app.graph_selected = Some(index);
            app.graph_keyboard_active = true;
            let Some(target) = node.target.as_ref() else {
                return (false, None);
            };
            (
                false,
                Some((
                    app.project_root.join(&target.file),
                    target.line,
                    target.column,
                )),
            )
        });
        if toggled {
            invalidate(window);
            return 0;
        }
        if let Some((path, line, column)) = target {
            APP.with(|cell| {
                let mut app = cell.borrow_mut();
                app.open_file(path);
                app.caret_line = line.min(app.lines.len().saturating_sub(1));
                app.caret_column = column;
                app.clamp_caret();
                app.graph_search_focused = false;
                app.status = format!(
                    "Jumped to relation at {}:{}",
                    app.file_name,
                    app.caret_line + 1
                );
            });
            invalidate(window);
            return 0;
        }
        APP.with(|cell| cell.borrow_mut().graph_search_focused = false);
    }
    if let Some((line, toggle)) = comment_bubble_at(window, x, y) {
        APP.with(|cell| {
            let mut app = cell.borrow_mut();
            if toggle {
                if !app.expanded_comments.remove(&line) {
                    app.expanded_comments.insert(line);
                }
            } else if let Some((_, _, end)) = app
                .lines
                .get(line)
                .and_then(|value| editor_core::comment_bounds(value, &app.file_name))
            {
                app.editing_comment = Some(line);
                app.caret_line = line;
                app.caret_column = app.lines[line][..end].chars().count();
                app.selection_anchor = None;
                app.status = "Comment bubble edit mode: boundaries protected".into();
            }
        });
        invalidate(window);
        return 0;
    }
    let panel = terminal_core::panel_metrics(
        area.right as f32,
        area.bottom as f32,
        scale,
        APP.with(|cell| cell.borrow().terminal_motion),
    );
    let terminal_top = (area.bottom as f32 - panel.height) as i32;
    if y >= terminal_top && y <= terminal_top + (34.0 * scale) as i32 {
        APP.with(|cell| {
            let mut app = cell.borrow_mut();
            if x as f32 >= 150.0 * scale && (x as f32) < 225.0 * scale {
                app.run_task(true);
            } else if x as f32 >= 225.0 * scale && (x as f32) < 310.0 * scale {
                app.run_task(false);
            } else {
                app.toggle_terminal();
            }
        });
        invalidate(window);
    } else if x as f32 >= panel.list_left && y as f32 >= panel.body_top {
        APP.with(|cell| {
            let mut app = cell.borrow_mut();
            if !app.terminal_open {
                return;
            }
            let local_y = y as f32 - panel.body_top;
            if local_y <= 30.0 * scale {
                if x as f32 <= area.right as f32 - 82.0 * scale {
                    app.add_terminal();
                } else {
                    drop(app);
                    let opened = open_detached_terminal();
                    APP.with(|cell| {
                        cell.borrow_mut().status = if opened {
                            "Current ConPTY terminal detached into a native window".into()
                        } else {
                            "Could not create the detached terminal window".into()
                        };
                    });
                }
            } else {
                let index = ((local_y / scale - 34.0) / 30.0).floor() as usize;
                if index < app.terminals.len() {
                    app.active_terminal = index;
                    app.status = format!("Active terminal: {}", app.terminals[index].name);
                }
            }
        });
        invalidate(window);
    } else if y as f32 >= panel.body_top
        && y as f32 <= panel.body_bottom
        && x as f32 >= panel.body_left
        && x as f32 <= panel.body_right
    {
        let started = APP.with(|cell| {
            let mut app = cell.borrow_mut();
            app.graph_keyboard_active = false;
            if !app.terminal_open {
                return false;
            }
            let point = terminal_point_at(&app, &panel, scale, x, y, false);
            match point {
                Some(point) => {
                    app.terminal_selection =
                        Some(terminal_core::TerminalSelection::new(point, point));
                    app.terminal_dragging = true;
                    app.status = "Terminal selection: drag to extend, Ctrl+Shift+C copies".into();
                    true
                }
                None => {
                    app.terminal_selection = None;
                    false
                }
            }
        });
        if started {
            unsafe { SetCapture(window) };
        }
        invalidate(window);
        return 0;
    } else if x as f32 <= (area.right as f32 * 0.19).max(220.0 * scale) {
        if y as f32 >= 56.0 * scale && y as f32 <= 88.0 * scale {
            APP.with(|cell| {
                let mut app = cell.borrow_mut();
                app.workspace_search_focused = true;
                app.graph_search_focused = false;
                app.create_kind = None;
            });
            invalidate(window);
            return 0;
        }
        let first_row = (48.0 + 58.0) * scale;
        if y as f32 >= first_row {
            let row = ((y as f32 - first_row) / (35.0 * scale)).floor() as usize;
            if row >= 2 {
                APP.with(|cell| {
                    let mut app = cell.borrow_mut();
                    let query = app.workspace_search.to_ascii_lowercase();
                    let path = app
                        .project_entries
                        .iter()
                        .zip(&app.project_entry_paths)
                        .zip(&app.project_entry_is_dir)
                        .filter(|((entry, _), _)| {
                            query.is_empty() || entry.to_ascii_lowercase().contains(&query)
                        })
                        .nth(app.project_scroll + row - 2)
                        .map(|((_, path), is_dir)| (path.clone(), *is_dir));
                    if let Some((path, is_directory)) = path {
                        if is_directory {
                            if !app.collapsed_directories.remove(&path) {
                                app.collapsed_directories.insert(path);
                            }
                            app.refresh_project();
                        } else {
                            app.open_file(path);
                        }
                    }
                });
                invalidate(window);
            }
        }
    } else if let Some(position) = editor_position_at(window, x, y) {
        APP.with(|cell| {
            let mut app = cell.borrow_mut();
            app.graph_keyboard_active = false;
            app.editing_comment = None;
            app.set_caret(position, platform::shift_pressed());
            app.dragging_selection = true;
        });
        unsafe { SetCapture(window) };
        invalidate(window);
    }
    0
}

/// Mouse release: finishes selections and drags.
pub(super) fn handle_left_button_up(window: Hwnd) -> Lresult {
    APP.with(|cell| cell.borrow_mut().pointer_pressed = false);
    let background_dragging = APP.with(|cell| {
        let mut app = cell.borrow_mut();
        let dragging = app.background_drag.take().is_some();
        if dragging {
            app.save_preferences();
        }
        dragging
    });
    if background_dragging {
        unsafe { ReleaseCapture() };
        invalidate(window);
        return 0;
    }
    let terminal_dragging = APP.with(|cell| {
        let mut app = cell.borrow_mut();
        let dragging = app.terminal_dragging;
        app.terminal_dragging = false;
        if app
            .terminal_selection
            .map(|selection| selection.is_empty())
            .unwrap_or(false)
        {
            app.terminal_selection = None;
        }
        dragging
    });
    if terminal_dragging {
        unsafe { ReleaseCapture() };
        let count = APP.with(|cell| {
            let app = cell.borrow();
            app.terminal_selection
                .and_then(|selection| {
                    terminal_core::selected_text(&terminal_buffer(&app), &selection)
                })
                .map(|text| text.chars().count())
                .unwrap_or(0)
        });
        APP.with(|cell| {
            cell.borrow_mut().status = if count > 0 {
                format!("Terminal selection: {count} characters · Ctrl+Shift+C copies")
            } else {
                "Terminal selection cleared".into()
            };
        });
        invalidate(window);
        return 0;
    }
    APP.with(|cell| cell.borrow_mut().dragging_selection = false);
    unsafe { ReleaseCapture() };
    0
}

/// Pointer motion: hover highlights and selection drags.
pub(super) fn handle_mouse_move(window: Hwnd, w_param: Wparam, l_param: Lparam) -> Lresult {
    let dragging = APP.with(|cell| cell.borrow().dragging_selection);
    let terminal_dragging = APP.with(|cell| cell.borrow().terminal_dragging);
    let x = (l_param as u16 as i16) as i32;
    let y = ((l_param >> 16) as u16 as i16) as i32;
    APP.with(|cell| {
        let mut app = cell.borrow_mut();
        app.pointer_x = x;
        app.pointer_y = y;
    });
    let background_dragging = APP.with(|cell| cell.borrow().background_drag.is_some());
    if background_dragging && w_param & 1 != 0 {
        let mut area = unsafe { std::mem::zeroed::<Rect>() };
        unsafe { GetClientRect(window, &mut area) };
        let scale = platform::dpi_for_window(window) as f32 / 96.0;
        let image = RENDERER.with(|renderer| {
            renderer
                .borrow()
                .as_ref()
                .and_then(DirectEditorRenderer::background_size)
        });
        if let Some(image) = image {
            APP.with(|cell| {
                let mut app = cell.borrow_mut();
                let Some((handle, last_x, last_y)) = app.background_drag else {
                    return;
                };
                let view = background_view_size(&app, &area, scale);
                let placed = background_core::placement(
                    image,
                    view,
                    app.background_zoom as f64,
                    (
                        app.background_offset_x as f64,
                        app.background_offset_y as f64,
                    ),
                );
                let crop = placed.crop(view, image);
                let preview_rect = background_preview_rect(&area, scale);
                let preview = background_core::preview_placement(
                    image,
                    (preview_rect.width, preview_rect.height),
                );
                let delta = (
                    (x - last_x) as f64 / preview.scale,
                    (y - last_y) as f64 / preview.scale,
                );
                let edit = background_core::drag_crop(
                    &crop,
                    handle,
                    delta,
                    image,
                    view,
                    (image.0.min(image.1) * 0.05).max(8.0),
                );
                app.background_zoom = edit.zoom as f32;
                app.background_offset_x = edit.offset.0 as f32;
                app.background_offset_y = edit.offset.1 as f32;
                app.background_drag = Some((handle, x, y));
                app.status = format!(
                    "Crop {:.0}×{:.0} at {:.0},{:.0} · zoom {:.1}×",
                    edit.crop.width, edit.crop.height, edit.crop.x, edit.crop.y, edit.zoom
                );
            });
            invalidate(window);
        }
        return 0;
    }
    if terminal_dragging && w_param & 1 != 0 {
        let mut area = unsafe { std::mem::zeroed::<Rect>() };
        unsafe { GetClientRect(window, &mut area) };
        let scale = platform::dpi_for_window(window) as f32 / 96.0;
        let changed = APP.with(|cell| {
            let mut app = cell.borrow_mut();
            let panel = terminal_core::panel_metrics(
                area.right as f32,
                area.bottom as f32,
                scale,
                app.terminal_motion,
            );
            let Some(point) = terminal_point_at(&app, &panel, scale, x, y, true) else {
                return false;
            };
            let Some(selection) = app.terminal_selection else {
                return false;
            };
            if selection.focus == point {
                return false;
            }
            app.terminal_selection = Some(selection.extended_to(point));
            true
        });
        if changed {
            invalidate(window);
        }
        return 0;
    }
    if dragging && w_param & 1 != 0 {
        if let Some(position) = editor_position_at(window, x, y) {
            APP.with(|cell| cell.borrow_mut().set_caret(position, true));
            invalidate(window);
        }
    } else {
        let position = editor_position_at(window, x, y);
        let changed = APP.with(|cell| {
            let mut app = cell.borrow_mut();
            let hovered = position.and_then(|position| {
                editor_core::symbol_at(&app.lines, &app.file_name, position.line, position.column)
            });
            if hovered == app.hovered_symbol {
                false
            } else {
                app.hovered_symbol = hovered;
                app.graph_scroll = 0;
                true
            }
        });
        if changed {
            invalidate(window);
        }
    }
    0
}

/// Wheel scrolling for the editor, graph, tree, terminal panel and overlays.
pub(super) fn handle_mouse_wheel(window: Hwnd, w_param: Wparam) -> Lresult {
    let delta = ((w_param >> 16) as u16 as i16) as i32;
    let mut area = unsafe { std::mem::zeroed::<Rect>() };
    unsafe { GetClientRect(window, &mut area) };
    let scale = platform::dpi_for_window(window) as f32 / 96.0;
    let graph_left = graph_left(&area, scale);
    APP.with(|cell| {
        let mut app = cell.borrow_mut();
        let panel = terminal_core::panel_metrics(
            area.right as f32,
            area.bottom as f32,
            scale,
            app.terminal_motion,
        );
        let over_panel = app.terminal_open
            && app.pointer_x as f32 >= panel.body_left
            && app.pointer_x as f32 <= panel.body_right
            && app.pointer_y as f32 >= panel.body_top;
        if over_panel && !app.settings_open && !app.extensions_open {
            let total = app
                .terminals
                .get(app.active_terminal)
                .and_then(|tab| {
                    tab.session
                        .view
                        .lock()
                        .ok()
                        .map(|view| view.lines.len() + usize::from(!view.current.is_empty()))
                })
                .unwrap_or(0);
            let viewport = terminal_core::TerminalViewport {
                total_lines: total,
                visible_rows: panel.visible_rows,
                scroll: app.terminal_scroll,
            };
            app.terminal_scroll = if delta > 0 {
                viewport.scroll_by(WHEEL_SCROLL_LINES)
            } else if delta < 0 {
                viewport.scroll_by(-WHEEL_SCROLL_LINES)
            } else {
                viewport.scroll
            };
        } else if app.extensions_open && app.extension_category == 0 {
            let count = app.filtered_language_indices().len();
            if delta > 0 {
                app.extension_scroll = app.extension_scroll.saturating_sub(1);
            } else if delta < 0 {
                app.extension_scroll = (app.extension_scroll + 1).min(count.saturating_sub(8));
            }
        } else if app.pointer_x as f32 >= graph_left {
            let rows =
                graph_core::visible_rows(48.0 * scale, area.bottom as f32 - panel.height, scale);
            if delta > 0 {
                app.graph_scroll = app.graph_scroll.saturating_sub(1);
            } else if delta < 0 {
                let symbols = editor_core::symbol_occurrences(&app.lines, &app.file_name);
                let graph = build_graph(&app, &symbols);
                app.graph_scroll = (app.graph_scroll + 1).min(graph.max_scroll(rows));
            }
        } else if app.pointer_x as f32 > (area.right as f32 * 0.19).max(220.0 * scale)
            && (app.pointer_x as f32) < graph_left
        {
            if platform::shift_pressed() {
                let columns = if delta > 0 { -4 } else { 4 };
                app.scroll_editor_columns(columns);
            } else {
                let lines = if delta > 0 { -3 } else { 3 };
                app.scroll_editor(lines);
            }
        } else if delta > 0 {
            app.project_scroll = app.project_scroll.saturating_sub(3);
        } else if delta < 0 {
            let query = app.workspace_search.to_ascii_lowercase();
            let count = app
                .project_entries
                .iter()
                .filter(|entry| query.is_empty() || entry.to_ascii_lowercase().contains(&query))
                .count();
            app.project_scroll = (app.project_scroll + 3).min(count.saturating_sub(1));
        }
    });
    invalidate(window);
    0
}

/// Right press: the workspace context menu.
pub(super) fn handle_right_button_down(window: Hwnd, l_param: Lparam) -> Lresult {
    let x = (l_param as u16 as i16) as i32;
    let y = ((l_param >> 16) as u16 as i16) as i32;
    let mut area = unsafe { std::mem::zeroed::<Rect>() };
    unsafe { GetClientRect(window, &mut area) };
    let scale = platform::dpi_for_window(window) as f32 / 96.0;
    if x as f32 <= (area.right as f32 * 0.19).max(220.0 * scale) {
        if let Some(command) = platform::workspace_context_menu(window, x, y) {
            perform_command(window, command);
        }
    }
    0
}
