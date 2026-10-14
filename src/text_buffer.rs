//! Shared document buffer.
//!
//! The editors used `Vec<String>`, which copies the whole document on every
//! undo step and reallocates on every edit in the middle of a large file. This
//! module keeps the text as a piece table — the original text plus an
//! append-only buffer of inserted text, addressed by a list of pieces — so an
//! edit only appends new text and rewrites the piece list, and undo/redo stores
//! the edited range instead of a document snapshot.
//!
//! Everything here is plain data with unit tests, including the Unicode rules
//! the editors rely on.
#![allow(dead_code)] // The backends adopt the pieces they need as they migrate.

/// Lines kept in an edit history entry.
const MAX_HISTORY: usize = 500;

/// One piece of the document: a byte range of one of the two sources.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Piece {
    /// `false` for the original text, `true` for the append buffer.
    added: bool,
    start: usize,
    length: usize,
}

/// An edit that can be applied or reverted.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Edit {
    Insert {
        offset: usize,
        text: String,
    },
    Delete {
        offset: usize,
        text: String,
    },
    /// A replacement, undoable as one step like the user performed it.
    Replace {
        offset: usize,
        removed: String,
        inserted: String,
    },
    /// Several edits the user performed as one action (typing, pasting).
    Group(Vec<Edit>),
}

/// Text of a document, stored as a piece table.
#[derive(Clone, Debug, Default)]
pub struct TextBuffer {
    original: String,
    added: String,
    pieces: Vec<Piece>,
    history: Vec<Edit>,
    future: Vec<Edit>,
    /// Cached length in bytes of the document.
    length: usize,
    /// Edits collected while a group is open.
    open_group: Option<Vec<Edit>>,
}

impl TextBuffer {
    /// Build a buffer from a document.
    pub fn from_text(text: &str) -> Self {
        let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
        let length = normalized.len();
        let pieces = if normalized.is_empty() {
            Vec::new()
        } else {
            vec![Piece {
                added: false,
                start: 0,
                length,
            }]
        };
        Self {
            original: normalized,
            added: String::new(),
            pieces,
            history: Vec::new(),
            future: Vec::new(),
            length,
            open_group: None,
        }
    }

    /// Lines of a document, the shape editors and tests use.
    pub fn from_lines(lines: &[String]) -> Self {
        Self::from_text(&lines.join("\n"))
    }

    /// Document length in bytes.
    pub fn len(&self) -> usize {
        self.length
    }

    pub fn is_empty(&self) -> bool {
        self.length == 0
    }

    /// Rebuild the document text.
    pub fn to_text(&self) -> String {
        let mut text = String::with_capacity(self.length);
        for piece in &self.pieces {
            let source = if piece.added {
                &self.added
            } else {
                &self.original
            };
            if let Some(slice) = source.get(piece.start..piece.start + piece.length) {
                text.push_str(slice);
            }
        }
        text
    }

    /// Document lines, without a trailing empty line caused by a final newline.
    pub fn to_lines(&self) -> Vec<String> {
        let text = self.to_text();
        let mut lines = text.split('\n').map(str::to_string).collect::<Vec<_>>();
        if lines.len() > 1 && lines.last().map(String::is_empty).unwrap_or(false) {
            lines.pop();
        }
        if lines.is_empty() {
            lines.push(String::new());
        }
        lines
    }

    /// Number of lines in the document.
    pub fn line_count(&self) -> usize {
        self.to_lines().len()
    }

    /// Text of one line, without its newline.
    pub fn line(&self, index: usize) -> String {
        self.to_lines().get(index).cloned().unwrap_or_default()
    }

    /// Character count of one line.
    pub fn line_chars(&self, index: usize) -> usize {
        self.line(index).chars().count()
    }

    /// Byte offset of a `(line, column)` position, clamped into the document.
    pub fn offset_of(&self, line: usize, column: usize) -> usize {
        let mut offset = 0usize;
        let mut current_line = 0usize;
        for character in self.to_text().chars() {
            if current_line == line {
                break;
            }
            offset += character.len_utf8();
            if character == '\n' {
                current_line += 1;
                if current_line == line {
                    break;
                }
            }
        }
        let text = self.to_text();
        let line_start = offset.min(text.len());
        let line_end = text[line_start..]
            .find('\n')
            .map(|position| line_start + position)
            .unwrap_or(text.len());
        let mut column_offset = line_start;
        for (count, character) in text[line_start..line_end].char_indices() {
            if count == 0 && column == 0 {
                break;
            }
            if text[line_start..line_start + count].chars().count() >= column {
                break;
            }
            column_offset = line_start + count + character.len_utf8();
        }
        column_offset.min(line_end)
    }

