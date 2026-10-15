//! Shared document session: the open documents, the caret and the edits.
//!
//! Every backend needs the same editing behaviour — typing over a selection
//! replaces it, backspace at the start of the file does nothing, a vertical move
//! keeps the column inside the target line, the viewport follows the caret — and
//! three copies of it would drift apart. The session owns that behaviour and
//! nothing else: no window, no drawing, no platform type. The backends keep
//! their own rendering, their own key mapping and their own geometry, and ask
//! the session what the document looks like.
//!
//! The backends adopt the pieces they need: the macOS shell drives the editing
//! today, while the tab bar, the scrollbars and the pixel scrolling of the other
//! backends are already answered here and covered by the unit tests.
#![allow(dead_code)]

use std::path::PathBuf;
use std::time::Instant;

use crate::motion_core;
use crate::text_buffer::{TextBuffer, Viewport};

/// One open document.
#[derive(Clone, Debug)]
pub struct Document {
    pub name: String,
    pub path: Option<PathBuf>,
    /// The text, as a piece table so an edit never copies the document.
    pub buffer: TextBuffer,
    /// The same text as lines, which is what the backends draw.
    pub lines: Vec<String>,
    pub caret_line: usize,
    pub caret_column: usize,
    /// Where a selection started, when there is one.
    pub selection_anchor: Option<(usize, usize)>,
    pub dirty: bool,
}

impl Document {
    /// A document from lines, with the caret at the start.
    pub fn from_lines(name: &str, path: Option<PathBuf>, lines: Vec<String>) -> Self {
        let lines = if lines.is_empty() {
            vec![String::new()]
        } else {
            lines
        };
        Self {
            name: name.to_string(),
            path,
            buffer: TextBuffer::from_lines(&lines),
            lines,
            caret_line: 0,
            caret_column: 0,
            selection_anchor: None,
            dirty: false,
        }
    }

    /// Rebuild the rendered lines from the piece table.
    pub fn sync(&mut self) {
        self.lines = self.buffer.to_lines();
    }

    /// The whole document as text.
    pub fn text(&self) -> String {
        self.lines.join("\n")
    }

    /// The selected range, ordered, when there is a selection.
    pub fn selection(&self) -> Option<((usize, usize), (usize, usize))> {
        let anchor = self.selection_anchor?;
        let caret = (self.caret_line, self.caret_column);
        Some(if anchor <= caret {
            (anchor, caret)
        } else {
            (caret, anchor)
        })
    }

    /// Length of a line in characters.
    pub fn line_chars(&self, line: usize) -> usize {
        self.lines
            .get(line)
            .map(|value| value.chars().count())
            .unwrap_or(0)
    }
}

/// What the session did with an edit, for a backend that wants to animate it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditKind {
    Insert,
    Delete,
    Replace,
}

/// An edit the session just made, in document coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EditRecord {
    pub kind: EditKind,
    pub line: usize,
    pub start: usize,
    pub end: usize,
}

/// The open documents, the caret and the viewport.
#[derive(Debug)]
pub struct EditorSession {
    documents: Vec<Document>,
    active: usize,
    pub viewport: Viewport,
    /// First line the view shows.
    pub scroll_lines: usize,
    /// First column the view shows.
    pub scroll_columns: usize,
    /// Spring the caret breathes with, from the shared motion model.
    pub caret_spring: motion_core::Spring,
    last_tick: Instant,
    /// The last edit, for a backend that highlights it.
    pub last_edit: Option<EditRecord>,
}

impl EditorSession {
    /// A session with one document.
    pub fn new(document: Document) -> Self {
        let now = Instant::now();
        Self {
            documents: vec![document],
            active: 0,
            viewport: Viewport::new(28, 96),
            scroll_lines: 0,
            scroll_columns: 0,
            caret_spring: motion_core::Spring::new(0.0),
            last_tick: now,
            last_edit: None,
        }
    }

    /// The open documents.
    pub fn documents(&self) -> &[Document] {
        &self.documents
    }

