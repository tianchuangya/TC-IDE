#![allow(clippy::upper_case_acronyms)]

use std::cell::RefCell;
use std::collections::{BTreeMap, HashSet};
use std::ffi::c_void;
use std::fs;
use std::path::PathBuf;
use std::ptr;
use std::time::Instant;

use crate::ai_core;
use crate::dap_core;
use crate::editor_core;
use crate::extension_core;
use crate::graph_core;
use crate::index_core;
use crate::lsp_core;
use crate::motion_core;
use crate::session_core;
use crate::syntax_core;
use crate::terminal_core;
use crate::text_buffer;
use crate::workspace_core;

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
type Hfont = *mut c_void;
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
const DT_CALCRECT: Uint = 0x0400;
const DT_SINGLELINE: Uint = 0x0020;
const DT_NOPREFIX: Uint = 0x0800;
const DT_RIGHT: Uint = 0x0002;
const MB_YESNO: Uint = 0x0004;
const MB_ICONWARNING: Uint = 0x0030;
const IDYES: i32 = 6;
const TRANSPARENT_MODE: i32 = 1;
const DEFAULT_CHARSET: u8 = 1;
const OUT_TT_PRECIS: u8 = 4;
const CLIP_DEFAULT_PRECIS: u8 = 0;
const CLEARTYPE_QUALITY: u8 = 5;
const FF_MODERN: u8 = 0x30;
const FIXED_PITCH: u8 = 0x01;
const FW_NORMAL: i32 = 400;
/// Detached terminal cell height in logical pixels.
const DETACHED_FONT_HEIGHT: f32 = 14.0;
/// Monospace face used by the detached terminal window.
const DETACHED_FONT_FACE: &str = "Cascadia Mono";
/// Height of the detached window header above the scrollback area.
const DETACHED_HEADER_HEIGHT: f32 = 76.0;
const DETACHED_TEXT_LEFT: f32 = 24.0;
const DETACHED_TEXT_RIGHT: f32 = 20.0;
const DETACHED_INPUT_GAP: f32 = 8.0;
/// Lines a single wheel notch scrolls.
const WHEEL_SCROLL_LINES: i32 = 3;
/// Seconds a detached terminal notice stays visible.
const NOTICE_SECONDS: f32 = 1.8;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct TextPosition {
    line: usize,
    column: usize,
}

/// Buffer of a document that is open but not active.
/// One row of the problems panel: its text and where it jumps to.
type ProblemRow = (String, Option<(PathBuf, usize, usize)>);

/// Language server request the UI is waiting for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LspRequest {
    Completion,
    Hover,
    Definition,
    References,
    Rename,
}

struct StoredBuffer {
    lines: Vec<String>,
    file_name: String,
    file_path: Option<PathBuf>,
    caret_line: usize,
    caret_column: usize,
    selection_anchor: Option<TextPosition>,
    undo_stack: Vec<EditorSnapshot>,
    redo_stack: Vec<EditorSnapshot>,
}

/// Caret state recorded for undo; the text itself lives in the piece table, so
/// an undo step no longer copies the document.
#[derive(Clone)]
struct EditorSnapshot {
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

/// View state that belongs to the detached terminal window only.
struct DetachedTerminalState {
    scroll: usize,
    selection: Option<terminal_core::TerminalSelection>,
    dragging: bool,
    metrics: Option<(f32, f32)>,
    metrics_dpi: u32,
    font: Hfont,
    font_dpi: u32,
    grid_signature: (u32, u32, u32, u32, usize),
    notice: Option<String>,
    notice_started: Option<Instant>,
    composition: String,
}

impl Default for DetachedTerminalState {
    fn default() -> Self {
        Self {
            scroll: 0,
            selection: None,
            dragging: false,
            metrics: None,
            metrics_dpi: 0,
            font: ptr::null_mut(),
            font_dpi: 0,
            grid_signature: (0, 0, 0, 0, 0),
            notice: None,
            notice_started: None,
            composition: String::new(),
        }
    }
}

/// Laid out geometry of the detached terminal window, in device pixels.
struct DetachedLayout {
    left: f32,
    right: f32,
    top: f32,
    input_top: f32,
    row_height: f32,
    cell_width: f32,
    visible_rows: usize,
    columns: i16,
    rows: i16,
}

/// Buffered output lines of the terminal the detached window shows.
struct DetachedContext {
    lines: Vec<String>,
    viewport: terminal_core::TerminalViewport,
    layout: DetachedLayout,
}

thread_local! {
    static APP: RefCell<WindowsAppState> = RefCell::new(WindowsAppState::new());
    static RENDERER: RefCell<Option<DirectEditorRenderer>> = const { RefCell::new(None) };
    static DETACHED: RefCell<DetachedTerminalState> = RefCell::new(DetachedTerminalState::default());
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
    project_index: index_core::IncrementalIndex,
    workspace_search: String,
    workspace_search_focused: bool,
    create_kind: Option<CreateKind>,
    create_name: String,
    create_error: String,
    create_location: PathBuf,
    project_language: usize,
    project_environment: usize,
    lines: Vec<String>,
    /// Text of the active document, stored as a piece table.
    buffer: text_buffer::TextBuffer,
    viewport: text_buffer::Viewport,
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
    ai_prompt: String,
    ai_prompt_focused: bool,
    ai_terminal: Option<usize>,
    ai_last_report: String,
    problems_open: bool,
    problems_selection: usize,
    problem_filter: lsp_core::SeverityFilter,
    chat_open: bool,
    chat_input: String,
    chat_input_focused: bool,
    chat_messages: ai_core::Conversation,
    chat_session: Option<ai_core::ChatSession>,
    chat_error: Option<String>,
    debug_session: Option<dap_core::DebugSession>,
    debug_state: dap_core::DebugState,
    breakpoints: Vec<(PathBuf, Vec<usize>)>,
    debug_stop_seen: String,
    language_server: Option<lsp_core::LanguageServer>,
    lsp_diagnostics: Vec<lsp_core::Diagnostic>,
    lsp_pending: Vec<(u64, LspRequest)>,
    lsp_completions: Vec<String>,
    lsp_hover: Option<(usize, usize, String)>,
    lsp_locations: Vec<(PathBuf, usize, usize)>,
    lsp_location_cursor: usize,
    rename_prompt: Option<String>,
    lsp_last_change: Option<Instant>,
    lsp_dirty: bool,
    lsp_completion_asked: Option<Instant>,
    index_request: Option<
        std::sync::mpsc::Sender<(
            PathBuf,
            index_core::IncrementalIndex,
            index_core::IndexStats,
        )>,
    >,
    index_result: Option<
        std::sync::mpsc::Receiver<(
            PathBuf,
            index_core::IncrementalIndex,
            index_core::IndexStats,
        )>,
    >,
    runtime_cache: extension_core::RuntimeCache,
    pending_extension: Option<extension_core::ExtensionSource>,
    hovered_symbol: Option<editor_core::SymbolOccurrence>,
    expanded_comments: HashSet<usize>,
    editing_comment: Option<usize>,
    pointer_x: i32,
    pointer_y: i32,
    graph_scroll: usize,
    graph_search: String,
    graph_search_focused: bool,
    graph_scope: Option<graph_core::GraphScope>,
    graph_folded: Vec<String>,
    graph_relation: graph_core::GraphRelation,
    session: session_core::Session,
    stored_buffers: BTreeMap<usize, StoredBuffer>,
    graph_selected: Option<usize>,
    graph_keyboard_active: bool,
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
    package_failed_command: Option<String>,
    signing_certificate: Option<PathBuf>,
    terminal_open: bool,
    terminal_motion: f32,
    terminal_velocity: f32,
    terminal_scroll: usize,
    terminal_selection: Option<terminal_core::TerminalSelection>,
    terminal_dragging: bool,
    terminals: Vec<TerminalTab>,
    active_terminal: usize,
    terminal_grid: terminal_core::GridSize,
    terminal_grid_signature: (u32, u32, u32, i32, usize, usize, bool),
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

    /// Restore the previous session: project, recent projects and open files.
    fn restore_session(&mut self) {
        let mut session = load_session();
        session.prune_recent_projects(|path| path.is_dir());
        if let Some(project) = session.project().cloned() {
            if project.is_dir() {
                self.open_project(project);
            }
        }
        for path in session.restorable_documents() {
            if path.is_file() {
                self.open_file(path);
            }
        }
        self.session = session;
        if let Some(project) = self.session.project().cloned() {
            self.session.set_project(Some(project));
        }
        if !self.session.is_empty() {
            self.status = format!(
                "Restored {} document(s) from the previous session",
                self.session.documents().len()
            );
        }
        save_session(&self.session);
    }

    fn new() -> Self {
        let preferences = preferences::load();
        let project_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let collapsed_directories = HashSet::new();
        let (project_entries, project_entry_paths, project_entry_is_dir) =
            Self::scan_project(&project_root, &collapsed_directories);
        let mut project_index = index_core::IncrementalIndex::new();
        project_index.refresh(Self::scan_project_fingerprints(&project_root), |relative| {
            Self::read_project_file(&project_root, relative)
        });
        let project_symbols = project_index.symbols();
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
            project_index,
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
            viewport: text_buffer::Viewport::new(24, 96),
            buffer: text_buffer::TextBuffer::from_text(
                "use tc_ide::workspace::Project;\n\npub struct Editor {\n    project: Project,\n}\n\nimpl Editor {\n    pub fn render(&mut self) {\n        self.project.resolve_symbols();\n    }\n}\n",
            ),
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
            ai_prompt: String::new(),
            ai_prompt_focused: false,
            ai_terminal: None,
            ai_last_report: String::new(),
            problems_open: false,
            problems_selection: 0,
            problem_filter: lsp_core::SeverityFilter::All,
            chat_open: false,
            chat_input: String::new(),
            chat_input_focused: false,
            chat_messages: ai_core::Conversation::new(),
            chat_session: None,
            chat_error: None,
            debug_session: None,
            debug_state: dap_core::DebugState::default(),
            breakpoints: Vec::new(),
            debug_stop_seen: String::new(),
            language_server: None,
            lsp_diagnostics: Vec::new(),
            lsp_pending: Vec::new(),
            lsp_completions: Vec::new(),
            lsp_hover: None,
            lsp_locations: Vec::new(),
            lsp_location_cursor: 0,
            rename_prompt: None,
            lsp_last_change: None,
            lsp_dirty: false,
            lsp_completion_asked: None,
            index_request: None,
            index_result: None,
            runtime_cache: load_runtime_cache(),
            pending_extension: None,
            hovered_symbol: None,
            expanded_comments: HashSet::new(),
            editing_comment: None,
            pointer_x: -1,
            pointer_y: -1,
            graph_scroll: 0,
            graph_search: String::new(),
            graph_search_focused: false,
            graph_scope: None,
            graph_folded: Vec::new(),
            graph_relation: graph_core::GraphRelation::Follow,
            session: session_core::Session::default(),
            stored_buffers: BTreeMap::new(),
            graph_selected: None,
            graph_keyboard_active: false,
            background_path: preferences.background_path.clone().map(PathBuf::from),
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
            package_failed_command: None,
            signing_certificate: None,
            terminal_open: false,
            terminal_motion: 0.0,
            terminal_velocity: 0.0,
            terminal_scroll: 0,
            terminal_selection: None,
            terminal_dragging: false,
            terminals: Vec::new(),
            active_terminal: 0,
            terminal_grid: terminal_core::GridSize {
                columns: 120,
                rows: 30,
            },
            terminal_grid_signature: (0, 0, 0, 0, 0, 0, false),
            detached_terminal: ptr::null_mut(),
            status: "DirectWrite / Direct2D editor ready".into(),
            started: Instant::now(),
            last_frame: Instant::now(),
            last_edit: None,
            create_motion: 0.0,
            create_velocity: 0.0,
        }
    }

    /// Remember which document the active buffer belongs to.
    fn active_document(&self) -> Option<usize> {
        let index = self.session.active();
        self.session.documents().get(index).map(|_| index)
    }

    /// Keep the active buffer and mark it unsaved.
    fn mark_active_dirty(&mut self) {
        if let Some(index) = self.active_document() {
            self.session.mark_dirty(index, true);
        }
    }

