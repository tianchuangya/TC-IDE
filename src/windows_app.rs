#![allow(clippy::upper_case_acronyms)]

use std::cell::RefCell;
use std::collections::HashSet;
use std::ffi::c_void;
use std::fs;
use std::path::PathBuf;
use std::ptr;
use std::time::Instant;

use crate::editor_core;

#[path = "windows_platform.rs"]
mod platform;
#[path = "windows_preferences.rs"]
mod preferences;
#[path = "windows_renderer.rs"]
mod renderer;
#[path = "windows_terminal.rs"]
mod terminal;

use renderer::{DirectEditorRenderer, WindowsScene};
use terminal::ConPtySession;

type Bool = i32;
type Dword = u32;
type Hbrush = *mut c_void;
type Hcursor = *mut c_void;
type Hdc = *mut c_void;
type Hicon = *mut c_void;
type Hinstance = *mut c_void;
type Hmenu = *mut c_void;
type Hwnd = *mut c_void;
type Lparam = isize;
type Lresult = isize;
type Uint = u32;
type Wparam = usize;

const CS_HREDRAW: Uint = 0x0002;
const CS_VREDRAW: Uint = 0x0001;
const CW_USEDEFAULT: i32 = i32::MIN;
const IDC_ARROW: *const u16 = 32512_usize as *const u16;
const SW_SHOW: i32 = 5;
const WM_CHAR: Uint = 0x0102;
const WM_COMMAND: Uint = 0x0111;
const WM_DESTROY: Uint = 0x0002;
const WM_DPICHANGED: Uint = 0x02e0;
const WM_ERASEBKGND: Uint = 0x0014;
const WM_IME_COMPOSITION: Uint = 0x010f;
const WM_IME_ENDCOMPOSITION: Uint = 0x010e;
const WM_IME_STARTCOMPOSITION: Uint = 0x010d;
const WM_KEYDOWN: Uint = 0x0100;
const WM_LBUTTONDOWN: Uint = 0x0201;
const WM_LBUTTONUP: Uint = 0x0202;
const WM_MOUSEMOVE: Uint = 0x0200;
const WM_MOUSEWHEEL: Uint = 0x020a;
const WM_PAINT: Uint = 0x000f;
const WM_SIZE: Uint = 0x0005;
const WM_TIMER: Uint = 0x0113;
const WS_OVERLAPPEDWINDOW: Dword = 0x00cf0000;
const WS_VISIBLE: Dword = 0x10000000;
const VK_BACK: u32 = 0x08;
const VK_TAB: u32 = 0x09;
const VK_DELETE: u32 = 0x2e;
const VK_DOWN: u32 = 0x28;
const VK_END: u32 = 0x23;
const VK_HOME: u32 = 0x24;
const VK_LEFT: u32 = 0x25;
const VK_RIGHT: u32 = 0x27;
const VK_UP: u32 = 0x26;
const GCS_COMPSTR: Lparam = 0x0008;
const GCS_RESULTSTR: Lparam = 0x0800;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct TextPosition {
    line: usize,
    column: usize,
}

#[derive(Clone)]
struct EditorSnapshot {
    lines: Vec<String>,
    caret: TextPosition,
    selection_anchor: Option<TextPosition>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CreateKind {
    Project,
    File,
    Folder,
}

struct TerminalTab {
    name: String,
    session: ConPtySession,
    input: String,
}

thread_local! {
    static APP: RefCell<WindowsAppState> = RefCell::new(WindowsAppState::new());
    static RENDERER: RefCell<Option<DirectEditorRenderer>> = const { RefCell::new(None) };
}

struct WindowsAppState {
    file_name: String,
    file_path: Option<PathBuf>,
    project_root: PathBuf,
    project_entries: Vec<String>,
    project_entry_paths: Vec<PathBuf>,
    project_entry_is_dir: Vec<bool>,
    collapsed_directories: HashSet<PathBuf>,
    project_scroll: usize,
    project_symbols: Vec<editor_core::ProjectSymbolOccurrence>,
    workspace_search: String,
    workspace_search_focused: bool,
    create_kind: Option<CreateKind>,
    create_name: String,
    create_error: String,
    create_location: PathBuf,
    project_language: usize,
    project_environment: usize,
    lines: Vec<String>,
    caret_line: usize,
    caret_column: usize,
    selection_anchor: Option<TextPosition>,
    dragging_selection: bool,
    undo_stack: Vec<EditorSnapshot>,
    redo_stack: Vec<EditorSnapshot>,
    ime_composition: String,
    light_theme: bool,
    ui_chinese: bool,
    code_zoom: i32,
    settings_open: bool,
    extensions_open: bool,
    extension_category: usize,
    selected_language: usize,
    language_installed: Vec<bool>,
    language_versions: Vec<usize>,
    hovered_symbol: Option<editor_core::SymbolOccurrence>,
    expanded_comments: HashSet<usize>,
    editing_comment: Option<usize>,
    pointer_x: i32,
    background_path: Option<PathBuf>,
    background_opacity: f32,
    background_offset_x: f32,
    background_offset_y: f32,
    background_zoom: f32,
    terminal_open: bool,
    terminal_motion: f32,
    terminal_velocity: f32,
    terminals: Vec<TerminalTab>,
    active_terminal: usize,
    status: String,
    started: Instant,
    last_frame: Instant,
    last_edit: Option<Instant>,
}

impl WindowsAppState {
    const PROJECT_LANGUAGES: [&'static str; 7] =
        ["Rust", "Python", "C", "C++", "Java", "Go", "Web"];

    fn project_language_name(&self) -> &'static str {
        Self::PROJECT_LANGUAGES[self.project_language.min(Self::PROJECT_LANGUAGES.len() - 1)]
    }

    fn project_environments(&self) -> &'static [&'static str] {
        match self.project_language_name() {
            "Python" => &["None", "venv", "uv", "conda", "poetry"],
            "C" => &["None", "GCC", "Clang", "CMake"],
            "C++" => &["None", "CMake + MSVC", "CMake + Clang", "Meson", "Conan"],
            "Java" => &["None", "JDK 25", "JDK 21 LTS", "Gradle", "Maven"],
            "Rust" => &["None", "cargo stable", "cargo nightly"],
            "Go" => &["None", "Go modules"],
            _ => &["None", "npm", "pnpm", "yarn", "bun"],
        }
    }

