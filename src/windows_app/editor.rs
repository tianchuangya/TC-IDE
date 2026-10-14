//! Editing core: buffer edits, caret, selection, undo/redo and open documents.

use super::events::confirm_discard;
use super::workspace::save_session;
use super::*;

impl WindowsAppState {
    /// Remember which document the active buffer belongs to.
    pub(super) fn active_document(&self) -> Option<usize> {
        let index = self.session.active();
        self.session.documents().get(index).map(|_| index)
    }

    /// Keep the active buffer and mark it unsaved.
    pub(super) fn mark_active_dirty(&mut self) {
        if let Some(index) = self.active_document() {
            self.session.mark_dirty(index, true);
        }
    }

    /// Save the active buffer fields under its document index.
    pub(super) fn stash_active_buffer(&mut self) {
        let Some(index) = self.active_document() else {
            return;
        };
        let buffer = StoredBuffer {
            lines: std::mem::take(&mut self.lines),
            file_name: std::mem::take(&mut self.file_name),
            file_path: self.file_path.take(),
            caret_line: self.caret_line,
            caret_column: self.caret_column,
            selection_anchor: self.selection_anchor,
            undo_stack: std::mem::take(&mut self.undo_stack),
            redo_stack: std::mem::take(&mut self.redo_stack),
        };
        self.stored_buffers.insert(index, buffer);
    }

    /// Load a document into the active buffer, reading it from disk if needed.
    pub(super) fn load_document(&mut self, index: usize) {
        if let Some(buffer) = self.stored_buffers.remove(&index) {
            self.buffer = text_buffer::TextBuffer::from_lines(&buffer.lines);
            self.lines = buffer.lines;
            self.file_name = buffer.file_name;
            self.file_path = buffer.file_path;
            self.caret_line = buffer.caret_line;
            self.caret_column = buffer.caret_column;
            self.selection_anchor = buffer.selection_anchor;
            self.undo_stack = buffer.undo_stack;
            self.redo_stack = buffer.redo_stack;
        } else if let Some(document) = self.session.documents().get(index).cloned() {
            self.file_name = document.name.clone();
            self.file_path = document.path.clone();
            self.lines = match document
                .path
                .as_ref()
                .and_then(|path| fs::read_to_string(path).ok())
            {
                Some(content) => {
                    let mut lines = content.lines().map(str::to_string).collect::<Vec<_>>();
                    if lines.is_empty() {
                        lines.push(String::new());
                    }
                    self.buffer = text_buffer::TextBuffer::from_lines(&lines);
                    lines
                }
                None => {
                    self.buffer = text_buffer::TextBuffer::default();
                    vec![String::new()]
                }
            };
            self.caret_line = 0;
            self.caret_column = 0;
            self.selection_anchor = None;
            self.undo_stack.clear();
            self.redo_stack.clear();
        }
        self.clamp_caret();
        self.ime_composition.clear();
        self.editing_comment = None;
    }

    /// Switch to another document, keeping the current buffer.
    pub(super) fn switch_document(&mut self, index: usize) -> bool {
        if index == self.session.active() || index >= self.session.documents().len() {
            return false;
        }
        self.stash_active_buffer();
        self.session.activate(index);
        self.load_document(index);
        self.lsp_diagnostics.clear();
        self.lsp_completions.clear();
        self.lsp_locations.clear();
        self.ensure_language_server();
        self.status = format!("Active document: {}", self.file_name);
        true
    }