    /// Index of the active document.
    pub fn active(&self) -> usize {
        self.active
    }

    /// The active document.
    pub fn document(&self) -> &Document {
        let index = self.active.min(self.documents.len().saturating_sub(1));
        &self.documents[index]
    }

    /// The active document, to mutate.
    pub fn document_mut(&mut self) -> &mut Document {
        let index = self.active.min(self.documents.len().saturating_sub(1));
        &mut self.documents[index]
    }

    /// Add a document and make it active.
    pub fn add_document(&mut self, document: Document) -> usize {
        self.documents.push(document);
        self.active = self.documents.len() - 1;
        self.scroll_lines = 0;
        self.scroll_columns = 0;
        self.active
    }

    /// Close a document, unless it is the last one.
    pub fn close_document(&mut self, index: usize) -> bool {
        if self.documents.len() <= 1 || index >= self.documents.len() {
            return false;
        }
        self.documents.remove(index);
        self.active = self.active.min(self.documents.len() - 1);
        true
    }

    /// Change the active document.
    pub fn activate(&mut self, index: usize) -> bool {
        if index >= self.documents.len() {
            return false;
        }
        self.active = index;
        self.scroll_lines = 0;
        self.scroll_columns = 0;
        true
    }

    /// Rows the viewport shows.
    pub fn visible_rows(&self) -> usize {
        self.viewport.height.max(1)
    }

    /// Whether the active document has unsaved changes.
    pub fn dirty(&self) -> bool {
        self.document().dirty
    }

    /// Names of every open document, for a tab bar.
    pub fn titles(&self) -> Vec<String> {
        self.documents
            .iter()
            .map(|document| {
                if document.dirty {
                    format!("{} •", document.name)
                } else {
                    document.name.clone()
                }
            })
            .collect()
    }

    /// Replace the whole document, the way opening a scratch buffer would.
    pub fn replace_all(&mut self, text: &str) {
        let lines = text.split('\n').map(str::to_string).collect::<Vec<_>>();
        let document = self.document_mut();
        document.buffer = TextBuffer::from_lines(&lines);
        document.lines = lines;
        let last = document.lines.len().saturating_sub(1);
        document.caret_line = last;
        document.caret_column = document.line_chars(last);
        document.selection_anchor = None;
        document.dirty = false;
        self.scroll_lines = 0;
        self.scroll_columns = 0;
        self.last_edit = None;
    }

    /// Remove the selection, if there is one.
    pub fn delete_selection(&mut self) -> bool {
        let Some((start, end)) = self.document().selection() else {
            return false;
        };
        if start == end {
            self.document_mut().selection_anchor = None;
            return false;
        }
        self.document_mut().buffer.delete(start, end);
        let document = self.document_mut();
        document.caret_line = start.0;
        document.caret_column = start.1;
        document.selection_anchor = None;
        document.sync();
        self.last_edit = Some(EditRecord {
            kind: EditKind::Delete,
            line: start.0,
            start: start.1,
            end: start.1,
        });
        true
    }

    /// Type text at the caret, replacing a selection and splitting on newlines.
    pub fn insert_text(&mut self, value: &str) {
        if value.is_empty() {
            return;
        }
        let replaced = self.delete_selection();
        let (mut line, mut column) = {
            let document = self.document();
            (document.caret_line, document.caret_column)
        };
        let start = (line, column);
        for character in value.chars() {
            if character == '\n' {
                self.document_mut().buffer.insert(line, column, "\n");
                line += 1;
                column = 0;
            } else if !character.is_control() {
                self.document_mut()
                    .buffer
                    .insert(line, column, &character.to_string());
                column += 1;
            }
        }
        let document = self.document_mut();
        document.caret_line = line;
        document.caret_column = column;
        document.selection_anchor = None;
        document.sync();
        document.dirty = true;
        self.last_edit = Some(EditRecord {
            kind: if replaced {
                EditKind::Replace
            } else {
                EditKind::Insert
            },
            line: start.0,
            start: start.1,
            end: if start.0 == line { column } else { 0 },
        });
        self.clamp_caret();
        self.reveal_caret();
    }

