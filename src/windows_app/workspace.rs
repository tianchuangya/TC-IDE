//! Workspace: project scanning, templates, sessions and preferences.

use super::*;

impl WindowsAppState {
    /// Re-parse only the file being edited, so the relation graph stays fresh.
    pub(super) fn update_active_index_entry(&mut self) {
        let Some(path) = self.file_path.clone() else {
            return;
        };
        let Ok(relative) = path.strip_prefix(&self.project_root) else {
            return;
        };
        let relative = relative.to_string_lossy().replace('\\', "/");
        let bytes = self.buffer.len() as u64;
        // The modified time is irrelevant here: the editor is the source of truth.
        let fingerprint = index_core::FileFingerprint::new(
            relative,
            bytes,
            self.started.elapsed().as_millis() as u64,
        );
        if self.project_index.update_file(fingerprint, &self.lines) {
            self.refresh_index_tables();
        }
    }

    /// Re-read both index tables after a file changed.
    pub(super) fn refresh_index_tables(&mut self) {
        self.project_symbols = self.project_index.symbols();
        self.project_declarations = self.project_index.declarations();
        self.sync_tcide_project_document();
    }

    /// Persist the editor's current index into the human-readable project file.
    /// The graph is derived from this same data, so an AI tool opening the
    /// `.tcIDE` document sees the same files, declarations and references.
    pub(super) fn sync_tcide_project_document(&mut self) {
        let mut document = self.tcide_project.clone();
        document.name = self
            .project_root
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("TC-IDE 项目")
            .to_string();
        document.root = ".".into();
        document.files = self
            .project_entry_paths
            .iter()
            .zip(self.project_entry_is_dir.iter())
            .filter(|(_, is_dir)| !**is_dir)
            .filter_map(|(path, _)| {
                let relative = path
                    .strip_prefix(&self.project_root)
                    .ok()?
                    .to_string_lossy()
                    .replace('\\', "/");
                let language = editor_core::language_for_file(&relative);
                (language != "Plain Text").then(|| tcide_project::ProjectFile {
                    path: relative,
                    language: language.to_string(),
                })
            })
            .collect();
        document.symbols = self
            .project_symbols
            .iter()
            .map(|entry| tcide_project::ProjectSymbol {
                id: format!(
                    "symbol:{}:{}:{}",
                    entry.file,
                    entry.symbol.line + 1,
                    entry.symbol.name
                ),
                file: entry.file.clone(),
                name: entry.symbol.name.clone(),
                line: entry.symbol.line + 1,
                role: match entry.symbol.role {
                    editor_core::SymbolRole::Declaration => "declaration",
                    editor_core::SymbolRole::Reference => "reference",
                }
                .into(),
            })
            .collect();
        let declarations = document
            .symbols
            .iter()
            .filter(|symbol| symbol.role == "declaration")
            .map(|symbol| (symbol.name.clone(), symbol.id.clone()))
            .collect::<std::collections::BTreeMap<_, _>>();
        document.relations = document
            .symbols
            .iter()
            .filter(|symbol| symbol.role == "reference")
            .filter_map(|symbol| {
                declarations
                    .get(&symbol.name)
                    .map(|to| tcide_project::ProjectRelation {
                        from: symbol.id.clone(),
                        to: to.clone(),
                        kind: "reference".into(),
                        confidence: "indexed".into(),
                    })
            })
            .collect();
        document.view.folded = self.graph_folded.clone();
        document.view.focused_file = self
            .file_path
            .as_ref()
            .and_then(|path| path.strip_prefix(&self.project_root).ok())
            .map(|path| path.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default();
        self.tcide_project = document;
        let _ = tcide_project::save(&self.project_root, &self.tcide_project);
    }

    pub(super) fn scan_project(
        root: &std::path::Path,
        collapsed: &HashSet<PathBuf>,
    ) -> (Vec<String>, Vec<PathBuf>, Vec<bool>) {
        fn visit(
            root: &std::path::Path,
            depth: usize,
            collapsed: &HashSet<PathBuf>,
            output: &mut Vec<(String, PathBuf, bool)>,
        ) {
            if depth >= 8 || output.len() >= 500 {
                return;
            }
            let mut children = fs::read_dir(root)
                .ok()
                .into_iter()
                .flatten()
                .filter_map(Result::ok)
                .collect::<Vec<_>>();
            children.sort_by_key(|entry| {
                let is_file = entry.file_type().map(|kind| kind.is_file()).unwrap_or(true);
                (
                    is_file,
                    entry.file_name().to_string_lossy().to_ascii_lowercase(),
                )
            });
            for entry in children {
                if output.len() >= 500 {
                    break;
                }
                let path = entry.path();
                let is_directory = entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false);
                let arrow = if is_directory {
                    if collapsed.contains(&path) {
                        "▸ "
                    } else {
                        "▾ "
                    }
                } else {
                    "  "
                };
                output.push((
                    format!(
                        "{}{}{}",
                        "  ".repeat(depth),
                        arrow,
                        entry.file_name().to_string_lossy()
                    ),
                    path.clone(),
                    is_directory,
                ));
                if is_directory && !collapsed.contains(&path) {
                    visit(&path, depth + 1, collapsed, output);
                }
            }
        }

        let mut entries = Vec::new();
        visit(root, 0, collapsed, &mut entries);
        (
            entries.iter().map(|(name, _, _)| name.clone()).collect(),
            entries.iter().map(|(_, path, _)| path.clone()).collect(),
            entries.into_iter().map(|(_, _, is_dir)| is_dir).collect(),
        )
    }

