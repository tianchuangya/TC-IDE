//! Language tooling: LSP session, diagnostics, problems panel and formatting.

use super::*;

impl WindowsAppState {
    /// Document text the language server should index.
    pub(super) fn document_text(&self) -> String {
        self.lines.join("\n")
    }

    /// Real installation state of a language's tooling.
    pub(super) fn language_tooling(&self, language: &str) -> extension_core::LanguageTools {
        extension_core::language_tools(language, |tool| {
            lsp_core::program_runs(tool, &["--version"])
        })
    }

    /// Summary of the tooling for the selected extension language.
    pub(super) fn selected_language_tooling(&self) -> String {
        let language = WindowsAppState::EXTENSION_LANGUAGES[self
            .selected_language
            .min(WindowsAppState::EXTENSION_LANGUAGES.len() - 1)];
        // The registry names differ from the extension list, so map the common
        // ones onto the names the tooling table understands.
        let language = match language {
            "Web" => "TypeScript / JavaScript",
            other => other,
        };
        self.language_tooling(language).summary()
    }

    /// Start the language server for the active file, when it is installed.
    pub(super) fn ensure_language_server(&mut self) {
        let path = self
            .file_path
            .clone()
            .or_else(|| Some(self.project_root.join(&self.file_name)));
        let Some(path) = path else {
            return;
        };
        let name = self.file_name.clone();
        let Some((spec, installed)) = lsp_core::server_status(&name) else {
            self.lsp_diagnostics.clear();
            self.shutdown_language_server();
            return;
        };
        if !installed {
            self.lsp_diagnostics.clear();
            self.shutdown_language_server();
            self.status = format!(
                "{} is not installed; using the built-in symbol index for {}",
                spec.program, name
            );
            return;
        }
        let already_running = self
            .language_server
            .as_mut()
            .map(|server| server.is_running() && server.spec().program == spec.program)
            .unwrap_or(false);
        if already_running {
            let path_text = path.to_string_lossy().into_owned();
            let text = self.document_text();
            if let Some(server) = self.language_server.as_mut() {
                let _ = server.change_document(&path_text, &text);
            }
            self.lsp_last_change = Some(Instant::now());
            return;
        }
        self.shutdown_language_server();
        let root = self.project_root.clone();
        match lsp_core::LanguageServer::start(&spec, &root) {
            Ok(mut server) => {
                if server.initialize().is_err() || server.notify_initialized().is_err() {
                    self.status = format!("{} did not answer initialize", spec.program);
                    return;
                }
                if let Err(error) =
                    server.open_document(&path.to_string_lossy(), &self.document_text())
                {
                    self.status = error;
                    return;
                }
                self.language_server = Some(server);
                self.lsp_pending.clear();
                self.lsp_completions.clear();
                self.lsp_hover = None;
                self.status = format!("{} started for {}", spec.program, name);
            }
            Err(error) => self.status = error,
        }
    }

    pub(super) fn shutdown_language_server(&mut self) {
        if let Some(server) = self.language_server.as_mut() {
            if let Some(path) = self.file_path.clone() {
                let _ = server.close_document(&path.to_string_lossy());
            }
        }
        self.language_server = None;
        self.lsp_pending.clear();
        self.lsp_completions.clear();
    }

    /// Tell the server about an edit, coalescing rapid typing.
    pub(super) fn notify_language_server(&mut self) {
        let Some(path) = self
            .file_path
            .as_ref()
            .map(|path| path.to_string_lossy().into_owned())
        else {
            return;
        };
        let text = self.document_text();
        let running = self
            .language_server
            .as_mut()
            .map(|server| server.is_running())
            .unwrap_or(false);
        if !running {
            return;
        }
        if let Some(server) = self.language_server.as_mut() {
            let _ = server.change_document(&path, &text);
        }
        self.lsp_last_change = Some(Instant::now());
        self.lsp_pending
            .retain(|(_, request)| *request != LspRequest::Hover);
    }