    /// Remove the character before the caret, or join with the line above.
    pub fn backspace(&mut self) {
        if self.selection_anchor_is_set() {
            self.delete_selection();
            self.mark_edited();
            return;
        }
        let (line, column) = {
            let document = self.document();
            (document.caret_line, document.caret_column)
        };
        if column > 0 {
            self.document_mut()
                .buffer
                .delete((line, column - 1), (line, column));
            self.document_mut().caret_column = column - 1;
        } else if line > 0 {
            let previous = self.document().buffer.line_chars(line - 1);
            self.document_mut()
                .buffer
                .delete((line - 1, previous), (line, 0));
            let document = self.document_mut();
            document.caret_line = line - 1;
            document.caret_column = previous;
        } else {
            return;
        }
        self.mark_edited();
        self.clamp_caret();
        self.reveal_caret();
    }

    /// Remove the character after the caret, or join with the line below.
    pub fn delete_forward(&mut self) {
        if self.selection_anchor_is_set() {
            self.delete_selection();
            self.mark_edited();
            return;
        }
        let (line, column) = {
            let document = self.document();
            (document.caret_line, document.caret_column)
        };
        let length = self.document().buffer.line_chars(line);
        if column < length {
            self.document_mut()
                .buffer
                .delete((line, column), (line, column + 1));
        } else if line + 1 < self.document().buffer.line_count() {
            self.document_mut()
                .buffer
                .delete((line, length), (line + 1, 0));
        } else {
            return;
        }
        self.mark_edited();
    }

    /// Move the caret, keeping the column inside the line it lands on.
    pub fn move_caret(&mut self, delta_line: i64, delta_column: i64) {
        let (line, column) = {
            let document = self.document();
            (document.caret_line as i64, document.caret_column as i64)
        };
        let lines = self.document().lines.len() as i64;
        let line = (line + delta_line).clamp(0, (lines - 1).max(0)) as usize;
        let length = self.document().line_chars(line) as i64;
        let column = if delta_line != 0 {
            column.min(length)
        } else {
            (column + delta_column).clamp(0, length)
        };
        let document = self.document_mut();
        document.caret_line = line;
        document.caret_column = column as usize;
        document.selection_anchor = None;
        self.clamp_caret();
        self.reveal_caret();
    }

    /// Put the caret at the start or the end of its line.
    pub fn move_to_line_end(&mut self, end: bool) {
        let line = self.document().caret_line;
        let length = self.document().line_chars(line);
        let document = self.document_mut();
        document.caret_column = if end { length } else { 0 };
        document.selection_anchor = None;
        self.clamp_caret();
        self.reveal_caret();
    }

    /// Select the whole document.
    pub fn select_all(&mut self) {
        let (last_line, last_column) = {
            let document = self.document();
            let last = document.lines.len().saturating_sub(1);
            (last, document.line_chars(last))
        };
        let document = self.document_mut();
        document.selection_anchor = Some((0, 0));
        document.caret_line = last_line;
        document.caret_column = last_column;
    }

    /// Write the active document back to its path.
    pub fn save(&mut self) -> Result<PathBuf, String> {
        let Some(path) = self.document().path.clone() else {
            return Err("Save the file with Save As first".into());
        };
        let text = self.document().text();
        std::fs::write(&path, format!("{text}\n"))
            .map_err(|error| format!("Could not save {}: {error}", path.display()))?;
        self.document_mut().dirty = false;
        Ok(path)
    }

    /// Move the caret to a document position, for example from a click.
    pub fn set_caret(&mut self, line: usize, column: usize) {
        let line = line.min(self.document().lines.len().saturating_sub(1));
        let column = column.min(self.document().line_chars(line));
        let document = self.document_mut();
        document.caret_line = line;
        document.caret_column = column;
        document.selection_anchor = None;
        self.clamp_caret();
        self.reveal_caret();
    }

