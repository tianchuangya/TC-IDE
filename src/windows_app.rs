#![allow(clippy::upper_case_acronyms)]

use std::cell::RefCell;
use std::collections::HashSet;
use std::ffi::c_void;
use std::fs;
use std::path::PathBuf;
use std::ptr;
use std::time::Instant;

use crate::editor_core;
use crate::terminal_core;

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
const WM_CLOSE: Uint = 0x0010;
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
const WM_RBUTTONDOWN: Uint = 0x0204;
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
    code_font_family: usize,
    font_search: String,
    font_search_focused: bool,
    settings_open: bool,
    settings_motion: f32,
    settings_velocity: f32,
    extensions_open: bool,
    extensions_motion: f32,
    extensions_velocity: f32,
    extension_category: usize,
    selected_language: usize,
    extension_search: String,
    extension_search_focused: bool,
    extension_scroll: usize,
    language_installed: Vec<bool>,
    language_versions: Vec<usize>,
    ai_endpoint: String,
    ai_model: String,
    ai_key: String,
    ai_field_focused: Option<usize>,
    hovered_symbol: Option<editor_core::SymbolOccurrence>,
    expanded_comments: HashSet<usize>,
    editing_comment: Option<usize>,
    pointer_x: i32,
    graph_scroll: usize,
    background_path: Option<PathBuf>,
    background_opacity: f32,
    background_offset_x: f32,
    background_offset_y: f32,
    background_zoom: f32,
    package_open: bool,
    package_motion: f32,
    package_velocity: f32,
    package_targets: [bool; 3],
    package_running: bool,
    package_progress: f32,
    package_started: Option<Instant>,
    package_marker: String,
    package_failure_marker: String,
    package_reveal_pending: bool,
    terminal_open: bool,
    terminal_motion: f32,
    terminal_velocity: f32,
    terminals: Vec<TerminalTab>,
    active_terminal: usize,
    terminal_grid: terminal_core::GridSize,
    terminal_grid_signature: (u32, u32, u32, i32, usize),
    detached_terminal: Hwnd,
    status: String,
    started: Instant,
    last_frame: Instant,
    last_edit: Option<Instant>,
    create_motion: f32,
    create_velocity: f32,
}

impl WindowsAppState {
    const PROJECT_LANGUAGES: [&'static str; 7] =
        ["Rust", "Python", "C", "C++", "Java", "Go", "Web"];
    const EXTENSION_LANGUAGES: [&'static str; 16] = [
        "Rust", "Python", "C", "C++", "Java", "Go", "Web", "C#", "Kotlin", "Swift", "Zig", "Lua",
        "Ruby", "PHP", "SQL", "Shell",
    ];
    const CODE_FONTS: [&'static str; 9] = [
        "Cascadia Mono",
        "JetBrains Mono",
        "Fira Code",
        "Cascadia Code",
        "Source Code Pro",
        "Hack",
        "Consolas",
        "Noto Sans Mono CJK SC",
        "DejaVu Sans Mono",
    ];

    fn filtered_font_indices(&self) -> Vec<usize> {
        let needle = self.font_search.trim().to_ascii_lowercase();
        Self::CODE_FONTS
            .iter()
            .enumerate()
            .filter(|(_, family)| {
                needle.is_empty() || family.to_ascii_lowercase().contains(&needle)
            })
            .map(|(index, _)| index)
            .collect()
    }

    fn filtered_language_indices(&self) -> Vec<usize> {
        let needle = self.extension_search.trim().to_ascii_lowercase();
        Self::EXTENSION_LANGUAGES
            .iter()
            .enumerate()
            .filter(|(_, language)| {
                needle.is_empty() || language.to_ascii_lowercase().contains(&needle)
            })
            .map(|(index, _)| index)
            .collect()
    }

    fn project_language_name(&self) -> &'static str {
        Self::PROJECT_LANGUAGES[self.project_language.min(Self::PROJECT_LANGUAGES.len() - 1)]
    }

