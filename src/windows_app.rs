#![allow(clippy::upper_case_acronyms)]

use std::cell::RefCell;
use std::collections::{BTreeMap, HashSet};
use std::ffi::c_void;
use std::fs;
use std::path::PathBuf;
use std::ptr;
use std::time::Instant;

use crate::ai_core;
use crate::background_core;
use crate::dap_core;
use crate::editor_core;
use crate::extension_core;
use crate::graph_core;
use crate::index_core;
use crate::lsp_core;
use crate::motion_core;
use crate::package_motion;
use crate::session_core;
use crate::syntax_core;
use crate::tcide_project;
use crate::terminal_core;
use crate::text_buffer;
use crate::theme_core;
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

mod ai;
mod debug;
mod detached;
mod editor;
mod events;
mod extensions;
mod keys;
mod language;
mod motion;
mod packaging;
mod panel;
mod pointer;
mod scene;
mod selfcheck;
mod workspace;

use self::events::window_proc;
use self::extensions::load_runtime_cache;
use self::workspace::{load_session, save_session};

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
    /// Project-level `.tcIDE` document shared by the editor, graph view and AI.
    tcide_project: tcide_project::ProjectDocument,
    project_entries: Vec<String>,
    project_entry_paths: Vec<PathBuf>,
    project_entry_is_dir: Vec<bool>,
    /// Where each project tree row is drawn while the list is changing.
    tree_row_shift: motion_core::RowShift,
    /// Row positions of the last frame, in the order the rows are listed.
    tree_row_positions: Vec<f32>,
    collapsed_directories: HashSet<PathBuf>,
    project_scroll: usize,
    project_symbols: Vec<editor_core::ProjectSymbolOccurrence>,
    /// Declarations of the indexed project, with their owners, for resolving
    /// callees across files.
    project_declarations: Vec<syntax_core::DeclarationRef>,
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
    /// Row the keyboard is on in the settings panel, when it is on one.
    settings_focus: Option<background_core::SettingsRow>,
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
    /// Breakpoints of the session, per file, with their conditions.
    breakpoints: dap_core::Breakpoints,
    /// Condition being typed for the breakpoint on the caret line.
    breakpoint_prompt: Option<String>,
    /// Formatter run in flight: the file to reload, the marker that says the
    /// tool has finished with it and when the run started.
    format_pending: Option<(PathBuf, String, Instant)>,
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
    /// Whether the left mouse button is down, for the pressed look of a control.
    pointer_pressed: bool,
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
    /// Active drag in the settings background preview.
    background_drag: Option<(background_core::Handle, i32, i32)>,
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
    /// Highlights of the edits of the last fraction of a second.
    edit_pulses: motion_core::EditPulses,
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

    /// Put the keyboard on the settings row a click landed on.
    ///
    /// The rows are drawn as a list of fixed heights, so a click has to land on
    /// the same row the keyboard walks to; both read the shared row list.
    fn focus_settings_row_at(&mut self, local_y: f32) {
        let index = match local_y {
            value if (62.0..116.0).contains(&value) => 1,
            value if (116.0..170.0).contains(&value) => 2,
            value if (170.0..224.0).contains(&value) => 3,
            value if (224.0..380.0).contains(&value) => 4,
            value if (380.0..434.0).contains(&value) => 5,
            value if (434.0..680.0).contains(&value) => 6,
            value if value >= 680.0 => 8,
            _ => return,
        };
        let row = background_core::SettingsRow::all()[index];
        let controls = background_core::background_controls(
            self.background_path.is_some(),
            self.background_opacity,
            self.background_zoom as f64,
        );
        if row.is_enabled(controls) {
            self.settings_focus = Some(row);
        }
    }

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
        let project_declarations = project_index.declarations();
        let package_targets = Self::load_package_targets(&project_root);
        let create_location = project_root.clone();
        Self {
            file_name: "main.rs".into(),
            file_path: None,
            project_root,
            tcide_project: tcide_project::ProjectDocument::for_root(&create_location),
            project_entries,
            project_entry_paths,
            project_entry_is_dir,
            tree_row_shift: motion_core::RowShift::new(),
            tree_row_positions: Vec::new(),
            collapsed_directories,
            project_scroll: 0,
            project_symbols,
            project_declarations,
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
            settings_focus: None,
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
            breakpoints: dap_core::Breakpoints::new(),
            breakpoint_prompt: None,
            format_pending: None,
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
            pointer_pressed: false,
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
            background_drag: None,
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
            edit_pulses: motion_core::EditPulses::new(),
            create_motion: 0.0,
            create_velocity: 0.0,
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

/// Client area of a window.
fn area_for(window: Hwnd) -> Rect {
    let mut area: Rect = unsafe { std::mem::zeroed() };
    unsafe { GetClientRect(window, &mut area) };
    area
}

fn inside(rect: &Rect, x: i32, y: i32) -> bool {
    x >= rect.left && x <= rect.right && y >= rect.top && y <= rect.bottom
}

pub fn run() {
    if std::env::args().any(|argument| argument == "--self-check") {
        std::process::exit(selfcheck::self_check());
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
    use std::path::PathBuf;

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

    #[test]
    fn folding_a_folder_slides_the_rows_below_it() {
        let mut app = WindowsAppState::new();
        app.project_entries = ["src", "src/a.rs", "src/b.rs", "README.md"]
            .iter()
            .map(|value| (*value).to_string())
            .collect();
        app.project_entry_paths = app.project_entries.iter().map(PathBuf::from).collect();
        app.update_tree_rows(1.0 / 60.0);
        assert_eq!(app.tree_row_positions, [0.0, 1.0, 2.0, 3.0]);

        // `src` folds: its children leave the list and the last row slides up.
        app.project_entries = ["src", "README.md"]
            .iter()
            .map(|value| (*value).to_string())
            .collect();
        app.project_entry_paths = app.project_entries.iter().map(PathBuf::from).collect();
        app.update_tree_rows(1.0 / 60.0);
        let sliding = app.tree_row_position(1);
        assert!(
            sliding > 1.0 && sliding < 3.0,
            "the row below the fold should slide, not jump: {sliding}"
        );
        for _ in 0..240 {
            app.update_tree_rows(1.0 / 60.0);
        }
        assert_eq!(app.tree_row_positions, [0.0, 1.0]);
        assert!(app.tree_row_shift.settled());
    }
}
