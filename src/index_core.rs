//! Shared incremental project index.
//!
//! Re-parsing every file of a project on every edit does not scale, and the
//! editors only ever change one file at a time. This module keeps the symbol
//! occurrences of each file together with a fingerprint of the file it parsed,
//! so a refresh re-parses exactly the files whose fingerprint changed, drops the
//! files that disappeared and reuses everything else.
//!
//! The file contents are supplied through a reader closure, so the policy —
//! what counts as changed, what happens to a file that cannot be read — is
//! testable without touching a disk.
#![allow(dead_code)] // The backends adopt the pieces they need as they migrate.

use std::collections::BTreeMap;

use crate::editor_core::{self, ProjectSymbolOccurrence};

/// Identity of a file version, cheap enough to compute while scanning.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileFingerprint {
    /// Path relative to the project root, with `/` separators.
    pub path: String,
    /// Size in bytes.
    pub bytes: u64,
    /// Modification time in milliseconds since the Unix epoch, when known.
    pub modified: u64,
}

impl FileFingerprint {
    pub fn new(path: impl Into<String>, bytes: u64, modified: u64) -> Self {
        Self {
            path: path.into(),
            bytes,
            modified,
        }
    }
}

/// What a refresh did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct IndexStats {
    /// Files whose symbols were parsed again.
    pub reparsed: usize,
    /// Files whose cached symbols were kept.
    pub reused: usize,
    /// Files dropped because they disappeared.
    pub removed: usize,
    /// Files that could not be read and were not indexed.
    pub skipped: usize,
    /// Symbol occurrences in the index afterwards.
    pub symbols: usize,
}

/// Symbols of every known file, keyed by the file's relative path.
#[derive(Clone, Debug, Default)]
pub struct IncrementalIndex {
    files: BTreeMap<String, (FileFingerprint, Vec<ProjectSymbolOccurrence>)>,
}

impl IncrementalIndex {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn file_count(&self) -> usize {
        self.files.len()
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// All symbol occurrences, in path order.
    pub fn symbols(&self) -> Vec<ProjectSymbolOccurrence> {
        let mut symbols = self
            .files
            .values()
            .flat_map(|(_, symbols)| symbols.iter().cloned())
            .collect::<Vec<_>>();
        symbols.sort_by(|left, right| {
            left.file
                .cmp(&right.file)
                .then_with(|| left.symbol.line.cmp(&right.symbol.line))
        });
        symbols
    }

    /// Symbols of one file, when it is indexed.
    pub fn file_symbols(&self, path: &str) -> Option<&[ProjectSymbolOccurrence]> {
        self.files.get(path).map(|(_, symbols)| symbols.as_slice())
    }

    /// Fingerprint the index holds for a file.
    pub fn fingerprint(&self, path: &str) -> Option<&FileFingerprint> {
        self.files.get(path).map(|(fingerprint, _)| fingerprint)
    }

    /// Reparse only the files whose fingerprint changed.
    pub fn refresh<F>(&mut self, scanned: Vec<FileFingerprint>, mut read: F) -> IndexStats
    where
        F: FnMut(&str) -> Option<Vec<String>>,
    {
        let mut stats = IndexStats::default();
        let mut seen = Vec::with_capacity(scanned.len());
        for fingerprint in scanned {
            seen.push(fingerprint.path.clone());
            let unchanged = self
                .files
                .get(&fingerprint.path)
                .map(|(known, _)| *known == fingerprint)
                .unwrap_or(false);
            if unchanged {
                stats.reused += 1;
                continue;
            }
            let Some(lines) = read(&fingerprint.path) else {
                stats.skipped += 1;
                continue;
            };
            let symbols =
                editor_core::project_symbol_occurrences(&[(fingerprint.path.clone(), lines)]);
            if symbols.is_empty() {
                // Keep the file in the index so a later refresh knows it was seen.
                self.files
                    .insert(fingerprint.path.clone(), (fingerprint, Vec::new()));
            } else {
                self.files
                    .insert(fingerprint.path.clone(), (fingerprint, symbols));
            }
            stats.reparsed += 1;
        }
        let known = self.files.keys().cloned().collect::<Vec<_>>();
        for path in known {
            if !seen.iter().any(|seen| seen == &path) {
                self.files.remove(&path);
                stats.removed += 1;
            }
        }
        stats.symbols = self.files.values().map(|(_, symbols)| symbols.len()).sum();
        stats
    }

    /// Replace the symbols of a single file, for an editor that just changed it.
    pub fn update_file(&mut self, fingerprint: FileFingerprint, lines: &[String]) -> bool {
        let unchanged = self
            .files
            .get(&fingerprint.path)
            .map(|(known, _)| *known == fingerprint)
            .unwrap_or(false);
        if unchanged {
            return false;
        }
        let symbols =
            editor_core::project_symbol_occurrences(&[(fingerprint.path.clone(), lines.to_vec())]);
        let path = fingerprint.path.clone();
        self.files.insert(path, (fingerprint, symbols));
        true
    }

