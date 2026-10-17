//! AI chat: chat session, history and the AI terminal.

use super::*;

impl WindowsAppState {
    /// Add the checked-in `.tcIDE` project map to every request.  This keeps
    /// the AI grounded in the same files, symbols and relations shown by the
    /// graph instead of making it infer the project from a bare prompt.
    fn prompt_with_project_context(&self, prompt: &str) -> String {
        const MAX_CONTEXT: usize = 12_000;
        let document = tcide_project::serialize(&self.tcide_project);
        let context = if document.len() > MAX_CONTEXT {
            let end = document
                .char_indices()
                .take_while(|(index, _)| *index < MAX_CONTEXT)
                .map(|(index, character)| index + character.len_utf8())
                .last()
                .unwrap_or(0)
                .min(document.len());
            format!("{}\n[project context truncated]", &document[..end])
        } else {
            document
        };
        format!(
            "{prompt}\n\n[TC-IDE project context — read-only]\n{context}\n[/TC-IDE project context]"
        )
    }

    /// Send the prompt in the chat panel to the configured endpoint.
    pub(super) fn send_chat_prompt(&mut self) {
        let user_prompt = self.chat_input.trim().to_string();
        if user_prompt.is_empty() {
            return;
        }
        let prompt = self.prompt_with_project_context(&user_prompt);
        if self.ai_key.is_empty() {
            self.chat_error = Some("An API key is required for this session".into());
            return;
        }
        let settings = self.ai_settings();
        match ai_core::chat_url(&settings.endpoint) {
            Ok(_) => {}
            Err(error) => {
                self.chat_error = Some(error.message());
                return;
            }
        }
        let Some(body_path) =
            self.write_ai_body(&ai_core::chat_body(&settings.model, &prompt, true))
        else {
            self.chat_error = Some("Could not write the request body".into());
            return;
        };
        match ai_core::ChatSession::start(
            &settings,
            &self.ai_key,
            &prompt,
            &body_path,
            ai_core::DEFAULT_TIMEOUT_SECONDS,
        ) {
            Ok(session) => {
                self.chat_messages.push_user(&user_prompt);
                self.chat_input.clear();
                self.chat_error = None;
                self.chat_session = Some(session);
                self.status = "AI request running; Esc cancels it".into();
            }
            Err(error) => self.chat_error = Some(error.message()),
        }
    }

    /// Collect streamed chat events.
    pub(super) fn poll_chat_session(&mut self) {
        let Some(mut session) = self.chat_session.take() else {
            return;
        };
        let prompt = session.prompt().to_string();
        let events = session.poll();
        let mut finished = false;
        let mut failure = None;
        for event in events {
            match event {
                ai_core::ChatEvent::Delta(text) => self.chat_messages.append_answer(&text),
                ai_core::ChatEvent::Done(status) => {
                    finished = true;
                    if let Some(status) = status {
                        if status >= 400 {
                            failure = Some(ai_core::describe_status(status, &prompt));
                        }
                    }
                }
                ai_core::ChatEvent::Error(error) => {
                    finished = true;
                    failure = Some(error);
                }
            }
        }
        let running = session.is_running();
        if finished || !running {
            if let Some(error) = failure {
                self.chat_error = Some(error.clone());
                self.status = error;
            } else {
                self.status = "AI answer received".into();
            }
            self.save_chat_history();
        } else {
            self.chat_session = Some(session);
        }
    }

    /// Cancel a running chat request.
    pub(super) fn cancel_chat(&mut self) {
        if let Some(mut session) = self.chat_session.take() {
            session.cancel();
            self.status = "AI request cancelled; the partial answer is kept".into();
        }
    }

    /// Conversation turns for the panel, oldest first.
    pub(super) fn chat_lines(&self) -> Vec<(bool, String)> {
        self.chat_messages.turns().to_vec()
    }

    /// Where the conversation of a project is stored.
    pub(super) fn chat_history_path(&self) -> PathBuf {
        self.project_root.join(".tc-ide-ai").join("history.conf")
    }

    /// Load the saved conversation for the current project.
    pub(super) fn load_chat_history(&mut self) {
        let path = self.chat_history_path();
        self.chat_messages = match fs::read_to_string(&path) {
            Ok(content) => ai_core::Conversation::parse(&content),
            Err(_) => ai_core::Conversation::new(),
        };
        if !self.chat_messages.is_empty() {
            self.status = format!(
                "Restored {} AI turn(s) from {}",
                self.chat_messages.turns().len(),
                path.display()
            );
        }
    }