    /// Ask the server for completion, hover or definition at the caret.
    pub(super) fn request_language_feature(&mut self, request: LspRequest) {
        let Some(server) = self.language_server.as_mut() else {
            return;
        };
        let Some(path) = self
            .file_path
            .as_ref()
            .map(|path| path.to_string_lossy().into_owned())
        else {
            return;
        };
        let line = self.caret_line as u32;
        let character = self.caret_column as u32;
        let rename = request == LspRequest::Rename;
        let method = match request {
            LspRequest::Completion => "textDocument/completion",
            LspRequest::Hover => "textDocument/hover",
            LspRequest::Definition => "textDocument/definition",
            LspRequest::References => "textDocument/references",
            LspRequest::Rename => "textDocument/rename",
        };
        let result = if rename {
            let new_name = self.rename_prompt.clone().unwrap_or_default();
            if new_name.is_empty() {
                self.status = "Rename needs a new name".into();
                return;
            }
            server.rename(&path, line, character, &new_name)
        } else {
            server.position_request(method, &path, line, character)
        };
        match result {
            Ok(id) => self.lsp_pending.push((id, request)),
            Err(error) => self.status = error,
        }
    }

    /// Jump to the next reference the server reported.
    pub(super) fn jump_to_next_reference(&mut self) {
        if self.lsp_locations.is_empty() {
            self.status = "No references from the language server yet".into();
            return;
        }
        let index = self.lsp_location_cursor % self.lsp_locations.len();
        let (path, line, column) = self.lsp_locations[index].clone();
        self.lsp_location_cursor = index + 1;
        if path.is_file() {
            let name = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            if self.file_path.as_deref() != Some(path.as_path()) {
                self.open_file(path);
            } else {
                self.status = format!("Reference in {name}");
            }
            self.caret_line = line.min(self.lines.len().saturating_sub(1));
            self.caret_column = column;
            self.clamp_caret();
            self.reveal_caret();
            self.status = format!(
                "Reference {} of {} at {}:{}",
                index + 1,
                self.lsp_locations.len(),
                self.file_name,
                self.caret_line + 1
            );
        } else {
            self.status = format!("Reference file is not available: {}", path.display());
        }
    }

    /// Apply the edits a rename response asked for.
    pub(super) fn apply_rename_edits(&mut self, edits: &[lsp_core::RenameEdit]) {
        if edits.is_empty() {
            self.status = "The language server returned no rename edits".into();
            return;
        }
        let mut touched = 0usize;
        let mut grouped: Vec<(PathBuf, Vec<lsp_core::RenameEdit>)> = Vec::new();
        for edit in edits {
            let path = PathBuf::from(&edit.path);
            match grouped.iter_mut().find(|(known, _)| *known == path) {
                Some((_, list)) => list.push(edit.clone()),
                None => grouped.push((path, vec![edit.clone()])),
            }
        }
        for (path, list) in grouped {
            let is_active = self.file_path.as_deref() == Some(path.as_path());
            let text = if is_active {
                self.buffer.to_text()
            } else {
                match fs::read_to_string(&path) {
                    Ok(content) => content.replace("\r\n", "\n"),
                    Err(error) => {
                        self.status = format!("Rename could not read {}: {error}", path.display());
                        return;
                    }
                }
            };
            let updated = lsp_core::apply_edits(&text, &list);
            if is_active {
                self.buffer = text_buffer::TextBuffer::from_text(&updated);
                self.sync_lines();
                self.reveal_caret();
                touched += 1;
                continue;
            }
            match fs::write(&path, updated.replace('\n', "\r\n")) {
                Ok(()) => touched += 1,
                Err(error) => {
                    self.status = format!("Rename could not write {}: {error}", path.display());
                    return;
                }
            }
        }
        self.request_project_index();
        self.status = format!("Rename applied to {touched} file(s)");
    }

