//! Debugging: DAP session, breakpoints and stepping.

use super::*;

impl WindowsAppState {
    /// Breakpoint lines of the active file.
    pub(super) fn active_breakpoints(&self) -> Vec<usize> {
        let Some(path) = self.file_path.clone() else {
            return Vec::new();
        };
        self.breakpoints
            .iter()
            .find(|(known, _)| *known == path)
            .map(|(_, lines)| lines.clone())
            .unwrap_or_default()
    }

    /// Add or remove a breakpoint on the caret line.
    pub(super) fn toggle_breakpoint(&mut self) {
        let Some(path) = self.file_path.clone() else {
            self.status = "Open a file before setting a breakpoint".into();
            return;
        };
        let line = self.caret_line;
        let entry = self
            .breakpoints
            .iter_mut()
            .find(|(known, _)| *known == path);
        match entry {
            Some((_, lines)) => {
                if let Some(position) = lines.iter().position(|existing| *existing == line) {
                    lines.remove(position);
                    self.status = format!("Breakpoint removed from line {}", line + 1);
                } else {
                    lines.push(line);
                    lines.sort_unstable();
                    self.status = format!("Breakpoint set on line {}", line + 1);
                }
                if lines.is_empty() {
                    self.breakpoints.retain(|(known, _)| *known != path);
                }
            }
            None => {
                self.breakpoints.push((path, vec![line]));
                self.status = format!("Breakpoint set on line {}", line + 1);
            }
        }
        if self.debug_session.is_some() {
            let path_text = self
                .file_path
                .as_ref()
                .map(|path| path.to_string_lossy().into_owned())
                .unwrap_or_default();
            let lines = self.active_breakpoints();
            let arguments = dap_core::set_breakpoints_arguments(&path_text, &lines);
            if let Some(session) = self.debug_session.as_mut() {
                let _ = session.request(dap_core::PendingRequest::SetBreakpoints, &arguments);
            }
        }
    }

    /// Start debugging the active file, or continue when already paused.
    pub(super) fn start_or_continue_debugging(&mut self) {
        if let Some(session) = self.debug_session.as_mut() {
            if self.debug_state.stopped.is_some() {
                let _ = session.control(dap_core::PendingRequest::Continue);
                self.status = "Continuing".into();
                return;
            }
            let _ = session.control(dap_core::PendingRequest::Pause);
            return;
        }
        let language = editor_core::language_for_file(&self.file_name);
        let Some(spec) = dap_core::adapter_for(language) else {
            self.status = format!("No debug adapter is configured for {language}");
            return;
        };
        if !lsp_core::executable_available(&spec.program) {
            self.status = format!("{} is not installed", spec.program);
            return;
        }
        let program = self
            .file_path
            .as_ref()
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_default();
        if program.is_empty() {
            self.status = "Save the file before debugging".into();
            return;
        }
        let cwd = self.project_root.to_string_lossy().into_owned();
        let breakpoints = self
            .breakpoints
            .iter()
            .map(|(path, lines)| (path.to_string_lossy().into_owned(), lines.clone()))
            .collect::<Vec<_>>();
        match dap_core::DebugSession::start(&spec, &self.project_root) {
            Ok(mut session) => {
                session
                    .initialize_and_launch(&program, &cwd, &breakpoints)
                    .unwrap_or_else(|error| self.status = error);
                self.debug_session = Some(session);
                self.debug_state = dap_core::DebugState::default();
                self.debug_stop_seen.clear();
                self.status = format!("Debugging {} with {}", self.file_name, spec.program);
            }
            Err(error) => self.status = error,
        }
    }

    /// Step the paused program.
    pub(super) fn step_debugging(&mut self, request: dap_core::PendingRequest) {
        let Some(session) = self.debug_session.as_mut() else {
            self.status = "Nothing is being debugged".into();
            return;
        };
        if self.debug_state.stopped.is_none() {
            self.status = "The program is not paused".into();
            return;
        }
        let _ = session.control(request);
    }