    /// Fingerprints of the project files, without reading their contents.
    pub(super) fn scan_project_fingerprints(
        root: &std::path::Path,
    ) -> Vec<index_core::FileFingerprint> {
        fn collect(
            root: &std::path::Path,
            path: &std::path::Path,
            out: &mut Vec<index_core::FileFingerprint>,
        ) {
            if out.len() >= 300 {
                return;
            }
            let Ok(entries) = fs::read_dir(path) else {
                return;
            };
            for entry in entries.filter_map(Result::ok) {
                if out.len() >= 300 {
                    break;
                }
                let path = entry.path();
                if path.is_dir() {
                    let ignored = path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .map(|name| matches!(name, ".git" | "target" | "node_modules" | ".venv"))
                        .unwrap_or(false);
                    if !ignored {
                        collect(root, &path, out);
                    }
                    continue;
                }
                let display = path
                    .strip_prefix(root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('\\', "/");
                if editor_core::language_for_file(&display) == "Plain Text" {
                    continue;
                }
                let Ok(metadata) = path.metadata() else {
                    continue;
                };
                if metadata.len() > 1_000_000 {
                    continue;
                }
                let modified = metadata
                    .modified()
                    .ok()
                    .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|duration| duration.as_millis() as u64)
                    .unwrap_or(0);
                out.push(index_core::FileFingerprint::new(
                    display,
                    metadata.len(),
                    modified,
                ));
            }
        }
        let mut fingerprints = Vec::new();
        collect(root, root, &mut fingerprints);
        fingerprints
    }

    /// Read one project file for the index.
    pub(super) fn read_project_file(root: &std::path::Path, relative: &str) -> Option<Vec<String>> {
        let path = root.join(relative.replace('/', std::path::MAIN_SEPARATOR_STR));
        let content = fs::read_to_string(path).ok()?;
        let mut lines = content.lines().map(str::to_string).collect::<Vec<_>>();
        if lines.is_empty() {
            lines.push(String::new());
        }
        Some(lines)
    }