    /// Position of a byte offset, as `(line, column)` in characters.
    pub fn position_of(&self, offset: usize) -> (usize, usize) {
        let text = self.to_text();
        let offset = offset.min(text.len());
        let prefix = &text[..offset];
        let line = prefix.matches('\n').count();
        let line_start = prefix.rfind('\n').map(|position| position + 1).unwrap_or(0);
        (line, text[line_start..offset].chars().count())
    }

    /// Insert text at a position, splitting characters correctly.
    pub fn insert(&mut self, line: usize, column: usize, text: &str) -> usize {
        let offset = self.offset_of(line, column);
        self.insert_at(offset, text)
    }

    /// Insert text at a byte offset.
    pub fn insert_at(&mut self, offset: usize, text: &str) -> usize {
        if text.is_empty() {
            return offset;
        }
        let offset = offset.min(self.length);
        let start = self.added.len();
        self.added.push_str(text);
        let (index, _) = self.split_at(offset);
        self.pieces.insert(
            index,
            Piece {
                added: true,
                start,
                length: text.len(),
            },
        );
        self.length += text.len();
        self.record(Edit::Insert {
            offset,
            text: text.to_string(),
        });
        offset + text.len()
    }

    /// Delete a range given as two positions, returning the removed text.
    pub fn delete(&mut self, start: (usize, usize), end: (usize, usize)) -> String {
        let start_offset = self.offset_of(start.0, start.1);
        let end_offset = self.offset_of(end.0, end.1);
        self.delete_range(start_offset, end_offset)
    }

    /// Delete a byte range, returning the removed text.
    pub fn delete_range(&mut self, start: usize, end: usize) -> String {
        if start >= end || start >= self.length {
            return String::new();
        }
        let end = end.min(self.length);
        let removed = self
            .to_text()
            .get(start..end)
            .unwrap_or_default()
            .to_string();
        let (start_index, start_inside) = self.split_at(start);
        let (end_index, end_inside) = self.split_at(end);
        let remove_from = start_index + usize::from(start_inside);
        let remove_to = end_index + usize::from(end_inside);
        if remove_from < remove_to {
            self.pieces.drain(remove_from..remove_to);
        }
        self.length -= removed.len();
        self.record(Edit::Delete {
            offset: start,
            text: removed.clone(),
        });
        removed
    }

    /// Replace a range with new text, undoable as a single edit.
    pub fn replace(&mut self, start: (usize, usize), end: (usize, usize), text: &str) -> String {
        let start_offset = self.offset_of(start.0, start.1);
        let end_offset = self.offset_of(end.0, end.1);
        let removed = self
            .to_text()
            .get(start_offset..end_offset.max(start_offset))
            .unwrap_or_default()
            .to_string();
        // Apply both halves without touching the history, then record one edit.
        let history_depth = self.history.len();
        self.remove_raw(start_offset, end_offset.max(start_offset));
        if !text.is_empty() {
            self.insert_raw(start_offset, text);
        }
        self.history.truncate(history_depth);
        self.record(Edit::Replace {
            offset: start_offset,
            removed: removed.clone(),
            inserted: text.to_string(),
        });
        removed
    }

    /// Undo the last edit; returns whether anything changed.
    pub fn undo(&mut self) -> bool {
        let Some(edit) = self.history.pop() else {
            return false;
        };
        if let Edit::Group(edits) = &edit {
            for inner in edits.iter().rev() {
                self.revert(inner);
            }
            self.future.push(edit);
            return true;
        }
        self.revert(&edit);
        self.future.push(edit);
        true
    }