    /// Drop everything, for a project change.
    pub fn clear(&mut self) {
        self.files.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fingerprint(path: &str, modified: u64) -> FileFingerprint {
        FileFingerprint::new(path, 64 + modified, modified)
    }

    fn source(lines: &[&str]) -> Vec<String> {
        lines.iter().map(|line| (*line).to_string()).collect()
    }

    #[test]
    fn the_first_refresh_parses_every_file() {
        let mut index = IncrementalIndex::new();
        let stats = index.refresh(
            vec![fingerprint("src/a.rs", 1), fingerprint("src/b.rs", 1)],
            |path| match path {
                "src/a.rs" => Some(source(&["pub fn run() {}"])),
                "src/b.rs" => Some(source(&["pub fn helper() {}"])),
                _ => None,
            },
        );
        assert_eq!(stats.reparsed, 2);
        assert_eq!(stats.reused, 0);
        assert_eq!(index.file_count(), 2);
        assert!(stats.symbols >= 2);
        assert!(index.file_symbols("src/a.rs").is_some());
    }

    #[test]
    fn unchanged_files_are_not_read_again() {
        let mut index = IncrementalIndex::new();
        index.refresh(
            vec![fingerprint("src/a.rs", 1), fingerprint("src/b.rs", 1)],
            |_| Some(source(&["pub fn run() {}"])),
        );
        let mut reads = Vec::new();
        let stats = index.refresh(
            vec![fingerprint("src/a.rs", 1), fingerprint("src/b.rs", 2)],
            |path| {
                reads.push(path.to_string());
                Some(source(&["pub fn run() {}", "pub fn helper() {}"]))
            },
        );
        assert_eq!(reads, ["src/b.rs"], "only the changed file should be read");
        assert_eq!(stats.reused, 1);
        assert_eq!(stats.reparsed, 1);
        // The changed file gained a symbol; the reused one kept what it had.
        assert_eq!(index.file_symbols("src/a.rs").map(<[_]>::len), Some(1));
        assert_eq!(index.file_symbols("src/b.rs").map(<[_]>::len), Some(2));
    }

    #[test]
    fn removed_files_leave_the_index() {
        let mut index = IncrementalIndex::new();
        index.refresh(vec![fingerprint("a.rs", 1), fingerprint("b.rs", 1)], |_| {
            Some(source(&["pub fn run() {}"]))
        });
        let stats = index.refresh(vec![fingerprint("a.rs", 1)], |_| {
            Some(source(&["pub fn run() {}"]))
        });
        assert_eq!(stats.removed, 1);
        assert_eq!(index.file_count(), 1);
        assert!(index.file_symbols("b.rs").is_none());
    }

    #[test]
    fn unreadable_files_are_skipped_without_losing_the_rest() {
        let mut index = IncrementalIndex::new();
        index.refresh(vec![fingerprint("a.rs", 1)], |_| {
            Some(source(&["pub fn run() {}"]))
        });
        let stats = index.refresh(
            vec![fingerprint("a.rs", 1), fingerprint("broken.rs", 1)],
            |path| {
                if path == "broken.rs" {
                    None
                } else {
                    Some(source(&["pub fn run() {}"]))
                }
            },
        );
        assert_eq!(stats.skipped, 1);
        assert_eq!(stats.removed, 0);
        assert!(index.file_symbols("a.rs").is_some());
        assert!(index.file_symbols("broken.rs").is_none());
    }

    #[test]
    fn a_single_file_can_be_updated_while_typing() {
        let mut index = IncrementalIndex::new();
        index.refresh(vec![fingerprint("main.rs", 1)], |_| {
            Some(source(&["pub fn run() {}"]))
        });
        assert!(index.update_file(
            fingerprint("main.rs", 2),
            &source(&["pub fn run() {}", "pub fn added() {}"])
        ));
        // The same fingerprint is a no-op, so a keystroke cannot re-parse twice.
        assert!(!index.update_file(
            fingerprint("main.rs", 2),
            &source(&["pub fn run() {}", "pub fn added() {}"])
        ));
        assert_eq!(index.file_symbols("main.rs").map(<[_]>::len), Some(2));
        assert_eq!(
            index.fingerprint("main.rs").map(|value| value.modified),
            Some(2)
        );
    }

    #[test]
    fn symbols_of_every_file_come_back_in_path_order() {
        let mut index = IncrementalIndex::new();
        index.refresh(
            vec![fingerprint("src/z.rs", 1), fingerprint("src/a.rs", 1)],
            |path| {
                Some(source(&[if path.ends_with("z.rs") {
                    "pub fn zed() {}"
                } else {
                    "pub fn alpha() {}"
                }]))
            },
        );
        let symbols = index.symbols();
        assert_eq!(
            symbols.first().map(|entry| entry.file.as_str()),
            Some("src/a.rs")
        );
        assert_eq!(
            symbols.last().map(|entry| entry.file.as_str()),
            Some("src/z.rs")
        );

        index.clear();
        assert!(index.is_empty());
        assert!(index.symbols().is_empty());
    }
}