    /// Stop debugging.
    pub(super) fn stop_debugging(&mut self) {
        self.debug_session = None;
        self.debug_state = dap_core::DebugState::default();
        self.debug_stop_seen.clear();
        self.status = "Debugging stopped".into();
    }

    /// Collect debug adapter messages and walk the state forward.
    pub(super) fn poll_debug_session(&mut self) {
        let Some(mut session) = self.debug_session.take() else {
            return;
        };
        if !session.is_running() {
            let name = session.spec().program.clone();
            let errors = session.stderr_lines();
            self.debug_state = dap_core::DebugState::default();
            self.status = match errors.first() {
                Some(line) => format!("{name} stopped: {line}"),
                None => format!("{name} stopped"),
            };
            return;
        }
        while session.poll().is_some() {}
        self.debug_state = session.state();

        // Walk the state forward: stack, then scopes, then variables.
        let mut problem = None;
        if let Some(stopped) = self.debug_state.stopped.clone() {
            if self.debug_state.frames.is_empty()
                && !session.has_pending(dap_core::PendingRequest::StackTrace)
            {
                if let Err(error) = session.refresh_state() {
                    problem = Some(error);
                }
            } else if !self.debug_state.frames.is_empty()
                && self.debug_state.scopes.is_empty()
                && !session.has_pending(dap_core::PendingRequest::Scopes)
            {
                if let Err(error) = session.request_scope_variables() {
                    problem = Some(error);
                }
            } else if !self.debug_state.scopes.is_empty()
                && self.debug_state.variables.is_empty()
                && !session.has_pending(dap_core::PendingRequest::Variables)
            {
                if let Err(error) = session.request_variables_of_first_scope() {
                    problem = Some(error);
                }
            }
            let key = format!(
                "{}:{}:{:?}",
                stopped.reason,
                stopped.line.unwrap_or(0),
                stopped.file
            );
            if key != self.debug_stop_seen {
                self.debug_stop_seen = key;
                if let Some(frame) = self.debug_state.frames.first().cloned() {
                    if let Some(file) = frame.file.clone() {
                        let path = PathBuf::from(file);
                        if path.is_file() && self.file_path.as_deref() != Some(path.as_path()) {
                            self.open_file(path);
                        }
                    }
                    self.caret_line = frame.line.min(self.lines.len().saturating_sub(1));
                    self.caret_column = frame.column;
                    self.clamp_caret();
                    self.reveal_caret();
                }
                let variables = self
                    .debug_state
                    .variables
                    .iter()
                    .take(4)
                    .map(|variable| format!("{} = {}", variable.name, variable.value))
                    .collect::<Vec<_>>()
                    .join(", ");
                self.status = match (self.debug_state.stopped.as_ref(), variables.is_empty()) {
                    (Some(stopped), false) => format!("{}  ·  {variables}", stopped.describe()),
                    (Some(stopped), true) => stopped.describe(),
                    (None, _) => "Debugging".into(),
                };
            }
        }
        if let Some(error) = problem {
            self.status = error;
        }
        self.debug_session = Some(session);
    }

    /// Start the debug adapter that fits the active file, when one is known.
    pub(super) fn start_debug_adapter(&mut self) {
        let language = editor_core::language_for_file(&self.file_name);
        let name = self
            .file_path
            .as_ref()
            .and_then(|path| path.file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.file_name.clone());
        let Some((tool, command)) = workspace_core::debug_adapter_for(language, &name) else {
            self.status = format!("No debug adapter is configured for {language}");
            return;
        };
        self.terminal_open = true;
        self.ensure_terminal();
        if let Some(tab) = self.terminals.last() {
            let _ = tab.session.write(&format!("{command}\r"));
            self.active_terminal = self.terminals.len() - 1;
        }
        self.status = format!("Started {tool} in the terminal; adapter logs appear there");
    }
}
