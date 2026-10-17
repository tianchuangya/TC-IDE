//! Scene layout: window scene assembly and graph layout helpers.

use super::panel::terminal_buffer;
use super::*;

pub(super) fn render_window(window: Hwnd) {
    let mut area = Rect {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    unsafe { GetClientRect(window, &mut area) };
    let width = (area.right - area.left).max(1) as u32;
    let height = (area.bottom - area.top).max(1) as u32;
    let dpi = platform::dpi_for_window(window);
    APP.with(|app_cell| {
        RENDERER.with(|renderer_cell| {
            let app = app_cell.borrow();
            let active_terminal = app.terminals.get(app.active_terminal);
            let terminal_buffer = terminal_buffer(&app);
            let panel = terminal_core::panel_metrics(
                width as f32,
                height as f32,
                dpi as f32 / 96.0,
                app.terminal_motion,
            );
            let viewport = terminal_core::TerminalViewport {
                total_lines: terminal_buffer.len(),
                visible_rows: panel.visible_rows,
                scroll: app.terminal_scroll,
            };
            let terminal_lines = terminal_core::visible_lines(&terminal_buffer, &viewport).to_vec();
            let terminal_selection = app.terminal_selection.map(|selection| {
                let (start, end) = selection.ordered();
                ((start.line, start.column), (end.line, end.column))
            });
            let terminal_names = app
                .terminals
                .iter()
                .map(|tab| tab.name.clone())
                .collect::<Vec<_>>();
            let completions = app.completions();
            let first_line = app.viewport.clamped(app.lines.len()).first_line;
            let visible_lines = app
                .buffer
                .line_window(first_line, app.viewport.height.max(1));
            let visible_lines = if visible_lines.is_empty() {
                vec![String::new()]
            } else {
                visible_lines
            };
            let all_symbols = editor_core::symbol_occurrences(&app.lines, &app.file_name);
            // Symbol rails only make sense for lines that are on screen.
            let symbols = all_symbols
                .iter()
                .filter(|symbol| symbol.line >= first_line)
                .map(|symbol| {
                    let mut symbol = symbol.clone();
                    symbol.line -= first_line;
                    symbol
                })
                .collect::<Vec<_>>();
            let relation_symbol = app.relation_symbol.as_ref().and_then(|symbol| {
                if symbol.line < first_line || symbol.line >= first_line + visible_lines.len() {
                    return None;
                }
                let mut symbol = symbol.clone();
                symbol.line -= first_line;
                Some(symbol)
            });
            let graph_rows = graph_visible_rows(&area, dpi as f32 / 96.0, app.terminal_motion);
            let graph = build_graph(&app, &symbols);
            let tabs = app
                .session
                .documents()
                .iter()
                .map(|document| document.title())
                .collect::<Vec<_>>();
            let active_tab = app.session.active();
            let signing_certificate = app
                .signing_certificate
                .as_ref()
                .and_then(|path| path.file_name())
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            let language_tooling = app.selected_language_tooling();
            let chat_messages = app.chat_lines();
            let problem_rows = app
                .problem_rows()
                .into_iter()
                .map(|(text, _)| text)
                .collect::<Vec<_>>();
            let ai_cache_summary = format!(
                "runtime cache: {} entries · {}",
                app.runtime_cache.entries().len(),
                extension_core::format_bytes(app.runtime_cache.total_bytes())
            );
            let breakpoints = app.active_breakpoints();
            let diagnostic_lines = app
                .diagnostic_lines()
                .into_iter()
                .filter(|(line, _)| *line >= first_line)
                .map(|(line, is_error)| (line - first_line, is_error))
                .collect::<Vec<_>>();
            let status_line = if let Some(prompt) = app.breakpoint_prompt.as_ref() {
                format!("Breakpoint condition: {prompt}")
            } else if let Some(prompt) = app.rename_prompt.as_ref() {
                format!("Rename to: {prompt}")
            } else if let Some(diagnostic) = app
                .lsp_diagnostics
                .iter()
                .find(|diagnostic| diagnostic.line as usize == app.caret_line)
            {
                diagnostic.line_text()
            } else {
                match &app.lsp_hover {
                    Some((_, _, text)) => text.lines().next().unwrap_or("").to_string(),
                    None => app.status.clone(),
                }
            };
            let graph_first_index = graph.clamped_scroll(app.graph_scroll, graph_rows);
            let graph_nodes = graph.visible(app.graph_scroll, graph_rows).to_vec();
            let graph_summary = graph.footer(app.graph_scroll, graph_rows);
            let graph_scope_label = if app.graph_scope.is_none() {
                format!("auto · {}", graph.scope_label())
            } else {
                graph.scope_label().to_string()
            };
            let expanded_comments = app
                .expanded_comments
                .iter()
                .filter(|line| **line >= first_line)
                .map(|line| line - first_line)
                .collect::<Vec<_>>();
            let comment_expansion = app
                .comment_expansion
                .iter()
                .filter(|(line, _)| **line >= first_line)
                .map(|(line, motion)| (line - first_line, motion.0))
                .collect::<Vec<_>>();
            let create_location = app.create_location.to_string_lossy().into_owned();
            let background_path = app
                .background_path
                .as_ref()
                .map(|path| path.to_string_lossy().into_owned());
            let query = app.workspace_search.to_ascii_lowercase();
            let visible_row_count =
                ((height as f32 / (dpi as f32 / 96.0) - 132.0) / 35.0).max(1.0) as usize;
            let listed_rows = app
                .project_entries
                .iter()
                .enumerate()
                .filter(|(_, entry)| {
                    query.is_empty() || entry.to_ascii_lowercase().contains(&query)
                })
                .map(|(index, _)| index)
                .collect::<Vec<_>>();
            let visible_entries = listed_rows
                .iter()
                .skip(app.project_scroll)
                .take(visible_row_count)
                .map(|index| app.project_entries[*index].clone())
                .collect::<Vec<_>>();
            // The rows slide while a folder folds, so the row the tree draws is
            // the position the app's row shift reports, not the list index.
            let tree_row_positions = listed_rows
                .iter()
                .skip(app.project_scroll)
                .take(visible_row_count)
                .enumerate()
                .map(|(row, index)| app.tree_row_position(*index) - row as f32)
                .collect::<Vec<_>>();
            let scene = WindowsScene {
                file_name: &app.file_name,
                project_name: app
                    .project_root
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("PROJECT"),
                project_entries: &visible_entries,
                tree_row_positions: &tree_row_positions,
                workspace_search: &app.workspace_search,
                workspace_search_focused: app.workspace_search_focused,
                create_kind: app.create_kind.map(|kind| match kind {
                    CreateKind::Project => "New project",
                    CreateKind::File => "New file",
                    CreateKind::Folder => "New folder",
                }),
                create_name: &app.create_name,
                create_error: &app.create_error,
                create_location: &create_location,
                create_language: app.project_language_name(),
                create_version: app.project_version_name(),
                create_environment: app.project_environments()[app
                    .project_environment
                    .min(app.project_environments().len() - 1)],
                lines: &visible_lines,
                caret_line: app.caret_line.saturating_sub(first_line),
                caret_column: app.caret_column,
                scroll_columns: app.viewport.first_column,
                first_line,
                total_lines: app.lines.len(),
                selection: app.selection_range().and_then(|(start, end)| {
                    if end.line < first_line {
                        return None;
                    }
                    let start_line = start.line.max(first_line) - first_line;
                    let end_line = end.line - first_line;
                    Some(((start_line, start.column), (end_line, end.column)))
                }),
                ime_composition: &app.ime_composition,
                completions: &completions,
                light_theme: app.light_theme,
                ui_chinese: app.ui_chinese,
                code_zoom: app.code_zoom,
                code_font_family: app.code_font_family,
                font_search: &app.font_search,
                font_search_focused: app.font_search_focused,
                settings_focus: app.settings_focus,
                font_options: &WindowsAppState::CODE_FONTS,
                settings_motion: app.settings_motion,
                extensions_motion: app.extensions_motion,
                extension_category: app.extension_category,
                selected_language: app.selected_language,
                extension_search: &app.extension_search,
                extension_search_focused: app.extension_search_focused,
                extension_scroll: app.extension_scroll,
                language_options: &WindowsAppState::EXTENSION_LANGUAGES,
                language_installed: &app.language_installed,
                language_versions: &app.language_versions,
                ai_endpoint: &app.ai_endpoint,
                ai_model: &app.ai_model,
                ai_key_present: !app.ai_key.is_empty(),
                ai_prompt: &app.ai_prompt,
                ai_prompt_focused: app.ai_prompt_focused,
                ai_cache_summary: &ai_cache_summary,
                problems_open: app.problems_open,
                problems: &problem_rows,
                problems_selected: app.problems_selection,
                problems_hint: &format!(
                    "filter {} · Tab cycles · Enter jumps · Esc closes{}",
                    app.problem_filter.label(),
                    if app.debug_session.is_some() {
                        " · debugging"
                    } else {
                        ""
                    }
                ),
                language_tooling: &language_tooling,
                chat_open: app.chat_open,
                chat_messages: &chat_messages,
                chat_input: &app.chat_input,
                chat_running: app.chat_session.is_some(),
                chat_error: app.chat_error.as_deref().unwrap_or_default(),
                ai_field_focused: app.ai_field_focused,
                symbols: &symbols,
                relation_symbol: relation_symbol.as_ref(),
                relation_focus_motion: app.relation_focus_motion,
                tabs: &tabs,
                active_tab,
                tab_dirty: app
                    .session
                    .documents()
                    .iter()
                    .map(|document| document.dirty)
                    .collect::<Vec<_>>(),
                graph_nodes: &graph_nodes,
                graph_first_index,
                graph_selected: app.graph_selected,
                graph_search: &app.graph_search,
                graph_search_focused: app.graph_search_focused,
                graph_scope_label: &graph_scope_label,
                graph_summary: &graph_summary,
                expanded_comments: &expanded_comments,
                comment_expansion: &comment_expansion,
                comment_bubbles_motion: app.comment_bubbles_motion,
                editing_comment: app
                    .editing_comment
                    .filter(|line| *line >= first_line)
                    .map(|line| line - first_line),
                pointer_x: app.pointer_x,
                pointer_pressed: app.pointer_pressed,
                pointer_y: app.pointer_y,
                project_total: app
                    .project_entries
                    .iter()
                    .filter(|entry| {
                        let query = app.workspace_search.to_ascii_lowercase();
                        query.is_empty() || entry.to_ascii_lowercase().contains(&query)
                    })
                    .count(),
                project_scroll: app.project_scroll,
                package_motion: app.package_motion,
                package_targets: app.package_targets,
                msix_available: app.msix_supported(),
                signing_certificate: &signing_certificate,
                package_running: app.package_running,
                package_progress: app.package_progress,
                background_path: background_path.as_deref(),
                background_opacity: app.background_opacity,
                background_offset_x: app.background_offset_x,
                background_offset_y: app.background_offset_y,
                background_zoom: app.background_zoom,
                status: &status_line,
                diagnostic_lines: &diagnostic_lines,
                breakpoint_lines: &breakpoints
                    .iter()
                    .filter(|line| **line >= first_line)
                    .map(|line| line - first_line)
                    .collect::<Vec<_>>(),
                terminal_lines: &terminal_lines,
                terminal_first_line: viewport.first_line(),
                terminal_selection,
                terminal_names: &terminal_names,
                active_terminal: app.active_terminal,
                terminal_input: active_terminal.map(|tab| tab.input.as_str()).unwrap_or(""),
                terminal_open: app.terminal_open,
                terminal_progress: app.terminal_motion,
                terminal_scrolled: viewport.clamped_scroll(),
                animation_seconds: app.started.elapsed().as_secs_f32(),
                edit_pulse: app
                    .edit_pulses
                    .on_line(app.caret_line)
                    .map(|pulse| pulse.eased()),
                create_motion: app.create_motion,
            };
            let mut renderer = renderer_cell.borrow_mut();
            if renderer.is_none() {
                *renderer = unsafe {
                    DirectEditorRenderer::new(
                        dpi,
                        app.code_zoom,
                        WindowsAppState::CODE_FONTS[app.code_font_family],
                    )
                }
                .ok();
            }
            if let Some(renderer) = renderer.as_mut() {
                let _ = unsafe { renderer.render(window, width, height, &scene) };
            }
        });
    });
}

/// Relation graph for the current editor, hover and search state.
pub(super) fn build_graph(
    app: &WindowsAppState,
    symbols: &[editor_core::SymbolOccurrence],
) -> graph_core::Graph {
    let project_name = app
        .project_root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("PROJECT");
    let inputs = graph_core::GraphInputs {
        project_name,
        current_file: &app.file_name,
        current_folder: graph_core::folder_of(&app.file_name),
        hovered: app.hovered_symbol.as_ref(),
        file_lines: &app.lines,
        file_symbols: symbols,
        project_symbols: &app.project_symbols,
        project_declarations: &app.project_declarations,
        folded: &app.graph_folded,
        relation: app.graph_relation,
    };
    let scope = graph_core::auto_scope(
        app.hovered_symbol.as_ref(),
        &app.graph_search,
        app.graph_scope,
    );
    graph_core::build_graph(&inputs, scope, &app.graph_search)
}

/// Left edge of the relation graph panel.
pub(super) fn graph_left(area: &Rect, scale: f32) -> f32 {
    area.right as f32 - (area.right as f32 * 0.27).max(300.0 * scale)
}

/// Cards the graph panel can show at the current window size.
pub(super) fn graph_visible_rows(area: &Rect, scale: f32, terminal_motion: f32) -> usize {
    let panel = terminal_core::panel_metrics(
        area.right as f32,
        area.bottom as f32,
        scale,
        terminal_motion,
    );
    graph_core::visible_rows(48.0 * scale, area.bottom as f32 - panel.height, scale)
}

/// Screen rectangle of the graph range chip.
pub(super) fn graph_scope_rect(area: &Rect, scale: f32) -> Rect {
    Rect {
        left: (area.right as f32 - 150.0 * scale) as i32,
        top: (60.0 * scale) as i32,
        right: (area.right as f32 - 18.0 * scale) as i32,
        bottom: (90.0 * scale) as i32,
    }
}

/// Screen rectangle of the graph search field.
pub(super) fn graph_search_rect(area: &Rect, scale: f32) -> Rect {
    Rect {
        left: (graph_left(area, scale) + 18.0 * scale) as i32,
        top: (94.0 * scale) as i32,
        right: (area.right as f32 - 18.0 * scale) as i32,
        bottom: (126.0 * scale) as i32,
    }
}
