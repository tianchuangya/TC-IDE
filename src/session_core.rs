//! Shared document session: open documents, dirty state, recent projects and
//! session restore.
//!
//! The backends own their buffers and their windows, but "which documents are
//! open, which one is active, which are unsaved and which projects were used
//! recently" is plain data with rules that deserve tests. Keeping it here means
//! Linux `cargo test` covers dirty tracking, tab switching, recent project
//! ordering and restore parsing.
#![allow(dead_code)] // The backends adopt the pieces they need as they migrate.

use std::path::{Path, PathBuf};

/// Recent projects kept for the project menu.
pub const MAX_RECENT_PROJECTS: usize = 8;
/// Documents restored when a session is reopened.
pub const MAX_RESTORED_DOCUMENTS: usize = 24;

/// Geometry and visibility of the panels around the editor.
///
/// Fractions are stored in thousandths rather than pixels, so the same layout
/// remains useful when the next launch uses a different monitor or DPI scale.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorkspaceLayout {
    pub project_tree_expanded: bool,
    pub graph_visible: bool,
    pub graph_current: bool,
    pub graph_fraction_milli: u16,
    pub terminal_open: bool,
    pub terminal_fraction_milli: u16,
}

impl Default for WorkspaceLayout {
    fn default() -> Self {
        Self {
            project_tree_expanded: true,
            graph_visible: true,
            graph_current: true,
            graph_fraction_milli: 390,
            terminal_open: false,
            terminal_fraction_milli: 230,
        }
    }
}

/// One open document as the session sees it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Document {
    pub path: Option<PathBuf>,
    pub name: String,
    pub dirty: bool,
}

impl Document {
    /// A document that has never been saved.
    pub fn untitled(index: usize) -> Self {
        Self {
            path: None,
            name: format!("Untitled-{index}"),
            dirty: false,
        }
    }

    pub fn from_path(path: &Path) -> Self {
        Self {
            path: Some(path.to_path_buf()),
            name: path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.to_string_lossy().into_owned()),
            dirty: false,
        }
    }

    /// Identity used to avoid opening the same file twice.
    pub fn key(&self) -> String {
        match &self.path {
            Some(path) => path.to_string_lossy().to_lowercase(),
            None => format!("untitled:{}", self.name.to_lowercase()),
        }
    }

    /// Label drawn on the tab, with a marker while the document is unsaved.
    pub fn title(&self) -> String {
        if self.dirty {
            format!("{} •", self.name)
        } else {
            self.name.clone()
        }
    }
}

/// Open documents, the active one, the project and the recent project list.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Session {
    documents: Vec<Document>,
    active: usize,
    caret: (usize, usize),
    project: Option<PathBuf>,
    recent_projects: Vec<PathBuf>,
    layout: WorkspaceLayout,
}

impl Session {
    pub fn documents(&self) -> &[Document] {
        &self.documents
    }

    pub fn active(&self) -> usize {
        self.active
    }

    pub fn active_document(&self) -> Option<&Document> {
        self.documents.get(self.active)
    }

    pub fn caret(&self) -> (usize, usize) {
        self.caret
    }

    pub fn set_caret(&mut self, line: usize, column: usize) {
        self.caret = (line, column);
    }

    pub fn layout(&self) -> WorkspaceLayout {
        self.layout
    }

    pub fn set_layout(&mut self, layout: WorkspaceLayout) {
        self.layout = layout;
    }

    pub fn project(&self) -> Option<&PathBuf> {
        self.project.as_ref()
    }

    pub fn recent_projects(&self) -> &[PathBuf] {
        &self.recent_projects
    }

    pub fn is_empty(&self) -> bool {
        self.documents.is_empty()
    }

    /// Activate a document; returns whether the active document changed.
    pub fn activate(&mut self, index: usize) -> bool {
        if index >= self.documents.len() || index == self.active {
            return false;
        }
        self.active = index;
        true
    }

    /// Open a document, reusing the existing tab when the file is already open.
    pub fn open(&mut self, path: Option<PathBuf>, name: String) -> usize {
        let document = match path {
            Some(path) => Document {
                path: Some(path),
                name,
                dirty: false,
            },
            None => Document {
                path: None,
                name,
                dirty: false,
            },
        };
        let key = document.key();
        if let Some(index) = self
            .documents
            .iter()
            .position(|existing| existing.key() == key)
        {
            self.active = index;
            return index;
        }
        self.documents.push(document);
        self.active = self.documents.len() - 1;
        self.active
    }

    /// Open a fresh untitled document.
    pub fn open_untitled(&mut self) -> usize {
        let index = self.documents.len() + 1;
        let document = Document::untitled(index);
        let name = document.name.clone();
        self.open(None, name)
    }

    /// Close a document and return it, moving the active tab to a neighbour.
    pub fn close(&mut self, index: usize) -> Option<Document> {
        if index >= self.documents.len() {
            return None;
        }
        let document = self.documents.remove(index);
        if self.documents.is_empty() {
            self.active = 0;
        } else if index < self.active {
            self.active -= 1;
        } else if index == self.active {
            self.active = self.active.min(self.documents.len() - 1);
        }
        Some(document)
    }