    /// Scroll so the caret is inside the viewport.
    pub fn reveal_caret(&mut self) {
        let rows = self.visible_rows();
        let line = self.document().caret_line;
        if line < self.scroll_lines {
            self.scroll_lines = line;
        } else if line >= self.scroll_lines + rows {
            self.scroll_lines = line + 1 - rows;
        }
        let column = self.document().caret_column;
        let width = self.viewport.width.max(1);
        if column < self.scroll_columns {
            self.scroll_columns = column;
        } else if column >= self.scroll_columns + width {
            self.scroll_columns = column + 1 - width;
        }
    }

    /// Scroll the view by a number of rows.
    pub fn scroll_rows(&mut self, rows: i64) {
        let last = self.document().lines.len().saturating_sub(1);
        let maximum = last.saturating_sub(self.visible_rows().saturating_sub(1));
        let next = self.scroll_lines as i64 + rows;
        self.scroll_lines = next.clamp(0, maximum as i64) as usize;
    }

    /// Scroll the view sideways, in columns.
    pub fn scroll_columns(&mut self, columns: i64) {
        let longest = self
            .document()
            .lines
            .iter()
            .map(|line| line.chars().count())
            .max()
            .unwrap_or(0);
        let maximum = longest.saturating_sub(self.viewport.width.max(1));
        let next = self.scroll_columns as i64 + columns;
        self.scroll_columns = next.clamp(0, maximum as i64) as usize;
    }

    /// Step the shared motion models once per frame.
    pub fn tick(&mut self) {
        let now = Instant::now();
        let dt = now.duration_since(self.last_tick).as_secs_f32().min(0.05);
        self.last_tick = now;
        let target = if self.dirty() { 1.0 } else { 0.0 };
        self.caret_spring.step(target, dt);
    }

    /// The documement line under a point, given the metric of the code area.
    pub fn line_at(&self, y: f64, body_top: f64, line_step: f64) -> usize {
        let step = if line_step > 0.0 { line_step } else { 1.0 };
        let row = ((y - body_top) / step).floor().max(0.0) as usize;
        (self.scroll_lines + row).min(self.document().lines.len().saturating_sub(1))
    }

    /// The column under a point, given the metric of the code area.
    pub fn column_at(&self, x: f64, text_left: f64, cell: f64, line: usize) -> usize {
        let cell = if cell > 0.0 { cell } else { 1.0 };
        let column = ((x - text_left) / cell).floor().max(0.0) as usize;
        (column + self.scroll_columns).min(self.document().line_chars(line))
    }

    /// A snapshot of the caret, for the backends that draw it.
    pub fn caret(&self) -> (usize, usize) {
        let document = self.document();
        (document.caret_line, document.caret_column)
    }

    fn selection_anchor_is_set(&self) -> bool {
        self.document().selection_anchor.is_some()
    }

    /// Keep the caret inside the document.
    ///
    /// One line past the last one is allowed while the text ends with a newline,
    /// which is where the caret sits after Enter at the end of the file.
    fn clamp_caret(&mut self) {
        let line_count = self.document().lines.len();
        let document = self.document_mut();
        if document.caret_line >= line_count {
            if document.caret_line == line_count && document.buffer.ends_with_newline() {
                document.caret_column = 0;
                return;
            }
            document.caret_line = line_count.saturating_sub(1);
        }
        let line = document.caret_line;
        let length = document.line_chars(line);
        document.caret_column = document.caret_column.min(length);
    }

    fn mark_edited(&mut self) {
        self.document_mut().sync();
        self.document_mut().dirty = true;
        self.clamp_caret();
    }
}

/// The metric a click is mapped through, so a check can use the same numbers a
/// backend uses.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClickMetrics {
    pub gutter: f64,
    pub cell: f64,
    pub line_step: f64,
    pub header: f64,
}