    pub(super) fn open_project(&mut self, root: PathBuf) {
        self.graph_folded.clear();
        self.graph_selected = None;
        self.graph_keyboard_active = false;
        self.session.set_project(Some(root.clone()));
        self.session.remember_project(&root);
        save_session(&self.session);
        self.project_root = root;
        self.tcide_project = tcide_project::load_or_create(&self.project_root);
        self.package_targets = Self::load_package_targets(&self.project_root);
        self.project_scroll = 0;
        // A different project has different rows: nothing should slide over
        // from the tree that was on screen before.
        self.tree_row_shift.clear();
        self.tree_row_positions.clear();
        self.collapsed_directories.clear();
        (
            self.project_entries,
            self.project_entry_paths,
            self.project_entry_is_dir,
        ) = Self::scan_project(&self.project_root, &self.collapsed_directories);
        self.shutdown_language_server();
        self.load_chat_history();
        self.request_project_index();
        self.file_path = None;
        self.file_name = "Select a file".into();
        self.buffer =
            text_buffer::TextBuffer::from_text("// Choose a source file from the project tree\n");
        self.lines = vec!["// Choose a source file from the project tree".into()];
        self.caret_line = 0;
        self.caret_column = 0;
        self.selection_anchor = None;
        self.expanded_comments.clear();
        self.editing_comment = None;
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.terminals.clear();
        self.active_terminal = 0;
        self.status = format!("Opened project {}", self.project_root.display());
        self.started = Instant::now();
    }

    pub(super) fn begin_create(&mut self, kind: CreateKind) {
        self.create_kind = Some(kind);
        self.create_name.clear();
        self.create_error.clear();
        self.create_location = self.project_root.clone();
        self.workspace_search_focused = false;
    }

    pub(super) fn save_preferences(&self) {
        let _ = preferences::save(&preferences::WindowsPreferences {
            light_theme: self.light_theme,
            ui_chinese: self.ui_chinese,
            code_zoom: self.code_zoom,
            code_font_family: self.code_font_family,
            project_language: self.project_language,
            project_environment: self.project_environment,
            language_installed: self.language_installed.clone(),
            language_versions: self.language_versions.clone(),
            background_path: self
                .background_path
                .as_ref()
                .map(|path| path.to_string_lossy().into_owned()),
            background_opacity: self.background_opacity,
            background_offset_x: self.background_offset_x,
            background_offset_y: self.background_offset_y,
            background_zoom: self.background_zoom,
            ai_endpoint: self.ai_endpoint.clone(),
            ai_model: self.ai_model.clone(),
        });
    }

    /// Re-read the tree after a fold or a change on disk.
    ///
    /// The rows keep their identity by path, so the ones the fold did not touch
    /// slide to their new place instead of jumping.
    pub(super) fn refresh_project(&mut self) {
        (
            self.project_entries,
            self.project_entry_paths,
            self.project_entry_is_dir,
        ) = Self::scan_project(&self.project_root, &self.collapsed_directories);
        self.project_scroll = self
            .project_scroll
            .min(self.project_entries.len().saturating_sub(1));
    }

    pub(super) fn create_requested_item(&mut self) {
        let Some(kind) = self.create_kind else {
            return;
        };
        let name = self.create_name.trim();
        if name.is_empty() || name.contains(['/', '\\']) {
            self.create_error = "Enter a valid name without path separators".into();
            return;
        }
        let target = if kind == CreateKind::Project {
            self.create_location.join(name)
        } else {
            self.project_root.join(name)
        };
        if target.exists() {
            self.create_error = format!("{} already exists", target.display());
            return;
        }
        let result = match kind {
            CreateKind::File => fs::write(&target, ""),
            CreateKind::Folder => fs::create_dir(&target),
            CreateKind::Project => self.create_project_template(&target),
        };
        match result {
            Ok(()) => {
                self.create_kind = None;
                self.create_error.clear();
                if kind == CreateKind::Project {
                    self.open_project(target);
                    self.start_project_environment_setup();
                } else {
                    self.refresh_project();
                    self.status = format!("Created {}", target.display());
                    if kind == CreateKind::File {
                        self.open_file(target);
                    }
                }
            }
            Err(error) => self.create_error = format!("Create failed: {error}"),
        }
    }