    /// Save the active buffer fields under its document index.
    fn stash_active_buffer(&mut self) {
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
    fn load_document(&mut self, index: usize) {
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
    fn switch_document(&mut self, index: usize) -> bool {
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
    fn close_document(&mut self, index: usize, window: Hwnd) {
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

    fn clamp_caret(&mut self) {
        self.caret_line = self.caret_line.min(self.lines.len().saturating_sub(1));
        self.caret_column = self
            .caret_column
            .min(self.lines[self.caret_line].chars().count());
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
            caret: self.position(),
            selection_anchor: self.selection_anchor,
        }
    }

    fn restore(&mut self, snapshot: EditorSnapshot) {
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
    fn sync_lines(&mut self) {
        self.lines = self.buffer.to_lines();
        self.clamp_caret();
    }

    fn record_undo(&mut self) {
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
    fn finish_edit(&mut self) {
        self.buffer.end_group();
        self.sync_lines();
        self.reveal_caret();
        self.update_active_index_entry();
        self.last_edit = Some(Instant::now());
    }

    /// Re-parse only the file being edited, so the relation graph stays fresh.
    fn update_active_index_entry(&mut self) {
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
            self.project_symbols = self.project_index.symbols();
        }
    }

    /// Scroll so the caret is inside the window.
    fn reveal_caret(&mut self) {
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
    fn scroll_editor(&mut self, lines: i32) {
        let line_count = self.lines.len();
        self.viewport = self.viewport.scrolled(lines, line_count);
    }

    /// Scroll the editor horizontally, for the mouse wheel with shift.
    fn scroll_editor_columns(&mut self, columns: i32) {
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
    fn resize_viewport(&mut self, width: i32, height: i32, scale: f32) {
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

    fn code_cell_width(&self) -> f32 {
        8.9 + self.code_zoom.clamp(-2, 8) as f32 * 0.6
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
        self.buffer
            .delete((start.line, start.column), (end.line, end.column));
        self.sync_lines();
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

    fn completions(&self) -> Vec<String> {
        if !self.lsp_completions.is_empty() {
            return self.lsp_completions.clone();
        }
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
        self.finish_edit();
        self.notify_language_server();
    }

    fn newline_raw(&mut self) {
        self.buffer.insert(self.caret_line, self.caret_column, "\n");
        self.caret_line += 1;
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
        self.finish_edit();
        self.notify_language_server();
        Some(value)
    }

    fn undo(&mut self) {
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

    fn redo(&mut self) {
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

    fn set_caret(&mut self, position: TextPosition, extend_selection: bool) {
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

    fn save(&mut self) {
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

    fn open_file(&mut self, path: PathBuf) {
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

    /// Fingerprints of the project files, without reading their contents.
    fn scan_project_fingerprints(root: &std::path::Path) -> Vec<index_core::FileFingerprint> {
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
    fn read_project_file(root: &std::path::Path, relative: &str) -> Option<Vec<String>> {
        let path = root.join(relative.replace('/', std::path::MAIN_SEPARATOR_STR));
        let content = fs::read_to_string(path).ok()?;
        let mut lines = content.lines().map(str::to_string).collect::<Vec<_>>();
        if lines.is_empty() {
            lines.push(String::new());
        }
        Some(lines)
    }

    fn open_project(&mut self, root: PathBuf) {
        self.graph_folded.clear();
        self.graph_selected = None;
        self.graph_keyboard_active = false;
        self.session.set_project(Some(root.clone()));
        self.session.remember_project(&root);
        save_session(&self.session);
        self.project_root = root;
        self.package_targets = Self::load_package_targets(&self.project_root);
        self.project_scroll = 0;
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

    fn refresh_project(&mut self) {
        (
            self.project_entries,
            self.project_entry_paths,
            self.project_entry_is_dir,
        ) = Self::scan_project(&self.project_root, &self.collapsed_directories);
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
    fn project_root_name(&self) -> String {
        self.create_name.trim().to_string()
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
        let signing = match (&self.signing_certificate, self.package_targets[2]) {
            (Some(certificate), true) => format!(
                "; $signtool=(Get-Command signtool.exe -ErrorAction SilentlyContinue).Source; if(!$signtool){{$signtool=Get-ChildItem \"${{env:ProgramFiles(x86)}}\\Windows Kits\\10\\bin\\*\\x64\\signtool.exe\" -ErrorAction SilentlyContinue | Sort-Object FullName -Descending | Select-Object -First 1 -ExpandProperty FullName}}; if(!$signtool){{throw 'Windows SDK signtool.exe is not installed'}}; & $signtool sign /fd SHA256 /f '{}' '.tc-ide-packages\\{project}.msix'",
                certificate.to_string_lossy()
            ),
            _ => String::new(),
        };
        self.package_marker = format!("__TC_PACKAGE_DONE_{}__", self.started.elapsed().as_nanos());
        self.package_failure_marker = format!(
            "__TC_PACKAGE_FAILED_{}__",
            self.started.elapsed().as_nanos()
        );
        let command = format!(
            "$ErrorActionPreference='Stop'; try{{New-Item -ItemType Directory -Force '{stage}' | Out-Null; {build}{zip}{msix}{signing}; Write-Output '{}'}}catch{{Write-Host $_.Exception.Message; Write-Output '{}'}}",
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

    /// Rows of the problems panel: diagnostics, then debug state.
    ///
    /// Each row carries where it should take the caret when it is opened.
    fn problem_rows(&self) -> Vec<ProblemRow> {
        let mut rows = Vec::new();
        for diagnostic in lsp_core::filter_diagnostics(&self.lsp_diagnostics, self.problem_filter) {
            let target = self
                .file_path
                .clone()
                .map(|path| (path, diagnostic.line as usize, diagnostic.column as usize));
            rows.push((diagnostic.line_text(), target));
        }
        if let Some(stopped) = self.debug_state.stopped.as_ref() {
            rows.push((stopped.describe(), None));
        }
        for (index, frame) in self.debug_state.frames.iter().enumerate() {
            let target = frame
                .file
                .as_ref()
                .map(|file| (PathBuf::from(file), frame.line, frame.column));
            rows.push((
                format!("#{index}  {}  :{}", frame.name, frame.line + 1),
                target,
            ));
        }
        for variable in self.debug_state.variables.iter().take(20) {
            rows.push((
                format!(
                    "{} = {}{}",
                    variable.name,
                    variable.value,
                    if variable.kind.is_empty() {
                        String::new()
                    } else {
                        format!("  ({})", variable.kind)
                    }
                ),
                None,
            ));
        }
        for (path, lines) in &self.breakpoints {
            let name = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            for line in lines {
                rows.push((
                    format!("breakpoint  {name}:{}", line + 1),
                    Some((path.clone(), *line, 0)),
                ));
            }
        }
        rows
    }

    /// Open or close the problems panel.
    fn toggle_problems(&mut self) {
        self.problems_open = !self.problems_open;
        self.problems_selection = 0;
        if self.problems_open {
            let count = self.problem_rows().len();
            self.status = if count == 0 {
                "Problems: nothing reported by the language server or the debugger".into()
            } else {
                format!("Problems: {count} row(s); Enter jumps, Esc closes")
            };
        }
    }

    /// Move the panel selection.
    fn move_problem_selection(&mut self, delta: i32) {
        let count = self.problem_rows().len();
        if count == 0 {
            self.problems_selection = 0;
            return;
        }
        let current = self.problems_selection.min(count - 1) as i32;
        let next = (current + delta).clamp(0, count as i32 - 1) as usize;
        self.problems_selection = next;
        if let Some((text, _)) = self.problem_rows().get(next) {
            self.status = text.clone();
        }
    }

    /// Jump to the selected problems panel row.
    fn activate_problem_row(&mut self) {
        let rows = self.problem_rows();
        let Some((text, target)) =
            rows.get(self.problems_selection.min(rows.len().saturating_sub(1)))
        else {
            return;
        };
        let text = text.clone();
        match target.clone() {
            Some((path, line, column)) => {
                if path.is_file() && self.file_path.as_deref() != Some(path.as_path()) {
                    self.open_file(path);
                }
                self.caret_line = line.min(self.lines.len().saturating_sub(1));
                self.caret_column = column;
                self.clamp_caret();
                self.reveal_caret();
                self.status = format!("Jumped to {text}");
            }
            None => self.status = text,
        }
    }

    /// Send the prompt in the chat panel to the configured endpoint.
    fn send_chat_prompt(&mut self) {
        let prompt = self.chat_input.trim().to_string();
        if prompt.is_empty() {
            return;
        }
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
                self.chat_messages.push_user(&prompt);
                self.chat_input.clear();
                self.chat_error = None;
                self.chat_session = Some(session);
                self.status = "AI request running; Esc cancels it".into();
            }
            Err(error) => self.chat_error = Some(error.message()),
        }
    }

    /// Collect streamed chat events.
    fn poll_chat_session(&mut self) {
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
    fn cancel_chat(&mut self) {
        if let Some(mut session) = self.chat_session.take() {
            session.cancel();
            self.status = "AI request cancelled; the partial answer is kept".into();
        }
    }

    /// Conversation turns for the panel, oldest first.
    fn chat_lines(&self) -> Vec<(bool, String)> {
        self.chat_messages.turns().to_vec()
    }

    /// Where the conversation of a project is stored.
    fn chat_history_path(&self) -> PathBuf {
        self.project_root.join(".tc-ide-ai").join("history.conf")
    }

    /// Load the saved conversation for the current project.
    fn load_chat_history(&mut self) {
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
    fn save_chat_history(&self) {
        let path = self.chat_history_path();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::write(path, self.chat_messages.serialize());
    }

    /// Breakpoint lines of the active file.
    fn active_breakpoints(&self) -> Vec<usize> {
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
    fn toggle_breakpoint(&mut self) {
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
    fn start_or_continue_debugging(&mut self) {
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
    fn step_debugging(&mut self, request: dap_core::PendingRequest) {
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
    fn stop_debugging(&mut self) {
        self.debug_session = None;
        self.debug_state = dap_core::DebugState::default();
        self.debug_stop_seen.clear();
        self.status = "Debugging stopped".into();
    }

    /// Collect debug adapter messages and walk the state forward.
    fn poll_debug_session(&mut self) {
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

    /// Document text the language server should index.
    fn document_text(&self) -> String {
        self.lines.join("\n")
    }

    /// Real installation state of a language's tooling.
    fn language_tooling(&self, language: &str) -> extension_core::LanguageTools {
        extension_core::language_tools(language, |tool| {
            lsp_core::program_runs(tool, &["--version"])
        })
    }

    /// Summary of the tooling for the selected extension language.
    fn selected_language_tooling(&self) -> String {
        let language = WindowsAppState::EXTENSION_LANGUAGES[self
            .selected_language
            .min(WindowsAppState::EXTENSION_LANGUAGES.len() - 1)];
        // The registry names differ from the extension list, so map the common
        // ones onto the names the tooling table understands.
        let language = match language {
            "Web" => "TypeScript / JavaScript",
            other => other,
        };
        self.language_tooling(language).summary()
    }

    /// Start the language server for the active file, when it is installed.
    fn ensure_language_server(&mut self) {
        let path = self
            .file_path
            .clone()
            .or_else(|| Some(self.project_root.join(&self.file_name)));
        let Some(path) = path else {
            return;
        };
        let name = self.file_name.clone();
        let Some((spec, installed)) = lsp_core::server_status(&name) else {
            self.lsp_diagnostics.clear();
            self.shutdown_language_server();
            return;
        };
        if !installed {
            self.lsp_diagnostics.clear();
            self.shutdown_language_server();
            self.status = format!(
                "{} is not installed; using the built-in symbol index for {}",
                spec.program, name
            );
            return;
        }
        let already_running = self
            .language_server
            .as_mut()
            .map(|server| server.is_running() && server.spec().program == spec.program)
            .unwrap_or(false);
        if already_running {
            let path_text = path.to_string_lossy().into_owned();
            let text = self.document_text();
            if let Some(server) = self.language_server.as_mut() {
                let _ = server.change_document(&path_text, &text);
            }
            self.lsp_last_change = Some(Instant::now());
            return;
        }
        self.shutdown_language_server();
        let root = self.project_root.clone();
        match lsp_core::LanguageServer::start(&spec, &root) {
            Ok(mut server) => {
                if server.initialize().is_err() || server.notify_initialized().is_err() {
                    self.status = format!("{} did not answer initialize", spec.program);
                    return;
                }
                if let Err(error) =
                    server.open_document(&path.to_string_lossy(), &self.document_text())
                {
                    self.status = error;
                    return;
                }
                self.language_server = Some(server);
                self.lsp_pending.clear();
                self.lsp_completions.clear();
                self.lsp_hover = None;
                self.status = format!("{} started for {}", spec.program, name);
            }
            Err(error) => self.status = error,
        }
    }

    fn shutdown_language_server(&mut self) {
        if let Some(server) = self.language_server.as_mut() {
            if let Some(path) = self.file_path.clone() {
                let _ = server.close_document(&path.to_string_lossy());
            }
        }
        self.language_server = None;
        self.lsp_pending.clear();
        self.lsp_completions.clear();
    }

    /// Tell the server about an edit, coalescing rapid typing.
    fn notify_language_server(&mut self) {
        let Some(path) = self
            .file_path
            .as_ref()
            .map(|path| path.to_string_lossy().into_owned())
        else {
            return;
        };
        let text = self.document_text();
        let running = self
            .language_server
            .as_mut()
            .map(|server| server.is_running())
            .unwrap_or(false);
        if !running {
            return;
        }
        if let Some(server) = self.language_server.as_mut() {
            let _ = server.change_document(&path, &text);
        }
        self.lsp_last_change = Some(Instant::now());
        self.lsp_pending
            .retain(|(_, request)| *request != LspRequest::Hover);
    }

    /// Ask the server for completion, hover or definition at the caret.
    fn request_language_feature(&mut self, request: LspRequest) {
        let Some(server) = self.language_server.as_mut() else {
            return;
        };
        let Some(path) = self
            .file_path
            .as_ref()
            .map(|path| path.to_string_lossy().into_owned())
        else {
            return;
        };
        let line = self.caret_line as u32;
        let character = self.caret_column as u32;
        let rename = request == LspRequest::Rename;
        let method = match request {
            LspRequest::Completion => "textDocument/completion",
            LspRequest::Hover => "textDocument/hover",
            LspRequest::Definition => "textDocument/definition",
            LspRequest::References => "textDocument/references",
            LspRequest::Rename => "textDocument/rename",
        };
        let result = if rename {
            let new_name = self.rename_prompt.clone().unwrap_or_default();
            if new_name.is_empty() {
                self.status = "Rename needs a new name".into();
                return;
            }
            server.rename(&path, line, character, &new_name)
        } else {
            server.position_request(method, &path, line, character)
        };
        match result {
            Ok(id) => self.lsp_pending.push((id, request)),
            Err(error) => self.status = error,
        }
    }

    /// Jump to the next reference the server reported.
    fn jump_to_next_reference(&mut self) {
        if self.lsp_locations.is_empty() {
            self.status = "No references from the language server yet".into();
            return;
        }
        let index = self.lsp_location_cursor % self.lsp_locations.len();
        let (path, line, column) = self.lsp_locations[index].clone();
        self.lsp_location_cursor = index + 1;
        if path.is_file() {
            let name = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            if self.file_path.as_deref() != Some(path.as_path()) {
                self.open_file(path);
            } else {
                self.status = format!("Reference in {name}");
            }
            self.caret_line = line.min(self.lines.len().saturating_sub(1));
            self.caret_column = column;
            self.clamp_caret();
            self.reveal_caret();
            self.status = format!(
                "Reference {} of {} at {}:{}",
                index + 1,
                self.lsp_locations.len(),
                self.file_name,
                self.caret_line + 1
            );
        } else {
            self.status = format!("Reference file is not available: {}", path.display());
        }
    }

    /// Apply the edits a rename response asked for.
    fn apply_rename_edits(&mut self, edits: &[lsp_core::RenameEdit]) {
        if edits.is_empty() {
            self.status = "The language server returned no rename edits".into();
            return;
        }
        let mut touched = 0usize;
        let mut grouped: Vec<(PathBuf, Vec<lsp_core::RenameEdit>)> = Vec::new();
        for edit in edits {
            let path = PathBuf::from(&edit.path);
            match grouped.iter_mut().find(|(known, _)| *known == path) {
                Some((_, list)) => list.push(edit.clone()),
                None => grouped.push((path, vec![edit.clone()])),
            }
        }
        for (path, list) in grouped {
            let is_active = self.file_path.as_deref() == Some(path.as_path());
            let text = if is_active {
                self.buffer.to_text()
            } else {
                match fs::read_to_string(&path) {
                    Ok(content) => content.replace("\r\n", "\n"),
                    Err(error) => {
                        self.status = format!("Rename could not read {}: {error}", path.display());
                        return;
                    }
                }
            };
            let updated = lsp_core::apply_edits(&text, &list);
            if is_active {
                self.buffer = text_buffer::TextBuffer::from_text(&updated);
                self.sync_lines();
                self.reveal_caret();
                touched += 1;
                continue;
            }
            match fs::write(&path, updated.replace('\n', "\r\n")) {
                Ok(()) => touched += 1,
                Err(error) => {
                    self.status = format!("Rename could not write {}: {error}", path.display());
                    return;
                }
            }
        }
        self.request_project_index();
        self.status = format!("Rename applied to {touched} file(s)");
    }

    /// Collect whatever the language server has sent since the last tick.
    ///
    /// The client is taken out of the state for the duration so the collected
    /// diagnostics and pending requests can be updated without fighting the
    /// borrow checker.
    fn poll_language_server(&mut self) {
        let Some(mut server) = self.language_server.take() else {
            return;
        };
        if !server.is_running() {
            let name = server.spec().program.clone();
            let errors = server.stderr_lines();
            self.lsp_pending.clear();
            self.status = match errors.first() {
                Some(line) => format!("{name} stopped: {line}"),
                None => format!("{name} stopped"),
            };
            return;
        }
        let file_name = self.file_name.clone();
        let mut diagnostics_changed = false;
        while let Some(message) = server.try_message() {
            if server.answer_automatically(&message) {
                continue;
            }
            if let Some(method) = lsp_core::message_method(&message) {
                if method == "textDocument/publishDiagnostics" {
                    self.lsp_diagnostics = lsp_core::parse_diagnostics(&message)
                        .into_iter()
                        .filter(|diagnostic| diagnostic.path.ends_with(&file_name))
                        .collect();
                    diagnostics_changed = true;
                }
                continue;
            }
            let Some(id) = lsp_core::message_id(&message) else {
                continue;
            };
            let Some(position) = self
                .lsp_pending
                .iter()
                .position(|(candidate, _)| *candidate == id)
            else {
                continue;
            };
            let (_, request) = self.lsp_pending.remove(position);
            match request {
                LspRequest::Completion => {
                    self.lsp_completions = lsp_core::parse_completions(&message)
                        .into_iter()
                        .map(|(label, detail)| {
                            if detail.is_empty() {
                                label
                            } else {
                                format!("{label}  {detail}")
                            }
                        })
                        .collect();
                }
                LspRequest::Hover => {
                    if let Some(text) = lsp_core::parse_hover(&message) {
                        self.lsp_hover = Some((self.caret_line, self.caret_column, text));
                    }
                }
                LspRequest::Definition => {
                    if let Some((path, line, column)) =
                        lsp_core::parse_locations(&message).into_iter().next()
                    {
                        let target = PathBuf::from(path);
                        if target.is_file() {
                            self.open_file(target);
                            self.caret_line = line as usize;
                            self.caret_column = column as usize;
                            self.clamp_caret();
                            self.reveal_caret();
                            self.status = "Jumped to the definition reported by the server".into();
                        }
                    }
                }
                LspRequest::References => {
                    self.lsp_locations = lsp_core::parse_locations(&message)
                        .into_iter()
                        .map(|(path, line, column)| {
                            (PathBuf::from(path), line as usize, column as usize)
                        })
                        .collect();
                    self.lsp_location_cursor = 0;
                    let count = self.lsp_locations.len();
                    self.status = if count == 0 {
                        "The language server reported no references".into()
                    } else {
                        format!("{count} reference(s); Ctrl+R again jumps to the next one")
                    };
                }
                LspRequest::Rename => {
                    let edits = lsp_core::parse_rename_edits(&message);
                    self.apply_rename_edits(&edits);
                }
            }
        }
        self.language_server = Some(server);
        if diagnostics_changed {
            let errors = self
                .lsp_diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.severity == lsp_core::DiagnosticSeverity::Error)
                .count();
            let warnings = self
                .lsp_diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.severity == lsp_core::DiagnosticSeverity::Warning)
                .count();
            self.status = match (errors, warnings) {
                (0, 0) => "Language server reports no problems".into(),
                (0, warnings) => format!("{warnings} warning(s) from the language server"),
                (errors, 0) => format!("{errors} error(s) from the language server"),
                (errors, warnings) => {
                    format!("{errors} error(s) and {warnings} warning(s) from the language server")
                }
            };
        }
    }

    /// Diagnostics grouped by line, for the gutter markers.
    fn diagnostic_lines(&self) -> Vec<(usize, bool)> {
        let mut lines: Vec<(usize, bool)> = Vec::new();
        for diagnostic in lsp_core::filter_diagnostics(&self.lsp_diagnostics, self.problem_filter) {
            let is_error = diagnostic.severity == lsp_core::DiagnosticSeverity::Error;
            let line = diagnostic.line as usize;
            match lines.iter_mut().find(|entry| entry.0 == line) {
                Some(entry) => entry.1 = entry.1 || is_error,
                None => lines.push((line, is_error)),
            }
        }
        lines
    }

    /// Start indexing the project on a background thread.
    ///
    /// Scanning a large project must never block the UI thread, so the scan runs
    /// on its own thread and the result is collected by `poll_project_index`.
    fn request_project_index(&mut self) {
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
    fn poll_project_index(&mut self) {
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
        self.project_symbols = index.symbols();
        self.project_index = index;
        self.status = format!(
            "Index: {} file(s) reused, {} re-parsed, {} removed",
            stats.reused, stats.reparsed, stats.removed
        );
    }

    /// Progress the packaging output has actually reached.    /// Progress the packaging output has actually reached.
    fn package_output_progress(&self) -> Option<f32> {
        let index = self.terminals.iter().position(|tab| tab.name == "Build")?;
        let output = self.terminals.get(index).and_then(|tab| {
            tab.session
                .view
                .lock()
                .ok()
                .map(|view| view.lines.join("\n"))
        })?;
        workspace_core::packaging_progress(&output)
    }

    /// Artifacts the last packaging run should have produced.
    fn package_reports(
        &self,
        failed_command: Option<String>,
    ) -> Vec<workspace_core::ArtifactReport> {
        let project = self
            .project_root
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "TC-IDE-project".into())
            .replace(['\'', '"'], "");
        let seconds = self
            .package_started
            .map(|started| started.elapsed().as_secs())
            .unwrap_or(0);
        let mut targets = Vec::new();
        if self.package_targets[0] {
            targets.push(workspace_core::PackageTarget::WindowsExecutable);
        }
        if self.package_targets[1] {
            targets.push(workspace_core::PackageTarget::PortableZip);
        }
        if self.package_targets[2] {
            targets.push(workspace_core::PackageTarget::Msix);
        }
        targets
            .into_iter()
            .map(|target| {
                let path = self
                    .project_root
                    .join(".tc-ide-packages")
                    .join(target.artifact_name(&project));
                let bytes = fs::metadata(&path).map(|meta| meta.len()).unwrap_or(0);
                let ok = bytes > 0;
                workspace_core::ArtifactReport {
                    target,
                    path: path.to_string_lossy().into_owned(),
                    bytes,
                    seconds,
                    ok,
                    failed_command: if ok {
                        None
                    } else {
                        Some(
                            failed_command
                                .clone()
                                .unwrap_or_else(|| "the packaging command".to_string()),
                        )
                    },
                }
            })
            .collect()
    }

    /// Write BUILD-RESULT.txt next to the artifacts and report the outcome.
    fn finish_package(&mut self, failed_command: Option<String>) {
        let reports = self.package_reports(failed_command);
        let project = self
            .project_root
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "TC-IDE-project".into());
        let report = workspace_core::packaging_report(&reports, &project);
        let directory = self.project_root.join(".tc-ide-packages");
        let _ = fs::write(directory.join(workspace_core::PACKAGE_REPORT_FILE), &report);
        let failed = reports.iter().filter(|report| !report.ok).count();
        self.status = if failed == 0 {
            format!("Packaging finished: {}", reports[0].line())
        } else {
            format!("Packaging failed for {failed} target(s); see BUILD-RESULT.txt")
        };
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

    /// Settings the AI page currently shows.
    fn ai_settings(&self) -> ai_core::AiSettings {
        ai_core::AiSettings {
            endpoint: self.ai_endpoint.clone(),
            model: self.ai_model.clone(),
        }
    }

    /// Terminal that runs AI requests, spawned with the key in its environment.
    ///
    /// The key never appears in a command line or a log; only the child process
    /// environment carries it, and it stays in memory for this session.
    fn ensure_ai_terminal(&mut self) -> Option<usize> {
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
    fn write_ai_body(&self, body: &str) -> Option<PathBuf> {
        let directory = self.project_root.join(".tc-ide-ai");
        fs::create_dir_all(&directory).ok()?;
        let path = directory.join("request.json");
        fs::write(&path, body).ok()?;
        Some(path)
    }

    /// Run one AI request in the visible terminal.
    fn run_ai_request(&mut self, prompt: &str, stream: bool) {
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
            ai_core::chat_body(&settings.model, prompt, stream)
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
    fn poll_ai_terminal(&mut self) {
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

    /// Record an install once the terminal shows the matching checksum.
    fn confirm_pending_extension(&mut self) {
        let Some(source) = self.pending_extension.clone() else {
            return;
        };
        let Some(index) = self
            .terminals
            .iter()
            .position(|tab| tab.name == "Output")
            .or_else(|| self.terminals.len().checked_sub(1))
        else {
            return;
        };
        let output = self
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
        let Some(digest) = extension_core::checksum_from_output(&output) else {
            return;
        };
        if !extension_core::verify_checksum(&digest, &source.sha256) {
            self.status = format!(
                "{} failed checksum verification; the download was not installed",
                source.name
            );
            self.pending_extension = None;
            return;
        }
        let root = extension_cache_root();
        let shell = if cfg!(windows) {
            extension_core::Shell::Windows
        } else {
            extension_core::Shell::Unix
        };
        let directory = extension_core::install_plan(&root.to_string_lossy(), &source, shell)
            .map(|plan| plan.directory)
            .unwrap_or_default();
        self.runtime_cache.record(extension_core::CacheEntry {
            id: source.id.clone(),
            version: source.version.clone(),
            sha256: digest,
            bytes: source.bytes,
            directory,
        });
        save_runtime_cache(&self.runtime_cache);
        self.pending_extension = None;
        self.status = format!(
            "{} {} verified and recorded in the runtime cache",
            source.name, source.version
        );
    }

    /// Catalog the project ships, when the user provides one.
    ///
    /// Real downloads need a real url and checksum, so the catalog is data the
    /// user supplies at `<project>/.tc-ide-catalog.conf` instead of a list this
    /// repository would have to invent.
    fn project_catalog(&self) -> Result<Vec<extension_core::ExtensionSource>, String> {
        let path = self.project_root.join(".tc-ide-catalog.conf");
        let content = fs::read_to_string(&path).map_err(|_| {
            format!(
                "No extension catalog at {}; add one to install for real",
                path.display()
            )
        })?;
        extension_core::parse_catalog(&content)
    }

    /// Install the catalog entry for a language, when the project has one.
    fn install_language_extension(&mut self, language: &str) {
        let catalog = match self.project_catalog() {
            Ok(catalog) => catalog,
            Err(message) => {
                self.status = message;
                return;
            }
        };
        let wanted = language.to_ascii_lowercase();
        let Some(source) = catalog.into_iter().find(|source| {
            source.name.to_ascii_lowercase() == wanted
                || source.id.to_ascii_lowercase().contains(&wanted)
        }) else {
            self.status = format!("No catalog entry matches {language}");
            return;
        };
        self.run_extension_install(&source);
    }

    /// Install an extension by planning the real download and unpack steps.
    fn run_extension_install(&mut self, source: &extension_core::ExtensionSource) {
        let shell = if cfg!(windows) {
            extension_core::Shell::Windows
        } else {
            extension_core::Shell::Unix
        };
        let root = extension_cache_root();
        let plan = match extension_core::install_plan(&root.to_string_lossy(), source, shell) {
            Ok(plan) => plan,
            Err(error) => {
                self.status = error;
                return;
            }
        };
        if let Some(entry) = self.runtime_cache.reuse(&source.id, &source.version) {
            self.status = format!(
                "{} {} is already cached in {}",
                source.name, entry.version, entry.directory
            );
            return;
        }
        self.terminal_open = true;
        self.ensure_terminal();
        let Some(index) = self.terminals.len().checked_sub(1) else {
            return;
        };
        if let Some(tab) = self.terminals.get(index) {
            for command in &plan.commands {
                let _ = tab.session.write(&format!(
                    "{command}
"
                ));
            }
            self.active_terminal = index;
            self.pending_extension = Some(source.clone());
            self.status = format!(
                "Installing {} {} into {}",
                source.name, source.version, plan.directory
            );
        }
    }

    /// Format the active file with the formatter for its language.
    fn format_current_file(&mut self) {
        let language = editor_core::language_for_file(&self.file_name);
        let name = self
            .file_path
            .as_ref()
            .and_then(|path| path.file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.file_name.clone());
        let Some(command) = workspace_core::format_command(language, &name) else {
            self.status = format!("No formatter is configured for {language}");
            return;
        };
        self.terminal_open = true;
        self.ensure_terminal();
        if let Some(tab) = self.terminals.last() {
            let _ = tab.session.write(&format!("{command}\r"));
            self.active_terminal = self.terminals.len() - 1;
        }
        if let Some(path) = self.file_path.clone() {
            if let Ok(content) = fs::read_to_string(&path) {
                let mut lines = content.lines().map(str::to_string).collect::<Vec<_>>();
                if lines.is_empty() {
                    lines.push(String::new());
                }
                self.buffer = text_buffer::TextBuffer::from_lines(&lines);
                self.lines = lines;
                self.clamp_caret();
                self.status = format!("Formatted {} with {command}", path.display());
                return;
            }
        }
        self.status = format!("Formatting with {command}");
    }

    /// Start the debug adapter that fits the active file, when one is known.
    fn start_debug_adapter(&mut self) {
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
            // Follow the real build stages; never advance without progress.
            let reported = self.package_output_progress().unwrap_or(0.0);
            self.package_progress = self.package_progress.max(reported).min(0.95);
            let done = self.terminals.first().is_some_and(|tab| {
                tab.session.view.lock().ok().is_some_and(|view| {
                    view.lines
                        .iter()
                        .any(|line| line.contains(&self.package_marker))
                        || view.current.contains(&self.package_marker)
                })
            });
            let failure_line = self.terminals.first().and_then(|tab| {
                tab.session.view.lock().ok().and_then(|view| {
                    view.lines
                        .iter()
                        .rev()
                        .find(|line| {
                            !line.contains(&self.package_failure_marker) && !line.is_empty()
                        })
                        .cloned()
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
                self.package_failed_command = failure_line;
            }
            if failed {
                self.package_running = false;
                self.package_progress = 0.0;
                self.package_started = None;
                let command = self.package_failed_command.clone();
                self.finish_package(command);
            } else if done {
                self.package_running = false;
                self.package_progress = 1.0;
                self.package_reveal_pending = true;
                self.package_started = Some(Instant::now());
                self.finish_package(None);
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

    /// Shared spring step, so overlays move the same way on every backend.
    fn step_overlay_motion(value: &mut f32, velocity: &mut f32, open: bool, dt: f32) {
        let target = if open { 1.0 } else { 0.0 };
        motion_core::step(value, velocity, target, dt, motion_core::OVERLAY);
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

#[repr(C)]
struct LogFontW {
    height: i32,
    width: i32,
    escapement: i32,
    orientation: i32,
    weight: i32,
    italic: u8,
    underline: u8,
    strike_out: u8,
    char_set: u8,
    out_precision: u8,
    clip_precision: u8,
    quality: u8,
    pitch_and_family: u8,
    face_name: [u16; 32],
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
    fn GetDC(window: Hwnd) -> Hdc;
    fn GetMessageW(message: *mut Msg, window: Hwnd, minimum: Uint, maximum: Uint) -> Bool;
    fn InvalidateRect(window: Hwnd, area: *const Rect, erase: Bool) -> Bool;
    fn MessageBoxW(window: Hwnd, text: *const u16, caption: *const u16, kind: Uint) -> i32;
    fn LoadCursorW(instance: Hinstance, cursor_name: *const u16) -> Hcursor;
    fn PostQuitMessage(exit_code: i32);
    fn RegisterClassW(class: *const WindowClass) -> u16;
    fn ReleaseCapture() -> Bool;
    fn ReleaseDC(window: Hwnd, dc: Hdc) -> i32;
    fn SetCapture(window: Hwnd) -> Hwnd;
    fn SetForegroundWindow(window: Hwnd) -> Bool;
    fn SetTimer(window: Hwnd, event: usize, milliseconds: Uint, callback: *const c_void) -> usize;
    fn ShowWindow(window: Hwnd, command: i32) -> Bool;
    fn TranslateMessage(message: *const Msg) -> Bool;
    fn UpdateWindow(window: Hwnd) -> Bool;
}

#[link(name = "gdi32")]
extern "system" {
    fn CreateFontIndirectW(log_font: *const LogFontW) -> Hfont;
    fn CreateSolidBrush(color: Dword) -> Hbrush;
    fn DeleteObject(object: *mut c_void) -> Bool;
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
                if platform::open_external_terminal(window, &app.project_root) {
                    app.status = format!(
                        "Opened an external terminal in {}",
                        app.project_root.display()
                    );
                } else if platform::open_in_explorer(window, &app.project_root) {
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
            let terminal_buffer = terminal_buffer(&app);
            let panel = terminal_core::panel_metrics(
                width as f32,
                height as f32,
                dpi as f32 / 96.0,
                app.terminal_motion,
            );
            let viewport = terminal_core::TerminalViewport {
                total_lines: terminal_buffer.len(),
                visible_rows: panel.visible_rows,
                scroll: app.terminal_scroll,
            };
            let terminal_lines = terminal_core::visible_lines(&terminal_buffer, &viewport).to_vec();
            let terminal_selection = app.terminal_selection.map(|selection| {
                let (start, end) = selection.ordered();
                ((start.line, start.column), (end.line, end.column))
            });
            let terminal_names = app
                .terminals
                .iter()
                .map(|tab| tab.name.clone())
                .collect::<Vec<_>>();
            let completions = app.completions();
            let first_line = app.viewport.clamped(app.lines.len()).first_line;
            let visible_lines = app
                .buffer
                .line_window(first_line, app.viewport.height.max(1));
            let visible_lines = if visible_lines.is_empty() {
                vec![String::new()]
            } else {
                visible_lines
            };
            let all_symbols = editor_core::symbol_occurrences(&app.lines, &app.file_name);
            // Symbol rails only make sense for lines that are on screen.
            let symbols = all_symbols
                .iter()
                .filter(|symbol| symbol.line >= first_line)
                .map(|symbol| {
                    let mut symbol = symbol.clone();
                    symbol.line -= first_line;
                    symbol
                })
                .collect::<Vec<_>>();
            let hovered_symbol = app.hovered_symbol.as_ref().and_then(|symbol| {
                if symbol.line < first_line {
                    return None;
                }
                let mut symbol = symbol.clone();
                symbol.line -= first_line;
                Some(symbol)
            });
            let graph_rows = graph_visible_rows(&area, dpi as f32 / 96.0, app.terminal_motion);
            let graph = build_graph(&app, &symbols);
            let tabs = app
                .session
                .documents()
                .iter()
                .map(|document| document.title())
                .collect::<Vec<_>>();
            let active_tab = app.session.active();
            let signing_certificate = app
                .signing_certificate
                .as_ref()
                .and_then(|path| path.file_name())
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            let language_tooling = app.selected_language_tooling();
            let chat_messages = app.chat_lines();
            let problem_rows = app
                .problem_rows()
                .into_iter()
                .map(|(text, _)| text)
                .collect::<Vec<_>>();
            let ai_cache_summary = format!(
                "runtime cache: {} entries · {}",
                app.runtime_cache.entries().len(),
                extension_core::format_bytes(app.runtime_cache.total_bytes())
            );
            let breakpoints = app.active_breakpoints();
            let diagnostic_lines = app
                .diagnostic_lines()
                .into_iter()
                .filter(|(line, _)| *line >= first_line)
                .map(|(line, is_error)| (line - first_line, is_error))
                .collect::<Vec<_>>();
            let status_line = if let Some(prompt) = app.rename_prompt.as_ref() {
                format!("Rename to: {prompt}")
            } else if let Some(diagnostic) = app
                .lsp_diagnostics
                .iter()
                .find(|diagnostic| diagnostic.line as usize == app.caret_line)
            {
                diagnostic.line_text()
            } else {
                match &app.lsp_hover {
                    Some((_, _, text)) => text.lines().next().unwrap_or("").to_string(),
                    None => app.status.clone(),
                }
            };
            let graph_first_index = graph.clamped_scroll(app.graph_scroll, graph_rows);
            let graph_nodes = graph.visible(app.graph_scroll, graph_rows).to_vec();
            let graph_summary = graph.footer(app.graph_scroll, graph_rows);
            let graph_scope_label = if app.graph_scope.is_none() {
                format!("auto · {}", graph.scope_label())
            } else {
                graph.scope_label().to_string()
            };
            let expanded_comments = app
                .expanded_comments
                .iter()
                .filter(|line| **line >= first_line)
                .map(|line| line - first_line)
                .collect::<Vec<_>>();
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
                lines: &visible_lines,
                caret_line: app.caret_line.saturating_sub(first_line),
                caret_column: app.caret_column,
                scroll_columns: app.viewport.first_column,
                first_line,
                total_lines: app.lines.len(),
                selection: app.selection_range().and_then(|(start, end)| {
                    if end.line < first_line {
                        return None;
                    }
                    let start_line = start.line.max(first_line) - first_line;
                    let end_line = end.line - first_line;
                    Some(((start_line, start.column), (end_line, end.column)))
                }),
                ime_composition: &app.ime_composition,
                completions: &completions,
                light_theme: app.light_theme,
                ui_chinese: app.ui_chinese,
                code_zoom: app.code_zoom,
                code_font_family: app.code_font_family,
                font_search: &app.font_search,
                font_search_focused: app.font_search_focused,
                font_options: &WindowsAppState::CODE_FONTS,
                settings_motion: app.settings_motion,
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
                ai_prompt: &app.ai_prompt,
                ai_prompt_focused: app.ai_prompt_focused,
                ai_cache_summary: &ai_cache_summary,
                problems_open: app.problems_open,
                problems: &problem_rows,
                problems_selected: app.problems_selection,
                problems_hint: &format!(
                    "filter {} · Tab cycles · Enter jumps · Esc closes{}",
                    app.problem_filter.label(),
                    if app.debug_session.is_some() {
                        " · debugging"
                    } else {
                        ""
                    }
                ),
                language_tooling: &language_tooling,
                chat_open: app.chat_open,
                chat_messages: &chat_messages,
                chat_input: &app.chat_input,
                chat_running: app.chat_session.is_some(),
                chat_error: app.chat_error.as_deref().unwrap_or_default(),
                ai_field_focused: app.ai_field_focused,
                symbols: &symbols,
                hovered_symbol: hovered_symbol.as_ref(),
                tabs: &tabs,
                active_tab,
                tab_dirty: app
                    .session
                    .documents()
                    .iter()
                    .map(|document| document.dirty)
                    .collect::<Vec<_>>(),
                graph_nodes: &graph_nodes,
                graph_first_index,
                graph_selected: app.graph_selected,
                graph_search: &app.graph_search,
                graph_search_focused: app.graph_search_focused,
                graph_scope_label: &graph_scope_label,
                graph_summary: &graph_summary,
                expanded_comments: &expanded_comments,
                editing_comment: app
                    .editing_comment
                    .filter(|line| *line >= first_line)
                    .map(|line| line - first_line),
                pointer_x: app.pointer_x,
                pointer_y: app.pointer_y,
                project_total: app
                    .project_entries
                    .iter()
                    .filter(|entry| {
                        let query = app.workspace_search.to_ascii_lowercase();
                        query.is_empty() || entry.to_ascii_lowercase().contains(&query)
                    })
                    .count(),
                project_scroll: app.project_scroll,
                package_motion: app.package_motion,
                package_targets: app.package_targets,
                signing_certificate: &signing_certificate,
                package_running: app.package_running,
                package_progress: app.package_progress,
                background_path: background_path.as_deref(),
                background_opacity: app.background_opacity,
                background_offset_x: app.background_offset_x,
                background_offset_y: app.background_offset_y,
                background_zoom: app.background_zoom,
                status: &status_line,
                diagnostic_lines: &diagnostic_lines,
                breakpoint_lines: &breakpoints
                    .iter()
                    .filter(|line| **line >= first_line)
                    .map(|line| line - first_line)
                    .collect::<Vec<_>>(),
                terminal_lines: &terminal_lines,
                terminal_first_line: viewport.first_line(),
                terminal_selection,
                terminal_names: &terminal_names,
                active_terminal: app.active_terminal,
                terminal_input: active_terminal.map(|tab| tab.input.as_str()).unwrap_or(""),
                terminal_open: app.terminal_open,
                terminal_progress: app.terminal_motion,
                terminal_scrolled: viewport.clamped_scroll(),
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
        } else if app.chat_input_focused {
            if !value.is_control() {
                app.chat_input.push(value);
            }
        } else if app.rename_prompt.is_some() {
            if !value.is_control() {
                if let Some(prompt) = app.rename_prompt.as_mut() {
                    prompt.push(value);
                }
            }
        } else if app.ai_prompt_focused {
            if !value.is_control() {
                app.ai_prompt.push(value);
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
        } else if app.graph_search_focused {
            if !value.is_control() {
                app.graph_search.push(value);
                app.graph_scroll = 0;
            }
        } else if app.terminal_open {
            app.terminal_scroll = 0;
            app.terminal_selection = None;
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

/// Relation graph for the current editor, hover and search state.
fn build_graph(
    app: &WindowsAppState,
    symbols: &[editor_core::SymbolOccurrence],
) -> graph_core::Graph {
    let project_name = app
        .project_root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("PROJECT");
    let inputs = graph_core::GraphInputs {
        project_name,
        current_file: &app.file_name,
        current_folder: graph_core::folder_of(&app.file_name),
        hovered: app.hovered_symbol.as_ref(),
        file_lines: &app.lines,
        file_symbols: symbols,
        project_symbols: &app.project_symbols,
        folded: &app.graph_folded,
        relation: app.graph_relation,
    };
    let scope = graph_core::auto_scope(
        app.hovered_symbol.as_ref(),
        &app.graph_search,
        app.graph_scope,
    );
    graph_core::build_graph(&inputs, scope, &app.graph_search)
}

/// Left edge of the relation graph panel.
fn graph_left(area: &Rect, scale: f32) -> f32 {
    area.right as f32 - (area.right as f32 * 0.27).max(300.0 * scale)
}

/// Cards the graph panel can show at the current window size.
fn graph_visible_rows(area: &Rect, scale: f32, terminal_motion: f32) -> usize {
    let panel = terminal_core::panel_metrics(
        area.right as f32,
        area.bottom as f32,
        scale,
        terminal_motion,
    );
    graph_core::visible_rows(48.0 * scale, area.bottom as f32 - panel.height, scale)
}

/// Screen rectangle of the graph range chip.
fn graph_scope_rect(area: &Rect, scale: f32) -> Rect {
    Rect {
        left: (area.right as f32 - 150.0 * scale) as i32,
        top: (60.0 * scale) as i32,
        right: (area.right as f32 - 18.0 * scale) as i32,
        bottom: (90.0 * scale) as i32,
    }
}

/// Screen rectangle of the graph search field.
fn graph_search_rect(area: &Rect, scale: f32) -> Rect {
    Rect {
        left: (graph_left(area, scale) + 18.0 * scale) as i32,
        top: (94.0 * scale) as i32,
        right: (area.right as f32 - 18.0 * scale) as i32,
        bottom: (126.0 * scale) as i32,
    }
}

/// Client area of a window.
fn area_for(window: Hwnd) -> Rect {
    let mut area: Rect = unsafe { std::mem::zeroed() };
    unsafe { GetClientRect(window, &mut area) };
    area
}

fn inside(rect: &Rect, x: i32, y: i32) -> bool {
    x >= rect.left && x <= rect.right && y >= rect.top && y <= rect.bottom
}

/// Buffered output of the terminal the panel and the IDE share.
fn terminal_buffer(app: &WindowsAppState) -> Vec<String> {
    app.terminals
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
        .unwrap_or_default()
}

/// Buffer position under a point inside the bottom terminal panel.
fn terminal_point_at(
    app: &WindowsAppState,
    panel: &terminal_core::TerminalPanel,
    scale: f32,
    x: i32,
    y: i32,
    clamp: bool,
) -> Option<terminal_core::TerminalPoint> {
    let row = (y as f32 - panel.body_top) / panel.row_height.max(1.0);
    let row = if row < 0.0 {
        if clamp {
            0.0
        } else {
            return None;
        }
    } else if row >= panel.visible_rows as f32 {
        if clamp {
            (panel.visible_rows - 1) as f32
        } else {
            return None;
        }
    } else {
        row
    };
    let lines = terminal_buffer(app);
    let viewport = terminal_core::TerminalViewport {
        total_lines: lines.len(),
        visible_rows: panel.visible_rows,
        scroll: app.terminal_scroll,
    };
    let line = viewport.line_at_row(row.floor() as usize)?;
    let cell_width = (terminal_core::code_cell_width(app.code_zoom) * scale).max(1.0);
    let cells = ((x as f32 - panel.body_left) / cell_width).max(0.0) as usize;
    let value = lines.get(line)?;
    Some(terminal_core::TerminalPoint {
        line,
        column: terminal_core::column_for_cells(value, cells),
    })
}

/// Copy the current bottom panel selection into the clipboard.
fn copy_terminal_selection(window: Hwnd) -> bool {
    let selection = APP.with(|cell| cell.borrow().terminal_selection);
    let Some(selection) = selection else {
        return false;
    };
    let lines = APP.with(|cell| terminal_buffer(&cell.borrow()));
    let Some(text) = terminal_core::selected_text(&lines, &selection) else {
        return false;
    };
    platform::write_clipboard(window, &text).is_ok()
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

/// Ask before throwing away unsaved edits. Uses a native Windows message box.
fn confirm_discard(window: Hwnd, name: &str) -> bool {
    let text = wide(&format!(
        "{name} has unsaved changes. Close it and discard them?"
    ));
    let caption = wide("TC-IDE - unsaved changes");
    let answer = unsafe {
        MessageBoxW(
            window,
            text.as_ptr(),
            caption.as_ptr(),
            MB_YESNO | MB_ICONWARNING,
        )
    };
    answer == IDYES
}

/// User level runtime cache for extensions and runtimes.
fn extension_cache_root() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("TC-IDE")
        .join("runtimes")
}

/// Manifest path of the runtime cache.
fn runtime_cache_path() -> PathBuf {
    extension_cache_root().join("cache.conf")
}

fn load_runtime_cache() -> extension_core::RuntimeCache {
    match fs::read_to_string(runtime_cache_path()) {
        Ok(content) => extension_core::RuntimeCache::parse(&content),
        Err(_) => extension_core::RuntimeCache::default(),
    }
}

fn save_runtime_cache(cache: &extension_core::RuntimeCache) {
    let path = runtime_cache_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(path, cache.serialize());
}

/// Where the session (open documents, project, recent projects) is stored./// Where the session (open documents, project, recent projects) is stored.
fn session_path() -> PathBuf {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("TC-IDE")
        .join("session.conf")
}

/// Read the previous session, if any.
fn load_session() -> session_core::Session {
    match fs::read_to_string(session_path()) {
        Ok(content) => session_core::Session::parse(&content),
        Err(_) => session_core::Session::default(),
    }
}

/// Persist the session. Documents are listed, never their contents or secrets.
fn save_session(session: &session_core::Session) {
    let path = session_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(path, session.serialize());
}

/// Grid the bottom terminal panel shows at the current client size./// Grid the bottom terminal panel shows at the current client size.
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
///
/// While a detached terminal window is open it owns the grid of the session it
/// shows, so the IDE keeps resizing the other sessions only.
fn sync_terminal_grid(window: Hwnd) {
    let mut area: Rect = unsafe { std::mem::zeroed::<Rect>() };
    unsafe { GetClientRect(window, &mut area) };
    let dpi = platform::dpi_for_window(window);
    let (code_zoom, count, active, detached) = APP.with(|cell| {
        let app = cell.borrow();
        (
            app.code_zoom,
            app.terminals.len(),
            app.active_terminal,
            !app.detached_terminal.is_null(),
        )
    });
    let signature = (
        (area.right - area.left).max(0) as u32,
        (area.bottom - area.top).max(0) as u32,
        dpi,
        code_zoom,
        count,
        active,
        detached,
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
        let detached = !app.detached_terminal.is_null();
        let active = app.active_terminal;
        for (index, terminal) in app.terminals.iter().enumerate() {
            if detached && index == active {
                continue;
            }
            terminal.session.resize(grid.columns, grid.rows);
        }
    });
}

unsafe fn draw_terminal_text(dc: Hdc, value: &str, x: i32, y: i32, right: i32, color: Dword) {
    draw_terminal_line(
        dc,
        value,
        Rect {
            left: x,
            top: y,
            right,
            bottom: y + 24,
        },
        DT_SINGLELINE | DT_NOPREFIX,
        color,
    );
}

unsafe fn draw_terminal_line(dc: Hdc, value: &str, area: Rect, format: Uint, color: Dword) {
    let wide = value.encode_utf16().collect::<Vec<_>>();
    let mut area = area;
    unsafe {
        SetTextColor(dc, color);
        DrawTextW(dc, wide.as_ptr(), wide.len() as i32, &mut area, format);
    }
}

/// Monospace font of the detached terminal, rebuilt when the window DPI changes.
unsafe fn detached_font(window: Hwnd) -> Hfont {
    let dpi = platform::dpi_for_window(window);
    let (current, current_dpi) = DETACHED.with(|cell| {
        let state = cell.borrow();
        (state.font, state.font_dpi)
    });
    if !current.is_null() && current_dpi == dpi {
        return current;
    }
    let scale = dpi.max(96) as f32 / 96.0;
    let mut log_font: LogFontW = unsafe { std::mem::zeroed() };
    log_font.height = -((DETACHED_FONT_HEIGHT * scale).round() as i32).max(9);
    log_font.weight = FW_NORMAL;
    log_font.char_set = DEFAULT_CHARSET;
    log_font.out_precision = OUT_TT_PRECIS;
    log_font.clip_precision = CLIP_DEFAULT_PRECIS;
    log_font.quality = CLEARTYPE_QUALITY;
    log_font.pitch_and_family = FF_MODERN | FIXED_PITCH;
    for (index, unit) in DETACHED_FONT_FACE.encode_utf16().take(31).enumerate() {
        log_font.face_name[index] = unit;
    }
    let font = unsafe { CreateFontIndirectW(&log_font) };
    DETACHED.with(|cell| {
        let mut state = cell.borrow_mut();
        let previous = state.font;
        state.font = font;
        state.font_dpi = dpi;
        if !previous.is_null() {
            unsafe { DeleteObject(previous) };
        }
    });
    font
}

/// Glyph advance and line height measured from the detached terminal font.
fn detached_cell_metrics(window: Hwnd) -> (f32, f32) {
    let dpi = platform::dpi_for_window(window);
    let cached = DETACHED.with(|cell| {
        let state = cell.borrow();
        if state.metrics_dpi == dpi {
            state.metrics
        } else {
            None
        }
    });
    if let Some(metrics) = cached {
        return metrics;
    }
    let metrics = measure_detached_cells(window);
    DETACHED.with(|cell| {
        let mut state = cell.borrow_mut();
        state.metrics = Some(metrics);
        state.metrics_dpi = dpi;
    });
    metrics
}

/// Measure the detached terminal font once per window DPI.
fn measure_detached_cells(window: Hwnd) -> (f32, f32) {
    unsafe {
        let font = detached_font(window);
        let dc = GetDC(window);
        if dc.is_null() {
            return (
                terminal_core::code_cell_width(0),
                DETACHED_FONT_HEIGHT * 1.35,
            );
        }
        let previous = if font.is_null() {
            ptr::null_mut()
        } else {
            SelectObject(dc, font)
        };
        let sample = "M".repeat(64);
        let sample_wide = sample.encode_utf16().collect::<Vec<_>>();
        let mut sample_area = Rect {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        DrawTextW(
            dc,
            sample_wide.as_ptr(),
            sample_wide.len() as i32,
            &mut sample_area,
            DT_CALCRECT | DT_SINGLELINE | DT_NOPREFIX,
        );
        let width = ((sample_area.right - sample_area.left).max(1) as f32) / 64.0;
        let glyph = wide("M");
        let mut glyph_area = Rect {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        DrawTextW(
            dc,
            glyph.as_ptr(),
            1,
            &mut glyph_area,
            DT_CALCRECT | DT_SINGLELINE | DT_NOPREFIX,
        );
        let height = (glyph_area.bottom - glyph_area.top).max(1) as f32;
        if !previous.is_null() {
            SelectObject(dc, previous);
        }
        ReleaseDC(window, dc);
        (width, height)
    }
}

/// Lay out header, scrollback area and input line of the detached window.
fn detached_layout(area: &Rect, cell_width: f32, cell_height: f32) -> DetachedLayout {
    let width = (area.right - area.left).max(1) as f32;
    let height = (area.bottom - area.top).max(1) as f32;
    let cell_width = if cell_width.is_finite() && cell_width > 0.0 {
        cell_width
    } else {
        8.0
    };
    let cell_height = if cell_height.is_finite() && cell_height > 0.0 {
        cell_height
    } else {
        16.0
    };
    let left = DETACHED_TEXT_LEFT;
    let right = (width - DETACHED_TEXT_RIGHT).max(left + cell_width);
    let top = DETACHED_HEADER_HEIGHT;
    let input_top = (height - cell_height - DETACHED_INPUT_GAP).max(top + cell_height);
    let bottom = (input_top - DETACHED_INPUT_GAP).max(top + 1.0);
    let visible_rows = (((bottom - top) / cell_height).floor().max(1.0)) as usize;
    let grid =
        terminal_core::GridSize::from_metrics(right - left, bottom - top, cell_width, cell_height);
    DetachedLayout {
        left,
        right,
        top,
        input_top,
        row_height: cell_height,
        cell_width,
        visible_rows,
        columns: grid.columns,
        rows: grid.rows,
    }
}

/// Buffered output of the terminal the detached window shares with the IDE.
fn detached_lines() -> Vec<String> {
    APP.with(|cell| {
        let app = cell.borrow();
        app.terminals
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
            .unwrap_or_default()
    })
}

/// Lines, scrollback window and layout used by painting and hit testing.
fn detached_context(window: Hwnd) -> DetachedContext {
    let mut area: Rect = unsafe { std::mem::zeroed() };
    unsafe { GetClientRect(window, &mut area) };
    let (cell_width, cell_height) = detached_cell_metrics(window);
    let layout = detached_layout(&area, cell_width, cell_height);
    let lines = detached_lines();
    let scroll = DETACHED.with(|cell| cell.borrow().scroll);
    let viewport = terminal_core::TerminalViewport {
        total_lines: lines.len(),
        visible_rows: layout.visible_rows,
        scroll,
    };
    DetachedContext {
        lines,
        viewport,
        layout,
    }
}

/// Buffer position under a window point, optionally clamped into the viewport.
fn detached_point_at(
    context: &DetachedContext,
    x: i32,
    y: i32,
    clamp: bool,
) -> Option<terminal_core::TerminalPoint> {
    let row = (y as f32 - context.layout.top) / context.layout.row_height;
    let row = if row < 0.0 {
        if clamp {
            0.0
        } else {
            return None;
        }
    } else if row >= context.viewport.visible_rows as f32 {
        if clamp {
            (context.viewport.visible_rows - 1) as f32
        } else {
            return None;
        }
    } else {
        row
    };
    let line = context.viewport.line_at_row(row.floor() as usize)?;
    let cells = ((x as f32 - context.layout.left) / context.layout.cell_width).max(0.0) as usize;
    let value = context.lines.get(line)?;
    Some(terminal_core::TerminalPoint {
        line,
        column: terminal_core::column_for_cells(value, cells),
    })
}

/// Push the detached window size into the ConPTY grid it shows.
fn sync_detached_terminal_grid(window: Hwnd) {
    let mut area: Rect = unsafe { std::mem::zeroed() };
    unsafe { GetClientRect(window, &mut area) };
    let (cell_width, cell_height) = detached_cell_metrics(window);
    let layout = detached_layout(&area, cell_width, cell_height);
    let (active, count) = APP.with(|cell| {
        let app = cell.borrow();
        (app.active_terminal, app.terminals.len())
    });
    let signature = (
        (area.right - area.left).max(0) as u32,
        (area.bottom - area.top).max(0) as u32,
        cell_width.to_bits(),
        cell_height.to_bits(),
        active,
    );
    let changed = DETACHED.with(|cell| {
        let mut state = cell.borrow_mut();
        if state.grid_signature == signature {
            return false;
        }
        state.grid_signature = signature;
        true
    });
    if !changed || count == 0 {
        return;
    }
    APP.with(|cell| {
        let app = cell.borrow();
        if let Some(tab) = app.terminals.get(active) {
            tab.session.resize(layout.columns, layout.rows);
        }
    });
}

/// Copy the detached window selection into the clipboard.
fn copy_detached_selection(window: Hwnd) -> bool {
    let Some(selection) = DETACHED.with(|cell| cell.borrow().selection) else {
        return false;
    };
    let lines = detached_lines();
    let Some(text) = terminal_core::selected_text(&lines, &selection) else {
        return false;
    };
    let copied = platform::write_clipboard(window, &text).is_ok();
    let notice = if copied {
        format!("Copied {} characters", text.chars().count())
    } else {
        "Clipboard unavailable".to_string()
    };
    DETACHED.with(|cell| {
        let mut state = cell.borrow_mut();
        state.notice = Some(notice);
        state.notice_started = Some(Instant::now());
    });
    copied
}

/// Scroll the detached viewport while keeping it inside the buffered history.
fn scroll_detached(window: Hwnd, rows: i32) {
    let context = detached_context(window);
    let scroll = context.viewport.scroll_by(rows);
    DETACHED.with(|cell| cell.borrow_mut().scroll = scroll);
    invalidate(window);
}

/// Return the detached viewport to the newest output.
fn scroll_detached_to_bottom() {
    DETACHED.with(|cell| cell.borrow_mut().scroll = 0);
}

unsafe extern "system" fn detached_terminal_proc(
    window: Hwnd,
    message: Uint,
    w_param: Wparam,
    l_param: Lparam,
) -> Lresult {
    match message {
        WM_PAINT => {
            let mut paint: PaintStruct = unsafe { std::mem::zeroed() };
            let dc = unsafe { BeginPaint(window, &mut paint) };
            let mut area: Rect = unsafe { std::mem::zeroed() };
            unsafe { GetClientRect(window, &mut area) };
            let context = detached_context(window);
            let layout = &context.layout;
            let (name, input) = APP.with(|cell| {
                let app = cell.borrow();
                let tab = app.terminals.get(app.active_terminal);
                (
                    tab.map(|tab| tab.name.clone())
                        .unwrap_or_else(|| "Terminal".to_string()),
                    tab.map(|tab| tab.input.clone()).unwrap_or_default(),
                )
            });
            let (selection, notice, composition) =
                DETACHED.with(|cell| {
                    let state = cell.borrow();
                    let notice = state.notice.as_ref().zip(state.notice_started).and_then(
                        |(text, started)| {
                            (started.elapsed().as_secs_f32() < NOTICE_SECONDS).then(|| text.clone())
                        },
                    );
                    (state.selection, notice, state.composition.clone())
                });
            let background = unsafe { CreateSolidBrush(0x001b1917) };
            let highlight = unsafe { CreateSolidBrush(0x00513419) };
            unsafe {
                FillRect(dc, &area, background);
                SetBkMode(dc, TRANSPARENT_MODE);
            }
            let font = detached_font(window);
            let previous_font = if font.is_null() {
                ptr::null_mut()
            } else {
                unsafe { SelectObject(dc, font) }
            };
            let hint = if context.viewport.clamped_scroll() > 0 {
                format!(
                    "history ↑{} of {} · wheel scrolls",
                    context.viewport.clamped_scroll(),
                    context.viewport.max_scroll()
                )
            } else {
                "wheel scrolls · drag selects · Ctrl+Shift+C copies · Ctrl+C interrupts".to_string()
            };
            unsafe {
                draw_terminal_text(
                    dc,
                    "TianChuangYa",
                    DETACHED_TEXT_LEFT as i32,
                    14,
                    area.right - 20,
                    0x00e6bc71,
                );
                draw_terminal_line(
                    dc,
                    &hint,
                    Rect {
                        left: (layout.right - 420.0).max(layout.left) as i32,
                        top: 14,
                        right: layout.right as i32,
                        bottom: 40,
                    },
                    DT_SINGLELINE | DT_NOPREFIX | DT_RIGHT,
                    0x008e8073,
                );
                draw_terminal_text(
                    dc,
                    &format!("TC-IDE · {name} · shared ConPTY session"),
                    DETACHED_TEXT_LEFT as i32,
                    42,
                    area.right - 20,
                    0x009d9690,
                );
                if let Some(notice) = notice.as_deref() {
                    draw_terminal_line(
                        dc,
                        notice,
                        Rect {
                            left: (layout.right - 300.0).max(layout.left) as i32,
                            top: 42,
                            right: layout.right as i32,
                            bottom: 68,
                        },
                        DT_SINGLELINE | DT_NOPREFIX | DT_RIGHT,
                        0x00dfb56b,
                    );
                }
            }
            let (selection_start, selection_end) = match selection.map(|it| it.ordered()) {
                Some((start, end)) => (Some(start), Some(end)),
                None => (None, None),
            };
            for (index, value) in terminal_core::visible_lines(&context.lines, &context.viewport)
                .iter()
                .enumerate()
            {
                let row_top = layout.top + index as f32 * layout.row_height;
                if let (Some(start), Some(end)) = (selection_start, selection_end) {
                    if let Some(line) = context.viewport.line_at_row(index) {
                        if line >= start.line && line <= end.line {
                            let length = value.chars().count();
                            let from = if line == start.line {
                                terminal_core::visual_cells(value, start.column)
                            } else {
                                0
                            };
                            let to = if line == end.line {
                                terminal_core::visual_cells(value, end.column)
                            } else {
                                terminal_core::visual_cells(value, length)
                            };
                            if to > from {
                                let band = Rect {
                                    left: (layout.left + from as f32 * layout.cell_width) as i32,
                                    top: row_top as i32,
                                    right: (layout.left + to as f32 * layout.cell_width).ceil()
                                        as i32,
                                    bottom: (row_top + layout.row_height).ceil() as i32,
                                };
                                unsafe { FillRect(dc, &band, highlight) };
                            }
                        }
                    }
                }
                unsafe {
                    draw_terminal_line(
                        dc,
                        value,
                        Rect {
                            left: layout.left as i32,
                            top: row_top as i32,
                            right: layout.right as i32,
                            bottom: (row_top + layout.row_height + 2.0) as i32,
                        },
                        DT_SINGLELINE | DT_NOPREFIX,
                        0x00e8e8e8,
                    );
                }
            }
            unsafe {
                draw_terminal_line(
                    dc,
                    &format!("> {input}{composition}▌"),
                    Rect {
                        left: layout.left as i32,
                        top: layout.input_top as i32,
                        right: layout.right as i32,
                        bottom: (layout.input_top + layout.row_height + 3.0) as i32,
                    },
                    DT_SINGLELINE | DT_NOPREFIX,
                    0x00dfb56b,
                );
                if !previous_font.is_null() {
                    SelectObject(dc, previous_font);
                }
                DeleteObject(background);
                DeleteObject(highlight);
                EndPaint(window, &paint);
            }
            0
        }
        WM_CHAR => {
            scroll_detached_to_bottom();
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
            scroll_detached_to_bottom();
            if platform::control_pressed() {
                let key = w_param as u32;
                if key == 0x43 && platform::shift_pressed() {
                    copy_detached_selection(window);
                    invalidate(window);
                    return 0;
                }
                if handle_terminal_control(window, key) {
                    invalidate(window);
                    return 0;
                }
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
        WM_IME_STARTCOMPOSITION => {
            DETACHED.with(|cell| cell.borrow_mut().composition.clear());
            invalidate(window);
            0
        }
        WM_IME_COMPOSITION => {
            if l_param & GCS_RESULTSTR != 0 {
                let result = ime_string(window, GCS_RESULTSTR as Dword);
                DETACHED.with(|cell| cell.borrow_mut().composition.clear());
                if !result.is_empty() {
                    APP.with(|cell| {
                        let mut app = cell.borrow_mut();
                        let active = app.active_terminal;
                        if let Some(tab) = app.terminals.get_mut(active) {
                            tab.input.push_str(&result);
                            let _ = tab.session.write(&result);
                        }
                    });
                }
            } else if l_param & GCS_COMPSTR != 0 {
                let composition = ime_string(window, GCS_COMPSTR as Dword);
                DETACHED.with(|cell| cell.borrow_mut().composition = composition);
            }
            invalidate(window);
            0
        }
        WM_IME_ENDCOMPOSITION => {
            DETACHED.with(|cell| cell.borrow_mut().composition.clear());
            invalidate(window);
            0
        }
        WM_MOUSEWHEEL => {
            let delta = ((w_param >> 16) as u16 as i16) as i32;
            if delta > 0 {
                scroll_detached(window, WHEEL_SCROLL_LINES);
            } else if delta < 0 {
                scroll_detached(window, -WHEEL_SCROLL_LINES);
            }
            0
        }
        WM_LBUTTONDOWN => {
            let x = (l_param as u16 as i16) as i32;
            let y = ((l_param >> 16) as u16 as i16) as i32;
            let context = detached_context(window);
            if let Some(point) = detached_point_at(&context, x, y, false) {
                DETACHED.with(|cell| {
                    let mut state = cell.borrow_mut();
                    state.selection = Some(terminal_core::TerminalSelection::new(point, point));
                    state.dragging = true;
                });
                unsafe { SetCapture(window) };
                invalidate(window);
            }
            0
        }
        WM_MOUSEMOVE => {
            let dragging = DETACHED.with(|cell| cell.borrow().dragging);
            if dragging && w_param & 1 != 0 {
                let x = (l_param as u16 as i16) as i32;
                let y = ((l_param >> 16) as u16 as i16) as i32;
                let context = detached_context(window);
                if let Some(point) = detached_point_at(&context, x, y, true) {
                    let changed = DETACHED.with(|cell| {
                        let mut state = cell.borrow_mut();
                        let Some(selection) = state.selection else {
                            return false;
                        };
                        if selection.focus == point {
                            return false;
                        }
                        state.selection = Some(selection.extended_to(point));
                        true
                    });
                    if changed {
                        invalidate(window);
                    }
                }
            }
            0
        }
        WM_LBUTTONUP => {
            let dragging = DETACHED.with(|cell| {
                let mut state = cell.borrow_mut();
                let dragging = state.dragging;
                state.dragging = false;
                dragging
            });
            if dragging {
                unsafe { ReleaseCapture() };
            }
            let cleared = DETACHED.with(|cell| {
                let mut state = cell.borrow_mut();
                if state.selection.map(|it| it.is_empty()).unwrap_or(false) {
                    state.selection = None;
                    true
                } else {
                    false
                }
            });
            if cleared || dragging {
                invalidate(window);
            }
            0
        }
        WM_TIMER | WM_SIZE => {
            sync_detached_terminal_grid(window);
            invalidate(window);
            0
        }
        WM_CLOSE => {
            unsafe { DestroyWindow(window) };
            0
        }
        WM_DESTROY => {
            unsafe { ReleaseCapture() };
            let font = DETACHED.with(|cell| {
                let mut state = cell.borrow_mut();
                let font = state.font;
                *state = DetachedTerminalState::default();
                font
            });
            if !font.is_null() {
                unsafe { DeleteObject(font) };
            }
            APP.with(|cell| cell.borrow_mut().detached_terminal = ptr::null_mut());
            0
        }
        _ => unsafe { DefWindowProcW(window, message, w_param, l_param) },
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
        sync_detached_terminal_grid(window);
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
            if platform::control_pressed() && w_param as u32 == 0x44 && platform::shift_pressed() {
                // Ctrl+Shift+D opens the problems and debug panel.
                APP.with(|cell| cell.borrow_mut().toggle_problems());
                invalidate(window);
                return 0;
            }
            if platform::control_pressed() && w_param as u32 == 0x41 && platform::shift_pressed() {
                APP.with(|cell| {
                    let mut app = cell.borrow_mut();
                    app.chat_open = !app.chat_open;
                    app.chat_input_focused = app.chat_open;
                    app.status = if app.chat_open {
                        "AI panel: type a prompt and press Enter, Esc cancels a running request"
                            .into()
                    } else {
                        "AI panel closed".into()
                    };
                });
                invalidate(window);
                return 0;
            }
            if platform::control_pressed() {
                let terminal_open = APP.with(|cell| cell.borrow().terminal_open);
                if terminal_open
                    && w_param as u32 == 0x43
                    && platform::shift_pressed()
                    && copy_terminal_selection(window)
                {
                    APP.with(|cell| {
                        cell.borrow_mut().status =
                            "Terminal selection copied; Ctrl+C still interrupts the shell".into()
                    });
                    invalidate(window);
                    return 0;
                }
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
                    0x49 => {
                        APP.with(|cell| {
                            cell.borrow_mut()
                                .request_language_feature(LspRequest::Hover)
                        });
                        invalidate(window);
                        return 0;
                    }
                    0x52 => {
                        // Ctrl+R lists references, then cycles through them.
                        APP.with(|cell| {
                            let mut app = cell.borrow_mut();
                            if app.lsp_locations.is_empty() {
                                app.request_language_feature(LspRequest::References);
                            } else {
                                app.jump_to_next_reference();
                            }
                        });
                        invalidate(window);
                        return 0;
                    }
                    0x57 => {
                        // Ctrl+W closes the active document.
                        APP.with(|cell| {
                            let mut app = cell.borrow_mut();
                            let index = app.session.active();
                            app.close_document(index, window);
                            save_session(&app.session);
                        });
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
            if w_param as u32 == 0x78 {
                APP.with(|cell| cell.borrow_mut().toggle_breakpoint());
                invalidate(window);
                return 0;
            }
            if w_param as u32 == 0x74 {
                APP.with(|cell| {
                    let mut app = cell.borrow_mut();
                    if platform::shift_pressed() {
                        app.stop_debugging();
                    } else {
                        app.start_or_continue_debugging();
                    }
                });
                invalidate(window);
                return 0;
            }
            if w_param as u32 == 0x79 {
                APP.with(|cell| {
                    cell.borrow_mut()
                        .step_debugging(dap_core::PendingRequest::Next)
                });
                invalidate(window);
                return 0;
            }
            if w_param as u32 == 0x7a {
                APP.with(|cell| {
                    let request = if platform::shift_pressed() {
                        dap_core::PendingRequest::StepOut
                    } else {
                        dap_core::PendingRequest::StepIn
                    };
                    cell.borrow_mut().step_debugging(request)
                });
                invalidate(window);
                return 0;
            }
            if w_param as u32 == 0x7b {
                APP.with(|cell| {
                    cell.borrow_mut()
                        .request_language_feature(LspRequest::Definition)
                });
                invalidate(window);
                return 0;
            }
            if w_param as u32 == 0x71 {
                // F2 asks for a new name and then for the rename edits.
                APP.with(|cell| {
                    let mut app = cell.borrow_mut();
                    app.rename_prompt = Some(String::new());
                    app.graph_search_focused = false;
                    app.status = "Rename: type the new name and press Enter".into();
                });
                invalidate(window);
                return 0;
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
                if app.problems_open {
                    match w_param as u32 {
                        0x1b => app.problems_open = false,
                        VK_TAB => {
                            app.problem_filter = app.problem_filter.next();
                            app.problems_selection = 0;
                            let count = app.problem_rows().len();
                            app.status = format!(
                                "Problems filter: {} ({count} row(s)); Tab cycles",
                                app.problem_filter.label()
                            );
                        }
                        VK_UP => app.move_problem_selection(-1),
                        VK_DOWN => app.move_problem_selection(1),
                        0x0d => app.activate_problem_row(),
                        _ => return,
                    }
                    return;
                }
                if app.chat_input_focused {
                    // The AI panel owns the keyboard while it is open.
                    match w_param as u32 {
                        0x1b => {
                            if app.chat_session.is_some() {
                                app.cancel_chat();
                            } else {
                                app.chat_input_focused = false;
                                app.chat_open = false;
                            }
                        }
                        0x0d => {
                            if app.chat_session.is_none() {
                                app.send_chat_prompt();
                            } else {
                                app.status = "An AI request is already running".into();
                            }
                        }
                        VK_BACK => {
                            app.chat_input.pop();
                        }
                        _ => return,
                    }
                    return;
                }
                if app.graph_search_focused {
                    match w_param as u32 {
                        0x1b => {
                            if app.graph_search.is_empty() {
                                app.graph_search_focused = false;
                            } else {
                                app.graph_search.clear();
                                app.graph_scroll = 0;
                            }
                        }
                        0x0d => app.graph_search_focused = false,
                        VK_BACK => {
                            app.graph_search.pop();
                            app.graph_scroll = 0;
                        }
                        VK_LEFT => {
                            app.graph_scope = graph_core::cycle_scope(app.graph_scope, false);
                            app.graph_scroll = 0;
                        }
                        VK_RIGHT | VK_TAB => {
                            app.graph_scope = graph_core::cycle_scope(app.graph_scope, true);
                            app.graph_scroll = 0;
                        }
                        _ => return,
                    }
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
                if app.graph_keyboard_active {
                    let columns = graph_visible_rows(
                        &area_for(window),
                        platform::dpi_for_window(window) as f32 / 96.0,
                        app.terminal_motion,
                    );
                    match w_param as u32 {
                        VK_UP | VK_DOWN | 0x0d | 0x20 | VK_LEFT | VK_RIGHT => {}
                        _ => {
                            app.graph_keyboard_active = false;
                            return;
                        }
                    }
                    if matches!(w_param as u32, VK_LEFT | VK_RIGHT) {
                        app.graph_relation = if w_param as u32 == VK_RIGHT {
                            app.graph_relation.next()
                        } else {
                            app.graph_relation.previous()
                        };
                        app.graph_scroll = 0;
                        app.graph_selected = None;
                        app.status = format!("Graph relation: {}", app.graph_relation.label());
                        return;
                    }
                    let symbols = editor_core::symbol_occurrences(&app.lines, &app.file_name);
                    let graph = build_graph(&app, &symbols);
                    let current = app
                        .graph_selected
                        .unwrap_or(0)
                        .min(graph.len().saturating_sub(1));
                    let index = match w_param as u32 {
                        VK_UP => current.saturating_sub(1),
                        VK_DOWN => (current + 1).min(graph.len().saturating_sub(1)),
                        _ => current,
                    };
                    app.graph_selected = Some(index);
                    let first = graph.clamped_scroll(app.graph_scroll, columns);
                    if index < first {
                        app.graph_scroll += first - index;
                    } else if index >= first + columns {
                        app.graph_scroll = app
                            .graph_scroll
                            .saturating_sub(index - (first + columns - 1));
                    }
                    match w_param as u32 {
                        0x0d => {
                            if let Some(target) =
                                graph.nodes.get(index).and_then(|node| node.target.as_ref())
                            {
                                let path = app.project_root.join(&target.file);
                                let line = target.line;
                                let column = target.column;
                                app.open_file(path);
                                app.caret_line = line.min(app.lines.len().saturating_sub(1));
                                app.caret_column = column;
                                app.clamp_caret();
                                app.status = format!("Jumped to relation at {}", app.file_name);
                            }
                        }
                        0x20 => {
                            if let Some(node) = graph.nodes.get(index) {
                                if node.has_children() {
                                    let key = node.key.clone();
                                    let label = node.label.clone();
                                    if let Some(position) =
                                        app.graph_folded.iter().position(|it| *it == key)
                                    {
                                        app.graph_folded.remove(position);
                                        app.status = format!("Expanded {label}");
                                    } else {
                                        app.graph_folded.push(key);
                                        app.status = format!("Collapsed {label}");
                                    }
                                }
                            }
                        }
                        _ => {
                            if let Some(node) = graph.nodes.get(index) {
                                app.status = format!("Graph selection: {}", node.caption());
                            }
                        }
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
                    app.reveal_caret();
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
                    app.terminal_scroll = 0;
                    app.terminal_selection = None;
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
                && y as f32 <= package_top + 240.0 * scale
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
                    } else if (166.0 * scale..196.0 * scale).contains(&local_y) {
                        if app.signing_certificate.is_some() {
                            app.signing_certificate = None;
                            app.status = "MSIX signing certificate cleared".into();
                        } else if let Some(path) = platform::choose_signing_certificate(window) {
                            app.status = format!("MSIX signing certificate: {}", path.display());
                            app.signing_certificate = Some(path);
                        } else {
                            app.status = "No signing certificate selected".into();
                        }
                    } else if local_y >= 200.0 * scale {
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
                                        let name =
                                            WindowsAppState::EXTENSION_LANGUAGES[language].to_string();
                                        app.language_installed[language] =
                                            !app.language_installed[language];
                                        if app.project_root.join(".tc-ide-catalog.conf").is_file() {
                                            app.install_language_extension(&name);
                                        } else {
                                            app.status = format!(
                                                "{name} marked as wanted; add .tc-ide-catalog.conf to the project to download it for real"
                                            );
                                        }
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
                        } else if matches!(app.extension_category, 2 | 3) && (348.0..392.0)
                            .contains(&((y as f32 - top) / scale))
                        {
                            if app.extension_category == 2 {
                                app.start_debug_adapter();
                            } else {
                                app.format_current_file();
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
                            let prompt_top = top + 500.0 * scale;
                            if x as f32 >= content_left
                                && x as f32 <= content_left + 600.0 * scale
                                && y as f32 >= prompt_top + 26.0 * scale
                                && y as f32 <= prompt_top + 68.0 * scale
                            {
                                app.ai_prompt_focused = true;
                                app.ai_field_focused = None;
                                return;
                            }
                            if y as f32 >= prompt_top + 78.0 * scale
                                && y as f32 <= prompt_top + 120.0 * scale
                            {
                                if x as f32 >= content_left
                                    && x as f32 <= content_left + 210.0 * scale
                                {
                                    let prompt = app.ai_prompt.clone();
                                    app.ai_prompt_focused = false;
                                    app.run_ai_request(&prompt, true);
                                } else if x as f32 >= content_left + 226.0 * scale
                                    && x as f32 <= content_left + 466.0 * scale
                                {
                                    app.ai_prompt_focused = false;
                                    app.run_ai_request("", false);
                                }
                                return;
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
            // Document tabs sit in the editor header.
            let tab_width = 132.0 * scale;
            let tab_top = 48.0 * scale + 4.0 * scale;
            let tab_bottom = 48.0 * scale + 30.0 * scale;
            let tab_left = (area.right as f32 * 0.19).max(220.0 * scale) + 16.0 * scale;
            let graph_panel_left = graph_left(&area, scale);
            if (y as f32) >= tab_top && (y as f32) <= tab_bottom && (x as f32) >= tab_left {
                let slot = (((x as f32) - tab_left) / tab_width).floor();
                if slot >= 0.0 {
                    let index = slot as usize;
                    let within = (x as f32) - tab_left - slot * tab_width;
                    let closable =
                        within >= tab_width - 30.0 * scale && (x as f32) < graph_panel_left;
                    let count = APP.with(|cell| cell.borrow().session.documents().len());
                    if index < count {
                        APP.with(|cell| {
                            let mut app = cell.borrow_mut();
                            if closable {
                                app.close_document(index, window);
                            } else {
                                app.switch_document(index);
                            }
                            save_session(&app.session);
                        });
                        invalidate(window);
                        return 0;
                    }
                }
            }
            let search_rect = graph_search_rect(&area, scale);
            let scope_rect = graph_scope_rect(&area, scale);
            if x as f32 >= graph_left(&area, scale) {
                if inside(&search_rect, x, y) {
                    APP.with(|cell| {
                        let mut app = cell.borrow_mut();
                        app.graph_search_focused = true;
                        app.status = "Graph search: type to filter symbols, esc clears".into();
                    });
                    invalidate(window);
                    return 0;
                }
                if inside(&scope_rect, x, y) {
                    APP.with(|cell| {
                        let mut app = cell.borrow_mut();
                        if platform::shift_pressed() {
                            app.graph_relation = app.graph_relation.next();
                            app.status = format!("Graph relation: {}", app.graph_relation.label());
                        } else {
                            app.graph_scope = graph_core::cycle_scope(app.graph_scope, true);
                            let label = match app.graph_scope {
                                Some(scope) => scope.label().to_string(),
                                None => "auto".to_string(),
                            };
                            app.status = format!("Graph range: {label}");
                        }
                        app.graph_scroll = 0;
                    });
                    invalidate(window);
                    return 0;
                }
                let rows =
                    graph_visible_rows(&area, scale, platform::dpi_for_window(window) as f32);
                let row = ((y as f32 - (48.0 * scale + graph_core::FIRST_ROW_TOP * scale))
                    / (graph_core::ROW_HEIGHT * scale))
                    .floor();
                let (toggled, target) = APP.with(|cell| {
                    let mut app = cell.borrow_mut();
                    if row < 0.0 {
                        return (false, None);
                    }
                    let symbols = editor_core::symbol_occurrences(&app.lines, &app.file_name);
                    let graph = build_graph(&app, &symbols);
                    let index = graph.clamped_scroll(app.graph_scroll, rows) + row as usize;
                    let Some(node) = graph.nodes.get(index) else {
                        return (false, None);
                    };
                    let panel_left = graph_left(&area, scale);
                    let (marker_left, marker_right) =
                        graph_core::marker_span(panel_left, node.depth, scale);
                    if node.has_children()
                        && (x as f32) >= marker_left
                        && (x as f32) <= marker_right
                    {
                        let key = node.key.clone();
                        let folded = node.folded;
                        let label = node.label.clone();
                        if let Some(position) = app.graph_folded.iter().position(|it| *it == key) {
                            app.graph_folded.remove(position);
                        } else {
                            app.graph_folded.push(key);
                        }
                        app.status = if folded {
                            format!("Expanded {label}")
                        } else {
                            format!("Collapsed {label}")
                        };
                        return (true, None);
                    }
                    app.graph_selected = Some(index);
                    app.graph_keyboard_active = true;
                    let Some(target) = node.target.as_ref() else {
                        return (false, None);
                    };
                    (
                        false,
                        Some((
                            app.project_root.join(&target.file),
                            target.line,
                            target.column,
                        )),
                    )
                });
                if toggled {
                    invalidate(window);
                    return 0;
                }
                if let Some((path, line, column)) = target {
                    APP.with(|cell| {
                        let mut app = cell.borrow_mut();
                        app.open_file(path);
                        app.caret_line = line.min(app.lines.len().saturating_sub(1));
                        app.caret_column = column;
                        app.clamp_caret();
                        app.graph_search_focused = false;
                        app.status = format!(
                            "Jumped to relation at {}:{}",
                            app.file_name,
                            app.caret_line + 1
                        );
                    });
                    invalidate(window);
                    return 0;
                }
                APP.with(|cell| cell.borrow_mut().graph_search_focused = false);
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
            } else if y as f32 >= panel.body_top
                && y as f32 <= panel.body_bottom
                && x as f32 >= panel.body_left
                && x as f32 <= panel.body_right
            {
                let started = APP.with(|cell| {
                    let mut app = cell.borrow_mut();
                    app.graph_keyboard_active = false;
                    if !app.terminal_open {
                        return false;
                    }
                    let point = terminal_point_at(&app, &panel, scale, x, y, false);
                    match point {
                        Some(point) => {
                            app.terminal_selection =
                                Some(terminal_core::TerminalSelection::new(point, point));
                            app.terminal_dragging = true;
                            app.status =
                                "Terminal selection: drag to extend, Ctrl+Shift+C copies".into();
                            true
                        }
                        None => {
                            app.terminal_selection = None;
                            false
                        }
                    }
                });
                if started {
                    unsafe { SetCapture(window) };
                }
                invalidate(window);
                return 0;
            } else if x as f32 <= (area.right as f32 * 0.19).max(220.0 * scale) {
                if y as f32 >= 56.0 * scale && y as f32 <= 88.0 * scale {
                    APP.with(|cell| {
                        let mut app = cell.borrow_mut();
                        app.workspace_search_focused = true;
                        app.graph_search_focused = false;
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
                    app.graph_keyboard_active = false;
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
            let graph_left = graph_left(&area, scale);
            APP.with(|cell| {
                let mut app = cell.borrow_mut();
                let panel = terminal_core::panel_metrics(
                    area.right as f32,
                    area.bottom as f32,
                    scale,
                    app.terminal_motion,
                );
                let over_panel = app.terminal_open
                    && app.pointer_x as f32 >= panel.body_left
                    && app.pointer_x as f32 <= panel.body_right
                    && app.pointer_y as f32 >= panel.body_top;
                if over_panel && !app.settings_open && !app.extensions_open {
                    let total = app
                        .terminals
                        .get(app.active_terminal)
                        .and_then(|tab| {
                            tab.session.view.lock().ok().map(|view| {
                                view.lines.len() + usize::from(!view.current.is_empty())
                            })
                        })
                        .unwrap_or(0);
                    let viewport = terminal_core::TerminalViewport {
                        total_lines: total,
                        visible_rows: panel.visible_rows,
                        scroll: app.terminal_scroll,
                    };
                    app.terminal_scroll = if delta > 0 {
                        viewport.scroll_by(WHEEL_SCROLL_LINES)
                    } else if delta < 0 {
                        viewport.scroll_by(-WHEEL_SCROLL_LINES)
                    } else {
                        viewport.scroll
                    };
                } else if app.extensions_open && app.extension_category == 0 {
                    let count = app.filtered_language_indices().len();
                    if delta > 0 {
                        app.extension_scroll = app.extension_scroll.saturating_sub(1);
                    } else if delta < 0 {
                        app.extension_scroll =
                            (app.extension_scroll + 1).min(count.saturating_sub(8));
                    }
                } else if app.pointer_x as f32 >= graph_left {
                    let rows = graph_core::visible_rows(
                        48.0 * scale,
                        area.bottom as f32 - panel.height,
                        scale,
                    );
                    if delta > 0 {
                        app.graph_scroll = app.graph_scroll.saturating_sub(1);
                    } else if delta < 0 {
                        let symbols = editor_core::symbol_occurrences(&app.lines, &app.file_name);
                        let graph = build_graph(&app, &symbols);
                        app.graph_scroll = (app.graph_scroll + 1).min(graph.max_scroll(rows));
                    }
                } else if app.pointer_x as f32 > (area.right as f32 * 0.19).max(220.0 * scale)
                    && (app.pointer_x as f32) < graph_left
                {
                    if platform::shift_pressed() {
                        let columns = if delta > 0 { -4 } else { 4 };
                        app.scroll_editor_columns(columns);
                    } else {
                        let lines = if delta > 0 { -3 } else { 3 };
                        app.scroll_editor(lines);
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
            let terminal_dragging = APP.with(|cell| cell.borrow().terminal_dragging);
            let x = (l_param as u16 as i16) as i32;
            let y = ((l_param >> 16) as u16 as i16) as i32;
            APP.with(|cell| {
                let mut app = cell.borrow_mut();
                app.pointer_x = x;
                app.pointer_y = y;
            });
            if terminal_dragging && w_param & 1 != 0 {
                let mut area = unsafe { std::mem::zeroed::<Rect>() };
                unsafe { GetClientRect(window, &mut area) };
                let scale = platform::dpi_for_window(window) as f32 / 96.0;
                let changed = APP.with(|cell| {
                    let mut app = cell.borrow_mut();
                    let panel = terminal_core::panel_metrics(
                        area.right as f32,
                        area.bottom as f32,
                        scale,
                        app.terminal_motion,
                    );
                    let Some(point) = terminal_point_at(&app, &panel, scale, x, y, true) else {
                        return false;
                    };
                    let Some(selection) = app.terminal_selection else {
                        return false;
                    };
                    if selection.focus == point {
                        return false;
                    }
                    app.terminal_selection = Some(selection.extended_to(point));
                    true
                });
                if changed {
                    invalidate(window);
                }
                return 0;
            }
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
            let terminal_dragging = APP.with(|cell| {
                let mut app = cell.borrow_mut();
                let dragging = app.terminal_dragging;
                app.terminal_dragging = false;
                if app
                    .terminal_selection
                    .map(|selection| selection.is_empty())
                    .unwrap_or(false)
                {
                    app.terminal_selection = None;
                }
                dragging
            });
            if terminal_dragging {
                unsafe { ReleaseCapture() };
                let count = APP.with(|cell| {
                    let app = cell.borrow();
                    app.terminal_selection
                        .and_then(|selection| {
                            terminal_core::selected_text(&terminal_buffer(&app), &selection)
                        })
                        .map(|text| text.chars().count())
                        .unwrap_or(0)
                });
                APP.with(|cell| {
                    cell.borrow_mut().status = if count > 0 {
                        format!("Terminal selection: {count} characters · Ctrl+Shift+C copies")
                    } else {
                        "Terminal selection cleared".into()
                    };
                });
                invalidate(window);
                return 0;
            }
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
                let scale = platform::dpi_for_window(window) as f32 / 96.0;
                cell.borrow_mut()
                    .resize_viewport(width as i32, height as i32, scale);
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
            APP.with(|cell| {
                let mut app = cell.borrow_mut();
                app.poll_ai_terminal();
                app.confirm_pending_extension();
                app.poll_project_index();
                app.poll_language_server();
                app.poll_debug_session();
                app.poll_chat_session();
                // Coalesce edits: tell the server after typing pauses.
                if app.lsp_dirty
                    && app
                        .lsp_last_change
                        .map(|started| started.elapsed().as_millis() > 220)
                        .unwrap_or(true)
                {
                    app.lsp_dirty = false;
                    app.lsp_completions.clear();
                    app.notify_language_server();
                }
                if app.language_server.is_some()
                    && app
                        .lsp_completion_asked
                        .map(|asked| asked.elapsed().as_millis() > 400)
                        .unwrap_or(true)
                {
                    app.lsp_completion_asked = Some(Instant::now());
                    app.request_language_feature(LspRequest::Completion);
                }
            });
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
            // Order matters on shutdown: stop the detached window first so its
            // timer cannot touch the sessions, then release every ConPTY session
            // (each one closes its pseudo console and ends its shell), then drop
            // the Direct2D/DirectWrite state.
            APP.with(|cell| {
                let detached = cell.borrow().detached_terminal;
                if !detached.is_null() {
                    unsafe { DestroyWindow(detached) };
                }
            });
            APP.with(|cell| {
                let mut app = cell.borrow_mut();
                app.terminals.clear();
                save_session(&app.session);
            });
            RENDERER.with(|cell| *cell.borrow_mut() = None);
            unsafe { PostQuitMessage(0) };
            0
        }
        _ => unsafe { DefWindowProcW(window, message, w_param, l_param) },
    }
}

/// Scriptable state layer check: `tc-ide --self-check` exercises the shared
/// cores inside the Windows binary and prints a report, so CI can run it on a
/// Windows runner without a display.
pub fn self_check() -> i32 {
    let mut failures = Vec::new();
    let mut checks = 0usize;
    let mut report = |name: &str, ok: bool, failures: &mut Vec<String>| {
        checks += 1;
        if !ok {
            failures.push(name.to_string());
        }
    };

    // Preferences round trip.
    let preferences = workspace_core::Preferences {
        light_theme: true,
        ui_chinese: true,
        code_zoom: 44,
        ..workspace_core::Preferences::default()
    };
    let text = workspace_core::serialize_preferences(&preferences);
    let parsed = workspace_core::parse_preferences(&text);
    report(
        "preferences serialize and parse",
        parsed.ui_chinese,
        &mut failures,
    );
    report(
        "preferences normalize zoom",
        parsed.code_zoom == 8,
        &mut failures,
    );

    // Project template and task command.
    let template = workspace_core::project_template("Demo App", "Rust", "stable", "None");
    report(
        "rust template writes Cargo.toml and main.rs",
        template.files.iter().any(|file| file.path == "Cargo.toml")
            && template.files.iter().any(|file| file.path == "src/main.rs"),
        &mut failures,
    );
    let facts = workspace_core::ProjectFacts {
        has_cargo: true,
        language: "Rust".into(),
        file_name: "main.rs".into(),
        file_stem: "main".into(),
        windows: true,
        ..workspace_core::ProjectFacts::default()
    };
    report(
        "rust task command",
        workspace_core::task_command(&facts, true) == "cargo build",
        &mut failures,
    );

    // Terminal grid, scrollback and selection.
    let grid = terminal_core::GridSize::from_metrics(800.0, 480.0, 8.0, 16.0);
    report(
        "terminal grid fits the window",
        grid.columns == 100 && grid.rows == 30,
        &mut failures,
    );
    let mut parser = terminal_core::TerminalParser::new();
    parser.feed("hello\r\nworld");
    let view = std::sync::Arc::new(std::sync::Mutex::new(terminal_core::TerminalView::default()));
    parser.publish(&view);
    let published = view
        .lock()
        .map(|view| view.current.clone())
        .unwrap_or_default();
    report(
        "VT parser publishes the live prompt",
        published == "world",
        &mut failures,
    );
    let buffer = vec!["alpha".to_string(), "beta".to_string()];
    let selection = terminal_core::line_selection(0, 5);
    report(
        "terminal selection copies text",
        terminal_core::selected_text(&buffer, &selection).as_deref() == Some("alpha"),
        &mut failures,
    );

    // Relation graph scope, folding and edges.
    let sources = vec![(
        "src/main.rs".to_string(),
        vec![
            "pub fn run() {".to_string(),
            "    helper();".to_string(),
            "}".to_string(),
            "pub fn helper() {}".to_string(),
        ],
    )];
    let project = editor_core::project_symbol_occurrences(&sources);
    let lines = sources[0].1.clone();
    let symbols = editor_core::symbol_occurrences(&lines, "src/main.rs");
    let folded = vec![graph_core::file_key("src/main.rs")];
    let inputs = graph_core::GraphInputs {
        project_name: "demo",
        current_file: "src/main.rs",
        current_folder: "src",
        hovered: None,
        file_lines: &lines,
        file_symbols: &symbols,
        project_symbols: &project,
        folded: &folded,
        relation: graph_core::GraphRelation::Follow,
    };
    let graph = graph_core::build_graph(&inputs, graph_core::GraphScope::File, "");
    report(
        "graph has a project root",
        graph.nodes[0].level == graph_core::GraphLevel::Project,
        &mut failures,
    );
    report("folding hides children", graph.len() == 2, &mut failures);

    // Session parsing and AI request building.
    let session =
        session_core::Session::parse("project=C:\\demo\ndocument=C:\\demo\\a.rs|a.rs|0\n");
    report(
        "session restores documents",
        session.documents().len() == 1 && session.project().is_some(),
        &mut failures,
    );
    let settings = ai_core::AiSettings::default();
    let command = ai_core::chat_command(&settings, "body.json", true, 30, ai_core::Shell::Windows);
    report(
        "AI command keeps the key in the environment",
        command
            .map(|command| command.contains("%TC_IDE_AI_KEY%") && !command.contains("Bearer sk-"))
            .unwrap_or(false),
        &mut failures,
    );

    // Extension catalog and runtime cache.
    let digest = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    let catalog = format!(
        "[extension]\nid=rust-analyzer\nname=Rust Analyzer\nkind=language\nversion=1.0\nurl=https://example.test/ra.zip\nsha256={digest}\n"
    );
    let sources = extension_core::parse_catalog(&catalog).unwrap_or_default();
    report(
        "extension catalog parses",
        sources.len() == 1 && sources[0].is_installable(),
        &mut failures,
    );
    report(
        "checksum verification",
        extension_core::verify_checksum(digest, &digest.to_uppercase()),
        &mut failures,
    );

    // Language tooling.
    report(
        "language server selection",
        lsp_core::server_for_file("main.rs")
            .map(|spec| spec.program == "rust-analyzer")
            .unwrap_or(false),
        &mut failures,
    );
    let (body, rest) = lsp_core::decode_messages(&lsp_core::encode_message("{\"id\":1}"));
    report(
        "LSP framing round trip",
        body == ["{\"id\":1}"] && rest.is_empty(),
        &mut failures,
    );
    report(
        "formatter selection",
        workspace_core::format_command("Rust", "src/main.rs").is_some(),
        &mut failures,
    );
    let mut reader = lsp_core::FrameReader::new();
    let framed = lsp_core::encode_message("{\"id\":7}");
    let bytes = framed.as_bytes();
    let split = bytes.len() - 2;
    let mut frames = reader.push(&bytes[..split]);
    frames.extend(reader.push(&bytes[split..]));
    report("LSP byte framing", frames == ["{\"id\":7}"], &mut failures);
    report(
        "LSP client request answering",
        lsp_core::automatic_reply(
            "{\"id\":2,\"method\":\"workspace/configuration\",\"params\":{\"items\":[{}]}}",
        )
        .map(|reply| reply.contains("[null]"))
        .unwrap_or(false),
        &mut failures,
    );
    report(
        "interpreter probe is behavioural",
        // A missing program must report false rather than a file name match.
        !lsp_core::program_runs("tc-ide-missing-interpreter", &["-c", "print(1)"]),
        &mut failures,
    );
    report(
        "language id mapping",
        lsp_core::language_id("app.py") == "python",
        &mut failures,
    );
    let applied = lsp_core::apply_edits(
        "let value = 1;\n",
        &[lsp_core::RenameEdit {
            path: "demo.rs".into(),
            line: 0,
            column: 4,
            end_line: 0,
            end_column: 9,
            new_text: "answer".into(),
        }],
    );
    report(
        "rename edits apply to a document",
        applied == "let answer = 1;\n",
        &mut failures,
    );
    let mut spring = motion_core::Spring::new(0.0);
    for _ in 0..120 {
        spring.step(1.0, 1.0 / 60.0);
    }
    report(
        "spring settles on its target",
        spring.is_settled(1.0) && spring.value == 1.0,
        &mut failures,
    );
    let tooling = extension_core::language_tools("Rust", |tool| tool == "rustfmt");
    report(
        "tooling state follows the real executables",
        tooling.formatter_installed
            && !tooling.server_installed
            && tooling.missing().contains(&"language server"),
        &mut failures,
    );
    let mut panel_state = WindowsAppState::new();
    // A jump target only exists once the panel knows which file is active.
    panel_state.file_path = Some(PathBuf::from("C:/demo/src/main.rs"));
    panel_state.lsp_diagnostics = vec![lsp_core::Diagnostic {
        path: "src/main.rs".into(),
        line: 3,
        column: 2,
        severity: lsp_core::DiagnosticSeverity::Error,
        message: "cannot find value `x`".into(),
    }];
    let rows = panel_state.problem_rows();
    report(
        "problems panel lists diagnostics with a jump target",
        rows.len() == 1 && rows[0].0.contains("cannot find value") && rows[0].1.is_some(),
        &mut failures,
    );
    let scan = syntax_core::scan(
        &[
            "class Engine {".to_string(),
            "    start() {".to_string(),
            "        helper();".to_string(),
            "    }".to_string(),
            "}".to_string(),
        ],
        "main.ts",
    );
    report(
        "syntax scan finds class members and calls",
        scan.scopes
            .iter()
            .any(|scope| scope.name.as_deref() == Some("Engine"))
            && scan
                .scopes
                .iter()
                .any(|scope| scope.name.as_deref() == Some("start") && scope.end_line == 3)
            && scan.declarations.iter().any(|entry| entry.name == "start"),
        &mut failures,
    );
    let mut index = index_core::IncrementalIndex::new();
    let stats = index.refresh(
        vec![index_core::FileFingerprint::new("src/a.rs", 10, 1)],
        |_| Some(vec!["pub fn run() {}".to_string()]),
    );
    let reused = index.refresh(
        vec![index_core::FileFingerprint::new("src/a.rs", 10, 1)],
        |_| panic!("an unchanged file must not be read again"),
    );
    report(
        "incremental index reuses unchanged files",
        stats.reparsed == 1 && reused.reused == 1 && !index.symbols().is_empty(),
        &mut failures,
    );
    report(
        "chat stream parsing",
        ai_core::parse_stream_line("data: {\"choices\":[{\"delta\":{\"content\":\"hi\"}}]}")
            == ai_core::StreamEvent::Delta("hi".into())
            && ai_core::parse_stream_line("data: [DONE]") == ai_core::StreamEvent::Done,
        &mut failures,
    );
    report(
        "debug adapter selection",
        dap_core::adapter_for("Python")
            .map(|spec| spec.program == "python")
            .unwrap_or(false)
            && dap_core::adapter_for("Markdown").is_none(),
        &mut failures,
    );
    let stopped = dap_core::parse_stopped(
        "{\"type\":\"event\",\"event\":\"stopped\",\"body\":{\"reason\":\"breakpoint\",\"threadId\":2,\"line\":5}}",
    );
    report(
        "debug stop parsing",
        stopped.reason == "breakpoint" && stopped.thread_id == Some(2),
        &mut failures,
    );
    report(
        "DAP message classification",
        dap_core::parse_dap_message("{\"type\":\"event\",\"event\":\"stopped\"}")
            == dap_core::DapMessage::Event {
                name: "stopped".into(),
            },
        &mut failures,
    );
    report(
        "reference locations parse",
        lsp_core::parse_locations(
            "{\"result\":[{\"uri\":\"file:///p/a.rs\",\"range\":{\"start\":{\"line\":2,\"character\":1},\"end\":{}}}]}",
        )
        .len()
            == 1,
        &mut failures,
    );

    println!(
        "TC-IDE self check: {checks} checks, {} failed",
        failures.len()
    );
    for name in &failures {
        println!("  FAILED: {name}");
    }
    if failures.is_empty() {
        println!("state layer OK");
        0
    } else {
        1
    }
}

pub fn run() {
    if std::env::args().any(|argument| argument == "--self-check") {
        std::process::exit(self_check());
    }
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
        APP.with(|cell| cell.borrow_mut().restore_session());
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
    use crate::text_buffer;

    #[test]
    fn the_piece_table_keeps_edits_and_undo_in_step() {
        let mut app = WindowsAppState::new();
        app.lines = vec!["one".into(), "two".into()];
        app.buffer = text_buffer::TextBuffer::from_lines(&app.lines);
        app.caret_line = 0;
        app.caret_column = 3;

        app.insert_text("!");
        assert_eq!(app.lines, ["one!", "two"]);
        app.newline();
        app.insert_text("inserted");
        assert_eq!(app.lines, ["one!", "inserted", "two"]);
        assert_eq!(app.buffer.to_lines(), app.lines);

        app.backspace();
        assert_eq!(app.lines, ["one!", "inserte", "two"]);
        app.undo();
        assert_eq!(app.lines, ["one!", "inserted", "two"]);
        // The typed word, then the line break, then the "!" each undo once.
        app.undo();
        assert_eq!(app.lines, ["one!", "", "two"]);
        app.undo();
        assert_eq!(app.lines, ["one!", "two"]);
        app.undo();
        assert_eq!(app.lines, ["one", "two"]);
        app.redo();
        assert_eq!(app.lines, ["one!", "two"]);

        // A paste is one undo step and keeps unicode intact.
        app.caret_line = 1;
        app.caret_column = 3;
        app.insert_text(" 你好");
        assert_eq!(app.lines, ["one!", "two 你好"]);
        app.undo();
        assert_eq!(app.lines, ["one!", "two"]);
    }

    #[test]
    fn multiline_selection_cut_and_undo_are_lossless() {
        let mut app = WindowsAppState::new();
        app.lines = vec!["alpha".into(), "beta".into()];
        app.buffer = text_buffer::TextBuffer::from_lines(&app.lines);
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
        app.buffer = text_buffer::TextBuffer::from_lines(&app.lines);
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
        app.buffer = text_buffer::TextBuffer::from_lines(&app.lines);
        app.caret_line = 1;
        app.caret_column = 0;
        app.move_left();
        assert_eq!((app.caret_line, app.caret_column), (0, 3));
        app.move_right();
        assert_eq!((app.caret_line, app.caret_column), (1, 0));
    }
}