/// Assertions about the session that every backend can run.
///
/// The macOS `--self-check` runs this on the runner, and a unit test runs it
/// here, so the same expectations are checked in both places and a failure shows
/// up in a second instead of on a machine nobody can read the log of.
pub fn state_check(metrics: ClickMetrics) -> Vec<String> {
    let mut failures = Vec::new();
    let mut check = |name: &str, ok: bool| {
        if !ok {
            failures.push(name.to_string());
        }
    };

    let mut session = EditorSession::new(Document::from_lines("check.rs", None, Vec::new()));
    session.replace_all("one\ntwo");
    check(
        "a scratch document holds its lines",
        session.document().lines == ["one", "two"] && session.caret() == (1, 3),
    );

    session.backspace();
    check(
        "backspace removes a character",
        session.document().lines == ["one", "tw"],
    );

    session.move_caret(-1, 0);
    session.move_to_line_end(true);
    session.insert_text("X");
    check(
        "the caret moves between lines",
        session.document().lines == ["oneX", "tw"],
    );
    check("an edit marks the document dirty", session.dirty());

    // A click inside the second line lands on column one of it.
    session.viewport = Viewport::new(20, 80);
    let line = session.line_at(
        metrics.header + metrics.line_step * 1.5,
        metrics.header,
        metrics.line_step,
    );
    let column = session.column_at(
        metrics.gutter + metrics.cell * 1.5,
        metrics.gutter,
        metrics.cell,
        line,
    );
    check("a click maps to the row it points at", line == 1);
    check("a click maps to the column it points at", column == 1);
    session.set_caret(line, column);
    check("the caret follows the click", session.caret() == (1, 1));

    // A click past the end of a short line stops at the line end.
    let far = session.column_at(
        metrics.gutter + metrics.cell * 40.0,
        metrics.gutter,
        metrics.cell,
        line,
    );
    session.set_caret(line, far);
    check(
        "a click past the line end stops at the line end",
        session.caret() == (1, 2),
    );

    // Select all then type replaces the document.
    session.select_all();
    session.insert_text("done");
    check(
        "typing over a selection replaces it",
        session.document().lines == ["done"] && session.caret() == (0, 4),
    );
    check(
        "the edit is reported for a highlight",
        matches!(
            session.last_edit,
            Some(EditRecord {
                kind: EditKind::Replace,
                line: 0,
                start: 0,
                end: 4,
            })
        ),
    );

    // Tabs keep their own caret and their own dirty mark.
    session.add_document(Document::from_lines("second.rs", None, vec!["two".into()]));
    session.insert_text("!");
    // Both documents carry an edit by now, so both show the dirty mark.
    check(
        "a second document is a tab",
        session.titles() == ["check.rs •", "second.rs •"],
    );
    check(
        "switching documents works",
        session.activate(0) && session.caret() == (0, 4),
    );
    check(
        "a document keeps its own text",
        session.activate(1) && session.document().lines == ["!two"],
    );

    failures
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(text: &str) -> EditorSession {
        EditorSession::new(Document::from_lines("main.rs", None, Vec::new())).tap(text)
    }

    trait Tap {
        fn tap(self, text: &str) -> Self;
    }

    impl Tap for EditorSession {
        fn tap(mut self, text: &str) -> Self {
            self.replace_all(text);
            self
        }
    }

    #[test]
    fn typing_replaces_a_selection_and_keeps_the_lines_in_step() {
        let mut session = scratch("hello\nworld");
        assert_eq!(session.document().lines, ["hello", "world"]);
        assert_eq!(session.caret(), (1, 5));

        session.select_all();
        session.insert_text("done");
        assert_eq!(session.document().lines, ["done"]);
        assert_eq!(session.caret(), (0, 4));
        assert!(session.dirty());
        match session.last_edit {
            Some(edit) => {
                assert_eq!(edit.kind, EditKind::Replace);
                assert_eq!((edit.line, edit.start, edit.end), (0, 0, 4));
            }
            None => panic!("the session should report the edit"),
        }
        assert_eq!(session.document().text(), "done");
    }

    #[test]
    fn backspace_and_forward_delete_join_lines_at_the_edges() {
        let mut session = scratch("hello\nworld");
        // Backspace at the start of a line joins it with the one above.
        session.move_to_line_end(false);
        session.backspace();
        assert_eq!(session.document().lines, ["helloworld"]);
        assert_eq!(session.caret(), (0, 5));

        // Backspace at the very start of the document does nothing.
        session.set_caret(0, 0);
        session.backspace();
        assert_eq!(session.document().lines, ["helloworld"]);
        assert_eq!(session.caret(), (0, 0));

        // Forward delete takes the character after the caret.
        session.delete_forward();
        assert_eq!(session.document().lines, ["elloworld"]);
        // At the end of the last line there is nothing to take.
        session.move_to_line_end(true);
        session.delete_forward();
        assert_eq!(session.document().lines, ["elloworld"]);

        // A selection deletes before anything is typed or removed.
        session.set_caret(0, 1);
        session.select_all();
        session.delete_forward();
        assert_eq!(session.document().lines, [""]);
        assert_eq!(session.caret(), (0, 0));
    }

    #[test]
    fn a_vertical_move_stays_inside_the_target_line() {
        let mut session = scratch("a long line\ns\nanother");
        session.set_caret(0, 11);
        assert_eq!(session.caret(), (0, 11));
        session.move_caret(1, 0);
        assert_eq!(session.caret(), (1, 1), "the column is clamped");
        session.move_caret(1, 0);
        assert_eq!(
            session.caret(),
            (2, 1),
            "the column follows the caret down and stays inside the line"
        );
        session.move_caret(1, 0);
        assert_eq!(session.caret(), (2, 1), "the caret stops at the last line");
        session.move_caret(-1, 0);
        assert_eq!(session.caret(), (1, 1));
        session.move_to_line_end(true);
        assert_eq!(session.caret(), (1, 1));
        session.move_to_line_end(false);
        assert_eq!(session.caret(), (1, 0));
        session.move_caret(0, -5);
        assert_eq!(session.caret(), (1, 0), "a horizontal move is clamped too");
    }

    #[test]
    fn typing_splits_lines_and_the_caret_lands_after_the_text() {
        let mut session = scratch("start");
        session.move_to_line_end(true);
        session.insert_text("\nsecond\nthird");
        assert_eq!(session.document().lines, ["start", "second", "third"]);
        assert_eq!(session.caret(), (2, 5));
        session.insert_text("\n");
        // A trailing newline does not add a line to the list; the caret waits on
        // the line after the last newline, and the next character lands there.
        assert_eq!(session.document().lines, ["start", "second", "third"]);
        assert_eq!(session.caret(), (3, 0));
        assert!(session.dirty());
        session.insert_text("x");
        assert_eq!(session.document().lines, ["start", "second", "third", "x"]);
        assert_eq!(session.caret(), (3, 1));
    }

    #[test]
    fn the_viewport_follows_the_caret_and_scrolls_back() {
        let mut session = scratch("a\nb\nc\nd\ne\nf\ng\nh");
        session.viewport = Viewport::new(4, 20);
        for _ in 0..8 {
            session.move_caret(-1, 0);
        }
        assert_eq!(session.caret(), (0, 1));
        assert_eq!(session.scroll_lines, 0);
        for line in 1..=7 {
            session.move_caret(1, 0);
            assert_eq!(session.caret().0, line);
            assert_eq!(session.scroll_lines, line.saturating_sub(3));
        }
        for _ in 0..7 {
            session.move_caret(-1, 0);
        }
        assert_eq!(session.caret(), (0, 1));
        assert_eq!(session.scroll_lines, 0);

        // Scrolling by hand stays inside the document.
        session.scroll_rows(40);
        assert_eq!(session.scroll_lines, 8 - 4);
        session.scroll_rows(-40);
        assert_eq!(session.scroll_lines, 0);
        session.scroll_columns(40);
        assert_eq!(
            session.scroll_columns, 0,
            "one column wide lines cannot scroll"
        );
    }

    #[test]
    fn a_click_maps_to_a_line_and_column() {
        let mut session = scratch("hello\nworld\nagain");
        session.viewport = Viewport::new(20, 80);
        let line = session.line_at(46.0 + 18.0 * 2.5, 46.0, 18.0);
        assert_eq!(line, 2);
        assert_eq!(session.column_at(54.0 + 7.4 * 3.5, 54.0, 7.4, line), 3);
        // A click above the text area lands on the first visible row.
        assert_eq!(session.line_at(10.0, 46.0, 18.0), 0);
        // With the view scrolled, the same point maps a row lower.
        session.scroll_lines = 1;
        assert_eq!(session.line_at(46.0 + 18.0 * 0.5, 46.0, 18.0), 1);
        assert_eq!(session.column_at(54.0, 54.0, 7.4, 1), 0);
        session.set_caret(1, 3);
        assert_eq!(session.caret(), (1, 3));
        session.set_caret(99, 99);
        assert_eq!(
            session.caret(),
            (2, 5),
            "the caret stays inside the document"
        );
    }

    #[test]
    fn a_click_past_the_line_end_stops_at_the_line_end() {
        // The sequence the macOS state check walks through, so a change to the
        // mapping is caught here instead of on the macOS runner.
        let mut session = scratch("one\ntwo");
        session.viewport = Viewport::new(20, 80);
        session.backspace();
        session.move_caret(-1, 0);
        session.move_to_line_end(true);
        session.insert_text("X");
        assert_eq!(session.document().lines, ["oneX", "tw"]);

        let line = session.line_at(46.0 + 18.0 * 1.5, 46.0, 18.0);
        assert_eq!(line, 1);
        let column = session.column_at(54.0 + 7.4 * 1.5, 54.0, 7.4, line);
        assert_eq!(column, 1);
        session.set_caret(line, column);
        assert_eq!(session.caret(), (1, 1));

        // Forty columns into a two character line is still column two.
        let far = session.column_at(54.0 + 7.4 * 40.0, 54.0, 7.4, line);
        assert_eq!(far, 2);
        session.set_caret(line, far);
        assert_eq!(session.caret(), (1, 2));
    }

    #[test]
    fn documents_are_kept_as_tabs_with_their_dirty_mark() {
        let mut session = scratch("one");
        session.add_document(Document::from_lines("other.rs", None, vec!["two".into()]));
        assert_eq!(session.documents().len(), 2);
        assert_eq!(session.active(), 1);
        assert_eq!(session.titles(), ["main.rs", "other.rs"]);
        session.insert_text("!");
        assert_eq!(session.titles(), ["main.rs", "other.rs •"]);
        assert!(session.activate(0));
        assert_eq!(session.caret(), (0, 3), "each document keeps its own caret");
        assert_eq!(session.scroll_lines, 0, "switching resets the scroll");
        assert!(!session.activate(9));
        assert!(session.close_document(1));
        assert_eq!(session.documents().len(), 1);
        assert!(!session.close_document(0), "the last document stays open");
    }

    #[test]
    fn saving_writes_the_text_and_clears_the_dirty_mark() {
        let directory = std::env::temp_dir().join("tc-ide-session-test");
        let _ = std::fs::create_dir_all(&directory);
        let path = directory.join("saved.txt");
        let mut session = EditorSession::new(Document::from_lines(
            "saved.txt",
            Some(path.clone()),
            vec!["first".into()],
        ));
        session.insert_text("second");
        assert!(session.dirty());
        assert_eq!(session.save().expect("save"), path);
        assert!(!session.dirty());
        assert_eq!(
            std::fs::read_to_string(&path).expect("read back"),
            "secondfirst\n"
        );

        let mut unsaved = scratch("nothing");
        unsaved.insert_text("edited");
        assert!(unsaved.save().is_err());
        assert!(unsaved.dirty(), "a failed save keeps the dirty mark");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn the_shared_state_check_passes_where_it_runs() {
        // The macOS runner runs exactly this through `--self-check`; running it
        // here as well means a change is caught in a second, not in four minutes.
        let failures = state_check(ClickMetrics {
            gutter: 54.0,
            cell: 7.4,
            line_step: 18.0,
            header: 46.0,
        });
        assert!(failures.is_empty(), "{failures:?}");
    }
}