    pub fn mark_dirty(&mut self, index: usize, dirty: bool) {
        if let Some(document) = self.documents.get_mut(index) {
            document.dirty = dirty;
        }
    }

    pub fn dirty_documents(&self) -> Vec<usize> {
        self.documents
            .iter()
            .enumerate()
            .filter(|(_, document)| document.dirty)
            .map(|(index, _)| index)
            .collect()
    }

    pub fn set_project(&mut self, project: Option<PathBuf>) {
        self.project = project;
    }

    /// Remember a project, newest first, without duplicates.
    pub fn remember_project(&mut self, path: &Path) -> bool {
        let path = path.to_path_buf();
        let before = self.recent_projects.clone();
        self.recent_projects.retain(|existing| existing != &path);
        self.recent_projects.insert(0, path);
        self.recent_projects.truncate(MAX_RECENT_PROJECTS);
        before != self.recent_projects
    }

    /// Drop recent projects that no longer exist.
    pub fn prune_recent_projects(&mut self, exists: impl Fn(&Path) -> bool) {
        self.recent_projects.retain(|path| exists(path));
    }

    pub fn forget_project(&mut self, path: &Path) {
        self.recent_projects.retain(|existing| existing != path);
    }

    /// Documents that can be reopened when the session is restored.
    pub fn restorable_documents(&self) -> Vec<PathBuf> {
        self.documents
            .iter()
            .filter_map(|document| document.path.clone())
            .take(MAX_RESTORED_DOCUMENTS)
            .collect()
    }

    /// Session file contents. Never contains secrets.
    pub fn serialize(&self) -> String {
        let mut text = String::new();
        text.push_str(&format!("active={}\n", self.active));
        text.push_str(&format!("caret={}|{}\n", self.caret.0, self.caret.1));
        let layout = self.layout;
        text.push_str(&format!(
            "layout={}|{}|{}|{}|{}|{}\n",
            u8::from(layout.project_tree_expanded),
            u8::from(layout.graph_visible),
            u8::from(layout.graph_current),
            layout.graph_fraction_milli,
            u8::from(layout.terminal_open),
            layout.terminal_fraction_milli
        ));
        if let Some(project) = &self.project {
            text.push_str(&format!("project={}\n", project.to_string_lossy()));
        }
        for project in &self.recent_projects {
            text.push_str(&format!("recent={}\n", project.to_string_lossy()));
        }
        for document in &self.documents {
            let path = document
                .path
                .as_ref()
                .map(|path| path.to_string_lossy().into_owned())
                .unwrap_or_default();
            text.push_str(&format!(
                "document={}|{}|{}\n",
                path,
                document.name,
                if document.dirty { "1" } else { "0" }
            ));
        }
        text
    }

    pub fn parse(content: &str) -> Session {
        let mut session = Session::default();
        let mut requested_active = None;
        for line in content.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            match key.trim() {
                "active" => requested_active = value.trim().parse::<usize>().ok(),
                "caret" => {
                    if let Some((line, column)) = value.split_once('|') {
                        if let (Ok(line), Ok(column)) =
                            (line.trim().parse::<usize>(), column.trim().parse::<usize>())
                        {
                            session.caret = (line, column);
                        }
                    }
                }
                "layout" => {
                    let parts = value.split('|').collect::<Vec<_>>();
                    if parts.len() == 6 {
                        let boolean = |value: &str| value.trim() == "1";
                        if let (Ok(graph), Ok(terminal)) =
                            (parts[3].parse::<u16>(), parts[5].parse::<u16>())
                        {
                            session.layout = WorkspaceLayout {
                                project_tree_expanded: boolean(parts[0]),
                                graph_visible: boolean(parts[1]),
                                graph_current: boolean(parts[2]),
                                graph_fraction_milli: graph.clamp(100, 800),
                                terminal_open: boolean(parts[4]),
                                terminal_fraction_milli: terminal.clamp(100, 800),
                            };
                        }
                    }
                }
                "project" if !value.trim().is_empty() => {
                    session.project = Some(PathBuf::from(value.trim()))
                }
                "recent" if !value.trim().is_empty() => {
                    let path = PathBuf::from(value.trim());
                    if !session.recent_projects.contains(&path) {
                        session.recent_projects.push(path);
                    }
                }
                "document" => {
                    let parts = value.split('|').collect::<Vec<_>>();
                    if parts.len() < 2 {
                        continue;
                    }
                    let path = if parts[0].is_empty() {
                        None
                    } else {
                        Some(PathBuf::from(parts[0]))
                    };
                    session.documents.push(Document {
                        path,
                        name: parts[1].to_string(),
                        // A restored document is never dirty: its buffer is gone.
                        dirty: false,
                    });
                }
                _ => {}
            }
        }
        session.recent_projects.truncate(MAX_RECENT_PROJECTS);
        session.documents.truncate(MAX_RESTORED_DOCUMENTS);
        session.active = requested_active
            .unwrap_or_else(|| session.documents.len().saturating_sub(1))
            .min(session.documents.len().saturating_sub(1));
        if session.documents.is_empty() {
            session.active = 0;
        }
        session
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session_with_documents() -> Session {
        let mut session = Session::default();
        session.open(Some(PathBuf::from("/p/src/main.rs")), "main.rs".into());
        session.open(Some(PathBuf::from("/p/src/lib.rs")), "lib.rs".into());
        session
    }