    /// Revert one edit without touching the history.
    fn revert(&mut self, edit: &Edit) {
        match edit {
            Edit::Insert { offset, text } => {
                self.remove_raw(*offset, offset + text.len());
            }
            Edit::Delete { offset, text } => {
                self.insert_raw(*offset, text);
            }
            Edit::Replace {
                offset,
                removed,
                inserted,
            } => {
                self.remove_raw(*offset, offset + inserted.len());
                if !removed.is_empty() {
                    self.insert_raw(*offset, removed);
                }
            }
            Edit::Group(edits) => {
                for inner in edits.iter().rev() {
                    self.revert(inner);
                }
            }
        }
    }

    /// Redo the last undone edit.
    pub fn redo(&mut self) -> bool {
        let Some(edit) = self.future.pop() else {
            return false;
        };
        if let Edit::Group(edits) = &edit {
            for inner in edits.iter() {
                self.apply(inner);
            }
            self.history.push(edit);
            return true;
        }
        self.apply(&edit);
        self.history.push(edit);
        true
    }

    /// Apply one edit without touching the history.
    fn apply(&mut self, edit: &Edit) {
        match edit {
            Edit::Insert { offset, text } => {
                self.insert_raw(*offset, text);
            }
            Edit::Delete { offset, text } => {
                self.remove_raw(*offset, offset + text.len());
            }
            Edit::Replace {
                offset,
                removed,
                inserted,
            } => {
                self.remove_raw(*offset, offset + removed.len());
                if !inserted.is_empty() {
                    self.insert_raw(*offset, inserted);
                }
            }
            Edit::Group(edits) => {
                for inner in edits.iter() {
                    self.apply(inner);
                }
            }
        }
    }

    /// Start grouping edits so a burst of typing undoes as one step.
    ///
    /// Nested calls are ignored: the outermost pair owns the group.
    pub fn begin_group(&mut self) {
        if self.open_group.is_none() {
            self.open_group = Some(Vec::new());
        }
    }

    /// Close the group opened by `begin_group`.
    pub fn end_group(&mut self) {
        let Some(edits) = self.open_group.take() else {
            return;
        };
        if edits.is_empty() {
            return;
        }
        if edits.len() == 1 {
            if let Some(edit) = edits.into_iter().next() {
                self.history.push(edit);
            }
        } else {
            self.history.push(Edit::Group(edits));
        }
        if self.history.len() > MAX_HISTORY {
            self.history.remove(0);
        }
    }

    /// Whether edits are currently being grouped.
    pub fn grouping(&self) -> bool {
        self.open_group.is_some()
    }

    /// How many edits can be undone.
    pub fn undo_depth(&self) -> usize {
        self.history.len()
    }

    pub fn redo_depth(&self) -> usize {
        self.future.len()
    }

    /// Record an edit, either into the open group or straight into the history.
    fn record(&mut self, edit: Edit) {
        match self.open_group.as_mut() {
            Some(group) => group.push(edit),
            None => {
                self.history.push(edit);
                if self.history.len() > MAX_HISTORY {
                    self.history.remove(0);
                }
                self.future.clear();
            }
        }
    }

    fn insert_raw(&mut self, offset: usize, text: &str) {
        let start = self.added.len();
        self.added.push_str(text);
        let (index, _) = self.split_at(offset);
        self.pieces.insert(
            index,
            Piece {
                added: true,
                start,
                length: text.len(),
            },
        );
        self.length += text.len();
    }

    fn remove_raw(&mut self, start: usize, end: usize) {
        if start >= end || start >= self.length {
            return;
        }
        let end = end.min(self.length);
        let (start_index, start_inside) = self.split_at(start);
        let (end_index, end_inside) = self.split_at(end);
        let remove_from = start_index + usize::from(start_inside);
        let remove_to = end_index + usize::from(end_inside);
        if remove_from < remove_to {
            self.pieces.drain(remove_from..remove_to);
        }
        self.length -= end - start;
    }

    /// Split the piece list at a byte offset so the offset starts a piece.
    fn split_at(&mut self, offset: usize) -> (usize, bool) {
        let mut position = 0usize;
        for index in 0..self.pieces.len() {
            let piece = self.pieces[index].clone();
            if position == offset {
                return (index, false);
            }
            if offset < position + piece.length {
                let inside = offset - position;
                let left = Piece {
                    added: piece.added,
                    start: piece.start,
                    length: inside,
                };
                let right = Piece {
                    added: piece.added,
                    start: piece.start + inside,
                    length: piece.length - inside,
                };
                self.pieces[index] = left;
                self.pieces.insert(index + 1, right);
                return (index + 1, false);
            }
            position += piece.length;
        }
        (self.pieces.len(), false)
    }