    /// Save the conversation next to the project.
    pub(super) fn save_chat_history(&self) {
        let path = self.chat_history_path();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::write(path, self.chat_messages.serialize());
    }

    /// Settings the AI page currently shows.
    pub(super) fn ai_settings(&self) -> ai_core::AiSettings {
        ai_core::AiSettings {
            endpoint: self.ai_endpoint.clone(),
            model: self.ai_model.clone(),
        }
    }

    /// Terminal that runs AI requests, spawned with the key in its environment.
    ///
    /// The key never appears in a command line or a log; only the child process
    /// environment carries it, and it stays in memory for this session.
    pub(super) fn ensure_ai_terminal(&mut self) -> Option<usize> {
        if let Some(index) = self.ai_terminal {
            if index < self.terminals.len() {
                return Some(index);
            }
        }
        let grid = self.terminal_grid;
        let mut extra = Vec::new();
        if !self.ai_key.is_empty() {
            extra.push((ai_core::API_KEY_ENV.to_string(), self.ai_key.clone()));
        }
        let cwd = self.project_root.clone();
        match unsafe { ConPtySession::spawn_with_env(&cwd, grid.columns, grid.rows, &extra) } {
            Ok(session) => {
                self.terminals.push(TerminalTab {
                    name: "AI".into(),
                    session,
                    input: String::new(),
                });
                self.ai_terminal = Some(self.terminals.len() - 1);
                self.active_terminal = self.terminals.len() - 1;
                self.terminal_open = true;
                self.ai_terminal
            }
            Err(error) => {
                self.status = format!("AI terminal could not start: {error}");
                None
            }
        }
    }

    /// Write a request body to the project cache and return its path.
    pub(super) fn write_ai_body(&self, body: &str) -> Option<PathBuf> {
        let directory = self.project_root.join(".tc-ide-ai");
        fs::create_dir_all(&directory).ok()?;
        let path = directory.join("request.json");
        fs::write(&path, body).ok()?;
        Some(path)
    }

    /// Run one AI request in the visible terminal.
    pub(super) fn run_ai_request(&mut self, prompt: &str, stream: bool) {
        if self.ai_key.is_empty() {
            self.status = "AI API key is required for this session".into();
            return;
        }
        let settings = self.ai_settings();
        let shell = if cfg!(windows) {
            ai_core::Shell::Windows
        } else {
            ai_core::Shell::Unix
        };
        let body = if prompt.is_empty() {
            ai_core::connection_test_body(&settings.model)
        } else {
            let contextual = self.prompt_with_project_context(prompt);
            ai_core::chat_body(&settings.model, &contextual, stream)
        };
        let Some(path) = self.write_ai_body(&body) else {
            self.status = "Could not write the AI request body".into();
            return;
        };
        let command = match ai_core::chat_command(
            &settings,
            &path.to_string_lossy(),
            stream,
            ai_core::DEFAULT_TIMEOUT_SECONDS,
            shell,
        ) {
            Ok(command) => command,
            Err(error) => {
                self.status = error.message();
                return;
            }
        };
        let Some(index) = self.ensure_ai_terminal() else {
            return;
        };
        if let Some(tab) = self.terminals.get(index) {
            let _ = tab.session.write(&format!(
                "{command}
"
            ));
            self.active_terminal = index;
            self.ai_last_report.clear();
            self.status = if prompt.is_empty() {
                "Testing the AI connection in the AI terminal".into()
            } else {
                "AI request running in the AI terminal (Ctrl+C cancels)".into()
            };
        }
    }

    /// Watch the AI terminal for the status code curl reports, and explain it.
    pub(super) fn poll_ai_terminal(&mut self) {
        let Some(index) = self.ai_terminal else {
            return;
        };
        let Some(tab) = self.terminals.get(index) else {
            return;
        };
        let Ok(view) = tab.session.view.lock() else {
            return;
        };
        let mut report = None;
        for line in view.lines.iter().rev().take(6) {
            let (_, status) = ai_core::split_status(line);
            if let Some(status) = status {
                report = Some((status, line.clone()));
                break;
            }
        }
        drop(view);
        let Some((status, line)) = report else {
            return;
        };
        if self.ai_last_report == line {
            return;
        }
        self.ai_last_report = line;
        if status < 400 {
            self.status = format!("AI request completed (HTTP {status})");
            return;
        }
        let body = self
            .terminals
            .get(index)
            .and_then(|tab| {
                tab.session.view.lock().ok().map(|view| {
                    view.lines.join(
                        "
",
                    )
                })
            })
            .unwrap_or_default();
        self.status = ai_core::describe_status(status, &body);
    }
}