    #[test]
    fn opening_a_file_twice_reuses_its_tab() {
        let mut session = session_with_documents();
        session.activate(0);
        let index = session.open(Some(PathBuf::from("/p/src/main.rs")), "main.rs".into());
        assert_eq!(index, 0);
        assert_eq!(session.documents().len(), 2);
        assert_eq!(session.active(), 0);
    }

    #[test]
    fn closing_tabs_keeps_a_sensible_active_tab() {
        let mut session = session_with_documents();
        session.activate(0);
        assert!(session.close(1).is_some());
        assert_eq!(session.documents().len(), 1);
        assert_eq!(session.active(), 0);

        let mut session = session_with_documents();
        session.activate(1);
        session.close(1);
        assert_eq!(session.active(), 0);
        assert_eq!(
            session.active_document().map(|doc| doc.name.as_str()),
            Some("main.rs")
        );

        let mut session = session_with_documents();
        session.activate(1);
        session.close(0);
        assert_eq!(session.active(), 0);
        assert_eq!(
            session.active_document().map(|doc| doc.name.as_str()),
            Some("lib.rs")
        );

        assert!(session.close(9).is_none());
    }

    #[test]
    fn untitled_documents_get_distinct_tabs() {
        let mut session = Session::default();
        session.open_untitled();
        session.open_untitled();
        assert_eq!(session.documents().len(), 2);
        assert_ne!(session.documents()[0].name, session.documents()[1].name);
        assert!(session.documents().iter().all(|doc| doc.path.is_none()));
    }

    #[test]
    fn dirty_documents_are_tracked_and_labelled() {
        let mut session = session_with_documents();
        session.mark_dirty(0, true);
        session.mark_dirty(1, true);
        assert_eq!(session.dirty_documents(), vec![0, 1]);
        assert_eq!(session.documents()[0].title(), "main.rs •");
        session.mark_dirty(0, false);
        assert_eq!(session.dirty_documents(), vec![1]);
        assert_eq!(session.documents()[0].title(), "main.rs");
    }

    #[test]
    fn recent_projects_are_unique_newest_first_and_capped() {
        let mut session = Session::default();
        for index in 0..(MAX_RECENT_PROJECTS + 3) {
            assert!(session.remember_project(Path::new(&format!("/p/{index}"))));
        }
        assert_eq!(session.recent_projects().len(), MAX_RECENT_PROJECTS);
        assert_eq!(
            session.recent_projects()[0],
            PathBuf::from(format!("/p/{}", MAX_RECENT_PROJECTS + 2))
        );
        assert!(!session.remember_project(Path::new(&format!("/p/{}", MAX_RECENT_PROJECTS + 2))));
        assert_eq!(
            session.recent_projects()[0],
            PathBuf::from(format!("/p/{}", MAX_RECENT_PROJECTS + 2))
        );

        session.prune_recent_projects(|path| path.ends_with("10"));
        assert_eq!(session.recent_projects().len(), 1);
        session.forget_project(Path::new("/p/10"));
        assert!(session.recent_projects().is_empty());
    }

    #[test]
    fn session_round_trips_through_its_file() {
        let mut session = session_with_documents();
        session.set_project(Some(PathBuf::from("/p")));
        session.remember_project(Path::new("/p"));
        session.remember_project(Path::new("/q"));
        session.mark_dirty(1, true);
        session.activate(0);
        session.set_caret(17, 9);
        session.set_layout(WorkspaceLayout {
            project_tree_expanded: false,
            graph_visible: true,
            graph_current: false,
            graph_fraction_milli: 315,
            terminal_open: true,
            terminal_fraction_milli: 420,
        });

        let text = session.serialize();
        assert!(text.contains("project=/p"));
        assert!(!text.to_lowercase().contains("key"));
        let restored = Session::parse(&text);
        assert_eq!(restored.project(), Some(&PathBuf::from("/p")));
        assert_eq!(
            restored.recent_projects(),
            &[PathBuf::from("/q"), PathBuf::from("/p")]
        );
        assert_eq!(restored.documents().len(), 2);
        assert_eq!(restored.active(), 0);
        assert_eq!(restored.caret(), (17, 9));
        assert_eq!(restored.layout(), session.layout());
        // Unsaved edits cannot be restored, so restored documents start clean.
        assert!(restored.documents().iter().all(|doc| !doc.dirty));
        assert_eq!(restored.restorable_documents().len(), 2);
    }

    #[test]
    fn parsing_tolerates_broken_lines_and_empty_sessions() {
        let parsed = Session::parse("nonsense\ndocument=\nproject=\nrecent=\n");
        assert!(parsed.is_empty());
        assert!(parsed.project().is_none());
        assert!(parsed.recent_projects().is_empty());
        assert_eq!(Session::parse("").active(), 0);
    }
}