    /// Collect whatever the language server has sent since the last tick.
    ///
    /// The client is taken out of the state for the duration so the collected
    /// diagnostics and pending requests can be updated without fighting the
    /// borrow checker.
    pub(super) fn poll_language_server(&mut self) {
        let Some(mut server) = self.language_server.take() else {
            return;
        };
        if !server.is_running() {
            let name = server.spec().program.clone();
            let errors = server.stderr_lines();
            self.lsp_pending.clear();
            self.status = match errors.first() {
                Some(line) => format!("{name} stopped: {line}"),
                None => format!("{name} stopped"),
            };
            return;
        }
        let file_name = self.file_name.clone();
        let mut diagnostics_changed = false;
        while let Some(message) = server.try_message() {
            if server.answer_automatically(&message) {
                continue;
            }
            if let Some(method) = lsp_core::message_method(&message) {
                if method == "textDocument/publishDiagnostics" {
                    self.lsp_diagnostics = lsp_core::parse_diagnostics(&message)
                        .into_iter()
                        .filter(|diagnostic| diagnostic.path.ends_with(&file_name))
                        .collect();
                    diagnostics_changed = true;
                }
                continue;
            }
            let Some(id) = lsp_core::message_id(&message) else {
                continue;
            };
            let Some(position) = self
                .lsp_pending
                .iter()
                .position(|(candidate, _)| *candidate == id)
            else {
                continue;
            };
            let (_, request) = self.lsp_pending.remove(position);
            match request {
                LspRequest::Completion => {
                    self.lsp_completions = lsp_core::parse_completions(&message)
                        .into_iter()
                        .map(|(label, detail)| {
                            if detail.is_empty() {
                                label
                            } else {
                                format!("{label}  {detail}")
                            }
                        })
                        .collect();
                }
                LspRequest::Hover => {
                    if let Some(text) = lsp_core::parse_hover(&message) {
                        self.lsp_hover = Some((self.caret_line, self.caret_column, text));
                    }
                }
                LspRequest::Definition => {
                    if let Some((path, line, column)) =
                        lsp_core::parse_locations(&message).into_iter().next()
                    {
                        let target = PathBuf::from(path);
                        if target.is_file() {
                            self.open_file(target);
                            self.caret_line = line as usize;
                            self.caret_column = column as usize;
                            self.clamp_caret();
                            self.reveal_caret();
                            self.status = "Jumped to the definition reported by the server".into();
                        }
                    }
                }
                LspRequest::References => {
                    self.lsp_locations = lsp_core::parse_locations(&message)
                        .into_iter()
                        .map(|(path, line, column)| {
                            (PathBuf::from(path), line as usize, column as usize)
                        })
                        .collect();
                    self.lsp_location_cursor = 0;
                    let count = self.lsp_locations.len();
                    self.status = if count == 0 {
                        "The language server reported no references".into()
                    } else {
                        format!("{count} reference(s); Ctrl+R again jumps to the next one")
                    };
                }
                LspRequest::Rename => {
                    let edits = lsp_core::parse_rename_edits(&message);
                    self.apply_rename_edits(&edits);
                }
            }
        }
        self.language_server = Some(server);
        if diagnostics_changed {
            let errors = self
                .lsp_diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.severity == lsp_core::DiagnosticSeverity::Error)
                .count();
            let warnings = self
                .lsp_diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.severity == lsp_core::DiagnosticSeverity::Warning)
                .count();
            self.status = match (errors, warnings) {
                (0, 0) => "Language server reports no problems".into(),
                (0, warnings) => format!("{warnings} warning(s) from the language server"),
                (errors, 0) => format!("{errors} error(s) from the language server"),
                (errors, warnings) => {
                    format!("{errors} error(s) and {warnings} warning(s) from the language server")
                }
            };
        }
    }

    /// Diagnostics grouped by line, for the gutter markers.
    pub(super) fn diagnostic_lines(&self) -> Vec<(usize, bool)> {
        let mut lines: Vec<(usize, bool)> = Vec::new();
        for diagnostic in lsp_core::filter_diagnostics(&self.lsp_diagnostics, self.problem_filter) {
            let is_error = diagnostic.severity == lsp_core::DiagnosticSeverity::Error;
            let line = diagnostic.line as usize;
            match lines.iter_mut().find(|entry| entry.0 == line) {
                Some(entry) => entry.1 = entry.1 || is_error,
                None => lines.push((line, is_error)),
            }
        }
        lines
    }

    /// Rows of the problems panel: diagnostics, then debug state.
    ///
    /// Each row carries where it should take the caret when it is opened.
    pub(super) fn problem_rows(&self) -> Vec<ProblemRow> {
        let mut rows = Vec::new();
        for diagnostic in lsp_core::filter_diagnostics(&self.lsp_diagnostics, self.problem_filter) {
            let target = self
                .file_path
                .clone()
                .map(|path| (path, diagnostic.line as usize, diagnostic.column as usize));
            rows.push((diagnostic.line_text(), target));
        }
        if let Some(stopped) = self.debug_state.stopped.as_ref() {
            rows.push((stopped.describe(), None));
        }
        for (index, frame) in self.debug_state.frames.iter().enumerate() {
            let target = frame
                .file
                .as_ref()
                .map(|file| (PathBuf::from(file), frame.line, frame.column));
            rows.push((
                format!("#{index}  {}  :{}", frame.name, frame.line + 1),
                target,
            ));
        }
        for variable in self.debug_state.variables.iter().take(20) {
            rows.push((
                format!(
                    "{} = {}{}",
                    variable.name,
                    variable.value,
                    if variable.kind.is_empty() {
                        String::new()
                    } else {
                        format!("  ({})", variable.kind)
                    }
                ),
                None,
            ));
        }
        for (path, lines) in &self.breakpoints {
            let name = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            for line in lines {
                rows.push((
                    format!("breakpoint  {name}:{}", line + 1),
                    Some((path.clone(), *line, 0)),
                ));
            }
        }
        rows
    }

    /// Open or close the problems panel.
    pub(super) fn toggle_problems(&mut self) {
        self.problems_open = !self.problems_open;
        self.problems_selection = 0;
        if self.problems_open {
            let count = self.problem_rows().len();
            self.status = if count == 0 {
                "Problems: nothing reported by the language server or the debugger".into()
            } else {
                format!("Problems: {count} row(s); Enter jumps, Esc closes")
            };
        }
    }

    /// Move the panel selection.
    pub(super) fn move_problem_selection(&mut self, delta: i32) {
        let count = self.problem_rows().len();
        if count == 0 {
            self.problems_selection = 0;
            return;
        }
        let current = self.problems_selection.min(count - 1) as i32;
        let next = (current + delta).clamp(0, count as i32 - 1) as usize;
        self.problems_selection = next;
        if let Some((text, _)) = self.problem_rows().get(next) {
            self.status = text.clone();
        }
    }

    /// Jump to the selected problems panel row.
    pub(super) fn activate_problem_row(&mut self) {
        let rows = self.problem_rows();
        let Some((text, target)) =
            rows.get(self.problems_selection.min(rows.len().saturating_sub(1)))
        else {
            return;
        };
        let text = text.clone();
        match target.clone() {
            Some((path, line, column)) => {
                if path.is_file() && self.file_path.as_deref() != Some(path.as_path()) {
                    self.open_file(path);
                }
                self.caret_line = line.min(self.lines.len().saturating_sub(1));
                self.caret_column = column;
                self.clamp_caret();
                self.reveal_caret();
                self.status = format!("Jumped to {text}");
            }
            None => self.status = text,
        }
    }

    /// Format the active file with the formatter for its language.
    pub(super) fn format_current_file(&mut self) {
        let language = editor_core::language_for_file(&self.file_name);
        let name = self
            .file_path
            .as_ref()
            .and_then(|path| path.file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.file_name.clone());
        let Some(command) = workspace_core::format_command(language, &name) else {
            self.status = format!("No formatter is configured for {language}");
            return;
        };
        self.terminal_open = true;
        self.ensure_terminal();
        if let Some(tab) = self.terminals.last() {
            let _ = tab.session.write(&format!("{command}\r"));
            self.active_terminal = self.terminals.len() - 1;
        }
        if let Some(path) = self.file_path.clone() {
            if let Ok(content) = fs::read_to_string(&path) {
                let mut lines = content.lines().map(str::to_string).collect::<Vec<_>>();
                if lines.is_empty() {
                    lines.push(String::new());
                }
                self.buffer = text_buffer::TextBuffer::from_lines(&lines);
                self.lines = lines;
                self.clamp_caret();
                self.status = format!("Formatted {} with {command}", path.display());
                return;
            }
        }
        self.status = format!("Formatting with {command}");
    }
}