    /// Close a document, asking before discarding unsaved work.
    pub(super) fn close_document(&mut self, index: usize, window: Hwnd) {
        if index >= self.session.documents().len() {
            return;
        }
        if self.session.documents()[index].dirty {
            let name = self.session.documents()[index].name.clone();
            if !confirm_discard(window, &name) {
                self.status = format!("Kept {name} open");
                return;
            }
        }
        let was_active = index == self.session.active();
        if was_active && self.stored_buffers.contains_key(&index) {
            self.stored_buffers.remove(&index);
        } else if was_active {
            // The active buffer belongs to this document; clear it before switching.
            self.stored_buffers.remove(&index);
        }
        self.session.close(index);
        let reindexed = std::mem::take(&mut self.stored_buffers);
        self.stored_buffers = reindexed
            .into_iter()
            .map(|(key, buffer)| (if key > index { key - 1 } else { key }, buffer))
            .collect();
        if self.session.is_empty() {
            self.buffer = text_buffer::TextBuffer::default();
            self.lines = vec![String::new()];
            self.file_name = "untitled".into();
            self.file_path = None;
            self.caret_line = 0;
            self.caret_column = 0;
            self.undo_stack.clear();
            self.redo_stack.clear();
            self.status = "All documents closed".into();
            return;
        }
        self.load_document(self.session.active());
        self.status = format!("Closed document; active: {}", self.file_name);
    }

    pub(super) fn clamp_caret(&mut self) {
        self.caret_line = self.caret_line.min(self.lines.len().saturating_sub(1));
        self.caret_column = self
            .caret_column
            .min(self.lines[self.caret_line].chars().count());
    }

    pub(super) fn byte_at(value: &str, column: usize) -> usize {
        value
            .char_indices()
            .nth(column)
            .map(|(index, _)| index)
            .unwrap_or(value.len())
    }

    pub(super) fn comment_edit_bounds(&self) -> Option<(usize, usize, usize)> {
        let line = self.editing_comment?;
        let value = self.lines.get(line)?;
        let (_, start, end) = editor_core::comment_bounds(value, &self.file_name)?;
        Some((
            line,
            value[..start].chars().count(),
            value[..end].chars().count(),
        ))
    }

    pub(super) fn clamp_comment_caret(&mut self) {
        if let Some((line, start, end)) = self.comment_edit_bounds() {
            self.caret_line = line;
            self.caret_column = self.caret_column.clamp(start, end);
            self.selection_anchor = None;
        }
    }

    pub(super) fn position(&self) -> TextPosition {
        TextPosition {
            line: self.caret_line,
            column: self.caret_column,
        }
    }

    pub(super) fn snapshot(&self) -> EditorSnapshot {
        EditorSnapshot {
            caret: self.position(),
            selection_anchor: self.selection_anchor,
        }
    }

    pub(super) fn restore(&mut self, snapshot: EditorSnapshot) {
        self.caret_line = snapshot.caret.line;
        self.caret_column = snapshot.caret.column;
        self.selection_anchor = snapshot.selection_anchor;
        self.clamp_caret();
        self.last_edit = Some(Instant::now());
    }

    /// Rebuild the rendered lines from the piece table.
    ///
    /// The editors, the graph and the symbol index read `lines`; the piece table
    /// owns the text so an edit never copies the document into the undo stack.
    pub(super) fn sync_lines(&mut self) {
        self.lines = self.buffer.to_lines();
        self.clamp_caret();
    }