    fn new() -> Self {
        let preferences = preferences::load();
        let project_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let collapsed_directories = HashSet::new();
        let (project_entries, project_entry_paths, project_entry_is_dir) =
            Self::scan_project(&project_root, &collapsed_directories);
        let project_symbols = Self::index_project(&project_root);
        let create_location = project_root.clone();
        Self {
            file_name: "main.rs".into(),
            file_path: None,
            project_root,
            project_entries,
            project_entry_paths,
            project_entry_is_dir,
            collapsed_directories,
            project_scroll: 0,
            project_symbols,
            workspace_search: String::new(),
            workspace_search_focused: false,
            create_kind: None,
            create_name: String::new(),
            create_error: String::new(),
            create_location,
            project_language: preferences.project_language,
            project_environment: preferences.project_environment,
            lines: vec![
                "use tc_ide::workspace::Project;".into(),
                String::new(),
                "pub struct Editor {".into(),
                "    project: Project,".into(),
                "}".into(),
                String::new(),
                "impl Editor {".into(),
                "    pub fn render(&mut self) {".into(),
                "        self.project.resolve_symbols();".into(),
                "    }".into(),
                "}".into(),
            ],
            caret_line: 0,
            caret_column: 0,
            selection_anchor: None,
            dragging_selection: false,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            ime_composition: String::new(),
            light_theme: preferences.light_theme,
            ui_chinese: preferences.ui_chinese,
            code_zoom: preferences.code_zoom,
            settings_open: false,
            extensions_open: false,
            extension_category: 0,
            selected_language: 0,
            language_installed: (0..7)
                .map(|index| {
                    preferences
                        .language_installed
                        .get(index)
                        .copied()
                        .unwrap_or(false)
                })
                .collect(),
            language_versions: (0..7)
                .map(|index| {
                    preferences
                        .language_versions
                        .get(index)
                        .copied()
                        .unwrap_or(0)
                        .min(2)
                })
                .collect(),
            hovered_symbol: None,
            expanded_comments: HashSet::new(),
            editing_comment: None,
            pointer_x: -1,
            background_path: preferences.background_path,
            background_opacity: preferences.background_opacity,
            background_offset_x: preferences.background_offset_x,
            background_offset_y: preferences.background_offset_y,
            background_zoom: preferences.background_zoom,
            terminal_open: false,
            terminal_motion: 0.0,
            terminal_velocity: 0.0,
            terminals: Vec::new(),
            active_terminal: 0,
            status: "DirectWrite / Direct2D editor ready".into(),
            started: Instant::now(),
            last_frame: Instant::now(),
            last_edit: None,
        }
    }

    fn clamp_caret(&mut self) {
        self.caret_line = self.caret_line.min(self.lines.len().saturating_sub(1));
        self.caret_column = self
            .caret_column
            .min(self.lines[self.caret_line].chars().count());
    }

    fn byte_at_caret(&self) -> usize {
        Self::byte_at(&self.lines[self.caret_line], self.caret_column)
    }

    fn byte_at(value: &str, column: usize) -> usize {
        value
            .char_indices()
            .nth(column)
            .map(|(index, _)| index)
            .unwrap_or(value.len())
    }

    fn comment_edit_bounds(&self) -> Option<(usize, usize, usize)> {
        let line = self.editing_comment?;
        let value = self.lines.get(line)?;
        let (_, start, end) = editor_core::comment_bounds(value, &self.file_name)?;
        Some((
            line,
            value[..start].chars().count(),
            value[..end].chars().count(),
        ))
    }

    fn clamp_comment_caret(&mut self) {
        if let Some((line, start, end)) = self.comment_edit_bounds() {
            self.caret_line = line;
            self.caret_column = self.caret_column.clamp(start, end);
            self.selection_anchor = None;
        }
    }

    fn position(&self) -> TextPosition {
        TextPosition {
            line: self.caret_line,
            column: self.caret_column,
        }
    }

    fn snapshot(&self) -> EditorSnapshot {
        EditorSnapshot {
            lines: self.lines.clone(),
            caret: self.position(),
            selection_anchor: self.selection_anchor,
        }
    }

    fn restore(&mut self, snapshot: EditorSnapshot) {
        self.lines = snapshot.lines;
        self.caret_line = snapshot.caret.line;
        self.caret_column = snapshot.caret.column;
        self.selection_anchor = snapshot.selection_anchor;
        self.clamp_caret();
        self.last_edit = Some(Instant::now());
    }