    pub(super) fn create_project_template(&self, target: &std::path::Path) -> std::io::Result<()> {
        let language = self.project_language_name();
        fs::create_dir_all(target)?;
        let template = workspace_core::project_template(
            &self.project_root_name(),
            language,
            self.project_version_name(),
            self.project_environments()[self
                .project_environment
                .min(self.project_environments().len() - 1)],
        );
        for file in &template.files {
            let path = target.join(file.path.replace('/', std::path::MAIN_SEPARATOR_STR));
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(path, &file.contents)?;
        }
        fs::write(
            target.join(workspace_core::PROJECT_METADATA_FILE),
            &template.metadata,
        )
    }

    /// Folder name a new project is created under.
    pub(super) fn project_root_name(&self) -> String {
        self.create_name.trim().to_string()
    }

    /// Start indexing the project on a background thread.
    ///
    /// Scanning a large project must never block the UI thread, so the scan runs
    /// on its own thread and the result is collected by `poll_project_index`.
    pub(super) fn request_project_index(&mut self) {
        let root = self.project_root.clone();
        let fingerprints = Self::scan_project_fingerprints(&root);
        let previous = std::mem::take(&mut self.project_index);
        let (request, receive) = std::sync::mpsc::channel::<(
            PathBuf,
            index_core::IncrementalIndex,
            index_core::IndexStats,
        )>();
        let sender = request.clone();
        let scan_root = root.clone();
        std::thread::spawn(move || {
            let mut index = previous;
            let stats = index.refresh(fingerprints, |relative| {
                WindowsAppState::read_project_file(&scan_root, relative)
            });
            let _ = sender.send((scan_root, index, stats));
        });
        self.index_request = Some(request);
        self.index_result = Some(receive);
        self.status = "Refreshing the project index in the background".into();
    }

    /// Collect a finished background index, if one is ready.
    pub(super) fn poll_project_index(&mut self) {
        let Some(receiver) = self.index_result.as_ref() else {
            return;
        };
        let Ok((root, index, stats)) = receiver.try_recv() else {
            return;
        };
        self.index_result = None;
        self.index_request = None;
        if root != self.project_root {
            // The project changed while the scan was running; drop the result.
            return;
        }
        self.project_declarations = index.declarations();
        self.project_symbols = index.symbols();
        self.project_index = index;
        self.status = format!(
            "Index: {} file(s) reused, {} re-parsed, {} removed",
            stats.reused, stats.reparsed, stats.removed
        );
    }

    pub(super) fn task_command(&self, build: bool) -> String {
        let facts = workspace_core::ProjectFacts {
            has_cargo: self.project_root.join("Cargo.toml").exists(),
            has_package_json: self.project_root.join("package.json").exists(),
            has_go_mod: self.project_root.join("go.mod").exists(),
            language: editor_core::language_for_file(&self.file_name).to_string(),
            file_name: self
                .file_path
                .as_ref()
                .and_then(|path| path.file_name())
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default(),
            file_stem: self
                .file_path
                .as_ref()
                .and_then(|path| path.file_stem())
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default(),
            windows: cfg!(windows),
        };
        workspace_core::task_command(&facts, build)
    }

    pub(super) fn run_task(&mut self, build: bool) {
        self.terminal_open = true;
        self.ensure_terminal();
        let index = if build { 0 } else { 1 }.min(self.terminals.len().saturating_sub(1));
        let command = self.task_command(build);
        if let Some(tab) = self.terminals.get(index) {
            let _ = tab.session.write(&format!("{command}\r"));
            self.active_terminal = index;
            self.status = format!("{}: {command}", if build { "Build" } else { "Run" });
        }
    }
}

/// Where the session (open documents, project, recent projects) is stored./// Where the session (open documents, project, recent projects) is stored.
pub(super) fn session_path() -> PathBuf {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("TC-IDE")
        .join(workspace_core::SESSION_FILE)
}

/// Read the previous session, if any.
pub(super) fn load_session() -> session_core::Session {
    match fs::read_to_string(session_path()) {
        Ok(content) => session_core::Session::parse(&content),
        Err(_) => session_core::Session::default(),
    }
}

/// Persist the session. Documents are listed, never their contents or secrets.
pub(super) fn save_session(session: &session_core::Session) {
    let path = session_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(path, session.serialize());
}