    pub(super) fn record_undo(&mut self) {
        self.lsp_dirty = true;
        self.undo_stack.push(self.snapshot());
        if self.undo_stack.len() > 200 {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
        self.buffer.begin_group();
        self.mark_active_dirty();
    }

    /// Close the edit group opened by `record_undo` and refresh the view.
    pub(super) fn finish_edit(&mut self) {
        self.buffer.end_group();
        self.sync_lines();
        self.reveal_caret();
        self.update_active_index_entry();
        self.last_edit = Some(Instant::now());
    }

    /// Scroll so the caret is inside the window.
    pub(super) fn reveal_caret(&mut self) {
        let line_count = self.lines.len();
        let line_chars = self
            .lines
            .get(self.caret_line)
            .map(|value| value.chars().count())
            .unwrap_or(0);
        self.viewport = self
            .viewport
            .revealed(self.caret_line, line_count)
            .revealed_column(self.caret_column, line_chars);
    }

    /// Scroll the editor, for the wheel and the scrollbar.
    pub(super) fn scroll_editor(&mut self, lines: i32) {
        let line_count = self.lines.len();
        self.viewport = self.viewport.scrolled(lines, line_count);
    }

    /// Scroll the editor horizontally, for the mouse wheel with shift.
    pub(super) fn scroll_editor_columns(&mut self, columns: i32) {
        let line_chars = self
            .lines
            .get(self.caret_line)
            .map(|value| value.chars().count())
            .unwrap_or(0);
        let first = if columns >= 0 {
            self.viewport.first_column.saturating_add(columns as usize)
        } else {
            self.viewport
                .first_column
                .saturating_sub(columns.saturating_neg() as usize)
        };
        let maximum = line_chars.saturating_sub(self.viewport.width.max(1));
        self.viewport.first_column = first.min(maximum);
    }

    /// Resize the visible window and clamp the scroll offset.
    ///
    /// Uses the same geometry as the renderer: the project tree on the left, the
    /// relation graph on the right, and a gutter before the code column.
    pub(super) fn resize_viewport(&mut self, width: i32, height: i32, scale: f32) {
        let width = width as f32;
        let height = height as f32;
        let tree_w = (width * 0.19).max(220.0 * scale);
        let graph_w = (width * 0.27).max(300.0 * scale);
        let editor_right = width - graph_w;
        let code_left = tree_w + 52.0 * scale;
        let line_step = 25.0 * scale;
        let rows = (((height - 106.0 * scale) / line_step).floor().max(1.0)) as usize;
        let cell = (self.code_cell_width() * scale).max(1.0);
        let columns = (((editor_right - code_left - 12.0 * scale) / cell)
            .floor()
            .max(8.0)) as usize;
        self.viewport.height = rows;
        self.viewport.width = columns;
        self.viewport = self.viewport.clamped(self.lines.len());
        self.reveal_caret();
    }

    pub(super) fn code_cell_width(&self) -> f32 {
        8.9 + self.code_zoom.clamp(-2, 8) as f32 * 0.6
    }

    pub(super) fn selection_range(&self) -> Option<(TextPosition, TextPosition)> {
        let anchor = self.selection_anchor?;
        let caret = self.position();
        (anchor != caret).then_some(if anchor < caret {
            (anchor, caret)
        } else {
            (caret, anchor)
        })
    }

    pub(super) fn selected_text(&self) -> Option<String> {
        let (start, end) = self.selection_range()?;
        if start.line == end.line {
            let line = &self.lines[start.line];
            return Some(
                line[Self::byte_at(line, start.column)..Self::byte_at(line, end.column)].into(),
            );
        }
        let mut result = String::new();
        result.push_str(
            &self.lines[start.line][Self::byte_at(&self.lines[start.line], start.column)..],
        );
        for line in (start.line + 1)..end.line {
            result.push('\n');
            result.push_str(&self.lines[line]);
        }
        result.push('\n');
        result.push_str(&self.lines[end.line][..Self::byte_at(&self.lines[end.line], end.column)]);
        Some(result)
    }

    pub(super) fn delete_selection_raw(&mut self) -> bool {
        let Some((start, end)) = self.selection_range() else {
            return false;
        };
        self.buffer
            .delete((start.line, start.column), (end.line, end.column));
        self.sync_lines();
        self.caret_line = start.line;
        self.caret_column = start.column;
        self.selection_anchor = None;
        true
    }

    pub(super) fn insert_text(&mut self, value: &str) {
        let protected_comment = self.comment_edit_bounds();
        if let Some((line, start, end)) = protected_comment {
            self.caret_line = line;
            self.caret_column = self.caret_column.clamp(start, end);
            let selection_is_safe = self.selection_range().is_some_and(|(from, to)| {
                from.line == line && to.line == line && from.column >= start && to.column <= end
            });
            if !selection_is_safe {
                self.selection_anchor = None;
            }
        }
        self.record_undo();
        self.delete_selection_raw();
        let value = if protected_comment.is_some() {
            value.replace(['\r', '\n'], " ")
        } else {
            value.replace("\r\n", "\n").replace('\r', "\n")
        };
        for character in value.chars() {
            if character == '\n' {
                self.newline_raw();
            } else if !character.is_control() {
                self.buffer
                    .insert(self.caret_line, self.caret_column, &character.to_string());
                self.caret_column += 1;
                self.sync_lines();
            }
        }
        self.finish_edit();
        self.notify_language_server();
        self.status = "Edited with Unicode-aware caret mapping".into();
    }

    pub(super) fn completions(&self) -> Vec<String> {
        if !self.lsp_completions.is_empty() {
            return self.lsp_completions.clone();
        }
        editor_core::completions(
            &self.file_name,
            &self.lines[self.caret_line],
            self.caret_column,
        )
    }

    pub(super) fn accept_completion(&mut self) {
        let Some(candidate) = self.completions().into_iter().next() else {
            return;
        };
        let prefix =
            editor_core::completion_prefix(&self.lines[self.caret_line], self.caret_column);
        if prefix.is_empty() {
            return;
        }
        self.selection_anchor = Some(TextPosition {
            line: self.caret_line,
            column: self.caret_column.saturating_sub(prefix.chars().count()),
        });
        self.insert_text(&candidate);
        self.status = format!("Completed {candidate}");
    }

    pub(super) fn insert_char(&mut self, value: char) {
        self.insert_text(&value.to_string());
    }

    pub(super) fn newline(&mut self) {
        self.record_undo();
        self.delete_selection_raw();
        self.newline_raw();
        self.finish_edit();
        self.notify_language_server();
    }

    pub(super) fn newline_raw(&mut self) {
        self.buffer.insert(self.caret_line, self.caret_column, "\n");
        self.caret_line += 1;
        self.caret_column = 0;
    }

    pub(super) fn backspace(&mut self) {
        if let Some((line, start, _)) = self.comment_edit_bounds() {
            if self.caret_line != line || self.caret_column <= start {
                self.clamp_comment_caret();
                return;
            }
            self.selection_anchor = None;
        }
        if self.selection_range().is_some() {
            self.record_undo();
            self.delete_selection_raw();
            self.finish_edit();
            self.notify_language_server();
            return;
        }
        if self.caret_column == 0 && self.caret_line == 0 {
            return;
        }
        self.record_undo();
        if self.caret_column > 0 {
            self.buffer.delete(
                (self.caret_line, self.caret_column - 1),
                (self.caret_line, self.caret_column),
            );
            self.caret_column -= 1;
        } else if self.caret_line > 0 {
            let previous_length = self.buffer.line_chars(self.caret_line - 1);
            self.buffer
                .delete((self.caret_line - 1, previous_length), (self.caret_line, 0));
            self.caret_line -= 1;
            self.caret_column = previous_length;
        }
        self.finish_edit();
        self.notify_language_server();
    }

    pub(super) fn delete_forward(&mut self) {
        if let Some((line, _, end)) = self.comment_edit_bounds() {
            if self.caret_line != line || self.caret_column >= end {
                self.clamp_comment_caret();
                return;
            }
            self.selection_anchor = None;
        }
        if self.selection_range().is_some() {
            self.record_undo();
            self.delete_selection_raw();
        } else {
            let line_len = self.buffer.line_chars(self.caret_line);
            if self.caret_column < line_len {
                self.record_undo();
                self.buffer.delete(
                    (self.caret_line, self.caret_column),
                    (self.caret_line, self.caret_column + 1),
                );
            } else if self.caret_line + 1 < self.buffer.line_count() {
                self.record_undo();
                self.buffer
                    .delete((self.caret_line, line_len), (self.caret_line + 1, 0));
            } else {
                return;
            }
        }
        self.finish_edit();
        self.notify_language_server();
    }

    pub(super) fn move_left(&mut self) {
        if self.caret_column > 0 {
            self.caret_column -= 1;
        } else if self.caret_line > 0 {
            self.caret_line -= 1;
            self.caret_column = self.lines[self.caret_line].chars().count();
        }
    }

    pub(super) fn move_right(&mut self) {
        if self.caret_column < self.lines[self.caret_line].chars().count() {
            self.caret_column += 1;
        } else if self.caret_line + 1 < self.lines.len() {
            self.caret_line += 1;
            self.caret_column = 0;
        }
    }

    pub(super) fn select_all(&mut self) {
        self.selection_anchor = Some(TextPosition { line: 0, column: 0 });
        self.caret_line = self.lines.len().saturating_sub(1);
        self.caret_column = self.lines[self.caret_line].chars().count();
        self.status = "Selected all text".into();
    }

    pub(super) fn cut_selection(&mut self) -> Option<String> {
        let value = self.selected_text()?;
        self.record_undo();
        self.delete_selection_raw();
        self.finish_edit();
        self.notify_language_server();
        Some(value)
    }

    pub(super) fn undo(&mut self) {
        self.mark_active_dirty();
        if let Some(snapshot) = self.undo_stack.pop() {
            self.redo_stack.push(self.snapshot());
            if self.buffer.undo() {
                self.sync_lines();
            }
            self.restore(snapshot);
            self.status = "Undo".into();
        }
    }

    pub(super) fn redo(&mut self) {
        self.mark_active_dirty();
        if let Some(snapshot) = self.redo_stack.pop() {
            self.undo_stack.push(self.snapshot());
            if self.buffer.redo() {
                self.sync_lines();
            }
            self.restore(snapshot);
            self.status = "Redo".into();
        }
    }

    pub(super) fn set_caret(&mut self, position: TextPosition, extend_selection: bool) {
        self.reveal_caret();
        if extend_selection {
            if self.selection_anchor.is_none() {
                self.selection_anchor = Some(self.position());
            }
        } else {
            self.selection_anchor = None;
        }
        self.caret_line = position.line.min(self.lines.len().saturating_sub(1));
        self.caret_column = position.column;
        self.clamp_caret();
    }

    pub(super) fn save(&mut self) {
        let Some(path) = &self.file_path else {
            self.status = "Open a file before saving".into();
            return;
        };
        match fs::write(path, self.buffer.to_text().replace('\n', "\r\n")) {
            Ok(()) => {
                self.status = format!("Saved {}", path.display());
                self.update_active_index_entry();
                self.request_project_index();
                if let Some(index) = self.active_document() {
                    self.session.mark_dirty(index, false);
                }
            }
            Err(error) => self.status = format!("Save failed: {error}"),
        }
    }

    pub(super) fn open_file(&mut self, path: PathBuf) {
        let document = session_core::Document::from_path(&path);
        match fs::read_to_string(&path) {
            Ok(content) => {
                self.stash_active_buffer();
                let index = self.session.open(Some(path.clone()), document.name.clone());
                self.stored_buffers.remove(&index);
                self.lines = content.lines().map(str::to_string).collect();
                if self.lines.is_empty() {
                    self.lines.push(String::new());
                }
                self.buffer = text_buffer::TextBuffer::from_lines(&self.lines);
                self.file_name = path
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "untitled".into());
                self.file_path = Some(path.clone());
                if !path.starts_with(&self.project_root) {
                    let Some(parent) = path.parent() else {
                        return;
                    };
                    self.project_root = parent.to_path_buf();
                    self.collapsed_directories.clear();
                    (
                        self.project_entries,
                        self.project_entry_paths,
                        self.project_entry_is_dir,
                    ) = Self::scan_project(parent, &self.collapsed_directories);
                }
                self.caret_line = 0;
                self.caret_column = 0;
                self.selection_anchor = None;
                self.undo_stack.clear();
                self.redo_stack.clear();
                self.ime_composition.clear();
                self.expanded_comments.clear();
                self.editing_comment = None;
                self.started = Instant::now();
                self.request_project_index();
                self.session.remember_project(&self.project_root.clone());
                save_session(&self.session);
                self.ensure_language_server();
                self.status = format!(
                    "Opened {}  ·  {}",
                    path.display(),
                    editor_core::language_for_file(&self.file_name)
                );
            }
            Err(error) => self.status = format!("Open failed: {error}"),
        }
    }
}