    fn language_version_values(index: usize) -> &'static [&'static str; 3] {
        match index {
            0 => &["stable", "beta", "nightly"],
            1 => &["3.13", "3.12", "3.11"],
            2 => &["C23", "C17", "C11"],
            3 => &["C++23", "C++20", "C++17"],
            4 => &["25", "21 LTS", "17 LTS"],
            5 => &["1.25", "1.24", "1.23"],
            _ => &["ES2025", "ES2024", "ES2023"],
        }
    }

    fn project_version_name(&self) -> &'static str {
        let versions = Self::language_version_values(self.project_language);
        versions[self
            .language_versions
            .get(self.project_language)
            .copied()
            .unwrap_or(0)
            .min(2)]
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
        let package_targets = Self::load_package_targets(&project_root);
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
            code_font_family: preferences.code_font_family.min(Self::CODE_FONTS.len() - 1),
            font_search: String::new(),
            font_search_focused: false,
            settings_open: false,
            settings_motion: 0.0,
            settings_velocity: 0.0,
            extensions_open: false,
            extensions_motion: 0.0,
            extensions_velocity: 0.0,
            extension_category: 0,
            selected_language: 0,
            extension_search: String::new(),
            extension_search_focused: false,
            extension_scroll: 0,
            language_installed: (0..Self::EXTENSION_LANGUAGES.len())
                .map(|index| {
                    preferences
                        .language_installed
                        .get(index)
                        .copied()
                        .unwrap_or(false)
                })
                .collect(),
            language_versions: (0..Self::EXTENSION_LANGUAGES.len())
                .map(|index| {
                    preferences
                        .language_versions
                        .get(index)
                        .copied()
                        .unwrap_or(0)
                        .min(2)
                })
                .collect(),
            ai_endpoint: preferences.ai_endpoint,
            ai_model: preferences.ai_model,
            ai_key: String::new(),
            ai_field_focused: None,
            hovered_symbol: None,
            expanded_comments: HashSet::new(),
            editing_comment: None,
            pointer_x: -1,
            graph_scroll: 0,
            background_path: preferences.background_path,
            background_opacity: preferences.background_opacity,
            background_offset_x: preferences.background_offset_x,
            background_offset_y: preferences.background_offset_y,
            background_zoom: preferences.background_zoom,
            package_open: false,
            package_motion: 0.0,
            package_velocity: 0.0,
            package_targets,
            package_running: false,
            package_progress: 0.0,
            package_started: None,
            package_marker: String::new(),
            package_failure_marker: String::new(),
            package_reveal_pending: false,
            terminal_open: false,
            terminal_motion: 0.0,
            terminal_velocity: 0.0,
            terminals: Vec::new(),
            active_terminal: 0,
            terminal_grid: terminal_core::GridSize {
                columns: 120,
                rows: 30,
            },
            terminal_grid_signature: (0, 0, 0, 0, 0),
            detached_terminal: ptr::null_mut(),
            status: "DirectWrite / Direct2D editor ready".into(),
            started: Instant::now(),
            last_frame: Instant::now(),
            last_edit: None,
            create_motion: 0.0,
            create_velocity: 0.0,
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
        self.package_targets = Self::load_package_targets(&self.project_root);
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
            code_font_family: self.code_font_family,
            project_language: self.project_language,
            project_environment: self.project_environment,
            language_installed: self.language_installed.clone(),
            language_versions: self.language_versions.clone(),
            background_path: self.background_path.clone(),
            background_opacity: self.background_opacity,
            background_offset_x: self.background_offset_x,
            background_offset_y: self.background_offset_y,
            background_zoom: self.background_zoom,
            ai_endpoint: self.ai_endpoint.clone(),
            ai_model: self.ai_model.clone(),
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

    fn create_project_template(&self, target: &std::path::Path) -> std::io::Result<()> {
        let language = self.project_language_name();
        fs::create_dir_all(target)?;
        match language {
            "Rust" => {
                fs::create_dir_all(target.join("src"))?;
                fs::write(target.join("Cargo.toml"), "[package]\nname = \"tc-project\"\nversion = \"0.1.0\"\nedition = \"2024\"\n")?;
                fs::write(target.join("src/main.rs"), "fn main() {\n    println!(\"Hello from TC-IDE\");\n}\n")?;
            }
            "Python" => {
                fs::write(target.join("main.py"), "print(\"Hello from TC-IDE\")\n")?;
                fs::write(target.join(".gitignore"), ".venv/\n__pycache__/\n*.pyc\n")?;
            }
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
                "language={language}\nversion={}\nenvironment={}\n",
                self.project_version_name(),
                self.project_environments()[self
                    .project_environment
                    .min(self.project_environments().len() - 1)]
            ),
        )
    }

    fn start_project_environment_setup(&mut self) {
        let language = self.project_language_name();
        let environment = self.project_environments()[self
            .project_environment
            .min(self.project_environments().len() - 1)];
        let version_index = self
            .language_versions
            .get(self.project_language)
            .copied()
            .unwrap_or(0);
        let python_version = ["3.13", "3.12", "3.11"][version_index.min(2)];
        let command = match (language, environment) {
            (_, "None") => return,
            ("Python", "venv") => format!(
                "$py=(Get-Command py -ErrorAction SilentlyContinue); if($py){{py -{python_version} -m venv .venv}}else{{python -m venv .venv}}"
            ),
            ("Python", "uv") => format!("uv venv --python {python_version} .venv"),
            ("Python", "conda") => {
                format!("conda create -y -p .venv python={python_version}")
            }
            ("Python", "poetry") => format!("poetry env use {python_version}; poetry install"),
            ("Rust", "cargo nightly") => "rustup override set nightly".into(),
            ("Rust", "cargo stable") => "rustup override set stable".into(),
            ("Go", "Go modules") => "if(!(Test-Path go.mod)){go mod init tc-project}".into(),
            ("Web", manager) => format!("{manager} install"),
            _ => {
                self.status = format!(
                    "Project created; {} uses the system toolchain ({environment})",
                    language
                );
                return;
            }
        };
        let cache_root = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| self.project_root.clone())
            .join("TC-IDE")
            .join("runtime-cache")
            .join(language.to_ascii_lowercase())
            .join(if language == "Python" {
                python_version
            } else {
                environment
            });
        let _ = fs::create_dir_all(&cache_root);
        let _ = fs::write(
            cache_root.join("runtime.conf"),
            format!("language={language}\nversion={python_version}\nenvironment={environment}\n"),
        );
        self.terminal_open = true;
        self.ensure_terminal();
        if let Some(tab) = self.terminals.first() {
            let wrapped = format!(
                "$ErrorActionPreference='Stop'; Write-Host 'TC-IDE: preparing {language} {environment}'; try{{{command}; Write-Host 'TC-IDE: environment ready'}}catch{{Write-Host ('TC-IDE: environment setup failed: '+$_.Exception.Message)}}\r"
            );
            let _ = tab.session.write(&wrapped);
            self.active_terminal = 0;
            self.status = format!("Preparing {language} {environment} in the Build terminal");
        }
    }

    fn package_settings_path(root: &std::path::Path) -> PathBuf {
        root.join(".tc-ide-package-targets")
    }

    fn load_package_targets(root: &std::path::Path) -> [bool; 3] {
        let mut targets = [false; 3];
        if let Ok(value) = fs::read_to_string(Self::package_settings_path(root)) {
            for (index, item) in value.trim().split(',').take(3).enumerate() {
                targets[index] = item == "1";
            }
        }
        targets
    }

    fn save_package_targets(&self) {
        let value = self
            .package_targets
            .iter()
            .map(|selected| if *selected { "1" } else { "0" })
            .collect::<Vec<_>>()
            .join(",");
        let _ = fs::write(Self::package_settings_path(&self.project_root), value);
    }

    fn start_package(&mut self) {
        if !self.package_targets.iter().any(|selected| *selected) {
            self.status = "Select at least one package target".into();
            return;
        }
        self.terminal_open = true;
        self.ensure_terminal();
        let project = self
            .project_root
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "TC-IDE-project".into())
            .replace(['\'', '"'], "");
        let file = self
            .file_path
            .as_ref()
            .and_then(|path| path.file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "main".into())
            .replace('"', "");
        let stage = format!(".tc-ide-packages\\{project}");
        let is_rust = self.project_root.join("Cargo.toml").exists();
        let is_go = self.project_root.join("go.mod").exists();
        let language = editor_core::language_for_file(&self.file_name);
        let supports_windows_executable =
            is_rust || is_go || matches!(language, "C" | "C++" | "Python");
        if self.package_targets[2] && !supports_windows_executable {
            self.status = format!(
                "MSIX requires a Windows executable; {} packaging is not configured yet",
                language
            );
            return;
        }
        if self.package_targets[2] {
            let stage_path = self.project_root.join(".tc-ide-packages").join(&project);
            let assets = stage_path.join("Assets");
            let identity = project
                .chars()
                .map(|value| {
                    if value.is_ascii_alphanumeric() {
                        value
                    } else {
                        '.'
                    }
                })
                .collect::<String>();
            let identity = format!("TianChuangYa.{}", identity.trim_matches('.'));
            let display = project.replace(['&', '<', '>'], "");
            let manifest = format!(
                r#"<?xml version="1.0" encoding="utf-8"?>
<Package xmlns="http://schemas.microsoft.com/appx/manifest/foundation/windows10" xmlns:uap="http://schemas.microsoft.com/appx/manifest/uap/windows10" xmlns:rescap="http://schemas.microsoft.com/appx/manifest/foundation/windows10/restrictedcapabilities" IgnorableNamespaces="uap rescap">
  <Identity Name="{identity}" Publisher="CN=TianChuangYa" Version="0.2.0.0" />
  <Properties><DisplayName>{display}</DisplayName><PublisherDisplayName>TianChuangYa</PublisherDisplayName><Logo>Assets\StoreLogo.png</Logo></Properties>
  <Resources><Resource Language="en-us" /></Resources>
  <Dependencies><TargetDeviceFamily Name="Windows.Desktop" MinVersion="10.0.17763.0" MaxVersionTested="10.0.26100.0" /></Dependencies>
  <Applications><Application Id="TCIDE" Executable="{project}.exe" EntryPoint="Windows.FullTrustApplication"><uap:VisualElements DisplayName="{display}" Description="TC-IDE packaged project" BackgroundColor="transparent" Square150x150Logo="Assets\Square150x150Logo.png" Square44x44Logo="Assets\Square44x44Logo.png" /></Application></Applications>
  <Capabilities><rescap:Capability Name="runFullTrust" /></Capabilities>
</Package>
"#
            );
            let prepared = fs::create_dir_all(&assets)
                .and_then(|()| fs::write(stage_path.join("AppxManifest.xml"), manifest))
                .and_then(|()| {
                    fs::write(
                        assets.join("StoreLogo.png"),
                        include_bytes!("../assets/tc-ide-icon.png"),
                    )
                })
                .and_then(|()| {
                    fs::write(
                        assets.join("Square150x150Logo.png"),
                        include_bytes!("../assets/tc-ide-icon.png"),
                    )
                })
                .and_then(|()| {
                    fs::write(
                        assets.join("Square44x44Logo.png"),
                        include_bytes!("../assets/tc-ide-icon.png"),
                    )
                });
            if let Err(error) = prepared {
                self.status = format!("Could not prepare MSIX layout: {error}");
                return;
            }
        }
        let build = if is_rust {
            format!(
                "cargo build --release; $a=Get-ChildItem .\\target\\release\\*.exe | Where-Object {{$_.DirectoryName -notmatch 'deps'}} | Select-Object -First 1; if($a){{Copy-Item $a.FullName '{stage}\\{project}.exe' -Force}}"
            )
        } else if is_go {
            format!("go build -o '{stage}\\{project}.exe' .")
        } else {
            match language {
                "C" => format!("gcc \"{file}\" -o '{stage}\\{project}.exe'"),
                "C++" => format!("g++ \"{file}\" -o '{stage}\\{project}.exe'"),
                "Python" => format!("python -m PyInstaller --noconfirm --onefile --name \"{project}\" \"{file}\" --distpath '{stage}' --workpath .tc-ide-packages\\work --specpath .tc-ide-packages\\spec"),
                "Java" => format!("javac \"{file}\"; jar --create --file '{stage}\\{project}.jar' *.class"),
                _ => self.task_command(true),
            }
        };
        let zip = if self.package_targets[1] {
            format!("; Compress-Archive -Path '{stage}\\*' -DestinationPath '.tc-ide-packages\\{project}.zip' -Force")
        } else {
            String::new()
        };
        let msix = if self.package_targets[2] {
            format!("; $makeappx=(Get-Command MakeAppx.exe -ErrorAction SilentlyContinue).Source; if(!$makeappx){{$makeappx=Get-ChildItem \"${{env:ProgramFiles(x86)}}\\Windows Kits\\10\\bin\\*\\x64\\makeappx.exe\" -ErrorAction SilentlyContinue | Sort-Object FullName -Descending | Select-Object -First 1 -ExpandProperty FullName}}; if(!$makeappx){{throw 'Windows SDK MakeAppx.exe is not installed'}}; & $makeappx pack /d '{stage}' /p '.tc-ide-packages\\{project}.msix' /o")
        } else {
            String::new()
        };
        self.package_marker = format!("__TC_PACKAGE_DONE_{}__", self.started.elapsed().as_nanos());
        self.package_failure_marker = format!(
            "__TC_PACKAGE_FAILED_{}__",
            self.started.elapsed().as_nanos()
        );
        let command = format!(
            "$ErrorActionPreference='Stop'; try{{New-Item -ItemType Directory -Force '{stage}' | Out-Null; {build}{zip}{msix}; Write-Output '{}'}}catch{{Write-Host $_.Exception.Message; Write-Output '{}'}}",
            self.package_marker, self.package_failure_marker
        );
        if let Some(tab) = self.terminals.first() {
            let _ = tab.session.write(&format!("{command}\r"));
            self.active_terminal = 0;
            self.package_running = true;
            self.package_progress = 0.0;
            self.package_started = Some(Instant::now());
            self.package_open = false;
            self.status = "Packaging started in the Build terminal".into();
        }
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
        let grid = self.terminal_grid;
        match unsafe { ConPtySession::spawn(&cwd, grid.columns, grid.rows) } {
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

    fn task_command(&self, build: bool) -> String {
        let quoted_file = self
            .file_path
            .as_ref()
            .and_then(|path| path.file_name())
            .map(|name| format!("\"{}\"", name.to_string_lossy().replace('"', "\\\"")))
            .unwrap_or_else(|| "\"main\"".into());
        if self.project_root.join("Cargo.toml").exists() {
            return if build { "cargo build" } else { "cargo run" }.into();
        }
        if self.project_root.join("package.json").exists() {
            return if build {
                "npm run build"
            } else {
                "npm run dev"
            }
            .into();
        }
        if self.project_root.join("go.mod").exists() {
            return if build { "go build ./..." } else { "go run ." }.into();
        }
        match editor_core::language_for_file(&self.file_name) {
            "Python" => {
                if build {
                    "python -m compileall .".into()
                } else {
                    format!("python {quoted_file}")
                }
            }
            "C" => {
                if build {
                    format!("gcc {quoted_file} -o .\\tc-ide-build.exe")
                } else {
                    ".\\tc-ide-build.exe".into()
                }
            }
            "C++" => {
                if build {
                    format!("g++ {quoted_file} -o .\\tc-ide-build.exe")
                } else {
                    ".\\tc-ide-build.exe".into()
                }
            }
            "Java" => {
                if build {
                    format!("javac {quoted_file}")
                } else {
                    let class = self
                        .file_path
                        .as_ref()
                        .and_then(|path| path.file_stem())
                        .map(|name| name.to_string_lossy().into_owned())
                        .unwrap_or_else(|| "Main".into());
                    format!("java {class}")
                }
            }
            _ => format!(
                "Write-Host 'No {} task configured for this project'",
                if build { "build" } else { "run" }
            ),
        }
    }

    fn run_task(&mut self, build: bool) {
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

    fn probe_extension_tool(&mut self, label: &str, command: &str) {
        self.terminal_open = true;
        self.ensure_terminal();
        let index = 2.min(self.terminals.len().saturating_sub(1));
        let executable = command.split_whitespace().next().unwrap_or(command);
        let script = format!(
            "if(Get-Command '{executable}' -ErrorAction SilentlyContinue){{Write-Host 'TC-IDE: {label} detected'; {command}}}else{{Write-Host 'TC-IDE: {label} is not installed or not on PATH'}}\r"
        );
        if let Some(tab) = self.terminals.get(index) {
            let _ = tab.session.write(&script);
            self.active_terminal = index;
            self.status = format!("Checking {label} in the Output terminal");
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
        Self::step_overlay_motion(
            &mut self.settings_motion,
            &mut self.settings_velocity,
            self.settings_open,
            dt,
        );
        Self::step_overlay_motion(
            &mut self.extensions_motion,
            &mut self.extensions_velocity,
            self.extensions_open,
            dt,
        );
        Self::step_overlay_motion(
            &mut self.package_motion,
            &mut self.package_velocity,
            self.package_open,
            dt,
        );
        Self::step_overlay_motion(
            &mut self.create_motion,
            &mut self.create_velocity,
            self.create_kind.is_some(),
            dt,
        );
        if self.package_running {
            self.package_progress = self
                .package_started
                .map(|started| (started.elapsed().as_secs_f32() / 8.0).min(0.9))
                .unwrap_or(0.0);
            let done = self.terminals.first().is_some_and(|tab| {
                tab.session.view.lock().ok().is_some_and(|view| {
                    view.lines
                        .iter()
                        .any(|line| line.contains(&self.package_marker))
                        || view.current.contains(&self.package_marker)
                })
            });
            let failed = self.terminals.first().is_some_and(|tab| {
                tab.session.view.lock().ok().is_some_and(|view| {
                    view.lines
                        .iter()
                        .any(|line| line.contains(&self.package_failure_marker))
                        || view.current.contains(&self.package_failure_marker)
                })
            });
            if failed {
                self.package_running = false;
                self.package_progress = 0.0;
                self.package_started = None;
                self.status = "Packaging failed; see the Build terminal for details".into();
            } else if done {
                self.package_running = false;
                self.package_progress = 1.0;
                self.package_reveal_pending = true;
                self.package_started = Some(Instant::now());
                self.status = "Packaging finished; opening the artifact folder".into();
            }
        } else if self.package_progress >= 1.0
            && self
                .package_started
                .is_some_and(|started| started.elapsed().as_secs_f32() > 1.8)
        {
            self.package_progress = 0.0;
            self.package_started = None;
        }
    }

    fn step_overlay_motion(value: &mut f32, velocity: &mut f32, open: bool, dt: f32) {
        let target = if open { 1.0 } else { 0.0 };
        *velocity += (target - *value) * 300.0 * dt;
        *velocity *= (-19.0 * dt).exp();
        *value += *velocity * dt;
        *value = value.clamp(-0.04, 1.04);
        if (target - *value).abs() < 0.0008 && velocity.abs() < 0.0008 {
            *value = target;
            *velocity = 0.0;
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
    fn DestroyWindow(window: Hwnd) -> Bool;
    fn DrawTextW(dc: Hdc, text: *const u16, count: i32, rect: *mut Rect, format: Uint) -> i32;
    fn EndPaint(window: Hwnd, paint: *const PaintStruct) -> Bool;
    fn FillRect(dc: Hdc, rect: *const Rect, brush: Hbrush) -> i32;
    fn GetClientRect(window: Hwnd, area: *mut Rect) -> Bool;
    fn GetMessageW(message: *mut Msg, window: Hwnd, minimum: Uint, maximum: Uint) -> Bool;
    fn InvalidateRect(window: Hwnd, area: *const Rect, erase: Bool) -> Bool;
    fn LoadCursorW(instance: Hinstance, cursor_name: *const u16) -> Hcursor;
    fn PostQuitMessage(exit_code: i32);
    fn RegisterClassW(class: *const WindowClass) -> u16;
    fn ReleaseCapture() -> Bool;
    fn SetCapture(window: Hwnd) -> Hwnd;
    fn SetForegroundWindow(window: Hwnd) -> Bool;
    fn SetTimer(window: Hwnd, event: usize, milliseconds: Uint, callback: *const c_void) -> usize;
    fn ShowWindow(window: Hwnd, command: i32) -> Bool;
    fn TranslateMessage(message: *const Msg) -> Bool;
    fn UpdateWindow(window: Hwnd) -> Bool;
}

#[link(name = "gdi32")]
extern "system" {
    fn CreateSolidBrush(color: Dword) -> Hbrush;
    fn DeleteObject(object: *mut c_void) -> Bool;
    fn GetStockObject(index: i32) -> *mut c_void;
    fn SelectObject(dc: Hdc, object: *mut c_void) -> *mut c_void;
    fn SetBkMode(dc: Hdc, mode: i32) -> i32;
    fn SetTextColor(dc: Hdc, color: Dword) -> Dword;
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
            platform::COMMAND_REFRESH_PROJECT => {
                app.refresh_project();
                app.status = "Project tree refreshed".into();
            }
            platform::COMMAND_SETTINGS => {
                app.settings_open = !app.settings_open;
                app.extensions_open = false;
            }
            platform::COMMAND_EXTENSIONS => {
                app.extensions_open = !app.extensions_open;
                app.settings_open = false;
            }
            platform::COMMAND_PACKAGE => {
                app.package_open = !app.package_open;
                app.settings_open = false;
                app.extensions_open = false;
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
                create_version: app.project_version_name(),
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
                code_font_family: app.code_font_family,
                code_font_name: WindowsAppState::CODE_FONTS[app.code_font_family],
                font_search: &app.font_search,
                font_search_focused: app.font_search_focused,
                font_options: &WindowsAppState::CODE_FONTS,
                settings_open: app.settings_open,
                settings_motion: app.settings_motion,
                extensions_open: app.extensions_open,
                extensions_motion: app.extensions_motion,
                extension_category: app.extension_category,
                selected_language: app.selected_language,
                extension_search: &app.extension_search,
                extension_search_focused: app.extension_search_focused,
                extension_scroll: app.extension_scroll,
                language_options: &WindowsAppState::EXTENSION_LANGUAGES,
                language_installed: &app.language_installed,
                language_versions: &app.language_versions,
                ai_endpoint: &app.ai_endpoint,
                ai_model: &app.ai_model,
                ai_key_present: !app.ai_key.is_empty(),
                ai_field_focused: app.ai_field_focused,
                symbols: &symbols,
                project_symbols: &app.project_symbols,
                hovered_symbol: app.hovered_symbol.as_ref(),
                graph_scroll: app.graph_scroll,
                expanded_comments: &expanded_comments,
                editing_comment: app.editing_comment,
                pointer_x: app.pointer_x,
                package_open: app.package_open,
                package_motion: app.package_motion,
                package_targets: app.package_targets,
                package_running: app.package_running,
                package_progress: app.package_progress,
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
                create_motion: app.create_motion,
            };
            let mut renderer = renderer_cell.borrow_mut();
            if renderer.is_none() {
                *renderer = unsafe {
                    DirectEditorRenderer::new(
                        dpi,
                        app.code_zoom,
                        WindowsAppState::CODE_FONTS[app.code_font_family],
                    )
                }
                .ok();
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
        } else if app.font_search_focused {
            if !value.is_control() {
                app.font_search.push(value);
            }
        } else if let Some(field) = app.ai_field_focused {
            if !value.is_control() {
                match field {
                    0 => app.ai_endpoint.push(value),
                    1 => app.ai_model.push(value),
                    _ => app.ai_key.push(value),
                }
            }
        } else if app.extension_search_focused {
            if !value.is_control() {
                app.extension_search.push(value);
                app.extension_scroll = 0;
            }
        } else if app.settings_open || app.extensions_open || app.package_open {
            // Overlay shortcuts must never leak their printable WM_CHAR into the editor.
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

fn handle_terminal_control(window: Hwnd, key: u32) -> bool {
    if key == VK_TAB {
        APP.with(|cell| {
            let mut app = cell.borrow_mut();
            if !app.terminals.is_empty() {
                app.active_terminal = if platform::shift_pressed() {
                    app.active_terminal
                        .checked_sub(1)
                        .unwrap_or(app.terminals.len() - 1)
                } else {
                    (app.active_terminal + 1) % app.terminals.len()
                };
                app.status = format!(
                    "Active terminal: {}",
                    app.terminals[app.active_terminal].name
                );
            }
        });
        return true;
    }
    let paste = if key == 0x56 {
        platform::read_clipboard(window).ok()
    } else {
        None
    };
    let sequence = match key {
        0x41 => Some("\u{1}"),
        0x43 => Some("\u{3}"),
        0x4c => Some("\u{c}"),
        0x56 => paste.as_deref(),
        _ => return false,
    };
    if let Some(sequence) = sequence {
        APP.with(|cell| {
            let mut app = cell.borrow_mut();
            let active = app.active_terminal;
            if let Some(tab) = app.terminals.get_mut(active) {
                let _ = tab.session.write(sequence);
                if key == 0x56 {
                    tab.input.push_str(sequence);
                }
            }
        });
    }
    true
}

/// Grid the bottom terminal panel shows at the current client size.
fn panel_grid(window: Hwnd, code_zoom: i32) -> terminal_core::GridSize {
    let mut area = unsafe { std::mem::zeroed::<Rect>() };
    unsafe { GetClientRect(window, &mut area) };
    let scale = platform::dpi_for_window(window) as f32 / 96.0;
    let panel = terminal_core::panel_metrics(
        (area.right - area.left).max(1) as f32,
        (area.bottom - area.top).max(1) as f32,
        scale,
        1.0,
    );
    grid_for_panel(&panel, scale, code_zoom)
}

/// Fit a column/row grid into a laid out terminal panel body.
fn grid_for_panel(
    panel: &terminal_core::TerminalPanel,
    scale: f32,
    code_zoom: i32,
) -> terminal_core::GridSize {
    terminal_core::GridSize::from_metrics(
        (panel.body_right - panel.body_left).max(1.0),
        (panel.body_bottom - panel.body_top).max(1.0),
        terminal_core::code_cell_width(code_zoom) * scale,
        panel.row_height,
    )
}

/// Resize every ConPTY session when the client area, DPI, zoom or session set changed.
fn sync_terminal_grid(window: Hwnd) {
    let mut area = unsafe { std::mem::zeroed::<Rect>() };
    unsafe { GetClientRect(window, &mut area) };
    let dpi = platform::dpi_for_window(window);
    let (code_zoom, count) = APP.with(|cell| {
        let app = cell.borrow();
        (app.code_zoom, app.terminals.len())
    });
    let signature = (
        (area.right - area.left).max(0) as u32,
        (area.bottom - area.top).max(0) as u32,
        dpi,
        code_zoom,
        count,
    );
    let changed = APP.with(|cell| {
        let mut app = cell.borrow_mut();
        if app.terminal_grid_signature == signature {
            return false;
        }
        app.terminal_grid_signature = signature;
        true
    });
    if !changed {
        return;
    }
    let grid = panel_grid(window, code_zoom);
    APP.with(|cell| {
        let mut app = cell.borrow_mut();
        app.terminal_grid = grid;
        for terminal in &app.terminals {
            terminal.session.resize(grid.columns, grid.rows);
        }
    });
}

unsafe fn draw_terminal_text(dc: Hdc, value: &str, x: i32, y: i32, right: i32, color: Dword) {
    let wide = value.encode_utf16().collect::<Vec<_>>();
    let mut area = Rect {
        left: x,
        top: y,
        right,
        bottom: y + 24,
    };
    unsafe {
        SetTextColor(dc, color);
        DrawTextW(dc, wide.as_ptr(), wide.len() as i32, &mut area, 0x00000100);
    }
}

unsafe extern "system" fn detached_terminal_proc(
    window: Hwnd,
    message: Uint,
    w_param: Wparam,
    _l_param: Lparam,
) -> Lresult {
    match message {
        WM_PAINT => {
            let mut paint: PaintStruct = unsafe { std::mem::zeroed() };
            let dc = unsafe { BeginPaint(window, &mut paint) };
            let mut area: Rect = unsafe { std::mem::zeroed() };
            unsafe { GetClientRect(window, &mut area) };
            let brush = unsafe { CreateSolidBrush(0x001b1917) };
            unsafe {
                FillRect(dc, &area, brush);
                DeleteObject(brush.cast());
                SetBkMode(dc, 1);
            }
            let font = unsafe { GetStockObject(16) };
            let previous = unsafe { SelectObject(dc, font) };
            let (name, mut lines, input) = APP.with(|cell| {
                let app = cell.borrow();
                let Some(tab) = app.terminals.get(app.active_terminal) else {
                    return ("Terminal".to_string(), Vec::new(), String::new());
                };
                let lines = tab
                    .session
                    .view
                    .lock()
                    .ok()
                    .map(|view| {
                        let mut values = view.lines.clone();
                        if !view.current.is_empty() {
                            values.push(view.current.clone());
                        }
                        values
                    })
                    .unwrap_or_default();
                (tab.name.clone(), lines, tab.input.clone())
            });
            let visible = ((area.bottom - 94) / 22).max(1) as usize;
            if lines.len() > visible {
                lines.drain(..lines.len() - visible);
            }
            unsafe {
                draw_terminal_text(dc, "TianChuangYa", 24, 16, area.right - 20, 0x00e6bc71);
                draw_terminal_text(
                    dc,
                    &format!("TC-IDE · {name} · shared ConPTY session"),
                    24,
                    42,
                    area.right - 20,
                    0x009d9690,
                );
            }
            for (index, line) in lines.iter().enumerate() {
                unsafe {
                    draw_terminal_text(
                        dc,
                        line,
                        24,
                        76 + index as i32 * 22,
                        area.right - 20,
                        0x00e8e8e8,
                    )
                };
            }
            unsafe {
                draw_terminal_text(
                    dc,
                    &format!("> {input}▌"),
                    24,
                    area.bottom - 32,
                    area.right - 20,
                    0x00dfb56b,
                );
                SelectObject(dc, previous);
                EndPaint(window, &paint);
            }
            0
        }
        WM_CHAR => {
            if let Some(value) = char::from_u32(w_param as u32) {
                APP.with(|cell| {
                    let mut app = cell.borrow_mut();
                    let active = app.active_terminal;
                    if let Some(tab) = app.terminals.get_mut(active) {
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
                    }
                });
            }
            invalidate(window);
            0
        }
        WM_KEYDOWN => {
            if platform::control_pressed() && handle_terminal_control(window, w_param as u32) {
                invalidate(window);
                return 0;
            }
            let sequence = match w_param as u32 {
                VK_UP => Some("\u{1b}[A"),
                VK_DOWN => Some("\u{1b}[B"),
                VK_RIGHT => Some("\u{1b}[C"),
                VK_LEFT => Some("\u{1b}[D"),
                VK_HOME => Some("\u{1b}[H"),
                VK_END => Some("\u{1b}[F"),
                VK_DELETE => Some("\u{1b}[3~"),
                VK_TAB => Some("\t"),
                _ => None,
            };
            if let Some(sequence) = sequence {
                APP.with(|cell| {
                    let app = cell.borrow();
                    if let Some(tab) = app.terminals.get(app.active_terminal) {
                        let _ = tab.session.write(sequence);
                    }
                });
            }
            0
        }
        WM_TIMER | WM_SIZE => {
            invalidate(window);
            0
        }
        WM_CLOSE => {
            unsafe { DestroyWindow(window) };
            0
        }
        WM_DESTROY => {
            APP.with(|cell| cell.borrow_mut().detached_terminal = ptr::null_mut());
            0
        }
        _ => unsafe { DefWindowProcW(window, message, w_param, _l_param) },
    }
}

fn open_detached_terminal() -> bool {
    unsafe {
        let existing = APP.with(|cell| cell.borrow().detached_terminal);
        if !existing.is_null() {
            SetForegroundWindow(existing);
            return true;
        }
        APP.with(|cell| cell.borrow_mut().ensure_terminal());
        let instance = GetModuleHandleW(ptr::null());
        let class_name = wide("TC_IDE_DETACHED_TERMINAL");
        let class = WindowClass {
            style: CS_HREDRAW | CS_VREDRAW,
            window_proc: Some(detached_terminal_proc),
            class_extra: 0,
            window_extra: 0,
            instance,
            icon: ptr::null_mut(),
            cursor: LoadCursorW(ptr::null_mut(), IDC_ARROW),
            background: ptr::null_mut(),
            menu_name: ptr::null(),
            class_name: class_name.as_ptr(),
        };
        RegisterClassW(&class);
        let title = wide("TC-IDE — Detached ConPTY Terminal");
        let window = CreateWindowExW(
            0,
            class_name.as_ptr(),
            title.as_ptr(),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            1080,
            680,
            ptr::null_mut(),
            ptr::null_mut(),
            instance,
            ptr::null_mut(),
        );
        if window.is_null() {
            return false;
        }
        APP.with(|cell| cell.borrow_mut().detached_terminal = window);
        SetTimer(window, 2, 33, ptr::null());
        ShowWindow(window, SW_SHOW);
        UpdateWindow(window);
        true
    }
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
                let terminal_open = APP.with(|cell| cell.borrow().terminal_open);
                if terminal_open && handle_terminal_control(window, w_param as u32) {
                    invalidate(window);
                    return 0;
                }
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
                if app.package_open {
                    match w_param as u32 {
                        0x1b => app.package_open = false,
                        0x31 => app.package_targets[0] = !app.package_targets[0],
                        0x32 => app.package_targets[1] = !app.package_targets[1],
                        0x33 => app.package_targets[2] = !app.package_targets[2],
                        0x0d => app.start_package(),
                        _ => return,
                    }
                    app.save_package_targets();
                    return;
                }
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
                    if app.font_search_focused {
                        match w_param as u32 {
                            0x1b | 0x0d => app.font_search_focused = false,
                            VK_BACK => {
                                app.font_search.pop();
                            }
                            _ => return,
                        }
                        return;
                    }
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
                    if app.extension_search_focused {
                        match w_param as u32 {
                            0x1b | 0x0d => app.extension_search_focused = false,
                            VK_BACK => {
                                app.extension_search.pop();
                                app.extension_scroll = 0;
                            }
                            _ => return,
                        }
                        return;
                    }
                    if let Some(field) = app.ai_field_focused {
                        match w_param as u32 {
                            0x1b | 0x0d => app.ai_field_focused = None,
                            VK_BACK => match field {
                                0 => {
                                    app.ai_endpoint.pop();
                                }
                                1 => {
                                    app.ai_model.pop();
                                }
                                _ => {
                                    app.ai_key.pop();
                                }
                            },
                            _ => return,
                        }
                        return;
                    }
                    match w_param as u32 {
                        0x1b => {
                            app.extensions_open = false;
                            app.ai_field_focused = None;
                        }
                        VK_LEFT => {
                            app.extension_category = app.extension_category.saturating_sub(1)
                        }
                        VK_RIGHT => app.extension_category = (app.extension_category + 1) % 5,
                        VK_UP => app.selected_language = app.selected_language.saturating_sub(1),
                        VK_DOWN => {
                            app.selected_language = (app.selected_language + 1)
                                % WindowsAppState::EXTENSION_LANGUAGES.len()
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
            let create_kind = APP.with(|cell| cell.borrow().create_kind);
            if let Some(kind) = create_kind {
                let dialog_w = (520.0 * scale).min(area.right as f32 - 40.0 * scale);
                let dialog_h = if kind == CreateKind::Project {
                    350.0
                } else {
                    230.0
                } * scale;
                let left = (area.right as f32 - dialog_w) * 0.5;
                let top = (area.bottom as f32 - dialog_h) * 0.42;
                if x as f32 >= left
                    && x as f32 <= left + dialog_w
                    && y as f32 >= top
                    && y as f32 <= top + dialog_h
                {
                    let button_top = top + dialog_h - 56.0 * scale;
                    if y as f32 >= button_top {
                        APP.with(|cell| {
                            let mut app = cell.borrow_mut();
                            if (x as f32) < left + dialog_w * 0.5 {
                                app.create_kind = None;
                                app.create_error.clear();
                            } else {
                                app.create_requested_item();
                            }
                        });
                    } else if kind == CreateKind::Project
                        && y as f32 >= top + 162.0 * scale
                        && (y as f32) < top + 202.0 * scale
                    {
                        APP.with(|cell| {
                            let mut app = cell.borrow_mut();
                            if (x as f32) < left + dialog_w * 0.5 {
                                app.project_language = (app.project_language + 1)
                                    % WindowsAppState::PROJECT_LANGUAGES.len();
                                app.project_environment = 0;
                            } else {
                                app.project_environment = (app.project_environment + 1)
                                    % app.project_environments().len();
                            }
                            app.save_preferences();
                        });
                    } else if kind == CreateKind::Project
                        && y as f32 >= top + 202.0 * scale
                        && (y as f32) < top + 238.0 * scale
                    {
                        APP.with(|cell| {
                            let mut app = cell.borrow_mut();
                            let language = app.project_language;
                            app.language_versions[language] =
                                (app.language_versions[language] + 1) % 3;
                            app.save_preferences();
                        });
                    } else if kind == CreateKind::Project
                        && y as f32 >= top + 238.0 * scale
                        && (y as f32) < top + 282.0 * scale
                    {
                        if let Some(location) = platform::choose_folder(window) {
                            APP.with(|cell| cell.borrow_mut().create_location = location);
                        }
                    }
                    invalidate(window);
                    return 0;
                }
                return 0;
            }
            if y as f32 <= 48.0 * scale && x as f32 >= area.right as f32 - 155.0 * scale {
                APP.with(|cell| {
                    let mut app = cell.borrow_mut();
                    app.package_open = !app.package_open;
                    app.settings_open = false;
                    app.extensions_open = false;
                });
                invalidate(window);
                return 0;
            }
            let package_open = APP.with(|cell| cell.borrow().package_open);
            let package_left = area.right as f32 - 326.0 * scale;
            let package_top = 56.0 * scale;
            if package_open
                && x as f32 >= package_left
                && x as f32 <= area.right as f32 - 14.0 * scale
                && y as f32 >= package_top
                && y as f32 <= package_top + 214.0 * scale
            {
                APP.with(|cell| {
                    let mut app = cell.borrow_mut();
                    let local_y = y as f32 - package_top;
                    if (42.0 * scale..174.0 * scale).contains(&local_y) {
                        let index = ((local_y / scale - 42.0) / 42.0).floor() as usize;
                        if index < app.package_targets.len() {
                            app.package_targets[index] = !app.package_targets[index];
                            app.save_package_targets();
                        }
                    } else if local_y >= 174.0 * scale {
                        app.start_package();
                    }
                });
                invalidate(window);
                return 0;
            }
            let settings_open = APP.with(|cell| cell.borrow().settings_open);
            if settings_open {
                let left = area.right as f32 - 410.0 * scale;
                let top = 58.0 * scale;
                if x as f32 >= left
                    && x as f32 <= area.right as f32 - 20.0 * scale
                    && y as f32 >= top
                {
                    APP.with(|cell| {
                        let mut app = cell.borrow_mut();
                        let local_y = (y as f32 - top) / scale;
                        if (62.0..116.0).contains(&local_y) {
                            app.ui_chinese = !app.ui_chinese;
                        } else if (116.0..170.0).contains(&local_y) {
                            app.light_theme = !app.light_theme;
                        } else if (170.0..224.0).contains(&local_y) {
                            if x as f32 <= left + 195.0 * scale {
                                app.code_zoom = (app.code_zoom - 1).max(-2);
                            } else {
                                app.code_zoom = (app.code_zoom + 1).min(8);
                            }
                        } else if (224.0..278.0).contains(&local_y) {
                            app.font_search_focused = true;
                        } else if (278.0..448.0).contains(&local_y) {
                            let visible = ((local_y - 278.0) / 34.0).floor() as usize;
                            if let Some(index) = app.filtered_font_indices().get(visible).copied() {
                                app.code_font_family = index;
                                app.status =
                                    format!("Editor font: {}", WindowsAppState::CODE_FONTS[index]);
                            }
                        } else if (448.0..502.0).contains(&local_y) {
                            if let Some(path) = platform::choose_background(window) {
                                app.background_path = Some(path);
                            }
                        } else if (502.0..556.0).contains(&local_y) {
                            let local_x = (x as f32 - left) / (390.0 * scale);
                            if local_x < 0.25 {
                                app.background_opacity = (app.background_opacity - 0.05).max(0.05);
                            } else if local_x < 0.5 {
                                app.background_opacity = (app.background_opacity + 0.05).min(1.0);
                            } else if local_x < 0.75 {
                                app.background_zoom = (app.background_zoom - 0.1).max(1.0);
                            } else {
                                app.background_zoom = (app.background_zoom + 0.1).min(3.0);
                            }
                        } else if (556.0..610.0).contains(&local_y) {
                            app.background_path = None;
                        } else if local_y >= 610.0 {
                            app.settings_open = false;
                            app.font_search_focused = false;
                        }
                        app.save_preferences();
                    });
                    RENDERER.with(|renderer| *renderer.borrow_mut() = None);
                    invalidate(window);
                    return 0;
                }
            }
            let extensions_open = APP.with(|cell| cell.borrow().extensions_open);
            if extensions_open {
                let panel_w = (920.0 * scale).min(area.right as f32 - 50.0 * scale);
                let panel_h = (610.0 * scale).min(area.bottom as f32 - 80.0 * scale);
                let left = (area.right as f32 - panel_w) * 0.5;
                let top = 58.0 * scale;
                if x as f32 >= left
                    && x as f32 <= left + panel_w
                    && y as f32 >= top
                    && y as f32 <= top + panel_h
                {
                    APP.with(|cell| {
                        let mut app = cell.borrow_mut();
                        if x as f32 <= left + 184.0 * scale {
                            let category = ((y as f32 - top - 58.0 * scale) / (44.0 * scale))
                                .floor()
                                .clamp(0.0, 4.0)
                                as usize;
                            app.extension_category = category;
                            app.extension_search_focused = false;
                        } else if app.extension_category == 0 {
                            let local_y = (y as f32 - top) / scale;
                            if (72.0..116.0).contains(&local_y) {
                                app.extension_search_focused = true;
                            } else if (124.0..508.0).contains(&local_y) {
                                let visible = ((local_y - 124.0) / 48.0).floor() as usize;
                                let position = app.extension_scroll + visible;
                                if let Some(language) =
                                    app.filtered_language_indices().get(position).copied()
                                {
                                    app.selected_language = language;
                                    if x as f32 >= left + 445.0 * scale {
                                        app.language_installed[language] =
                                            !app.language_installed[language];
                                    } else if x as f32 >= left + 315.0 * scale {
                                        app.language_versions[language] =
                                            (app.language_versions[language] + 1) % 3;
                                    }
                                }
                            }
                        } else if app.extension_category == 1 {
                            let local_y = (y as f32 - top) / scale;
                            if (190.0..240.0).contains(&local_y) {
                                app.light_theme = false;
                                app.status = "Dark theme applied".into();
                            } else if (240.0..290.0).contains(&local_y) {
                                app.light_theme = true;
                                app.status = "Light theme applied".into();
                            }
                        } else if matches!(app.extension_category, 2 | 3) {
                            let local_y = (y as f32 - top) / scale;
                            if (190.0..340.0).contains(&local_y) {
                                let item = ((local_y - 190.0) / 50.0).floor() as usize;
                                let (label, command) = if app.extension_category == 2 {
                                    [
                                        ("LLDB", "lldb --version"),
                                        ("GDB", "gdb --version"),
                                        ("debugpy", "python -m debugpy --version"),
                                    ][item.min(2)]
                                } else {
                                    [
                                        ("Biome", "biome --version"),
                                        ("clang-format", "clang-format --version"),
                                        ("Ruff", "ruff --version"),
                                    ][item.min(2)]
                                };
                                app.probe_extension_tool(label, command);
                            }
                        } else if app.extension_category == 4 {
                            let content_left = left + 205.0 * scale;
                            for field in 0..3 {
                                let field_top = top + (142.0 + field as f32 * 92.0) * scale;
                                if x as f32 >= content_left
                                    && x as f32 <= content_left + 600.0 * scale
                                    && y as f32 >= field_top
                                    && y as f32 <= field_top + 42.0 * scale
                                {
                                    app.ai_field_focused = Some(field);
                                }
                            }
                            let save_top = top + 440.0 * scale;
                            if x as f32 >= content_left
                                && x as f32 <= content_left + 210.0 * scale
                                && y as f32 >= save_top
                                && y as f32 <= save_top + 42.0 * scale
                            {
                                if app.ai_endpoint.trim().is_empty()
                                    || app.ai_model.trim().is_empty()
                                {
                                    app.status = "AI endpoint and model are required".into();
                                } else {
                                    app.save_preferences();
                                    app.status =
                                        "AI API saved; API key remains in memory for this session"
                                            .into();
                                }
                            }
                        }
                        app.save_preferences();
                    });
                    invalidate(window);
                    return 0;
                }
            }
            let graph_left = area.right as f32 - (area.right as f32 * 0.27).max(300.0 * scale);
            if x as f32 >= graph_left {
                let first_y = (48.0 + 86.0) * scale;
                let row = ((y as f32 - first_y + 12.0 * scale) / (58.0 * scale)).floor();
                if row >= 2.0 {
                    let target = APP.with(|cell| {
                        let app = cell.borrow();
                        let focused = app.hovered_symbol.as_ref()?;
                        app.project_symbols
                            .iter()
                            .filter(|entry| entry.symbol.name == focused.name)
                            .skip(app.graph_scroll)
                            .nth(row as usize - 2)
                            .map(|entry| {
                                (
                                    app.project_root.join(&entry.file),
                                    entry.symbol.line,
                                    entry.symbol.start,
                                )
                            })
                    });
                    if let Some((path, line, column)) = target {
                        APP.with(|cell| {
                            let mut app = cell.borrow_mut();
                            app.open_file(path);
                            app.caret_line = line.min(app.lines.len().saturating_sub(1));
                            app.caret_column = column;
                            app.clamp_caret();
                            app.status = format!(
                                "Jumped to relation at {}:{}",
                                app.file_name,
                                app.caret_line + 1
                            );
                        });
                        invalidate(window);
                        return 0;
                    }
                }
            }
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
            let panel = terminal_core::panel_metrics(
                area.right as f32,
                area.bottom as f32,
                scale,
                APP.with(|cell| cell.borrow().terminal_motion),
            );
            let terminal_top = (area.bottom as f32 - panel.height) as i32;
            if y >= terminal_top && y <= terminal_top + (34.0 * scale) as i32 {
                APP.with(|cell| {
                    let mut app = cell.borrow_mut();
                    if x as f32 >= 150.0 * scale && (x as f32) < 225.0 * scale {
                        app.run_task(true);
                    } else if x as f32 >= 225.0 * scale && (x as f32) < 310.0 * scale {
                        app.run_task(false);
                    } else {
                        app.toggle_terminal();
                    }
                });
                invalidate(window);
            } else if x as f32 >= panel.list_left && y as f32 >= panel.body_top {
                APP.with(|cell| {
                    let mut app = cell.borrow_mut();
                    if !app.terminal_open {
                        return;
                    }
                    let local_y = y as f32 - panel.body_top;
                    if local_y <= 30.0 * scale {
                        if x as f32 <= area.right as f32 - 82.0 * scale {
                            app.add_terminal();
                        } else {
                            drop(app);
                            let opened = open_detached_terminal();
                            APP.with(|cell| {
                                cell.borrow_mut().status = if opened {
                                    "Current ConPTY terminal detached into a native window".into()
                                } else {
                                    "Could not create the detached terminal window".into()
                                };
                            });
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
            let mut area = unsafe { std::mem::zeroed::<Rect>() };
            unsafe { GetClientRect(window, &mut area) };
            let scale = platform::dpi_for_window(window) as f32 / 96.0;
            let graph_left = area.right as f32 - (area.right as f32 * 0.27).max(300.0 * scale);
            APP.with(|cell| {
                let mut app = cell.borrow_mut();
                if app.extensions_open && app.extension_category == 0 {
                    let count = app.filtered_language_indices().len();
                    if delta > 0 {
                        app.extension_scroll = app.extension_scroll.saturating_sub(1);
                    } else if delta < 0 {
                        app.extension_scroll =
                            (app.extension_scroll + 1).min(count.saturating_sub(8));
                    }
                } else if app.pointer_x as f32 >= graph_left {
                    if delta > 0 {
                        app.graph_scroll = app.graph_scroll.saturating_sub(1);
                    } else if delta < 0 {
                        let count = app
                            .hovered_symbol
                            .as_ref()
                            .map(|focused| {
                                app.project_symbols
                                    .iter()
                                    .filter(|entry| entry.symbol.name == focused.name)
                                    .count()
                            })
                            .unwrap_or(0);
                        app.graph_scroll = (app.graph_scroll + 1).min(count.saturating_sub(1));
                    }
                } else if delta > 0 {
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
        WM_RBUTTONDOWN => {
            let x = (l_param as u16 as i16) as i32;
            let y = ((l_param >> 16) as u16 as i16) as i32;
            let mut area = unsafe { std::mem::zeroed::<Rect>() };
            unsafe { GetClientRect(window, &mut area) };
            let scale = platform::dpi_for_window(window) as f32 / 96.0;
            if x as f32 <= (area.right as f32 * 0.19).max(220.0 * scale) {
                if let Some(command) = platform::workspace_context_menu(window, x, y) {
                    perform_command(window, command);
                }
            }
            0
        }
        WM_MOUSEMOVE => {
            let dragging = APP.with(|cell| cell.borrow().dragging_selection);
            let x = (l_param as u16 as i16) as i32;
            let y = ((l_param >> 16) as u16 as i16) as i32;
            APP.with(|cell| {
                let mut app = cell.borrow_mut();
                app.pointer_x = x;
            });
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
                        app.graph_scroll = 0;
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
            sync_terminal_grid(window);
            invalidate(window);
            0
        }
        WM_TIMER => {
            let reveal = APP.with(|cell| {
                let mut app = cell.borrow_mut();
                app.update_motion();
                if app.package_reveal_pending {
                    app.package_reveal_pending = false;
                    Some(app.project_root.join(".tc-ide-packages"))
                } else {
                    None
                }
            });
            if let Some(path) = reveal {
                let _ = platform::open_in_explorer(window, &path);
            }
            sync_terminal_grid(window);
            invalidate(window);
            0
        }
        WM_DPICHANGED => {
            platform::apply_dpi_change(window, l_param);
            RENDERER.with(|cell| *cell.borrow_mut() = None);
            sync_terminal_grid(window);
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