    fn record_undo(&mut self) {
        self.undo_stack.push(self.snapshot());
        if self.undo_stack.len() > 200 {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
    }

    fn selection_range(&self) -> Option<(TextPosition, TextPosition)> {
        let anchor = self.selection_anchor?;
        let caret = self.position();
        (anchor != caret).then_some(if anchor < caret {
            (anchor, caret)
        } else {
            (caret, anchor)
        })
    }

    fn selected_text(&self) -> Option<String> {
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

    fn delete_selection_raw(&mut self) -> bool {
        let Some((start, end)) = self.selection_range() else {
            return false;
        };
        if start.line == end.line {
            let start_byte = Self::byte_at(&self.lines[start.line], start.column);
            let end_byte = Self::byte_at(&self.lines[start.line], end.column);
            self.lines[start.line].replace_range(start_byte..end_byte, "");
        } else {
            let prefix = self.lines[start.line]
                [..Self::byte_at(&self.lines[start.line], start.column)]
                .to_string();
            let suffix = self.lines[end.line][Self::byte_at(&self.lines[end.line], end.column)..]
                .to_string();
            self.lines
                .splice(start.line..=end.line, [format!("{prefix}{suffix}")]);
        }
        self.caret_line = start.line;
        self.caret_column = start.column;
        self.selection_anchor = None;
        true
    }

    fn insert_text(&mut self, value: &str) {
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
        for value in value.chars() {
            if value == '\n' {
                self.newline_raw();
            } else if !value.is_control() {
                let byte = self.byte_at_caret();
                self.lines[self.caret_line].insert(byte, value);
                self.caret_column += 1;
            }
        }
        self.last_edit = Some(Instant::now());
        self.status = "Edited with Unicode-aware caret mapping".into();
    }

    fn completions(&self) -> Vec<String> {
        editor_core::completions(
            &self.file_name,
            &self.lines[self.caret_line],
            self.caret_column,
        )
    }

    fn accept_completion(&mut self) {
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

    fn insert_char(&mut self, value: char) {
        self.insert_text(&value.to_string());
    }

    fn newline(&mut self) {
        self.record_undo();
        self.delete_selection_raw();
        self.newline_raw();
        self.last_edit = Some(Instant::now());
    }

    fn newline_raw(&mut self) {
        let byte = self.byte_at_caret();
        let tail = self.lines[self.caret_line].split_off(byte);
        self.caret_line += 1;
        self.lines.insert(self.caret_line, tail);
        self.caret_column = 0;
    }

    fn backspace(&mut self) {
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
            self.last_edit = Some(Instant::now());
            return;
        }
        if self.caret_column == 0 && self.caret_line == 0 {
            return;
        }
        self.record_undo();
        if self.caret_column > 0 {
            let end = self.byte_at_caret();
            self.caret_column -= 1;
            let start = self.byte_at_caret();
            self.lines[self.caret_line].replace_range(start..end, "");
        } else if self.caret_line > 0 {
            let current = self.lines.remove(self.caret_line);
            self.caret_line -= 1;
            self.caret_column = self.lines[self.caret_line].chars().count();
            self.lines[self.caret_line].push_str(&current);
        }
        self.last_edit = Some(Instant::now());
    }

    fn delete_forward(&mut self) {
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
            let line_len = self.lines[self.caret_line].chars().count();
            if self.caret_column < line_len {
                self.record_undo();
                let start = self.byte_at_caret();
                let end = Self::byte_at(&self.lines[self.caret_line], self.caret_column + 1);
                self.lines[self.caret_line].replace_range(start..end, "");
            } else if self.caret_line + 1 < self.lines.len() {
                self.record_undo();
                let next = self.lines.remove(self.caret_line + 1);
                self.lines[self.caret_line].push_str(&next);
            } else {
                return;
            }
        }
        self.last_edit = Some(Instant::now());
    }

    fn move_left(&mut self) {
        if self.caret_column > 0 {
            self.caret_column -= 1;
        } else if self.caret_line > 0 {
            self.caret_line -= 1;
            self.caret_column = self.lines[self.caret_line].chars().count();
        }
    }

    fn move_right(&mut self) {
        if self.caret_column < self.lines[self.caret_line].chars().count() {
            self.caret_column += 1;
        } else if self.caret_line + 1 < self.lines.len() {
            self.caret_line += 1;
            self.caret_column = 0;
        }
    }

    fn select_all(&mut self) {
        self.selection_anchor = Some(TextPosition { line: 0, column: 0 });
        self.caret_line = self.lines.len().saturating_sub(1);
        self.caret_column = self.lines[self.caret_line].chars().count();
        self.status = "Selected all text".into();
    }

    fn cut_selection(&mut self) -> Option<String> {
        let value = self.selected_text()?;
        self.record_undo();
        self.delete_selection_raw();
        self.last_edit = Some(Instant::now());
        Some(value)
    }

    fn undo(&mut self) {
        if let Some(snapshot) = self.undo_stack.pop() {
            self.redo_stack.push(self.snapshot());
            self.restore(snapshot);
            self.status = "Undo".into();
        }
    }

    fn redo(&mut self) {
        if let Some(snapshot) = self.redo_stack.pop() {
            self.undo_stack.push(self.snapshot());
            self.restore(snapshot);
            self.status = "Redo".into();
        }
    }

    fn set_caret(&mut self, position: TextPosition, extend_selection: bool) {
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

    fn save(&mut self) {
        let Some(path) = &self.file_path else {
            self.status = "Open a file before saving".into();
            return;
        };
        match fs::write(path, self.lines.join("\r\n")) {
            Ok(()) => {
                self.status = format!("Saved {}", path.display());
                self.project_symbols = Self::index_project(&self.project_root);
            }
            Err(error) => self.status = format!("Save failed: {error}"),
        }
    }

    fn open_file(&mut self, path: PathBuf) {
        match fs::read_to_string(&path) {
            Ok(content) => {
                self.lines = content.lines().map(str::to_string).collect();
                if self.lines.is_empty() {
                    self.lines.push(String::new());
                }
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
                self.project_symbols = Self::index_project(&self.project_root);
                self.status = format!(
                    "Opened {}  ·  {}",
                    path.display(),
                    editor_core::language_for_file(&self.file_name)
                );
            }
            Err(error) => self.status = format!("Open failed: {error}"),
        }
    }

    fn scan_project(
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

    fn index_project(root: &std::path::Path) -> Vec<editor_core::ProjectSymbolOccurrence> {
        fn collect(
            root: &std::path::Path,
            path: &std::path::Path,
            sources: &mut Vec<(String, Vec<String>)>,
        ) {
            if sources.len() >= 300 {
                return;
            }
            let Ok(entries) = fs::read_dir(path) else {
                return;
            };
            for entry in entries.filter_map(Result::ok) {
                if sources.len() >= 300 {
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
                        collect(root, &path, sources);
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
                if let Ok(content) = fs::read_to_string(&path) {
                    sources.push((display, content.lines().map(str::to_string).collect()));
                }
            }
        }

        let mut sources = Vec::new();
        collect(root, root, &mut sources);
        editor_core::project_symbol_occurrences(&sources)
    }

    fn open_project(&mut self, root: PathBuf) {
        self.project_root = root;
        self.project_scroll = 0;
        self.collapsed_directories.clear();
        (
            self.project_entries,
            self.project_entry_paths,
            self.project_entry_is_dir,
        ) = Self::scan_project(&self.project_root, &self.collapsed_directories);
        self.project_symbols = Self::index_project(&self.project_root);
        self.file_path = None;
        self.file_name = "Select a file".into();
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

    fn begin_create(&mut self, kind: CreateKind) {
        self.create_kind = Some(kind);
        self.create_name.clear();
        self.create_error.clear();
        self.create_location = self.project_root.clone();
        self.workspace_search_focused = false;
    }

    fn save_preferences(&self) {
        let _ = preferences::save(&preferences::WindowsPreferences {
            light_theme: self.light_theme,
            ui_chinese: self.ui_chinese,
            code_zoom: self.code_zoom,
            project_language: self.project_language,
            project_environment: self.project_environment,
            language_installed: self.language_installed.clone(),
            language_versions: self.language_versions.clone(),
            background_path: self.background_path.clone(),
            background_opacity: self.background_opacity,
            background_offset_x: self.background_offset_x,
            background_offset_y: self.background_offset_y,
            background_zoom: self.background_zoom,
        });
    }

    fn refresh_project(&mut self) {
        (
            self.project_entries,
            self.project_entry_paths,
            self.project_entry_is_dir,
        ) = Self::scan_project(&self.project_root, &self.collapsed_directories);
        self.project_symbols = Self::index_project(&self.project_root);
        self.project_scroll = self
            .project_scroll
            .min(self.project_entries.len().saturating_sub(1));
    }

    fn create_requested_item(&mut self) {
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

    fn create_project_template(&self, target: &std::path::Path) -> std::io::Result<()> {
        let language = self.project_language_name();
        fs::create_dir_all(target)?;
        match language {
            "Rust" => {
                fs::create_dir_all(target.join("src"))?;
                fs::write(target.join("Cargo.toml"), "[package]\nname = \"tc-project\"\nversion = \"0.1.0\"\nedition = \"2024\"\n")?;
                fs::write(target.join("src/main.rs"), "fn main() {\n    println!(\"Hello from TC-IDE\");\n}\n")?;
            }
            "Python" => fs::write(target.join("main.py"), "print(\"Hello from TC-IDE\")\n")?,
            "C" => fs::write(target.join("main.c"), "#include <stdio.h>\n\nint main(void) {\n    puts(\"Hello from TC-IDE\");\n    return 0;\n}\n")?,
            "C++" => fs::write(target.join("main.cpp"), "#include <iostream>\n\nint main() {\n    std::cout << \"Hello from TC-IDE\\n\";\n}\n")?,
            "Java" => fs::write(target.join("Main.java"), "public class Main {\n    public static void main(String[] args) {\n        System.out.println(\"Hello from TC-IDE\");\n    }\n}\n")?,
            "Go" => {
                fs::write(target.join("go.mod"), "module tc-project\n\ngo 1.24\n")?;
                fs::write(target.join("main.go"), "package main\n\nimport \"fmt\"\n\nfunc main() { fmt.Println(\"Hello from TC-IDE\") }\n")?;
            }
            _ => fs::write(target.join("index.html"), "<!doctype html>\n<title>TC-IDE Project</title>\n<h1>Hello from TC-IDE</h1>\n")?,
        }
        fs::write(
            target.join(".tc-ide-project"),
            format!(
                "language={language}\nenvironment={}\n",
                self.project_environments()[self
                    .project_environment
                    .min(self.project_environments().len() - 1)]
            ),
        )
    }

    fn ensure_terminal(&mut self) {
        if !self.terminals.is_empty() {
            return;
        }
        for name in ["Build", "Run", "Output"] {
            self.add_named_terminal(name.to_string());
        }
        self.active_terminal = 0;
    }

    fn add_terminal(&mut self) {
        self.add_named_terminal(format!("Terminal {}", self.terminals.len() + 1));
    }

    fn add_named_terminal(&mut self, name: String) {
        let cwd = self.project_root.clone();
        match unsafe { ConPtySession::spawn(&cwd, 120, 30) } {
            Ok(session) => {
                self.terminals.push(TerminalTab {
                    name,
                    session,
                    input: String::new(),
                });
                self.active_terminal = self.terminals.len() - 1;
                self.status = format!("ConPTY started in {}", cwd.display());
            }
            Err(error) => self.status = format!("ConPTY start failed: {error}"),
        }
    }

    fn toggle_terminal(&mut self) {
        self.terminal_open = !self.terminal_open;
        if self.terminal_open {
            self.ensure_terminal();
        }
    }

    fn update_motion(&mut self) {
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame).as_secs_f32().min(0.04);
        self.last_frame = now;
        let target = if self.terminal_open { 1.0 } else { 0.0 };
        self.terminal_velocity += (target - self.terminal_motion) * 235.0 * dt;
        self.terminal_velocity *= (-17.0 * dt).exp();
        self.terminal_motion += self.terminal_velocity * dt;
        self.terminal_motion = self.terminal_motion.clamp(-0.05, 1.05);
        if (target - self.terminal_motion).abs() < 0.001 && self.terminal_velocity.abs() < 0.001 {
            self.terminal_motion = target;
            self.terminal_velocity = 0.0;
        }
    }
}

#[repr(C)]
struct Point {
    x: i32,
    y: i32,
}

#[repr(C)]
struct Msg {
    hwnd: Hwnd,
    message: Uint,
    w_param: Wparam,
    l_param: Lparam,
    time: Dword,
    point: Point,
    private: Dword,
}

#[repr(C)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[repr(C)]
struct PaintStruct {
    dc: Hdc,
    erase: Bool,
    paint: Rect,
    restore: Bool,
    update: Bool,
    reserved: [u8; 32],
}

type WindowProc = unsafe extern "system" fn(Hwnd, Uint, Wparam, Lparam) -> Lresult;

#[repr(C)]
struct WindowClass {
    style: Uint,
    window_proc: Option<WindowProc>,
    class_extra: i32,
    window_extra: i32,
    instance: Hinstance,
    icon: Hicon,
    cursor: Hcursor,
    background: Hbrush,
    menu_name: *const u16,
    class_name: *const u16,
}

#[link(name = "kernel32")]
extern "system" {
    fn GetModuleHandleW(module_name: *const u16) -> Hinstance;
}

#[link(name = "imm32")]
extern "system" {
    fn ImmGetContext(window: Hwnd) -> *mut c_void;
    fn ImmGetCompositionStringW(
        context: *mut c_void,
        index: Dword,
        buffer: *mut c_void,
        buffer_bytes: Dword,
    ) -> i32;
    fn ImmReleaseContext(window: Hwnd, context: *mut c_void) -> Bool;
}

#[link(name = "user32")]
extern "system" {
    fn BeginPaint(window: Hwnd, paint: *mut PaintStruct) -> Hdc;
    fn CreateWindowExW(
        extended_style: Dword,
        class_name: *const u16,
        window_name: *const u16,
        style: Dword,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        parent: Hwnd,
        menu: Hmenu,
        instance: Hinstance,
        parameter: *mut c_void,
    ) -> Hwnd;
    fn DefWindowProcW(window: Hwnd, message: Uint, w_param: Wparam, l_param: Lparam) -> Lresult;
    fn DispatchMessageW(message: *const Msg) -> Lresult;
    fn EndPaint(window: Hwnd, paint: *const PaintStruct) -> Bool;
    fn GetClientRect(window: Hwnd, area: *mut Rect) -> Bool;
    fn GetMessageW(message: *mut Msg, window: Hwnd, minimum: Uint, maximum: Uint) -> Bool;
    fn InvalidateRect(window: Hwnd, area: *const Rect, erase: Bool) -> Bool;
    fn LoadCursorW(instance: Hinstance, cursor_name: *const u16) -> Hcursor;
    fn PostQuitMessage(exit_code: i32);
    fn RegisterClassW(class: *const WindowClass) -> u16;
    fn ReleaseCapture() -> Bool;
    fn SetCapture(window: Hwnd) -> Hwnd;
    fn SetTimer(window: Hwnd, event: usize, milliseconds: Uint, callback: *const c_void) -> usize;
    fn ShowWindow(window: Hwnd, command: i32) -> Bool;
    fn TranslateMessage(message: *const Msg) -> Bool;
    fn UpdateWindow(window: Hwnd) -> Bool;
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn invalidate(window: Hwnd) {
    unsafe {
        InvalidateRect(window, ptr::null(), 0);
    }
}

fn ime_string(window: Hwnd, index: Dword) -> String {
    unsafe {
        let context = ImmGetContext(window);
        if context.is_null() {
            return String::new();
        }
        let bytes = ImmGetCompositionStringW(context, index, ptr::null_mut(), 0);
        if bytes <= 0 {
            ImmReleaseContext(window, context);
            return String::new();
        }
        let mut value = vec![0u16; bytes as usize / std::mem::size_of::<u16>()];
        let written =
            ImmGetCompositionStringW(context, index, value.as_mut_ptr().cast(), bytes as Dword);
        ImmReleaseContext(window, context);
        if written <= 0 {
            String::new()
        } else {
            String::from_utf16_lossy(&value[..written as usize / 2])
        }
    }
}

fn character_cells(value: char) -> usize {
    match value as u32 {
        0x1100..=0x115f
        | 0x2329..=0x232a
        | 0x2e80..=0xa4cf
        | 0xac00..=0xd7a3
        | 0xf900..=0xfaff
        | 0xfe10..=0xfe6f
        | 0xff00..=0xff60
        | 0xffe0..=0xffe6
        | 0x1f300..=0x1faff
        | 0x20000..=0x3fffd => 2,
        _ => 1,
    }
}

fn editor_position_at(window: Hwnd, x: i32, y: i32) -> Option<TextPosition> {
    let mut area = unsafe { std::mem::zeroed::<Rect>() };
    unsafe { GetClientRect(window, &mut area) };
    let width = area.right.max(1) as f32;
    let scale = platform::dpi_for_window(window) as f32 / 96.0;
    let tree_w = (width * 0.19).max(220.0 * scale);
    let graph_w = (width * 0.27).max(300.0 * scale);
    let editor_right = width - graph_w;
    let code_top = (48.0 + 58.0) * scale;
    if x as f32 <= tree_w + 42.0 * scale || x as f32 >= editor_right || y as f32 <= code_top {
        return None;
    }
    let line = ((y as f32 - code_top) / (25.0 * scale)).floor() as usize;
    APP.with(|cell| {
        let app = cell.borrow();
        let value = app.lines.get(line)?;
        let code_cell = 8.9 + app.code_zoom.clamp(-2, 8) as f32 * 0.6;
        let target = ((x as f32 - tree_w - 52.0 * scale) / (code_cell * scale)).max(0.0);
        let mut cells = 0.0;
        for (column, ch) in value.chars().enumerate() {
            let width = character_cells(ch) as f32;
            if target < cells + width * 0.5 {
                return Some(TextPosition { line, column });
            }
            cells += width;
        }
        Some(TextPosition {
            line,
            column: value.chars().count(),
        })
    })
}

fn comment_bubble_at(window: Hwnd, x: i32, y: i32) -> Option<(usize, bool)> {
    let mut area = unsafe { std::mem::zeroed::<Rect>() };
    unsafe { GetClientRect(window, &mut area) };
    let width = area.right.max(1) as f32;
    let height = area.bottom.max(1) as f32;
    let scale = platform::dpi_for_window(window) as f32 / 96.0;
    let tree_w = (width * 0.19).max(220.0 * scale);
    let graph_w = (width * 0.27).max(300.0 * scale);
    let editor_right = width - graph_w;
    if x as f32 <= tree_w || x as f32 >= editor_right {
        return None;
    }
    APP.with(|cell| {
        let app = cell.borrow();
        let terminal_h = (28.0 + 214.0 * app.terminal_motion.clamp(0.0, 1.0)) * scale;
        let body_bottom = height - terminal_h;
        let code_top = 106.0 * scale;
        let line_step = 25.0 * scale;
        let bubble_right = editor_right - 14.0 * scale;
        let bubble_left = (bubble_right - 286.0 * scale).max(tree_w + 130.0 * scale);
        for (line, value) in app.lines.iter().enumerate() {
            if editor_core::comment_bounds(value, &app.file_name).is_none() {
                continue;
            }
            let top = code_top + line as f32 * line_step - 5.0 * scale;
            if top >= body_bottom {
                break;
            }
            let height = if app.expanded_comments.contains(&line) {
                112.0
            } else {
                34.0
            } * scale;
            if x as f32 >= bubble_left
                && x as f32 <= bubble_right
                && y as f32 >= top
                && y as f32 <= top + height
            {
                return Some((line, x as f32 <= bubble_left + 76.0 * scale));
            }
        }
        None
    })
}

fn perform_command(window: Hwnd, command: Wparam) {
    APP.with(|cell| {
        let mut app = cell.borrow_mut();
        match command {
            platform::COMMAND_OPEN_FILE => {
                if let Some(path) = platform::choose_file(window) {
                    app.open_file(path);
                }
            }
            platform::COMMAND_COPY_DIAGNOSTIC => {
                let selected = app
                    .selected_text()
                    .unwrap_or_else(|| app.lines.get(app.caret_line).cloned().unwrap_or_default());
                match platform::write_clipboard(window, &selected) {
                    Ok(()) => app.status = "Copied Unicode text".into(),
                    Err(error) => app.status = format!("Clipboard write failed: {error}"),
                }
            }
            platform::COMMAND_PASTE => match platform::read_clipboard(window) {
                Ok(value) => app.insert_text(&value),
                Err(error) => app.status = format!("Clipboard read failed: {error}"),
            },
            platform::COMMAND_SELECT_ALL => {
                if let Some((line, start, end)) = app.comment_edit_bounds() {
                    app.selection_anchor = Some(TextPosition {
                        line,
                        column: start,
                    });
                    app.caret_line = line;
                    app.caret_column = end;
                } else {
                    app.select_all();
                }
            }
            platform::COMMAND_CUT => {
                if let Some(value) = app.cut_selection() {
                    match platform::write_clipboard(window, &value) {
                        Ok(()) => app.status = "Cut selection".into(),
                        Err(error) => app.status = format!("Clipboard write failed: {error}"),
                    }
                }
            }
            platform::COMMAND_UNDO => app.undo(),
            platform::COMMAND_REDO => app.redo(),
            platform::COMMAND_THEME_DARK => {
                app.light_theme = false;
                app.status = "Dark theme enabled".into();
            }
            platform::COMMAND_THEME_LIGHT => {
                app.light_theme = true;
                app.status = "Light theme enabled".into();
            }
            platform::COMMAND_LANGUAGE_TOGGLE => {
                app.ui_chinese = !app.ui_chinese;
                app.status = if app.ui_chinese {
                    "已切换为中文界面".into()
                } else {
                    "English interface enabled".into()
                };
            }
            platform::COMMAND_FONT_INCREASE => app.code_zoom = (app.code_zoom + 1).min(8),
            platform::COMMAND_FONT_DECREASE => app.code_zoom = (app.code_zoom - 1).max(-2),
            platform::COMMAND_NEW_PROJECT => {
                if let Some(location) = platform::choose_folder(window) {
                    app.begin_create(CreateKind::Project);
                    app.create_location = location;
                }
            }
            platform::COMMAND_NEW_FILE => app.begin_create(CreateKind::File),
            platform::COMMAND_NEW_FOLDER => app.begin_create(CreateKind::Folder),
            platform::COMMAND_WORKSPACE_SEARCH => {
                app.workspace_search_focused = true;
                app.create_kind = None;
            }
            platform::COMMAND_SETTINGS => {
                app.settings_open = !app.settings_open;
                app.extensions_open = false;
            }
            platform::COMMAND_EXTENSIONS => {
                app.extensions_open = !app.extensions_open;
                app.settings_open = false;
            }
            platform::COMMAND_OPEN_FOLDER => {
                if let Some(path) = platform::choose_folder(window) {
                    app.open_project(path);
                }
            }
            platform::COMMAND_REVEAL_FOLDER => {
                if platform::open_in_explorer(window, &app.project_root) {
                    app.status = format!("Opened Explorer at {}", app.project_root.display());
                } else {
                    app.status = "Could not open the current project in Explorer".into();
                }
            }
            platform::COMMAND_OPEN_TERMINAL => app.toggle_terminal(),
            _ => {}
        }
        app.save_preferences();
    });
    if matches!(
        command,
        platform::COMMAND_FONT_INCREASE | platform::COMMAND_FONT_DECREASE
    ) {
        RENDERER.with(|cell| *cell.borrow_mut() = None);
    }
    invalidate(window);
}

fn render_window(window: Hwnd) {
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
            let terminal_lines = app
                .terminals
                .get(app.active_terminal)
                .and_then(|tab| {
                    tab.session.view.lock().ok().map(|view| {
                        let mut lines = view.lines.clone();
                        if !view.current.is_empty() {
                            lines.push(view.current.clone());
                        }
                        lines
                    })
                })
                .unwrap_or_default();
            let terminal_names = app
                .terminals
                .iter()
                .map(|tab| tab.name.clone())
                .collect::<Vec<_>>();
            let completions = app.completions();
            let symbols = editor_core::symbol_occurrences(&app.lines, &app.file_name);
            let expanded_comments = app.expanded_comments.iter().copied().collect::<Vec<_>>();
            let create_location = app.create_location.to_string_lossy().into_owned();
            let background_path = app
                .background_path
                .as_ref()
                .map(|path| path.to_string_lossy().into_owned());
            let query = app.workspace_search.to_ascii_lowercase();
            let visible_row_count =
                ((height as f32 / (dpi as f32 / 96.0) - 132.0) / 35.0).max(1.0) as usize;
            let visible_entries = app
                .project_entries
                .iter()
                .filter(|entry| query.is_empty() || entry.to_ascii_lowercase().contains(&query))
                .skip(app.project_scroll)
                .take(visible_row_count)
                .cloned()
                .collect::<Vec<_>>();
            let scene = WindowsScene {
                file_name: &app.file_name,
                project_name: app
                    .project_root
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("PROJECT"),
                project_entries: &visible_entries,
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
                create_environment: app.project_environments()[app
                    .project_environment
                    .min(app.project_environments().len() - 1)],
                lines: &app.lines,
                caret_line: app.caret_line,
                caret_column: app.caret_column,
                selection: app
                    .selection_range()
                    .map(|(start, end)| ((start.line, start.column), (end.line, end.column))),
                ime_composition: &app.ime_composition,
                completions: &completions,
                light_theme: app.light_theme,
                ui_chinese: app.ui_chinese,
                code_zoom: app.code_zoom,
                settings_open: app.settings_open,
                extensions_open: app.extensions_open,
                extension_category: app.extension_category,
                selected_language: app.selected_language,
                language_installed: &app.language_installed,
                language_versions: &app.language_versions,
                symbols: &symbols,
                project_symbols: &app.project_symbols,
                hovered_symbol: app.hovered_symbol.as_ref(),
                expanded_comments: &expanded_comments,
                editing_comment: app.editing_comment,
                pointer_x: app.pointer_x,
                background_path: background_path.as_deref(),
                background_opacity: app.background_opacity,
                background_offset_x: app.background_offset_x,
                background_offset_y: app.background_offset_y,
                background_zoom: app.background_zoom,
                status: &app.status,
                terminal_lines: &terminal_lines,
                terminal_names: &terminal_names,
                active_terminal: app.active_terminal,
                terminal_input: active_terminal.map(|tab| tab.input.as_str()).unwrap_or(""),
                terminal_open: app.terminal_open,
                terminal_progress: app.terminal_motion,
                animation_seconds: app.started.elapsed().as_secs_f32(),
                edit_age: app.last_edit.map(|started| started.elapsed().as_secs_f32()),
            };
            let mut renderer = renderer_cell.borrow_mut();
            if renderer.is_none() {
                *renderer = unsafe { DirectEditorRenderer::new(dpi, app.code_zoom) }.ok();
            }
            if let Some(renderer) = renderer.as_mut() {
                let _ = unsafe { renderer.render(window, width, height, &scene) };
            }
        });
    });
}

fn handle_character(window: Hwnd, value: char) {
    APP.with(|cell| {
        let mut app = cell.borrow_mut();
        if app.create_kind.is_some() {
            if !value.is_control() {
                app.create_name.push(value);
                app.create_error.clear();
            }
        } else if app.workspace_search_focused {
            if !value.is_control() {
                app.workspace_search.push(value);
                app.project_scroll = 0;
            }
        } else if app.terminal_open {
            let active = app.active_terminal;
            let Some(tab) = app.terminals.get_mut(active) else {
                return;
            };
            match value {
                '\r' | '\n' => {
                    let _ = tab.session.write("\r");
                    tab.input.clear();
                }
                '\u{8}' => {
                    tab.input.pop();
                    let _ = tab.session.write("\u{7f}");
                }
                ch if !ch.is_control() => {
                    tab.input.push(ch);
                    let _ = tab.session.write(&ch.to_string());
                }
                _ => {}
            }
        } else if app.editing_comment.is_some() {
            if !value.is_control() {
                app.insert_char(value);
            }
        } else if value == '\r' || value == '\n' {
            app.newline();
        } else if !value.is_control() {
            app.insert_char(value);
        }
    });
    invalidate(window);
}

unsafe extern "system" fn window_proc(
    window: Hwnd,
    message: Uint,
    w_param: Wparam,
    l_param: Lparam,
) -> Lresult {
    match message {
        WM_PAINT => {
            let mut paint: PaintStruct = unsafe { std::mem::zeroed() };
            unsafe { BeginPaint(window, &mut paint) };
            render_window(window);
            unsafe { EndPaint(window, &paint) };
            0
        }
        WM_ERASEBKGND => 1,
        WM_COMMAND => {
            perform_command(window, w_param & 0xffff);
            0
        }
        WM_CHAR => {
            if let Some(value) = char::from_u32(w_param as u32) {
                handle_character(window, value);
            }
            0
        }
        WM_IME_STARTCOMPOSITION => {
            APP.with(|cell| cell.borrow_mut().ime_composition.clear());
            invalidate(window);
            0
        }
        WM_IME_COMPOSITION => {
            if l_param & GCS_RESULTSTR != 0 {
                let result = ime_string(window, GCS_RESULTSTR as Dword);
                APP.with(|cell| {
                    let mut app = cell.borrow_mut();
                    app.ime_composition.clear();
                    if !result.is_empty() {
                        app.insert_text(&result);
                    }
                });
            } else if l_param & GCS_COMPSTR != 0 {
                let composition = ime_string(window, GCS_COMPSTR as Dword);
                APP.with(|cell| cell.borrow_mut().ime_composition = composition);
            }
            invalidate(window);
            0
        }
        WM_IME_ENDCOMPOSITION => {
            APP.with(|cell| cell.borrow_mut().ime_composition.clear());
            invalidate(window);
            0
        }
        WM_KEYDOWN => {
            if platform::control_pressed() {
                let command = match w_param as u32 {
                    0x41 => platform::COMMAND_SELECT_ALL,
                    0x43 => platform::COMMAND_COPY_DIAGNOSTIC,
                    0x4f => platform::COMMAND_OPEN_FILE,
                    0x50 => platform::COMMAND_WORKSPACE_SEARCH,
                    0x54 => platform::COMMAND_OPEN_TERMINAL,
                    0x56 => platform::COMMAND_PASTE,
                    0x58 => platform::COMMAND_CUT,
                    0x59 => platform::COMMAND_REDO,
                    0x5a => platform::COMMAND_UNDO,
                    0xbb | 0x6b => platform::COMMAND_FONT_INCREASE,
                    0xbd | 0x6d => platform::COMMAND_FONT_DECREASE,
                    0x53 => {
                        APP.with(|cell| cell.borrow_mut().save());
                        invalidate(window);
                        return 0;
                    }
                    _ => 0,
                };
                if command != 0 {
                    perform_command(window, command);
                    return 0;
                }
            }
            APP.with(|cell| {
                let mut app = cell.borrow_mut();
                if app.create_kind.is_some() {
                    match w_param as u32 {
                        0x0d => app.create_requested_item(),
                        0x1b => {
                            app.create_kind = None;
                            app.create_error.clear();
                        }
                        VK_BACK => {
                            app.create_name.pop();
                            app.create_error.clear();
                        }
                        VK_UP => {
                            app.project_language = app.project_language.saturating_sub(1);
                            app.project_environment = 0;
                        }
                        VK_DOWN => {
                            app.project_language = (app.project_language + 1)
                                % WindowsAppState::PROJECT_LANGUAGES.len();
                            app.project_environment = 0;
                        }
                        VK_LEFT => {
                            app.project_environment = app.project_environment.saturating_sub(1);
                        }
                        VK_RIGHT => {
                            let environment_count = app.project_environments().len();
                            app.project_environment =
                                (app.project_environment + 1) % environment_count;
                        }
                        _ => return,
                    }
                    app.save_preferences();
                    return;
                }
                if app.settings_open {
                    match w_param as u32 {
                        0x1b => app.settings_open = false,
                        0x4c => app.ui_chinese = !app.ui_chinese,
                        0x54 => app.light_theme = !app.light_theme,
                        0x42 => {
                            if let Some(path) = platform::choose_background(window) {
                                app.background_path = Some(path);
                                app.status = "Background image loaded with Windows WIC".into();
                            }
                        }
                        0x43 => app.background_path = None,
                        0x4f => app.background_opacity = (app.background_opacity - 0.05).max(0.05),
                        0x50 => app.background_opacity = (app.background_opacity + 0.05).min(1.0),
                        0x41 => {
                            app.background_offset_x = (app.background_offset_x - 0.05).max(-1.0)
                        }
                        0x44 => app.background_offset_x = (app.background_offset_x + 0.05).min(1.0),
                        0x57 => {
                            app.background_offset_y = (app.background_offset_y - 0.05).max(-1.0)
                        }
                        0x53 => app.background_offset_y = (app.background_offset_y + 0.05).min(1.0),
                        0x5a => app.background_zoom = (app.background_zoom - 0.1).max(1.0),
                        0x58 => app.background_zoom = (app.background_zoom + 0.1).min(3.0),
                        VK_LEFT => app.code_zoom = (app.code_zoom - 1).max(-2),
                        VK_RIGHT => app.code_zoom = (app.code_zoom + 1).min(8),
                        _ => return,
                    }
                    app.save_preferences();
                    RENDERER.with(|renderer| *renderer.borrow_mut() = None);
                    return;
                }
                if app.extensions_open {
                    match w_param as u32 {
                        0x1b => app.extensions_open = false,
                        VK_LEFT => {
                            app.extension_category = app.extension_category.saturating_sub(1)
                        }
                        VK_RIGHT => app.extension_category = (app.extension_category + 1) % 5,
                        VK_UP => app.selected_language = app.selected_language.saturating_sub(1),
                        VK_DOWN => {
                            app.selected_language = (app.selected_language + 1)
                                % WindowsAppState::PROJECT_LANGUAGES.len()
                        }
                        0x20 if app.extension_category == 0 => {
                            let index = app.selected_language;
                            app.language_installed[index] = !app.language_installed[index];
                        }
                        0x56 if app.extension_category == 0 => {
                            let index = app.selected_language;
                            app.language_versions[index] = (app.language_versions[index] + 1) % 3;
                        }
                        _ => return,
                    }
                    app.save_preferences();
                    return;
                }
                if app.workspace_search_focused {
                    match w_param as u32 {
                        0x1b | 0x0d => app.workspace_search_focused = false,
                        VK_BACK => {
                            app.workspace_search.pop();
                            app.project_scroll = 0;
                        }
                        _ => return,
                    }
                    return;
                }
                if !app.terminal_open {
                    let old = app.position();
                    match w_param as u32 {
                        VK_BACK => app.backspace(),
                        VK_DELETE => app.delete_forward(),
                        VK_TAB => app.accept_completion(),
                        VK_HOME => app.caret_column = 0,
                        VK_END => {
                            app.caret_column = app.lines[app.caret_line].chars().count();
                        }
                        VK_LEFT => app.move_left(),
                        VK_RIGHT => app.move_right(),
                        VK_UP => app.caret_line = app.caret_line.saturating_sub(1),
                        VK_DOWN => app.caret_line += 1,
                        _ => return,
                    }
                    app.clamp_caret();
                    if app.editing_comment.is_some() {
                        app.clamp_comment_caret();
                    }
                    if !matches!(w_param as u32, VK_BACK | VK_DELETE) {
                        if platform::shift_pressed() {
                            if app.selection_anchor.is_none() {
                                app.selection_anchor = Some(old);
                            }
                        } else {
                            app.selection_anchor = None;
                        }
                    }
                } else {
                    let active = app.active_terminal;
                    let sequence = match w_param as u32 {
                        VK_UP => "\u{1b}[A",
                        VK_DOWN => "\u{1b}[B",
                        VK_RIGHT => "\u{1b}[C",
                        VK_LEFT => "\u{1b}[D",
                        VK_HOME => "\u{1b}[H",
                        VK_END => "\u{1b}[F",
                        VK_DELETE => "\u{1b}[3~",
                        VK_TAB => "\t",
                        _ => return,
                    };
                    if let Some(tab) = app.terminals.get(active) {
                        let _ = tab.session.write(sequence);
                    }
                }
            });
            invalidate(window);
            0
        }
        WM_LBUTTONDOWN => {
            let x = (l_param as u16 as i16) as i32;
            let y = ((l_param >> 16) as u16 as i16) as i32;
            let mut area = unsafe { std::mem::zeroed::<Rect>() };
            unsafe { GetClientRect(window, &mut area) };
            let scale = platform::dpi_for_window(window) as f32 / 96.0;
            if let Some((line, toggle)) = comment_bubble_at(window, x, y) {
                APP.with(|cell| {
                    let mut app = cell.borrow_mut();
                    if toggle {
                        if !app.expanded_comments.remove(&line) {
                            app.expanded_comments.insert(line);
                        }
                    } else if let Some((_, _, end)) = app
                        .lines
                        .get(line)
                        .and_then(|value| editor_core::comment_bounds(value, &app.file_name))
                    {
                        app.editing_comment = Some(line);
                        app.caret_line = line;
                        app.caret_column = app.lines[line][..end].chars().count();
                        app.selection_anchor = None;
                        app.status = "Comment bubble edit mode: boundaries protected".into();
                    }
                });
                invalidate(window);
                return 0;
            }
            let terminal_top = APP.with(|cell| {
                let motion = cell.borrow().terminal_motion.clamp(0.0, 1.0);
                area.bottom - ((28.0 + 214.0 * motion) * scale) as i32
            });
            if y >= terminal_top && y <= terminal_top + (34.0 * scale) as i32 {
                APP.with(|cell| cell.borrow_mut().toggle_terminal());
                invalidate(window);
            } else if x as f32 >= area.right as f32 - 168.0 * scale
                && y as f32 >= terminal_top as f32 + 38.0 * scale
            {
                APP.with(|cell| {
                    let mut app = cell.borrow_mut();
                    if !app.terminal_open {
                        return;
                    }
                    let local_y = y as f32 - terminal_top as f32 - 38.0 * scale;
                    if local_y <= 30.0 * scale {
                        if x as f32 <= area.right as f32 - 82.0 * scale {
                            app.add_terminal();
                        } else if platform::open_external_terminal(window, &app.project_root) {
                            app.status = format!(
                                "External Windows terminal opened at {}",
                                app.project_root.display()
                            );
                        } else {
                            app.status = "Could not open Windows Terminal or PowerShell".into();
                        }
                    } else {
                        let index = ((local_y / scale - 34.0) / 30.0).floor() as usize;
                        if index < app.terminals.len() {
                            app.active_terminal = index;
                            app.status = format!("Active terminal: {}", app.terminals[index].name);
                        }
                    }
                });
                invalidate(window);
            } else if x as f32 <= (area.right as f32 * 0.19).max(220.0 * scale) {
                if y as f32 >= 56.0 * scale && y as f32 <= 88.0 * scale {
                    APP.with(|cell| {
                        let mut app = cell.borrow_mut();
                        app.workspace_search_focused = true;
                        app.create_kind = None;
                    });
                    invalidate(window);
                    return 0;
                }
                let first_row = (48.0 + 58.0) * scale;
                if y as f32 >= first_row {
                    let row = ((y as f32 - first_row) / (35.0 * scale)).floor() as usize;
                    if row >= 2 {
                        APP.with(|cell| {
                            let mut app = cell.borrow_mut();
                            let query = app.workspace_search.to_ascii_lowercase();
                            let path = app
                                .project_entries
                                .iter()
                                .zip(&app.project_entry_paths)
                                .zip(&app.project_entry_is_dir)
                                .filter(|((entry, _), _)| {
                                    query.is_empty() || entry.to_ascii_lowercase().contains(&query)
                                })
                                .nth(app.project_scroll + row - 2)
                                .map(|((_, path), is_dir)| (path.clone(), *is_dir));
                            if let Some((path, is_directory)) = path {
                                if is_directory {
                                    if !app.collapsed_directories.remove(&path) {
                                        app.collapsed_directories.insert(path);
                                    }
                                    app.refresh_project();
                                } else {
                                    app.open_file(path);
                                }
                            }
                        });
                        invalidate(window);
                    }
                }
            } else if let Some(position) = editor_position_at(window, x, y) {
                APP.with(|cell| {
                    let mut app = cell.borrow_mut();
                    app.editing_comment = None;
                    app.set_caret(position, platform::shift_pressed());
                    app.dragging_selection = true;
                });
                unsafe { SetCapture(window) };
                invalidate(window);
            }
            0
        }
        WM_MOUSEWHEEL => {
            let delta = ((w_param >> 16) as u16 as i16) as i32;
            APP.with(|cell| {
                let mut app = cell.borrow_mut();
                if delta > 0 {
                    app.project_scroll = app.project_scroll.saturating_sub(3);
                } else if delta < 0 {
                    let query = app.workspace_search.to_ascii_lowercase();
                    let count = app
                        .project_entries
                        .iter()
                        .filter(|entry| {
                            query.is_empty() || entry.to_ascii_lowercase().contains(&query)
                        })
                        .count();
                    app.project_scroll = (app.project_scroll + 3).min(count.saturating_sub(1));
                }
            });
            invalidate(window);
            0
        }
        WM_MOUSEMOVE => {
            let dragging = APP.with(|cell| cell.borrow().dragging_selection);
            let x = (l_param as u16 as i16) as i32;
            let y = ((l_param >> 16) as u16 as i16) as i32;
            APP.with(|cell| cell.borrow_mut().pointer_x = x);
            if dragging && w_param & 1 != 0 {
                if let Some(position) = editor_position_at(window, x, y) {
                    APP.with(|cell| cell.borrow_mut().set_caret(position, true));
                    invalidate(window);
                }
            } else {
                let position = editor_position_at(window, x, y);
                let changed = APP.with(|cell| {
                    let mut app = cell.borrow_mut();
                    let hovered = position.and_then(|position| {
                        editor_core::symbol_at(
                            &app.lines,
                            &app.file_name,
                            position.line,
                            position.column,
                        )
                    });
                    if hovered == app.hovered_symbol {
                        false
                    } else {
                        app.hovered_symbol = hovered;
                        true
                    }
                });
                if changed {
                    invalidate(window);
                }
            }
            0
        }
        WM_LBUTTONUP => {
            APP.with(|cell| cell.borrow_mut().dragging_selection = false);
            unsafe { ReleaseCapture() };
            0
        }
        WM_SIZE => {
            let width = (l_param as u32 & 0xffff).max(1);
            let height = ((l_param as u32 >> 16) & 0xffff).max(1);
            RENDERER.with(|cell| {
                if let Some(renderer) = cell.borrow_mut().as_mut() {
                    unsafe { renderer.resize(width, height, platform::dpi_for_window(window)) };
                }
            });
            APP.with(|cell| {
                let app = cell.borrow();
                for terminal in &app.terminals {
                    terminal
                        .session
                        .resize((width / 9).min(i16::MAX as u32) as i16, 30);
                }
            });
            invalidate(window);
            0
        }
        WM_TIMER => {
            APP.with(|cell| cell.borrow_mut().update_motion());
            invalidate(window);
            0
        }
        WM_DPICHANGED => {
            platform::apply_dpi_change(window, l_param);
            RENDERER.with(|cell| *cell.borrow_mut() = None);
            APP.with(|cell| {
                cell.borrow_mut().status =
                    format!("DPI changed to {}", platform::dpi_for_window(window))
            });
            invalidate(window);
            0
        }
        WM_DESTROY => {
            APP.with(|cell| cell.borrow_mut().terminals.clear());
            RENDERER.with(|cell| *cell.borrow_mut() = None);
            unsafe { PostQuitMessage(0) };
            0
        }
        _ => unsafe { DefWindowProcW(window, message, w_param, l_param) },
    }
}

pub fn run() {
    unsafe {
        platform::enable_per_monitor_dpi();
        let instance = GetModuleHandleW(ptr::null());
        let class_name = wide("TC_IDE_WINDOWS_BACKEND");
        let title = wide("TC-IDE — DirectWrite / Direct2D / ConPTY");
        let class = WindowClass {
            style: CS_HREDRAW | CS_VREDRAW,
            window_proc: Some(window_proc),
            class_extra: 0,
            window_extra: 0,
            instance,
            icon: ptr::null_mut(),
            cursor: LoadCursorW(ptr::null_mut(), IDC_ARROW),
            background: ptr::null_mut(),
            menu_name: ptr::null(),
            class_name: class_name.as_ptr(),
        };
        if RegisterClassW(&class) == 0 {
            return;
        }
        let window = CreateWindowExW(
            0,
            class_name.as_ptr(),
            title.as_ptr(),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            1440,
            900,
            ptr::null_mut(),
            ptr::null_mut(),
            instance,
            ptr::null_mut(),
        );
        if window.is_null() {
            return;
        }
        platform::install_native_menu(window);
        SetTimer(window, 1, 16, ptr::null());
        ShowWindow(window, SW_SHOW);
        UpdateWindow(window);
        let mut message: Msg = std::mem::zeroed();
        while GetMessageW(&mut message, ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{TextPosition, WindowsAppState};

    #[test]
    fn multiline_selection_cut_and_undo_are_lossless() {
        let mut app = WindowsAppState::new();
        app.lines = vec!["alpha".into(), "beta".into()];
        app.selection_anchor = Some(TextPosition { line: 0, column: 2 });
        app.caret_line = 1;
        app.caret_column = 2;

        assert_eq!(app.cut_selection().as_deref(), Some("pha\nbe"));
        assert_eq!(app.lines, ["alta"]);
        app.undo();
        assert_eq!(app.lines, ["alpha", "beta"]);
    }

    #[test]
    fn unicode_edits_stay_on_character_boundaries() {
        let mut app = WindowsAppState::new();
        app.lines = vec!["你好 world".into()];
        app.caret_line = 0;
        app.caret_column = 2;
        app.insert_text("真的");
        assert_eq!(app.lines, ["你好真的 world"]);
        app.backspace();
        assert_eq!(app.lines, ["你好真 world"]);
    }

    #[test]
    fn arrow_navigation_crosses_line_boundaries() {
        let mut app = WindowsAppState::new();
        app.lines = vec!["one".into(), "two".into()];
        app.caret_line = 1;
        app.caret_column = 0;
        app.move_left();
        assert_eq!((app.caret_line, app.caret_column), (0, 3));
        app.move_right();
        assert_eq!((app.caret_line, app.caret_column), (1, 0));
    }
}