    /// Number of pieces, exposed so tests can prove edits stay local.
    pub fn piece_count(&self) -> usize {
        self.pieces.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    #[test]
    fn a_new_buffer_round_trips_the_document() {
        let buffer = TextBuffer::from_text("one\ntwo\nthree");
        assert_eq!(buffer.to_text(), "one\ntwo\nthree");
        assert_eq!(buffer.to_lines(), lines(&["one", "two", "three"]));
        assert_eq!(buffer.line_count(), 3);
        assert_eq!(buffer.len(), 13);
        assert_eq!(buffer.undo_depth(), 0);

        let empty = TextBuffer::from_text("");
        assert!(empty.is_empty());
        assert_eq!(empty.to_lines(), lines(&[""]));
        assert_eq!(empty.line(5), "");
    }

    #[test]
    fn carriage_returns_are_normalized_once() {
        let buffer = TextBuffer::from_text("one\r\ntwo\rthree\n");
        assert_eq!(buffer.to_lines(), lines(&["one", "two", "three"]));
        assert!(!buffer.to_text().contains('\r'));
    }

    #[test]
    fn inserts_and_deletes_keep_the_document_consistent() {
        let mut buffer = TextBuffer::from_text("alpha\nbeta");
        buffer.insert(0, 5, "!");
        assert_eq!(buffer.to_lines(), lines(&["alpha!", "beta"]));
        buffer.insert(1, 0, "> ");
        assert_eq!(buffer.to_lines(), lines(&["alpha!", "> beta"]));
        buffer.insert(0, 0, "start ");
        assert_eq!(buffer.to_lines(), lines(&["start alpha!", "> beta"]));

        let removed = buffer.delete((0, 0), (0, 6));
        assert_eq!(removed, "start ");
        assert_eq!(buffer.to_lines(), lines(&["alpha!", "> beta"]));

        // Deleting across lines joins them; column 5 is the "!" itself.
        let removed = buffer.delete((0, 5), (1, 2));
        assert_eq!(removed, "!\n> ");
        assert_eq!(buffer.to_lines(), lines(&["alphabeta"]));

        // Out of range deletes are ignored rather than panicking.
        assert_eq!(buffer.delete_range(100, 200), "");
        assert_eq!(buffer.to_lines(), lines(&["alphabeta"]));
    }

    #[test]
    fn unicode_positions_stay_on_character_boundaries() {
        let mut buffer = TextBuffer::from_text("你好 world");
        assert_eq!(buffer.line_chars(0), 8);
        buffer.insert(0, 2, "真的");
        assert_eq!(buffer.to_lines(), lines(&["你好真的 world"]));
        buffer.delete((0, 0), (0, 2));
        assert_eq!(buffer.to_lines(), lines(&["真的 world"]));

        let emoji = TextBuffer::from_text("a🙂b");
        assert_eq!(emoji.line_chars(0), 3);
        assert_eq!(emoji.offset_of(0, 1), 1);
        assert_eq!(emoji.offset_of(0, 2), 5);
        assert_eq!(emoji.position_of(5), (0, 2));
    }

    #[test]
    fn offsets_and_positions_round_trip() {
        let buffer = TextBuffer::from_text("one\ntwo\n\nfour");
        for (line, column) in [(0, 0), (0, 3), (1, 2), (2, 0), (3, 4)] {
            let offset = buffer.offset_of(line, column);
            assert_eq!(buffer.position_of(offset), (line, column));
        }
        // Columns beyond the line end clamp to the line end.
        let offset = buffer.offset_of(1, 99);
        assert_eq!(buffer.position_of(offset), (1, 3));
    }

    #[test]
    fn undo_and_redo_walk_the_history_without_snapshots() {
        let mut buffer = TextBuffer::from_text("hello");
        buffer.insert(0, 5, " world");
        buffer.insert(0, 11, "!");
        assert_eq!(buffer.to_text(), "hello world!");
        assert_eq!(buffer.undo_depth(), 2);

        assert!(buffer.undo());
        assert_eq!(buffer.to_text(), "hello world");
        assert!(buffer.undo());
        assert_eq!(buffer.to_text(), "hello");
        assert!(!buffer.undo());
        assert_eq!(buffer.redo_depth(), 2);

        assert!(buffer.redo());
        assert_eq!(buffer.to_text(), "hello world");
        assert!(buffer.redo());
        assert_eq!(buffer.to_text(), "hello world!");
        assert!(!buffer.redo());

        // A delete is undone by re-inserting the removed text.
        let removed = buffer.delete((0, 0), (0, 6));
        assert_eq!(removed, "hello ");
        assert_eq!(buffer.to_text(), "world!");
        assert!(buffer.undo());
        assert_eq!(buffer.to_text(), "hello world!");
    }

    #[test]
    fn grouped_edits_undo_as_one_step() {
        let mut buffer = TextBuffer::from_text("");
        buffer.begin_group();
        for character in "hello".chars() {
            buffer.insert_at(buffer.len(), &character.to_string());
        }
        buffer.end_group();
        assert_eq!(buffer.to_text(), "hello");
        assert_eq!(buffer.undo_depth(), 1);
        assert!(buffer.undo());
        assert_eq!(buffer.to_text(), "");
        assert!(buffer.redo());
        assert_eq!(buffer.to_text(), "hello");

        // An empty group records nothing.
        buffer.begin_group();
        buffer.end_group();
        assert_eq!(buffer.undo_depth(), 1);

        // A group of one edit is stored directly.
        buffer.begin_group();
        buffer.insert_at(0, "x");
        buffer.end_group();
        assert_eq!(buffer.undo_depth(), 2);
        assert!(!buffer.grouping());
    }

    #[test]
    fn a_new_edit_drops_the_redo_stack() {
        let mut buffer = TextBuffer::from_text("abc");
        buffer.insert(0, 3, "d");
        buffer.undo();
        assert_eq!(buffer.redo_depth(), 1);
        buffer.insert(0, 0, "x");
        assert_eq!(buffer.redo_depth(), 0);
        assert_eq!(buffer.to_text(), "xabc");
    }

    #[test]
    fn repeated_edits_keep_the_piece_list_bounded_enough() {
        let mut buffer = TextBuffer::from_text("start");
        for index in 0..200 {
            buffer.insert(0, 0, &format!("{index},"));
        }
        let text = buffer.to_text();
        assert!(text.starts_with("199,198,"));
        assert!(text.ends_with("start"));
        // Editing only appends pieces and splits one, so the list grows linearly
        // with the number of edits rather than copying the document.
        assert!(buffer.piece_count() <= 200 + 1);
    }

    #[test]
    fn large_documents_stay_responsive() {
        let mut lines = Vec::with_capacity(50_000);
        for index in 0..50_000 {
            lines.push(format!("line {index} with some content"));
        }
        let mut buffer = TextBuffer::from_lines(&lines);
        assert_eq!(buffer.line_count(), 50_000);

        let started = std::time::Instant::now();
        for index in 0..200 {
            buffer.insert(index, 0, "x");
        }
        let elapsed = started.elapsed();
        assert_eq!(buffer.line_count(), 50_000);
        assert!(
            elapsed.as_secs_f32() < 2.0,
            "200 edits took {elapsed:?}, which suggests a document copy per edit"
        );

        let started = std::time::Instant::now();
        let text = buffer.to_text();
        assert!(text.contains("xline 199"));
        assert!(started.elapsed().as_secs_f32() < 2.0);
    }

    #[test]
    fn replacing_a_range_returns_the_previous_text() {
        let mut buffer = TextBuffer::from_text("fn main() {\n}\n");
        let removed = buffer.replace((0, 3), (0, 7), "start");
        assert_eq!(removed, "main");
        assert_eq!(buffer.to_lines(), lines(&["fn start() {", "}"]));
        // A replacement is one undo step, like the user typed it.
        assert_eq!(buffer.undo_depth(), 1);
        assert!(buffer.undo());
        assert_eq!(buffer.to_lines(), lines(&["fn main() {", "}"]));
        assert!(buffer.redo());
        assert_eq!(buffer.to_lines(), lines(&["fn start() {", "}"]));
    }
}
