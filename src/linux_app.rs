#![allow(clippy::too_many_arguments)]

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::ffi::{c_void, CString};
use std::fs;
use std::io::Write;
use std::os::raw::{c_char, c_int, c_long, c_uint, c_ulong};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::ptr;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

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
use crate::terminal_core;
use crate::text_buffer;
use crate::theme_core;
use crate::workspace_core;

type Window = c_ulong;
type Drawable = c_ulong;
type Colormap = c_ulong;
type Atom = c_ulong;
type Gc = *mut c_void;
type Pixmap = c_ulong;
type XFontSet = *mut c_void;

#[repr(C)]
struct Display {
    _private: [u8; 0],
}

#[repr(C)]
struct Visual {
    _private: [u8; 0],
}

#[repr(C)]
struct CairoSurface {
    _private: [u8; 0],
}

#[repr(C)]
struct CairoContext {
    _private: [u8; 0],
}

#[repr(C)]
struct PangoLayout {
    _private: [u8; 0],
}

#[repr(C)]
struct PangoFontDescription {
    _private: [u8; 0],
}

#[repr(C)]
struct FcConfig {
    _private: [u8; 0],
}

#[repr(C)]
struct XFontStruct {
    ext_data: *mut c_void,
    fid: c_ulong,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct XColor {
    pixel: c_ulong,
    red: u16,
    green: u16,
    blue: u16,
    flags: c_char,
    pad: c_char,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct XPoint {
    x: i16,
    y: i16,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct XKeyEvent {
    type_: c_int,
    serial: c_ulong,
    send_event: c_int,
    display: *mut Display,
    window: Window,
    root: Window,
    subwindow: Window,
    time: c_ulong,
    x: c_int,
    y: c_int,
    x_root: c_int,
    y_root: c_int,
    state: c_uint,
    keycode: c_uint,
    same_screen: c_int,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct XMotionEvent {
    type_: c_int,
    serial: c_ulong,
    send_event: c_int,
    display: *mut Display,
    window: Window,
    root: Window,
    subwindow: Window,
    time: c_ulong,
    x: c_int,
    y: c_int,
    x_root: c_int,
    y_root: c_int,
    state: c_uint,
    is_hint: c_char,
    same_screen: c_int,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct XButtonEvent {
    type_: c_int,
    serial: c_ulong,
    send_event: c_int,
    display: *mut Display,
    window: Window,
    root: Window,
    subwindow: Window,
    time: c_ulong,
    x: c_int,
    y: c_int,
    x_root: c_int,
    y_root: c_int,
    state: c_uint,
    button: c_uint,
    same_screen: c_int,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct XConfigureEvent {
    type_: c_int,
    serial: c_ulong,
    send_event: c_int,
    display: *mut Display,
    event: Window,
    window: Window,
    x: c_int,
    y: c_int,
    width: c_int,
    height: c_int,
    border_width: c_int,
    above: Window,
    override_redirect: c_int,
}

#[repr(C)]
union XEvent {
    type_: c_int,
    key: XKeyEvent,
    motion: XMotionEvent,
    button: XButtonEvent,
    configure: XConfigureEvent,
    pad: [c_long; 24],
}

#[link(name = "X11")]
extern "C" {
    fn XOpenDisplay(name: *const c_char) -> *mut Display;
    fn XDefaultScreen(display: *mut Display) -> c_int;
    fn XDisplayWidth(display: *mut Display, screen: c_int) -> c_int;
    fn XDisplayHeight(display: *mut Display, screen: c_int) -> c_int;
    fn XDefaultDepth(display: *mut Display, screen: c_int) -> c_int;
    fn XDefaultVisual(display: *mut Display, screen: c_int) -> *mut Visual;
    fn XRootWindow(display: *mut Display, screen: c_int) -> Window;
    fn XDefaultColormap(display: *mut Display, screen: c_int) -> Colormap;
    fn XCreateSimpleWindow(
        display: *mut Display,
        parent: Window,
        x: c_int,
        y: c_int,
        width: c_uint,
        height: c_uint,
        border_width: c_uint,
        border: c_ulong,
        background: c_ulong,
    ) -> Window;
    fn XStoreName(display: *mut Display, window: Window, name: *const c_char) -> c_int;
    fn XSelectInput(display: *mut Display, window: Window, mask: c_long) -> c_int;
    fn XMapWindow(display: *mut Display, window: Window) -> c_int;
    fn XCreateGC(
        display: *mut Display,
        drawable: Drawable,
        mask: c_ulong,
        values: *mut c_void,
    ) -> Gc;
    fn XCreatePixmap(
        display: *mut Display,
        drawable: Drawable,
        width: c_uint,
        height: c_uint,
        depth: c_uint,
    ) -> Pixmap;
    fn XFreePixmap(display: *mut Display, pixmap: Pixmap) -> c_int;
    fn XCopyArea(
        display: *mut Display,
        source: Drawable,
        destination: Drawable,
        gc: Gc,
        source_x: c_int,
        source_y: c_int,
        width: c_uint,
        height: c_uint,
        destination_x: c_int,
        destination_y: c_int,
    ) -> c_int;
    fn XFreeGC(display: *mut Display, gc: Gc) -> c_int;
    fn XSetForeground(display: *mut Display, gc: Gc, foreground: c_ulong) -> c_int;
    fn XSetLineAttributes(
        display: *mut Display,
        gc: Gc,
        width: c_uint,
        style: c_int,
        cap: c_int,
        join: c_int,
    ) -> c_int;
    fn XFillRectangle(
        display: *mut Display,
        drawable: Drawable,
        gc: Gc,
        x: c_int,
        y: c_int,
        width: c_uint,
        height: c_uint,
    ) -> c_int;
    fn XDrawRectangle(
        display: *mut Display,
        drawable: Drawable,
        gc: Gc,
        x: c_int,
        y: c_int,
        width: c_uint,
        height: c_uint,
    ) -> c_int;
    fn XDrawLine(
        display: *mut Display,
        drawable: Drawable,
        gc: Gc,
        x1: c_int,
        y1: c_int,
        x2: c_int,
        y2: c_int,
    ) -> c_int;
    fn XFillArc(
        display: *mut Display,
        drawable: Drawable,
        gc: Gc,
        x: c_int,
        y: c_int,
        width: c_uint,
        height: c_uint,
        angle1: c_int,
        angle2: c_int,
    ) -> c_int;
    fn XFillPolygon(
        display: *mut Display,
        drawable: Drawable,
        gc: Gc,
        points: *mut XPoint,
        count: c_int,
        shape: c_int,
        mode: c_int,
    ) -> c_int;
    fn XDrawArc(
        display: *mut Display,
        drawable: Drawable,
        gc: Gc,
        x: c_int,
        y: c_int,
        width: c_uint,
        height: c_uint,
        angle1: c_int,
        angle2: c_int,
    ) -> c_int;
    fn XCreateFontSet(
        display: *mut Display,
        base_name: *const c_char,
        missing: *mut *mut *mut c_char,
        missing_count: *mut c_int,
        default_string: *mut *mut c_char,
    ) -> XFontSet;
    fn XFreeFontSet(display: *mut Display, font_set: XFontSet);
    fn XLoadQueryFont(display: *mut Display, name: *const c_char) -> *mut XFontStruct;
    fn XSetFont(display: *mut Display, gc: Gc, font: c_ulong) -> c_int;
    fn XFreeFont(display: *mut Display, font: *mut XFontStruct) -> c_int;
    fn XAllocNamedColor(
        display: *mut Display,
        map: Colormap,
        name: *const c_char,
        screen: *mut XColor,
        exact: *mut XColor,
    ) -> c_int;
    fn XPending(display: *mut Display) -> c_int;
    fn XNextEvent(display: *mut Display, event: *mut XEvent) -> c_int;
    fn XLookupString(
        event: *mut XKeyEvent,
        buffer: *mut c_char,
        length: c_int,
        keysym: *mut c_ulong,
        compose: *mut c_void,
    ) -> c_int;
    fn XInternAtom(display: *mut Display, name: *const c_char, only_if_exists: c_int) -> Atom;
    fn XSetWMProtocols(
        display: *mut Display,
        window: Window,
        protocols: *mut Atom,
        count: c_int,
    ) -> c_int;
    fn XFlush(display: *mut Display) -> c_int;
    fn XDestroyWindow(display: *mut Display, window: Window) -> c_int;
    fn XCloseDisplay(display: *mut Display) -> c_int;
}

extern "C" {
    fn cairo_xlib_surface_create(
        display: *mut Display,
        drawable: Drawable,
        visual: *mut Visual,
        width: c_int,
        height: c_int,
    ) -> *mut CairoSurface;
    fn cairo_xlib_surface_set_drawable(
        surface: *mut CairoSurface,
        drawable: Drawable,
        width: c_int,
        height: c_int,
    );
    fn cairo_surface_destroy(surface: *mut CairoSurface);
    fn cairo_image_surface_create_from_png(filename: *const c_char) -> *mut CairoSurface;
    fn cairo_image_surface_create(format: c_int, width: c_int, height: c_int) -> *mut CairoSurface;
    fn cairo_surface_write_to_png(surface: *mut CairoSurface, filename: *const c_char) -> c_int;
    fn cairo_paint(context: *mut CairoContext);
    fn cairo_image_surface_get_width(surface: *mut CairoSurface) -> c_int;
    fn cairo_image_surface_get_height(surface: *mut CairoSurface) -> c_int;
    fn cairo_surface_flush(surface: *mut CairoSurface);
    fn cairo_create(surface: *mut CairoSurface) -> *mut CairoContext;
    fn cairo_destroy(context: *mut CairoContext);
    fn cairo_set_source_rgb(context: *mut CairoContext, red: f64, green: f64, blue: f64);
    fn cairo_move_to(context: *mut CairoContext, x: f64, y: f64);
    fn cairo_save(context: *mut CairoContext);
    fn cairo_restore(context: *mut CairoContext);
    fn cairo_rectangle(context: *mut CairoContext, x: f64, y: f64, width: f64, height: f64);
    fn cairo_clip(context: *mut CairoContext);
    fn cairo_translate(context: *mut CairoContext, tx: f64, ty: f64);
    fn cairo_scale(context: *mut CairoContext, sx: f64, sy: f64);
    fn cairo_set_source_surface(
        context: *mut CairoContext,
        surface: *mut CairoSurface,
        x: f64,
        y: f64,
    );
    fn cairo_paint_with_alpha(context: *mut CairoContext, alpha: f64);
    fn pango_cairo_create_layout(context: *mut CairoContext) -> *mut PangoLayout;
    fn pango_cairo_show_layout(context: *mut CairoContext, layout: *mut PangoLayout);
    fn pango_layout_set_text(layout: *mut PangoLayout, text: *const c_char, length: c_int);
    fn pango_layout_set_font_description(
        layout: *mut PangoLayout,
        description: *const PangoFontDescription,
    );
    fn pango_layout_get_pixel_size(layout: *mut PangoLayout, width: *mut c_int, height: *mut c_int);
    fn pango_font_description_from_string(description: *const c_char) -> *mut PangoFontDescription;
    fn pango_font_description_free(description: *mut PangoFontDescription);
    fn g_object_unref(object: *mut c_void);
    fn FcConfigGetCurrent() -> *mut FcConfig;
    fn FcConfigAppFontAddFile(config: *mut FcConfig, file: *const u8) -> c_int;
    fn FcConfigBuildFonts(config: *mut FcConfig) -> c_int;
}

#[link(name = "c")]
extern "C" {
    fn setlocale(category: c_int, locale: *const c_char) -> *mut c_char;
}

const KEY_PRESS: c_int = 2;
const BUTTON_PRESS: c_int = 4;
const MOTION_NOTIFY: c_int = 6;
const EXPOSE: c_int = 12;
const CONFIGURE_NOTIFY: c_int = 22;
const CLIENT_MESSAGE: c_int = 33;
const KEY_PRESS_MASK: c_long = 1;
const BUTTON_PRESS_MASK: c_long = 1 << 2;
const POINTER_MOTION_MASK: c_long = 1 << 6;
const EXPOSURE_MASK: c_long = 1 << 15;
const STRUCTURE_NOTIFY_MASK: c_long = 1 << 17;
const CONTROL_MASK: c_uint = 1 << 2;
/// Shift is held down, in the state mask of a key event.
const SHIFT_MASK: c_uint = 1 << 0;
const BUTTON1_MASK: c_uint = 1 << 8;
const ESCAPE_KEYSYM: c_ulong = 0xff1b;
const BACKSPACE_KEYSYM: c_ulong = 0xff08;
const DELETE_KEYSYM: c_ulong = 0xffff;
const RETURN_KEYSYM: c_ulong = 0xff0d;
const TAB_KEYSYM: c_ulong = 0xff09;
/// Space, which activates the focused control like Enter does.
const SPACE_KEYSYM: c_ulong = 0x0020;
const LEFT_KEYSYM: c_ulong = 0xff51;
const UP_KEYSYM: c_ulong = 0xff52;
const RIGHT_KEYSYM: c_ulong = 0xff53;
const DOWN_KEYSYM: c_ulong = 0xff54;
const F5_KEYSYM: c_ulong = 0xffc2;
/// F2, which renames the symbol at the caret.
const F2_KEYSYM: c_ulong = 0xffbf;
/// F12, which asks the language server for the definition.
const F12_KEYSYM: c_ulong = 0xffc9;
const F9_KEYSYM: c_ulong = 0xffc6;
const F10_KEYSYM: c_ulong = 0xffc7;
const LC_ALL: c_int = 6;

fn shortcut_from_keysym(keysym: c_ulong) -> Option<char> {
    char::from_u32(keysym as u32).map(|value| value.to_ascii_lowercase())
}

const CODE_FONT_FAMILIES: &[&str] = &[
    "JetBrains Mono",
    "Fira Code",
    "Cascadia Code",
    "Source Code Pro",
    "Hack",
    "Ubuntu Mono",
    "DejaVu Sans Mono",
    "Liberation Mono",
    "Noto Sans Mono CJK SC",
];

struct FileBuffer {
    name: String,
    path: PathBuf,
    lines: Vec<String>,
    /// Piece table behind the lines, holding the edit history.
    buffer: text_buffer::TextBuffer,
}
impl FileBuffer {
    /// A buffer with its own edit history, so undo works per document.
    fn new(name: String, path: PathBuf, lines: Vec<String>) -> Self {
        Self {
            buffer: text_buffer::TextBuffer::from_lines(&lines),
            name,
            path,
            lines,
        }
    }

    /// Rebuild the rendered lines from the piece table.
    ///
    /// Edits go through the buffer and the lines are read back out of it, so
    /// the two cannot disagree and the history stays correct.
    fn sync(&mut self) {
        self.lines = self.buffer.to_lines();
    }
}

struct TerminalSession {
    name: String,
    input: String,
    output: Arc<Mutex<Vec<String>>>,
}

struct ParsedComment<'a> {
    code: &'a str,
    text: &'a str,
    content_column: usize,
    content_end: usize,
}

#[derive(Clone, Copy)]
struct UiMotion {
    value: f32,
    velocity: f32,
}

impl UiMotion {
    const fn new(value: f32) -> Self {
        Self {
            value,
            velocity: 0.0,
        }
    }
}

struct BubbleLayout {
    left: i32,
    top: i32,
    width: i32,
    height: i32,
    lines: Vec<String>,
    chars_per_line: usize,
}

struct LanguagePack {
    name: &'static str,
    extensions: &'static [&'static str],
    versions: &'static [&'static str],
    selected_version: usize,
    installed: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CreateKind {
    Project,
    Folder,
    File,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PanelDrag {
    CallHierarchy,
    Terminal,
}

/// One row of the File menu.
#[derive(Clone, PartialEq, Eq)]
enum FileMenuRow {
    /// A `New …` action.
    Create(CreateKind),
    /// Pick a file from the desktop's file chooser.
    OpenFile,
    /// Pick a folder and open it as the project.
    OpenFolder,
    /// A heading, drawn but not clickable.
    Heading(&'static str, &'static str),
    /// Reopen a project from the recent list.
    Recent(PathBuf),
}

impl FileMenuRow {
    /// Whether the row can be clicked.
    fn is_action(&self) -> bool {
        !matches!(self, FileMenuRow::Heading(..))
    }
}

/// The rows of the File menu.
///
/// Drawing, hit testing and the keyboard all read this one list, so a row can
/// never be drawn in one place and clicked in another.
fn file_menu_rows(recent: &[PathBuf]) -> Vec<FileMenuRow> {
    let mut rows = vec![
        FileMenuRow::Heading("NEW", "新建"),
        FileMenuRow::Create(CreateKind::Project),
        FileMenuRow::Create(CreateKind::File),
        FileMenuRow::Create(CreateKind::Folder),
        FileMenuRow::Heading("OPEN", "打开"),
        FileMenuRow::OpenFile,
        FileMenuRow::OpenFolder,
    ];
    if !recent.is_empty() {
        rows.push(FileMenuRow::Heading("RECENT", "最近"));
        for path in recent.iter().take(4) {
            rows.push(FileMenuRow::Recent(path.clone()));
        }
    }
    rows
}

/// Height of one File menu row.
const FILE_MENU_ROW: i32 = 30;

#[derive(Clone, Copy)]
struct Palette {
    bg: c_ulong,
    panel: c_ulong,
    panel_2: c_ulong,
    line: c_ulong,
    muted: c_ulong,
    text: c_ulong,
    bright: c_ulong,
    accent: c_ulong,
    keyword: c_ulong,
    syntax_type: c_ulong,
    function: c_ulong,
    constant: c_ulong,
    variable: c_ulong,
    parameter: c_ulong,
    property: c_ulong,
    macro_name: c_ulong,
    module: c_ulong,
    preprocessor: c_ulong,
    literal: c_ulong,
    comment: c_ulong,
    project: c_ulong,
    shadow: c_ulong,
}

/// A question the editor asks the language server.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LspRequest {
    /// What could be written at the caret.
    Completion,
    /// What the symbol under the caret is.
    Hover,
    /// Where the symbol under the caret is defined.
    Definition,
    /// Where the symbol under the caret is used.
    References,
    /// What the symbol under the caret should be called instead.
    Rename,
}

/// Buttons of the settings panel that answer to the pointer and the keyboard.
///
/// The rectangle of a control is computed in one place, so drawing, hit testing
/// and keyboard focus can never drift apart.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SettingsControl {
    LanguageEnglish,
    LanguageChinese,
    ThemeDark,
    ThemeLight,
    BackgroundChoose,
    BackgroundClear,
    OpacityDown,
    OpacityUp,
    CropLeft,
    CropRight,
    CropUp,
    CropDown,
    ZoomDown,
    ZoomUp,
}

impl SettingsControl {
    /// Every control, in the order `Tab` walks them.
    const ALL: [SettingsControl; 14] = [
        SettingsControl::LanguageEnglish,
        SettingsControl::LanguageChinese,
        SettingsControl::ThemeDark,
        SettingsControl::ThemeLight,
        SettingsControl::BackgroundChoose,
        SettingsControl::BackgroundClear,
        SettingsControl::OpacityDown,
        SettingsControl::OpacityUp,
        SettingsControl::CropLeft,
        SettingsControl::CropRight,
        SettingsControl::CropUp,
        SettingsControl::CropDown,
        SettingsControl::ZoomDown,
        SettingsControl::ZoomUp,
    ];
}

struct App {
    display: *mut Display,
    window: Window,
    gc: Gc,
    canvas: Pixmap,
    cairo_surface: *mut CairoSurface,
    cairo: *mut CairoContext,
    depth: c_int,
    font: *mut XFontStruct,
    font_set: XFontSet,
    code_font: *mut XFontStruct,
    code_font_base: i32,
    code_font_family: usize,
    background_surface: *mut CairoSurface,
    background_path: Option<PathBuf>,
    background_opacity: f64,
    background_offset_x: f64,
    background_offset_y: f64,
    background_zoom: f64,
    logo_surface: *mut CairoSurface,
    colors: Palette,
    dark_colors: Palette,
    light_colors: Palette,
    light_theme: bool,
    width: i32,
    height: i32,
    mouse_x: i32,
    mouse_y: i32,
    hovered: Option<usize>,
    /// Control the mouse button went down on, for the pressed look.
    settings_pressed: Option<SettingsControl>,
    /// Control the keyboard is on in the settings panel.
    settings_focus: Option<SettingsControl>,
    hovered_from_code: bool,
    hovered_code_span: Option<(usize, usize, usize)>,
    selected_file: usize,
    files: Vec<FileBuffer>,
    folders: Vec<String>,
    project_name: String,
    project_path: Option<PathBuf>,
    project_location: String,
    project_location_focused: bool,
    create_caret: usize,
    editor_focused: bool,
    caret_line: usize,
    caret_column: usize,
    selection_anchor: Option<(usize, usize)>,
    selection_head: Option<(usize, usize)>,
    clipboard: String,
    code_zoom: i32,
    font_search: String,
    font_search_focused: bool,
    edit_menu_open: bool,
    view_menu_open: bool,
    workspace_search: String,
    workspace_search_focused: bool,
    project_tree_expanded: bool,
    project_tree_amount: f32,
    project_tree_velocity: f32,
    folder_tree_motion: BTreeMap<String, (f32, f32)>,
    tree_motion_updated: Instant,
    new_menu_motion: UiMotion,
    edit_menu_motion: UiMotion,
    view_menu_motion: UiMotion,
    settings_motion: UiMotion,
    package_motion: UiMotion,
    extensions_motion: UiMotion,
    create_motion: UiMotion,
    terminal_motion: UiMotion,
    graph_motion: UiMotion,
    collapsed_folders: BTreeSet<String>,
    terminal_open: bool,
    terminal_height: i32,
    terminal_focused: bool,
    terminal_active: usize,
    terminals: Vec<TerminalSession>,
    terminal_menu_open: bool,
    graph_current: bool,
    graph_visible: bool,
    graph_fraction: f32,
    panel_drag: Option<PanelDrag>,
    breakpoints: dap_core::Breakpoints,
    /// Incremental index of the open files, so the graph is not rescanned
    /// on every frame.
    project_index: index_core::IncrementalIndex,
    /// Terminal cells the pointer has selected, when there are any.
    terminal_selection: Option<terminal_core::TerminalSelection>,
    /// Whether the pointer is dragging a terminal selection.
    terminal_selecting: bool,
    /// Text of the condition prompt, when it is open.
    breakpoint_prompt: Option<String>,
    /// Caret position inside the condition prompt.
    condition_caret: usize,
    /// Whether the condition prompt has the keyboard.
    condition_focused: bool,
    debug_session: Option<dap_core::DebugSession>,
    debug_state: dap_core::DebugState,
    /// Whether the call stack and scope panel is shown.
    debug_panel_open: bool,
    /// Stack frame the panel highlights.
    debug_frame: usize,
    debug_stop_seen: String,
    graph_zoom: f32,
    session: session_core::Session,
    runtime_cache: extension_core::RuntimeCache,
    /// Cached extensions the project catalog has a newer version of.
    extension_upgrades: usize,
    /// Crop box drag in the background preview: the handle grabbed and where the
    /// pointer was when it was grabbed.
    background_drag: Option<(background_core::Handle, i32, i32)>,
    graph_relation: graph_core::GraphRelation,
    /// Fold points that are closed, by the line each region starts on.
    folded: Vec<usize>,
    /// Line the user asked to jump to, and whether the prompt has the keyboard.
    goto_prompt: Option<String>,
    goto_focused: bool,
    /// The brackets the caret is between, refreshed once a frame.
    matched_brackets: Option<editor_core::BracketPair>,
    /// Language server for the active file, when one is installed.
    language_server: Option<lsp_core::LanguageServer>,
    /// Diagnostics the server last published, and the file they describe.
    lsp_diagnostics: Vec<lsp_core::Diagnostic>,
    lsp_diagnostic_file: String,
    /// Severity the user asked to see.
    lsp_severity: lsp_core::SeverityFilter,
    /// When the document last changed, so the server is not told every key.
    lsp_last_change: Option<Instant>,
    /// Questions asked and not yet answered, with what they were for.
    lsp_pending: Vec<(u64, LspRequest)>,
    /// What the server said about the symbol under the caret, when shown.
    lsp_hover: Option<String>,
    /// References the server listed, and which one we are on.
    lsp_locations: Vec<(String, u32, u32)>,
    lsp_reference_index: usize,
    /// Completions the server offered, and which one is selected.
    lsp_completions: Vec<String>,
    lsp_completion_index: usize,
    /// New name being typed for a rename, and whether it has the keyboard.
    rename_prompt: Option<String>,
    rename_focused: bool,
    /// Whether the AI panel is open, and whether it has the keyboard.
    ai_open: bool,
    ai_focused: bool,
    /// Prompt being typed.
    ai_prompt: String,
    /// Conversation with the project, loaded when it opens.
    ai_messages: ai_core::Conversation,
    /// Request in flight, when there is one.
    ai_session: Option<ai_core::ChatSession>,
    ai_error: Option<String>,
    /// Range the graph shows, or `None` to follow the caret. This is the
    /// shared model rather than an index into a hand written list, so Linux
    /// offers the same four ranges and the same automatic state as the others.
    graph_scope: Option<graph_core::GraphScope>,
    graph_scroll: usize,
    graph_search: String,
    graph_search_focused: bool,
    graph_folded: BTreeSet<String>,
    project_symbols: Vec<editor_core::ProjectSymbolOccurrence>,
    hovered_symbol: Option<editor_core::SymbolOccurrence>,
    package_open: bool,
    package_targets: [bool; 4],
    build_started: Option<Instant>,
    new_menu_open: bool,
    workspace_context_menu: Option<(i32, i32)>,
    extensions_open: bool,
    settings_open: bool,
    ui_chinese: bool,
    languages: Vec<LanguagePack>,
    downloading: Option<(usize, Instant)>,
    extension_scroll: usize,
    extension_category: usize,
    extension_search: String,
    extension_search_focused: bool,
    selected_language: Option<usize>,
    version_menu_open: bool,
    ai_endpoint: String,
    ai_model: String,
    ai_key: String,
    ai_field_focused: Option<usize>,
    create_kind: Option<CreateKind>,
    new_file_name: String,
    project_language: usize,
    project_environment: usize,
    project_language_menu: bool,
    project_environment_menu: bool,
    expanded_comments: BTreeSet<(usize, usize)>,
    /// Highlights of the edits of the last fraction of a second.
    edit_pulses: motion_core::EditPulses,
    status_message: String,
    started: Instant,
}

impl App {
    fn scale(&self) -> f32 {
        (self.width as f32 / 1440.0)
            .min(self.height as f32 / 900.0)
            .clamp(0.85, 2.0)
    }

    fn s(&self, value: i32) -> i32 {
        (value as f32 * self.scale()).round() as i32
    }

    fn tr<'a>(&self, english: &'a str, chinese: &'a str) -> &'a str {
        if self.ui_chinese {
            chinese
        } else {
            english
        }
    }

    fn code_line_step(&self) -> i32 {
        (self.s(28) + self.code_zoom * self.s(2)).max(self.s(20))
    }

    fn graph_left(&self) -> i32 {
        if self.graph_motion.value <= 0.001 {
            return self.width;
        }
        let workspace = (self.width as f32 * 0.17) as i32;
        let base = (self.width as f32 * self.graph_fraction).round().clamp(
            self.s(260) as f32,
            (self.width - workspace - self.s(360)) as f32,
        );
        self.width - (base * self.graph_motion.value.clamp(0.0, 1.0)).round() as i32
    }

    fn terminal_top(&self) -> i32 {
        if self.terminal_open {
            self.height
                - self
                    .terminal_height
                    .clamp(self.s(150), self.height - self.s(150))
        } else {
            self.height - self.s(28)
        }
    }

    fn active_breakpoint_path(&self) -> String {
        self.files[self.selected_file]
            .path
            .to_string_lossy()
            .into_owned()
    }

    /// Open the prompt that edits the condition of the caret line's breakpoint.
    ///
    /// A conditional breakpoint stops only while an expression holds, which is
    /// the difference between stepping through a loop a hundred times and
    /// stopping on the one pass that matters. Windows had the prompt already;
    /// this is the same flow on Linux.
    fn begin_breakpoint_condition(&mut self) {
        let path = self.active_breakpoint_path();
        let line = self.caret_line;
        let Some(spec) = self
            .breakpoints
            .specs(&path)
            .iter()
            .find(|spec| spec.line == line)
            .cloned()
        else {
            self.status_message = format!(
                "{} {}",
                self.tr(
                    "No breakpoint on line to condition",
                    "此行没有可设置条件的断点"
                ),
                line + 1
            );
            return;
        };
        let existing = spec.condition.clone().unwrap_or_default();
        self.condition_caret = existing.chars().count();
        self.breakpoint_prompt = Some(existing);
        self.condition_focused = true;
        self.status_message = format!(
            "{} {}",
            self.tr(
                "Condition for line: type an expression, Enter applies, Esc clears",
                "该行条件：输入表达式，回车应用，Esc 清除"
            ),
            line + 1
        );
    }

    /// Apply the typed condition to the caret line's breakpoint.
    fn apply_breakpoint_condition(&mut self) {
        let Some(prompt) = self.breakpoint_prompt.take() else {
            return;
        };
        self.condition_focused = false;
        let path = self.active_breakpoint_path();
        let line = self.caret_line;
        let condition = prompt.trim().to_string();
        let text = if condition.is_empty() {
            None
        } else {
            Some(condition.as_str())
        };
        if !self.breakpoints.condition(&path, line, text) {
            self.status_message = format!(
                "{} {}",
                self.tr("No breakpoint on line any more", "此行已没有断点"),
                line + 1
            );
            return;
        }
        self.status_message = if condition.is_empty() {
            format!(
                "{} {}",
                self.tr(
                    "Breakpoint now stops every time on line",
                    "断点现在每次都会停在第"
                ),
                line + 1
            )
        } else {
            format!(
                "{} {}: {condition}",
                self.tr("Breakpoint stops when line", "断点将在该行满足条件时停下"),
                line + 1
            )
        };
        if let Some(session) = self.debug_session.as_mut() {
            let arguments =
                dap_core::set_breakpoints_arguments(&path, self.breakpoints.specs(&path));
            let _ = session.request(dap_core::PendingRequest::SetBreakpoints, &arguments);
        }
    }

    /// Whether a line holds a breakpoint, and whether it has a condition.
    fn breakpoint_mark(&self, line: usize) -> Option<bool> {
        let path = self.active_breakpoint_path();
        self.breakpoints
            .specs(&path)
            .iter()
            .find(|spec| spec.line == line)
            .map(|spec| spec.is_conditional())
    }

    fn open_breakpoint_condition(&mut self) {
        self.begin_breakpoint_condition();
    }

    fn toggle_breakpoint(&mut self) {
        let path = self.active_breakpoint_path();
        let enabled = self.breakpoints.toggle(&path, self.caret_line);
        self.status_message = format!(
            "{} {}",
            self.tr(
                if enabled {
                    "Breakpoint set on line"
                } else {
                    "Breakpoint removed from line"
                },
                if enabled {
                    "已在此行设置断点"
                } else {
                    "已移除此行断点"
                },
            ),
            self.caret_line + 1
        );
        if let Some(session) = self.debug_session.as_mut() {
            let arguments =
                dap_core::set_breakpoints_arguments(&path, self.breakpoints.specs(&path));
            let _ = session.request(dap_core::PendingRequest::SetBreakpoints, &arguments);
        }
    }

    fn debug_program(&mut self, language: &str) -> Result<PathBuf, String> {
        let source = self.files[self.selected_file].path.clone();
        let root = self.active_root();
        match language {
            "C" | "C++" => {
                let output = root.join(".tc-ide-debug");
                let compiler = if language == "C++" { "g++" } else { "gcc" };
                let status = Command::new(compiler)
                    .args(["-g", "-O0"])
                    .arg(&source)
                    .arg("-o")
                    .arg(&output)
                    .current_dir(&root)
                    .status()
                    .map_err(|error| format!("{compiler} could not start: {error}"))?;
                if !status.success() {
                    return Err(self
                        .tr(
                            "Debug build failed; see compiler output",
                            "调试构建失败，请检查编译输出",
                        )
                        .into());
                }
                Ok(output)
            }
            "Python" => Ok(source),
            _ => Ok(source),
        }
    }

    fn start_or_continue_debugging(&mut self) {
        if let Some(session) = self.debug_session.as_mut() {
            let action = if self.debug_state.stopped.is_some() {
                dap_core::PendingRequest::Continue
            } else {
                dap_core::PendingRequest::Pause
            };
            let _ = session.control(action);
            return;
        }
        self.save_current_file();
        let language = self.detected_language_for_file(&self.files[self.selected_file].name);
        let Some(mut spec) = dap_core::adapter_for(language) else {
            self.status_message = format!("No debug adapter is configured for {language}");
            return;
        };
        if !lsp_core::executable_available(&spec.program)
            && matches!(language, "C" | "C++" | "Rust")
            && lsp_core::executable_available("gdb")
        {
            spec.program = "gdb".into();
            spec.arguments = vec!["-q".into(), "-i".into(), "dap".into()];
        }
        if !lsp_core::executable_available(&spec.program) {
            self.status_message = format!("{} is not installed", spec.program);
            return;
        }
        let program = match self.debug_program(language) {
            Ok(program) => program,
            Err(error) => {
                self.status_message = error;
                return;
            }
        };
        let root = self.active_root();
        match dap_core::DebugSession::start(&spec, &root) {
            Ok(mut session) => {
                let program = program.to_string_lossy();
                let cwd = root.to_string_lossy();
                if let Err(error) =
                    session.initialize_and_launch(&program, &cwd, &self.breakpoints.files())
                {
                    self.status_message = error;
                    return;
                }
                self.debug_session = Some(session);
                self.debug_state = dap_core::DebugState::default();
                self.debug_stop_seen.clear();
                self.terminal_open = true;
                self.status_message = format!("Debugging with {}", spec.program);
            }
            Err(error) => self.status_message = error,
        }
    }

    fn step_debugging(&mut self) {
        if let Some(session) = self.debug_session.as_mut() {
            let _ = session.control(dap_core::PendingRequest::Next);
        } else {
            self.status_message = self.tr("Start debugging first", "请先启动调试").into();
        }
    }

    fn stop_debugging(&mut self) {
        if let Some(session) = self.debug_session.as_mut() {
            let _ = session.control(dap_core::PendingRequest::Disconnect);
        }
        self.debug_session = None;
        self.debug_state = dap_core::DebugState::default();
        self.debug_stop_seen.clear();
        self.status_message = self.tr("Debugging stopped", "调试已停止").into();
    }

    fn poll_debug_session(&mut self) {
        let Some(mut session) = self.debug_session.take() else {
            return;
        };
        if !session.is_running() {
            self.debug_state = dap_core::DebugState::default();
            self.status_message = format!("{} stopped", session.spec().program);
            return;
        }
        while session.poll().is_some() {}
        self.debug_state = session.state();
        if self.debug_state.stopped.is_some() {
            if self.debug_state.frames.is_empty()
                && !session.has_pending(dap_core::PendingRequest::StackTrace)
            {
                let _ = session.refresh_state();
            } else if self.debug_state.scopes.is_empty()
                && !session.has_pending(dap_core::PendingRequest::Scopes)
            {
                let _ = session.request_scope_variables();
            } else if self.debug_state.variables.is_empty()
                && !session.has_pending(dap_core::PendingRequest::Variables)
            {
                let _ = session.request_variables_of_first_scope();
            }
            if let Some(frame) = self.debug_state.frames.first() {
                self.caret_line = frame
                    .line
                    .min(self.files[self.selected_file].lines.len().saturating_sub(1));
                self.caret_column = frame
                    .column
                    .min(self.files[self.selected_file].lines[self.caret_line].len());
                let key = format!("{}:{}", frame.name, frame.line);
                if key != self.debug_stop_seen {
                    self.debug_stop_seen = key;
                    self.status_message = self
                        .debug_state
                        .stopped
                        .as_ref()
                        .map(dap_core::StopEvent::describe)
                        .unwrap_or_else(|| self.tr("Paused", "已暂停").into());
                }
            }
        }
        self.debug_session = Some(session);
    }

    /// The theme colour behind an X11 pixel, for the code that blends colours.
    ///
    /// The palette is shared with the Windows renderer, so this reads it back
    /// instead of repeating the numbers.
    fn color_rgb(&self, color: c_ulong) -> (f64, f64, f64) {
        let palette = theme_core::Palette::for_scheme(self.light_theme);
        let hex = palette_colour_of(&self.colors, &palette, color);
        (
            ((hex >> 16) & 255) as f64 / 255.0,
            ((hex >> 8) & 255) as f64 / 255.0,
            (hex & 255) as f64 / 255.0,
        )
    }

    fn font_description(&self, code: bool) -> String {
        if code {
            let family =
                CODE_FONT_FAMILIES[self.code_font_family.min(CODE_FONT_FAMILIES.len() - 1)];
            let size = (self.code_font_base + self.code_zoom * 2).clamp(10, 36);
            format!("{family} {size}")
        } else {
            let size = if self.width > 2200 { 16 } else { 12 };
            format!("Noto Sans CJK SC {size}")
        }
    }

    fn pango_text_size(&self, value: &str, code: bool) -> (i32, i32) {
        let clean = value.replace('\0', "");
        let Ok(text) = CString::new(clean) else {
            return (0, 0);
        };
        let Ok(font) = CString::new(self.font_description(code)) else {
            return (0, 0);
        };
        unsafe {
            let layout = pango_cairo_create_layout(self.cairo);
            if layout.is_null() {
                return (0, 0);
            }
            let description = pango_font_description_from_string(font.as_ptr());
            pango_layout_set_text(layout, text.as_ptr(), text.as_bytes().len() as i32);
            pango_layout_set_font_description(layout, description);
            let mut width = 0;
            let mut height = 0;
            pango_layout_get_pixel_size(layout, &mut width, &mut height);
            pango_font_description_free(description);
            g_object_unref(layout.cast());
            (width, height)
        }
    }

    fn code_text_width(&self, value: &str) -> i32 {
        self.pango_text_size(value, true).0
    }

    fn byte_index_at_code_x(&self, value: &str, pixels: i32) -> usize {
        let mut best = 0;
        for (index, _) in value
            .char_indices()
            .chain(std::iter::once((value.len(), '\0')))
        {
            if self.code_text_width(&value[..index]) <= pixels {
                best = index;
            } else {
                break;
            }
        }
        best
    }

    fn byte_index_at_ui_x(&self, value: &str, pixels: i32) -> usize {
        let mut best = 0;
        for (index, _) in value
            .char_indices()
            .chain(std::iter::once((value.len(), '\0')))
        {
            if self.pango_text_size(&value[..index], false).0 <= pixels {
                best = index;
            } else {
                break;
            }
        }
        best
    }

    fn reload_code_font(&mut self) {
        self.code_zoom = self.code_zoom.clamp(-2, 8);
        save_ui_preferences(
            self.ui_chinese,
            self.code_font_family,
            self.code_zoom,
            self.light_theme,
        );
    }

    fn set_light_theme(&mut self, light: bool) {
        self.light_theme = light;
        self.colors = if light {
            self.light_colors
        } else {
            self.dark_colors
        };
        save_ui_preferences(
            self.ui_chinese,
            self.code_font_family,
            self.code_zoom,
            self.light_theme,
        );
        self.status_message = self
            .tr(
                if light {
                    "Light theme enabled"
                } else {
                    "Dark theme enabled"
                },
                if light {
                    "已启用浅色主题"
                } else {
                    "已启用深色主题"
                },
            )
            .into();
    }

    fn filtered_font_indices(&self) -> Vec<usize> {
        let needle = self.font_search.trim().to_ascii_lowercase();
        CODE_FONT_FAMILIES
            .iter()
            .enumerate()
            .filter(|(_, family)| {
                needle.is_empty() || family.to_ascii_lowercase().contains(&needle)
            })
            .map(|(index, _)| index)
            .collect()
    }

    unsafe fn replace_background(&mut self, path: Option<PathBuf>) {
        if !self.background_surface.is_null() {
            cairo_surface_destroy(self.background_surface);
            self.background_surface = ptr::null_mut();
        }
        self.background_path = path.clone();
        if let Some(path) = path {
            if let Ok(value) = CString::new(path.to_string_lossy().as_bytes()) {
                let surface = cairo_image_surface_create_from_png(value.as_ptr());
                if !surface.is_null()
                    && cairo_image_surface_get_width(surface) > 0
                    && cairo_image_surface_get_height(surface) > 0
                {
                    self.background_surface = surface;
                } else if !surface.is_null() {
                    cairo_surface_destroy(surface);
                }
            }
        }
        save_background_preferences(
            self.background_path.as_deref(),
            self.background_opacity,
            self.background_offset_x,
            self.background_offset_y,
            self.background_zoom,
        );
    }

    unsafe fn choose_background(&mut self) {
        let output = Command::new("zenity")
            .args([
                "--file-selection",
                "--title=TC-IDE Background",
                "--file-filter=Images | *.png *.jpg *.jpeg *.webp",
            ])
            .output();
        match output {
            Ok(output) if output.status.success() => {
                let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !path.is_empty() {
                    let source = PathBuf::from(path);
                    let selected = if source
                        .extension()
                        .and_then(|value| value.to_str())
                        .map(|value| value.eq_ignore_ascii_case("png"))
                        .unwrap_or(false)
                    {
                        source
                    } else {
                        let cache = runtime_cache_root()
                            .parent()
                            .unwrap_or_else(|| std::path::Path::new("."))
                            .join("background.png");
                        if let Some(parent) = cache.parent() {
                            let _ = fs::create_dir_all(parent);
                        }
                        let converted = Command::new("ffmpeg")
                            .args(["-y", "-loglevel", "error", "-i"])
                            .arg(&source)
                            .arg(&cache)
                            .status()
                            .map(|status| status.success())
                            .unwrap_or(false);
                        if !converted {
                            self.status_message = self
                                .tr(
                                    "Could not convert this background image",
                                    "无法转换该背景图片",
                                )
                                .into();
                            return;
                        }
                        cache
                    };
                    self.replace_background(Some(selected));
                    self.status_message = self
                        .tr("Editor background updated", "编辑器背景已更新")
                        .into();
                }
            }
            _ => {
                self.status_message = self
                    .tr(
                        "Install zenity to choose a background image",
                        "需要安装 zenity 才能选择背景图片",
                    )
                    .into();
            }
        }
    }

    fn choose_project_location(&mut self) {
        let output = Command::new("zenity")
            .args([
                "--file-selection",
                "--directory",
                "--title=TC-IDE Project Location",
            ])
            .output();
        match output {
            Ok(output) if output.status.success() => {
                let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !path.is_empty() {
                    self.project_location = path;
                    self.project_location_focused = false;
                    self.status_message = self
                        .tr("Project location selected", "已选择项目位置")
                        .into();
                }
            }
            Ok(_) => {}
            Err(error) => {
                self.status_message = format!("Could not open folder picker: {error}");
            }
        }
    }

    /// Rectangle of a settings control, in screen coordinates.
    fn settings_control_rect(&self, control: SettingsControl) -> (i32, i32, i32, i32) {
        let s = |v| self.s(v);
        let (x, y) = self.settings_origin();
        use SettingsControl::*;
        match control {
            LanguageEnglish => (x + s(16), y + s(69), s(164), s(31)),
            LanguageChinese => (x + s(194), y + s(69), s(164), s(31)),
            ThemeDark => (x + s(110), y + s(106), s(112), s(30)),
            ThemeLight => (x + s(234), y + s(106), s(112), s(30)),
            BackgroundChoose => (x + s(166), y + s(468), s(112), s(27)),
            BackgroundClear => (x + s(286), y + s(468), s(82), s(27)),
            OpacityDown => (x + s(166), y + s(508), s(34), s(27)),
            OpacityUp => (x + s(300), y + s(508), s(34), s(27)),
            CropLeft => (x + s(166), y + s(548), s(34), s(27)),
            CropRight => (x + s(214), y + s(548), s(34), s(27)),
            CropUp => (x + s(262), y + s(548), s(34), s(27)),
            CropDown => (x + s(310), y + s(548), s(34), s(27)),
            ZoomDown => (x + s(166), y + s(588), s(34), s(27)),
            ZoomUp => (x + s(300), y + s(588), s(34), s(27)),
        }
    }

    /// Whether a control can be used right now.
    fn settings_control_enabled(&self, control: SettingsControl) -> bool {
        let has_image = !self.background_surface.is_null();
        use SettingsControl::*;
        match control {
            BackgroundClear => self.background_path.is_some(),
            OpacityDown => has_image && self.background_opacity > 0.05,
            OpacityUp => has_image && self.background_opacity < 1.0,
            CropLeft => has_image && self.background_offset_x > -1.0,
            CropRight => has_image && self.background_offset_x < 1.0,
            CropUp => has_image && self.background_offset_y > -1.0,
            CropDown => has_image && self.background_offset_y < 1.0,
            ZoomDown => has_image && self.background_zoom > 1.0,
            ZoomUp => has_image && self.background_zoom < 3.0,
            _ => true,
        }
    }

    /// Which control sits under the pointer.
    fn settings_control_at(&self, x: i32, y: i32) -> Option<SettingsControl> {
        SettingsControl::ALL.into_iter().find(|control| {
            let (left, top, width, height) = self.settings_control_rect(*control);
            x >= left && x <= left + width && y >= top && y <= top + height
        })
    }

    /// State a control is drawn in.
    fn settings_control_state(&self, control: SettingsControl) -> theme_core::ControlState {
        if !self.settings_control_enabled(control) {
            return theme_core::ControlState::Disabled;
        }
        if self.settings_control_at(self.mouse_x, self.mouse_y) == Some(control) {
            if self.settings_pressed == Some(control) {
                return theme_core::ControlState::Pressed;
            }
            return theme_core::ControlState::Hovered;
        }
        theme_core::ControlState::Normal
    }

    /// Colours a control draws with, using the palette the window already uses.
    fn settings_control_colors(
        &self,
        control: SettingsControl,
        emphasised: bool,
    ) -> (c_ulong, c_ulong, c_ulong) {
        use theme_core::ControlState::*;
        match self.settings_control_state(control) {
            Normal => (
                if emphasised {
                    self.colors.accent
                } else {
                    self.colors.line
                },
                self.colors.panel_2,
                if emphasised {
                    self.colors.accent
                } else {
                    self.colors.text
                },
            ),
            Hovered => (self.colors.accent, self.colors.line, self.colors.bright),
            Pressed => (self.colors.accent, self.colors.bg, self.colors.bright),
            Disabled => (self.colors.line, self.colors.panel_2, self.colors.muted),
        }
    }

    /// Draw a settings control, with its fill, border, label and focus ring.
    unsafe fn draw_settings_control(
        &self,
        control: SettingsControl,
        label: &str,
        emphasised: bool,
    ) {
        let s = |v| self.s(v);
        let (x, y, width, height) = self.settings_control_rect(control);
        let (border, fill, text) = self.settings_control_colors(control, emphasised);
        let (x, y) = if self.settings_control_state(control) == theme_core::ControlState::Pressed {
            (x, y + s(1))
        } else {
            (x, y)
        };
        self.rect(fill, x, y, width, height);
        self.outline(border, x, y, width, height);
        if self.settings_focus == Some(control) {
            self.outline(
                self.colors.accent,
                x - s(2),
                y - s(2),
                width + s(4),
                height + s(4),
            );
        }
        let text_x = x + s(9);
        self.text(text, text_x, y + s(20), label);
    }

    /// Activate a settings control, from the pointer or the keyboard.
    fn activate_settings_control(&mut self, control: SettingsControl) {
        if !self.settings_control_enabled(control) {
            self.status_message = self
                .tr("That control is not available", "该控件当前不可用")
                .into();
            return;
        }
        use SettingsControl::*;
        match control {
            LanguageEnglish | LanguageChinese => {
                self.ui_chinese = control == LanguageChinese;
                save_ui_preferences(
                    self.ui_chinese,
                    self.code_font_family,
                    self.code_zoom,
                    self.light_theme,
                );
                self.status_message = if self.ui_chinese {
                    "界面语言已切换为简体中文".into()
                } else {
                    "Interface language changed to English".into()
                };
            }
            ThemeDark => self.set_light_theme(false),
            ThemeLight => self.set_light_theme(true),
            BackgroundChoose => unsafe { self.choose_background() },
            BackgroundClear => {
                unsafe { self.replace_background(None) };
                save_background_preferences(
                    None,
                    self.background_opacity,
                    self.background_offset_x,
                    self.background_offset_y,
                    self.background_zoom,
                );
                self.status_message = self
                    .tr("Editor background cleared", "编辑器背景已清除")
                    .into();
            }
            OpacityDown => self.nudge_background(0.0, 0.0, -0.05),
            OpacityUp => self.nudge_background(0.0, 0.0, 0.05),
            CropLeft => self.nudge_background(-0.1, 0.0, 0.0),
            CropRight => self.nudge_background(0.1, 0.0, 0.0),
            CropUp => self.nudge_background(0.0, -0.1, 0.0),
            CropDown => self.nudge_background(0.0, 0.1, 0.0),
            ZoomDown => {
                self.set_background_zoom(-0.1);
            }
            ZoomUp => {
                self.set_background_zoom(0.1);
            }
        }
    }

    /// Move the crop, change the opacity, or both, and persist the result.
    fn nudge_background(&mut self, dx: f64, dy: f64, opacity: f64) {
        self.background_offset_x = (self.background_offset_x + dx).clamp(-1.0, 1.0);
        self.background_offset_y = (self.background_offset_y + dy).clamp(-1.0, 1.0);
        self.background_opacity = (self.background_opacity + opacity).clamp(0.0, 1.0);
        save_background_preferences(
            self.background_path.as_deref(),
            self.background_opacity,
            self.background_offset_x,
            self.background_offset_y,
            self.background_zoom,
        );
    }

    /// Change the image zoom and persist it.
    fn set_background_zoom(&mut self, delta: f64) {
        self.background_zoom = (self.background_zoom + delta).clamp(1.0, 3.0);
        save_background_preferences(
            self.background_path.as_deref(),
            self.background_opacity,
            self.background_offset_x,
            self.background_offset_y,
            self.background_zoom,
        );
    }

    /// Origin of the settings overlay.
    fn settings_origin(&self) -> (i32, i32) {
        (
            self.width - self.s(410),
            self.s(43) + self.overlay_offset(self.settings_motion),
        )
    }

    /// Rectangle of the background preview inside the settings panel.
    fn background_preview_rect(&self) -> (i32, i32, i32, i32) {
        let (x, y) = self.settings_origin();
        (x + self.s(16), y + self.s(664), self.s(356), self.s(150))
    }

    /// Size of the region the background is drawn into behind the editor.
    fn background_view_size(&self) -> (f64, f64) {
        let left = (self.width as f32 * 0.17) as i32;
        let right = self.graph_left();
        (
            (right - left - 1).max(1) as f64,
            (self.height - self.s(154)).max(1) as f64,
        )
    }

    /// Size of the loaded background image.
    fn background_image_size(&self) -> Option<(f64, f64)> {
        if self.background_surface.is_null() {
            return None;
        }
        let width = unsafe { cairo_image_surface_get_width(self.background_surface) }.max(1);
        let height = unsafe { cairo_image_surface_get_height(self.background_surface) }.max(1);
        Some((width as f64, height as f64))
    }

    /// The visible part of the image, and the placement that shows the whole
    /// picture inside the preview.
    fn background_crop_box(
        &self,
        image: (f64, f64),
    ) -> Option<(background_core::Rect, background_core::Placement)> {
        self.background_image_size()?;
        let (_, _, width, height) = self.background_preview_rect();
        let preview = background_core::preview_placement(image, (width as f64, height as f64));
        let view = self.background_view_size();
        let placed = background_core::placement(
            image,
            view,
            self.background_zoom,
            (self.background_offset_x, self.background_offset_y),
        );
        let crop = placed.crop(view, image);
        let _ = image;
        Some((crop, preview))
    }

    /// The crop box as it appears in the preview, in screen coordinates.
    fn background_crop_on_screen(&self, image: (f64, f64)) -> Option<background_core::Rect> {
        let (crop, preview) = self.background_crop_box(image)?;
        let (px, py, _, _) = self.background_preview_rect();
        let drawn = preview.project(&crop);
        Some(background_core::Rect::new(
            px as f64 + drawn.x,
            py as f64 + drawn.y,
            drawn.width,
            drawn.height,
        ))
    }

    /// Which part of the crop box a pointer grabbed.
    fn background_handle_at(
        &self,
        x: i32,
        y: i32,
        image: (f64, f64),
    ) -> Option<background_core::Handle> {
        let box_on_screen = self.background_crop_on_screen(image)?;
        background_core::handle_at(&box_on_screen, (x as f64, y as f64), self.s(12) as f64)
    }

    /// Move or resize the crop box while it is being dragged.
    fn update_background_drag(&mut self, x: i32, y: i32, state: c_uint) {
        let Some((handle, last_x, last_y)) = self.background_drag else {
            return;
        };
        if state & BUTTON1_MASK == 0 {
            self.background_drag = None;
            return;
        }
        let Some(image) = self.background_image_size() else {
            self.background_drag = None;
            return;
        };
        let (_, _, width, height) = self.background_preview_rect();
        let preview = background_core::preview_placement(image, (width as f64, height as f64));
        let Some((crop, _)) = self.background_crop_box(image) else {
            return;
        };
        // The pointer moves in preview pixels, the crop lives in image pixels.
        let delta = (
            (x - last_x) as f64 / preview.scale,
            (y - last_y) as f64 / preview.scale,
        );
        let minimum = (image.0.min(image.1) * 0.05).max(8.0);
        let edit = background_core::drag_crop(
            &crop,
            handle,
            delta,
            image,
            self.background_view_size(),
            minimum,
        );
        self.background_zoom = edit.zoom;
        self.background_offset_x = edit.offset.0;
        self.background_offset_y = edit.offset.1;
        self.background_drag = Some((handle, x, y));
        self.status_message = format!(
            "Crop {:.0}x{:.0} at {:.0},{:.0} · zoom {:.1}x",
            edit.crop.width, edit.crop.height, edit.crop.x, edit.crop.y, edit.zoom
        );
    }

    /// Paint the background preview with the crop box and its handles.
    unsafe fn draw_background_preview(&self, image: (f64, f64)) {
        let s = |v| self.s(v);
        let (px, py, width, height) = self.background_preview_rect();
        self.rect(self.colors.bg, px, py, width, height);
        let preview = background_core::preview_placement(image, (width as f64, height as f64));
        unsafe {
            cairo_save(self.cairo);
            cairo_rectangle(
                self.cairo,
                px as f64,
                py as f64,
                width as f64,
                height as f64,
            );
            cairo_clip(self.cairo);
            cairo_translate(
                self.cairo,
                px as f64 + preview.origin.0,
                py as f64 + preview.origin.1,
            );
            cairo_scale(self.cairo, preview.scale, preview.scale);
            cairo_set_source_surface(self.cairo, self.background_surface, 0.0, 0.0);
            cairo_paint_with_alpha(self.cairo, 0.85);
            cairo_restore(self.cairo);
        }
        if let Some(crop) = self.background_crop_on_screen(image) {
            let (left, top) = (crop.x.round() as i32, crop.y.round() as i32);
            let (right, bottom) = (crop.right().round() as i32, crop.bottom().round() as i32);
            for (from, to) in [
                ((left, top), (right, top)),
                ((right, top), (right, bottom)),
                ((right, bottom), (left, bottom)),
                ((left, bottom), (left, top)),
            ] {
                self.line(self.colors.accent, 2, from, to);
            }
            // Four handles, so a corner reads as something to grab.
            for (hx, hy) in [(left, top), (right, top), (right, bottom), (left, bottom)] {
                self.rect(self.colors.accent, hx - s(3), hy - s(3), s(7), s(7));
            }
            self.text(
                self.colors.muted,
                px,
                py - s(10),
                self.tr(
                    "Drag the frame to crop, corners to zoom",
                    "拖动取景框裁剪，拖角缩放",
                ),
            );
        }
        self.outline(self.colors.line, px, py, width, height);
    }

    unsafe fn draw_background_region(&self, x: i32, y: i32, width: i32, height: i32) {
        if self.background_surface.is_null() || self.background_opacity <= 0.0 {
            return;
        }
        let image_w = cairo_image_surface_get_width(self.background_surface).max(1) as f64;
        let image_h = cairo_image_surface_get_height(self.background_surface).max(1) as f64;
        // Cover, zoom and offset are the shared background model, which the crop
        // box uses as well.
        let placed = background_core::placement(
            (image_w, image_h),
            (width as f64, height as f64),
            self.background_zoom,
            (self.background_offset_x, self.background_offset_y),
        );
        let scale = placed.scale;
        let (offset_x, offset_y) = placed.origin;
        cairo_save(self.cairo);
        cairo_rectangle(self.cairo, x as f64, y as f64, width as f64, height as f64);
        cairo_clip(self.cairo);
        cairo_translate(self.cairo, x as f64 + offset_x, y as f64 + offset_y);
        cairo_scale(self.cairo, scale, scale);
        cairo_set_source_surface(self.cairo, self.background_surface, 0.0, 0.0);
        cairo_paint_with_alpha(self.cairo, self.background_opacity.clamp(0.0, 1.0));
        cairo_restore(self.cairo);
        cairo_surface_flush(self.cairo_surface);
    }

    fn detected_language_for_file(&self, name: &str) -> &'static str {
        self.language_pack_index_for_file(name)
            .map(|index| self.languages[index].name)
            .unwrap_or("Plain Text")
    }

    fn language_for_file(&self, name: &str) -> String {
        if let Some(index) = self.language_pack_index_for_file(name) {
            let pack = &self.languages[index];
            if pack.installed {
                format!("{} {}", pack.name, pack.versions[pack.selected_version])
            } else {
                format!("{} - extension available", pack.name)
            }
        } else {
            "Plain Text".into()
        }
    }

    fn language_pack_index_for_file(&self, name: &str) -> Option<usize> {
        let extension = name.rsplit('.').next()?.to_ascii_lowercase();
        self.languages
            .iter()
            .position(|pack| pack.extensions.iter().any(|item| *item == extension))
    }

    fn completion_words(&self) -> &'static [&'static str] {
        completion_words_for_language(
            self.detected_language_for_file(&self.files[self.selected_file].name),
        )
    }

    fn current_prefix(&self) -> String {
        let line = &self.files[self.selected_file].lines[self.caret_line];
        let upto = &line[..self.caret_column.min(line.len())];
        upto.chars()
            .rev()
            .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
            .collect::<String>()
            .chars()
            .rev()
            .collect()
    }

    fn completions(&self) -> Vec<String> {
        let prefix = self.current_prefix();
        if prefix.is_empty() {
            return Vec::new();
        }
        let language = self.detected_language_for_file(&self.files[self.selected_file].name);
        let mut candidates = Vec::new();
        for word in self.completion_words() {
            if word.starts_with(&prefix) && *word != prefix {
                candidates.push((*word).to_string());
            }
        }
        for file in &self.files {
            if self.detected_language_for_file(&file.name) != language {
                continue;
            }
            for line in &file.lines {
                for symbol in identifiers_in(line) {
                    if symbol.starts_with(&prefix)
                        && symbol != prefix
                        && !candidates.contains(&symbol)
                    {
                        candidates.push(symbol);
                    }
                }
            }
        }
        candidates.truncate(6);
        candidates
    }

    fn accept_completion(&mut self) {
        if let Some(candidate) = self.completions().first().cloned() {
            let prefix = self.current_prefix();
            let suffix = &candidate[prefix.len()..];
            let start = self.caret_column;
            self.files[self.selected_file].lines[self.caret_line]
                .insert_str(self.caret_column, suffix);
            self.caret_column += suffix.len();
            self.note_edit(
                motion_core::EditKind::Insert,
                self.caret_line,
                start,
                self.caret_column,
            );
            self.status_message = format!("Completed {candidate}");
        }
    }

    fn comment_parts<'a>(&self, line: &'a str) -> Option<ParsedComment<'a>> {
        let file = &self.files[self.selected_file].name;
        let (marker_start, content_column, content_end) = comment_content_bounds(line, file)?;
        let text = &line[content_column..content_end];
        Some(ParsedComment {
            code: &line[..marker_start],
            text,
            content_column,
            content_end,
        })
    }

    fn active_comment_edit_bounds(&self) -> Option<(usize, usize)> {
        if !self
            .expanded_comments
            .contains(&(self.selected_file, self.caret_line))
        {
            return None;
        }
        let parsed = self.comment_parts(&self.files[self.selected_file].lines[self.caret_line])?;
        (self.caret_column >= parsed.content_column && self.caret_column <= parsed.content_end)
            .then_some((parsed.content_column, parsed.content_end))
    }

    fn filtered_language_indices(&self) -> Vec<usize> {
        let needle = self.extension_search.to_ascii_lowercase();
        self.languages
            .iter()
            .enumerate()
            .filter(|(_, pack)| {
                needle.is_empty()
                    || pack.name.to_ascii_lowercase().contains(&needle)
                    || pack.extensions.iter().any(|ext| ext.contains(&needle))
            })
            .map(|(index, _)| index)
            .collect()
    }

    unsafe fn set_color(&self, color: c_ulong) {
        XSetForeground(self.display, self.gc, color);
    }

    unsafe fn rect(&self, color: c_ulong, x: i32, y: i32, width: i32, height: i32) {
        if width <= 0 || height <= 0 {
            return;
        }
        self.set_color(color);
        XFillRectangle(
            self.display,
            self.canvas,
            self.gc,
            x,
            y,
            width as u32,
            height as u32,
        );
    }

    unsafe fn rounded_rect(
        &self,
        color: c_ulong,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        radius: i32,
    ) {
        if width <= 0 || height <= 0 {
            return;
        }
        let radius = radius.min(width / 2).min(height / 2).max(1);
        self.rect(color, x + radius, y, width - radius * 2, height);
        self.rect(color, x, y + radius, width, height - radius * 2);
        self.set_color(color);
        for (cx, cy) in [
            (x + radius, y + radius),
            (x + width - radius, y + radius),
            (x + radius, y + height - radius),
            (x + width - radius, y + height - radius),
        ] {
            XFillArc(
                self.display,
                self.canvas,
                self.gc,
                cx - radius,
                cy - radius,
                (radius * 2) as u32,
                (radius * 2) as u32,
                0,
                360 * 64,
            );
        }
    }

    unsafe fn floating_panel(&self, x: i32, y: i32, width: i32, height: i32) {
        let radius = self.s(11);
        self.rounded_rect(
            self.colors.shadow,
            x + self.s(2),
            y + self.s(5),
            width,
            height,
            radius,
        );
        self.rounded_rect(self.colors.panel_2, x, y, width, height, radius);
        self.line(
            self.colors.line,
            1,
            (x + radius, y),
            (x + width - radius, y),
        );
    }

    unsafe fn outline(&self, color: c_ulong, x: i32, y: i32, width: i32, height: i32) {
        self.set_color(color);
        XDrawRectangle(
            self.display,
            self.canvas,
            self.gc,
            x,
            y,
            width.max(1) as u32,
            height.max(1) as u32,
        );
    }

    unsafe fn line(&self, color: c_ulong, width: u32, a: (i32, i32), b: (i32, i32)) {
        self.set_color(color);
        XSetLineAttributes(self.display, self.gc, width, 0, 1, 1);
        XDrawLine(self.display, self.canvas, self.gc, a.0, a.1, b.0, b.1);
    }

    /// The rectangle of the terminal's disclosure button.
    ///
    /// Drawing and hit testing read this, so the visible button is exactly the
    /// region that folds the panel.
    fn terminal_toggle_rect(&self) -> (i32, i32, i32, i32) {
        let s = |v: i32| (v as f32 * self.scale()).round() as i32;
        let expanded = self.terminal_height.clamp(s(150), self.height - s(150));
        let panel_h =
            s(28) + ((expanded - s(28)) as f32 * self.terminal_motion.value.clamp(0.0, 1.0)) as i32;
        let top = self.height - panel_h;
        (s(12), top + s(4), s(20), s(20))
    }

    /// Whether the pointer is over the terminal's disclosure button.
    fn terminal_toggle_hovered(&self) -> bool {
        let (x, y, w, h) = self.terminal_toggle_rect();
        self.mouse_x >= x && self.mouse_x <= x + w && self.mouse_y >= y && self.mouse_y <= y + h
    }

    /// The rectangle of the call hierarchy panel's disclosure button.
    fn graph_toggle_rect(&self) -> (i32, i32, i32, i32) {
        let s = |v: i32| (v as f32 * self.scale()).round() as i32;
        (self.graph_left() + s(10), s(58), s(20), s(20))
    }

    /// Whether the pointer is over the call hierarchy's disclosure button.
    fn graph_toggle_hovered(&self) -> bool {
        let (x, y, w, h) = self.graph_toggle_rect();
        self.mouse_x >= x && self.mouse_x <= x + w && self.mouse_y >= y && self.mouse_y <= y + h
    }

    /// A disclosure control: a chevron in a small bordered button.
    ///
    /// A panel that can be folded has to say so. The `>` that used to be typed
    /// into the terminal title was easy to miss, so this draws the control the
    /// user is meant to press: a bordered hit target with a chevron that turns
    /// from pointing right to pointing down, and an accent border on hover.
    ///
    /// # Safety
    /// Requires a live Cairo context.
    unsafe fn disclosure(&self, x: i32, y: i32, expanded: bool, hovered: bool, progress: f32) {
        let s = |v| self.s(v);
        let size = s(20);
        self.rect(
            if hovered {
                self.colors.panel_2
            } else {
                self.colors.panel
            },
            x,
            y,
            size,
            size,
        );
        self.outline(
            if hovered {
                self.colors.accent
            } else {
                self.colors.line
            },
            x,
            y,
            size,
            size,
        );
        self.chevron_colored(
            x + s(6),
            y + s(6),
            if expanded {
                1.0
            } else {
                progress.clamp(0.0, 1.0)
            },
            if hovered {
                self.colors.accent
            } else {
                self.colors.text
            },
        );
    }

    /// The chevron, in a colour the caller picks.
    ///
    /// # Safety
    /// Requires a live Cairo context.
    unsafe fn chevron_colored(&self, x: i32, y: i32, progress: f32, color: c_ulong) {
        let s = |v| self.s(v);
        let t = progress.clamp(0.0, 1.0);
        let mix = |from: i32, to: i32| from + ((to - from) as f32 * t) as i32;
        let a = (x + s(mix(0, 0)), y + s(mix(-4, -3)));
        let b = (x + s(mix(5, 4)), y + s(mix(0, 2)));
        let c = (x + s(mix(0, 8)), y + s(mix(4, -3)));
        self.line(color, 2, a, b);
        self.line(color, 2, b, c);
    }

    unsafe fn chevron(&self, x: i32, y: i32, expanded: bool) {
        self.chevron_progress(x, y, if expanded { 1.0 } else { 0.0 });
    }

    unsafe fn chevron_progress(&self, x: i32, y: i32, progress: f32) {
        let s = |v| self.s(v);
        let t = progress.clamp(0.0, 1.0);
        let mix = |from: i32, to: i32| from + ((to - from) as f32 * t) as i32;
        let a = (x + s(mix(0, 0)), y + s(mix(-4, -3)));
        let b = (x + s(mix(5, 4)), y + s(mix(0, 2)));
        let c = (x + s(mix(0, 8)), y + s(mix(4, -3)));
        self.line(self.colors.muted, 1, a, b);
        self.line(self.colors.muted, 1, b, c);
    }

    unsafe fn package_icon(&self, kind: usize, x: i32, y: i32, label: &str) {
        let s = |v| self.s(v);
        let color = self.colors.accent;
        match kind {
            0 => {
                for (dx, dy) in [(-13, -13), (2, -13), (-13, 2), (2, 2)] {
                    self.outline(color, x + s(dx), y + s(dy), s(11), s(11));
                }
            }
            1 => {
                let points = [
                    (-15, -7),
                    (-7, -15),
                    (7, -15),
                    (15, -7),
                    (15, 8),
                    (0, 16),
                    (-15, 8),
                ];
                for pair in points.windows(2) {
                    self.line(
                        color,
                        2,
                        (x + s(pair[0].0), y + s(pair[0].1)),
                        (x + s(pair[1].0), y + s(pair[1].1)),
                    );
                }
                self.line(
                    color,
                    2,
                    (x + s(points[6].0), y + s(points[6].1)),
                    (x + s(points[0].0), y + s(points[0].1)),
                );
            }
            2 => {
                self.circle(self.colors.bg, color, x, y, s(16));
                self.line(color, 2, (x, y - s(10)), (x - s(8), y + s(9)));
                self.line(color, 2, (x, y - s(10)), (x + s(8), y + s(9)));
                self.line(color, 2, (x - s(5), y + s(3)), (x + s(5), y + s(3)));
            }
            _ => {
                self.outline(color, x - s(16), y - s(12), s(32), s(24));
                self.line(color, 1, (x - s(12), y + s(5)), (x + s(12), y + s(5)));
                self.circle(self.colors.bg, color, x + s(9), y - s(4), s(2));
            }
        }
        self.text(self.colors.bright, x + s(24), y + s(6), label);
    }

    unsafe fn abstract_vortex(&self, cx: i32, cy: i32, elapsed: f32, absorbing: bool) {
        let s = |v| self.s(v);
        for ring in 0..10 {
            let phase = elapsed * if absorbing { -235.0 } else { 210.0 } + ring as f32 * 29.0;
            let wobble = (elapsed * 3.1 + ring as f32 * 1.7).sin();
            let radius_x = s(18 + ring * 7) + (wobble * s(3) as f32) as i32;
            let radius_y = s(11 + ring * 4) - (wobble * s(2) as f32) as i32;
            let drift_x = ((ring as f32 * 0.9 + elapsed * 2.0).sin() * s(4) as f32) as i32;
            let drift_y = ((ring as f32 * 1.3 - elapsed * 1.6).cos() * s(3) as f32) as i32;
            let color = if absorbing {
                if ring % 3 == 0 {
                    self.colors.line
                } else {
                    self.colors.bg
                }
            } else if ring % 3 == 0 {
                self.colors.bright
            } else {
                self.colors.accent
            };
            self.set_color(color);
            XSetLineAttributes(
                self.display,
                self.gc,
                if ring % 4 == 0 { 3 } else { 2 },
                0,
                1,
                1,
            );
            let sweep = (205 + ((ring * 17) % 95)) * 64;
            XDrawArc(
                self.display,
                self.canvas,
                self.gc,
                cx + drift_x - radius_x,
                cy + drift_y - radius_y,
                (radius_x * 2).max(1) as u32,
                (radius_y * 2).max(1) as u32,
                phase as i32 * 64,
                if absorbing { -sweep } else { sweep },
            );
        }
        self.circle(
            if absorbing {
                self.colors.bg
            } else {
                self.colors.bright
            },
            if absorbing {
                self.colors.line
            } else {
                self.colors.accent
            },
            cx,
            cy,
            s(5),
        );
    }

    fn file_tree_path(&self, file: &FileBuffer) -> String {
        self.project_path
            .as_ref()
            .and_then(|root| file.path.strip_prefix(root).ok())
            .map(|path| path.to_string_lossy().into_owned())
            .filter(|path| !path.is_empty())
            .unwrap_or_else(|| file.name.clone())
    }

    fn file_visible_in_tree(&self, file: &FileBuffer) -> bool {
        let path = self.file_tree_path(file);
        let needle = self.workspace_search.to_ascii_lowercase();
        if !needle.is_empty() {
            return path.to_ascii_lowercase().contains(&needle);
        }
        !self
            .collapsed_folders
            .iter()
            .any(|folder| path == *folder || path.starts_with(&format!("{folder}/")))
    }

    fn folder_reveal_amount(&self, folder: &str) -> f32 {
        let mut amount = self.project_tree_amount.clamp(0.0, 1.0);
        for parent in &self.folders {
            if folder != parent && folder.starts_with(&format!("{parent}/")) {
                amount *= self
                    .folder_tree_motion
                    .get(parent)
                    .map(|motion| motion.0.clamp(0.0, 1.0))
                    .unwrap_or(1.0);
            }
        }
        amount
    }

    fn file_reveal_amount(&self, file: &FileBuffer) -> f32 {
        let path = self.file_tree_path(file);
        let mut amount = self.project_tree_amount.clamp(0.0, 1.0);
        for folder in &self.folders {
            if path == *folder || path.starts_with(&format!("{folder}/")) {
                amount *= self
                    .folder_tree_motion
                    .get(folder)
                    .map(|motion| motion.0.clamp(0.0, 1.0))
                    .unwrap_or(1.0);
            }
        }
        amount
    }

    /// Shared spring step, so drawers and trees move the same way everywhere.
    fn spring_step(value: &mut f32, velocity: &mut f32, target: f32, dt: f32) {
        motion_core::step(value, velocity, target, dt, motion_core::DRAWER);
    }

    fn step_ui_motion(motion: &mut UiMotion, visible: bool, dt: f32) {
        Self::spring_step(
            &mut motion.value,
            &mut motion.velocity,
            if visible { 1.0 } else { 0.0 },
            dt,
        );
    }

    fn overlay_offset(&self, motion: UiMotion) -> i32 {
        self.s(((1.0 - motion.value.clamp(0.0, 1.0)) * -12.0).round() as i32)
    }

    /// Highlight an edit so a paste, a deletion or a completion animates the
    /// same way a single character does.
    fn note_edit(&mut self, kind: motion_core::EditKind, line: usize, start: usize, end: usize) {
        self.edit_pulses.push(kind, line, start, end);
    }

    fn file_tree_row(&self, wanted: usize) -> Option<i32> {
        if !self.project_tree_expanded && self.workspace_search.is_empty() {
            return None;
        }
        let needle = self.workspace_search.to_ascii_lowercase();
        let mut row = 176;
        for folder in &self.folders {
            let has_match = needle.is_empty()
                || folder.to_ascii_lowercase().contains(&needle)
                || self.files.iter().any(|file| {
                    let path = self.file_tree_path(file);
                    path.starts_with(&format!("{folder}/"))
                        && path.to_ascii_lowercase().contains(&needle)
                });
            if has_match {
                row += 28;
            }
        }
        for (index, file) in self.files.iter().enumerate() {
            if !self.file_visible_in_tree(file) {
                continue;
            }
            if index == wanted {
                return Some(self.s(row));
            }
            row += 29;
        }
        None
    }

    unsafe fn circle(&self, fill: c_ulong, stroke: c_ulong, x: i32, y: i32, radius: i32) {
        self.set_color(fill);
        XFillArc(
            self.display,
            self.canvas,
            self.gc,
            x - radius,
            y - radius,
            (radius * 2) as u32,
            (radius * 2) as u32,
            0,
            360 * 64,
        );
        self.set_color(stroke);
        XSetLineAttributes(self.display, self.gc, 1, 0, 1, 1);
        XDrawArc(
            self.display,
            self.canvas,
            self.gc,
            x - radius,
            y - radius,
            (radius * 2) as u32,
            (radius * 2) as u32,
            0,
            360 * 64,
        );
    }

    fn comment_bubble_layout(
        &self,
        anchor_x: i32,
        y: i32,
        value: &str,
        expanded: bool,
        max_right: i32,
    ) -> BubbleLayout {
        let s = |v| self.s(v);
        let preferred_width = s(300);
        let left = (max_right - preferred_width)
            .max(anchor_x + s(35))
            .min(max_right - s(112));
        let available = (max_right - left).max(s(112));
        let chars_per_line = ((available - s(24)) / s(11).max(1)).clamp(6, 34) as usize;
        let lines = wrap_bubble_text(value, chars_per_line, if expanded { 8 } else { 1 });
        let longest = lines
            .iter()
            .map(|line| line.chars().count())
            .max()
            .unwrap_or(7) as i32;
        let width = (longest * s(11) + s(24)).max(s(112)).min(available);
        let height = if expanded {
            s(31) + lines.len() as i32 * s(22)
        } else {
            s(31)
        };
        BubbleLayout {
            left,
            top: y - s(21),
            width,
            height,
            lines,
            chars_per_line,
        }
    }

    unsafe fn comment_bubble(
        &self,
        anchor_x: i32,
        y: i32,
        value: &str,
        expanded: bool,
        editing: bool,
        max_right: i32,
    ) {
        let s = |v| self.s(v);
        let layout = self.comment_bubble_layout(anchor_x, y, value, expanded, max_right);
        if layout.left > anchor_x + s(8) {
            self.line(
                self.colors.accent,
                1,
                (anchor_x, y - s(7)),
                (layout.left, y - s(7)),
            );
        }
        self.rect(
            self.colors.bright,
            layout.left,
            layout.top,
            layout.width,
            layout.height,
        );
        if expanded {
            self.line(
                self.colors.line,
                1,
                (layout.left, layout.top + s(27)),
                (layout.left + layout.width, layout.top + s(27)),
            );
        }
        let tail_center = layout.top + s(14);
        let mut tail = [
            XPoint {
                x: (layout.left - s(7)) as i16,
                y: tail_center as i16,
            },
            XPoint {
                x: layout.left as i16,
                y: (tail_center - s(7)) as i16,
            },
            XPoint {
                x: layout.left as i16,
                y: (tail_center + s(7)) as i16,
            },
        ];
        self.set_color(self.colors.bright);
        XFillPolygon(
            self.display,
            self.canvas,
            self.gc,
            tail.as_mut_ptr(),
            3,
            0,
            0,
        );
        self.text(
            self.colors.bg,
            layout.left + s(9),
            layout.top + s(20),
            if expanded { "v" } else { ">" },
        );
        self.text(
            self.colors.bg,
            layout.left + s(29),
            layout.top + s(20),
            if editing {
                self.tr("EDITING", "编辑中")
            } else {
                self.tr("COMMENT", "注释")
            },
        );
        self.text(
            self.colors.bg,
            layout.left + layout.width - s(48),
            layout.top + s(20),
            self.tr("EDIT", "编辑"),
        );
        if expanded {
            for (index, line) in layout.lines.iter().enumerate() {
                self.text(
                    self.colors.bg,
                    layout.left + s(12),
                    layout.top + s(48) + index as i32 * s(22),
                    line,
                );
            }
        }
    }

    unsafe fn text(&self, color: c_ulong, x: i32, y: i32, value: &str) {
        self.pango_text(color, x, y, value, false);
    }

    unsafe fn pango_text(&self, color: c_ulong, x: i32, y: i32, value: &str, code: bool) {
        let clean = value.replace('\0', "");
        let Ok(value) = CString::new(clean) else {
            return;
        };
        let Ok(font) = CString::new(self.font_description(code)) else {
            return;
        };
        let layout = pango_cairo_create_layout(self.cairo);
        if layout.is_null() {
            return;
        }
        let description = pango_font_description_from_string(font.as_ptr());
        pango_layout_set_text(layout, value.as_ptr(), value.as_bytes().len() as i32);
        pango_layout_set_font_description(layout, description);
        let (_, height) = self.pango_text_size(value.to_str().unwrap_or(""), code);
        let (red, green, blue) = self.color_rgb(color);
        cairo_set_source_rgb(self.cairo, red, green, blue);
        cairo_move_to(self.cairo, x as f64, (y - height + self.s(2)) as f64);
        pango_cairo_show_layout(self.cairo, layout);
        pango_font_description_free(description);
        g_object_unref(layout.cast());
    }

    fn code_symbol_bounds(&self, line: usize, start: usize, end: usize) -> (i32, i32, i32, i32) {
        let left = (self.width as f32 * 0.17) as i32;
        let y = self.s(119) + line as i32 * self.code_line_step();
        let value = &self.files[self.selected_file].lines[line];
        let x = left + self.s(52) + self.code_text_width(&value[..start]);
        let width = self.code_text_width(&value[start..end]);
        (x - self.s(4), y - self.s(21), width + self.s(8), self.s(27))
    }

    unsafe fn highlighted_text(&self, x: i32, y: i32, value: &str, language: &str) {
        let mut cursor = x;
        for token in
            editor_core::syntax_tokens(value, &format!("file.{}", extension_for_language(language)))
        {
            let color = match token.kind {
                editor_core::TokenKind::Keyword => self.colors.keyword,
                editor_core::TokenKind::Type => self.colors.syntax_type,
                editor_core::TokenKind::Function => self.colors.function,
                editor_core::TokenKind::Constant => self.colors.constant,
                editor_core::TokenKind::Variable => self.colors.variable,
                editor_core::TokenKind::Parameter => self.colors.parameter,
                editor_core::TokenKind::Property => self.colors.property,
                editor_core::TokenKind::Macro => self.colors.macro_name,
                editor_core::TokenKind::Module => self.colors.module,
                editor_core::TokenKind::Preprocessor => self.colors.preprocessor,
                editor_core::TokenKind::Literal => self.colors.literal,
                editor_core::TokenKind::Comment => self.colors.comment,
                editor_core::TokenKind::Plain => self.colors.text,
            };
            self.pango_text(color, cursor, y, &token.text, true);
            cursor += self.code_text_width(&token.text);
        }
    }

    /// The terminal cell a point of the panel is over.
    ///
    /// The panel draws its newest lines at the bottom, so the row is counted
    /// back from the last line of output rather than forwards.
    fn terminal_point_at(&self, x: i32, y: i32) -> Option<terminal_core::TerminalPoint> {
        if !self.terminal_open {
            return None;
        }
        let s = |v: i32| (v as f32 * self.scale()).round() as i32;
        let top = self.terminal_top();
        let step = s(24);
        if y < top + s(30) || step <= 0 {
            return None;
        }
        let rows = self.terminal_visible_rows();
        let row = ((y - top - s(30)) / step).max(0) as usize;
        if row >= rows {
            return None;
        }
        let cell = s(8).max(1);
        let column = ((x - s(24)).max(0) / cell) as usize;
        // Row zero of the panel is the oldest of the visible lines.
        let line = self.terminal_lines().len().saturating_sub(rows) + row;
        Some(terminal_core::TerminalPoint { line, column })
    }

    /// How many terminal rows the panel shows.
    fn terminal_visible_rows(&self) -> usize {
        let s = |v: i32| (v as f32 * self.scale()).round() as i32;
        let height = self.height - self.terminal_top() - s(34);
        ((height / s(24).max(1)).max(1)) as usize
    }

    /// The lines the terminal panel is showing.
    fn terminal_lines(&self) -> Vec<String> {
        self.terminals
            .get(
                self.terminal_active
                    .min(self.terminals.len().saturating_sub(1)),
            )
            .map(|terminal| {
                terminal
                    .output
                    .lock()
                    .map(|lines| lines.clone())
                    .unwrap_or_default()
            })
            .unwrap_or_default()
    }

    /// Copy the terminal selection, the way `Ctrl+Shift+C` does.
    fn copy_terminal_selection(&mut self) -> bool {
        let Some(selection) = self.terminal_selection else {
            return false;
        };
        let lines = self.terminal_lines();
        let Some(text) = terminal_core::selected_text(&lines, &selection) else {
            return false;
        };
        if text.is_empty() {
            return false;
        }
        self.clipboard = text.clone();
        self.status_message = format!(
            "{} {}",
            self.tr(
                "Copied characters from the terminal:",
                "已从终端复制字符数："
            ),
            text.chars().count()
        );
        true
    }

    /// The project index, rebuilt only when the open files change.
    ///
    /// The relation graph is drawn every frame, and scanning every open file
    /// on every frame cost more than a frame has: twelve files of three
    /// hundred lines took about a hundred and forty milliseconds, so the
    /// window fell to single figure frames a second while the panel was open.
    /// The fingerprints say which files changed, so the rest are reused.
    fn refresh_index(&mut self) {
        let fingerprints = self
            .files
            .iter()
            .map(|file| {
                let bytes = file.lines.iter().map(|line| line.len() as u64 + 1).sum();
                index_core::FileFingerprint::new(file.name.clone(), bytes, file.lines.len() as u64)
            })
            .collect::<Vec<_>>();
        let contents = self
            .files
            .iter()
            .map(|file| (file.name.clone(), file.lines.clone()))
            .collect::<std::collections::HashMap<_, _>>();
        self.project_index
            .refresh(fingerprints, |path| contents.get(path).cloned());
    }

    /// Open or close the AI panel.
    fn toggle_ai(&mut self) {
        self.ai_open = !self.ai_open;
        self.ai_focused = self.ai_open;
        if self.ai_open {
            self.status_message = self
                .tr(
                    "AI: type a prompt, Enter sends, Esc cancels",
                    "AI：输入提示词，回车发送，Esc 取消",
                )
                .into();
        }
        // The panel starts with the prompt, not with a settings field.
        self.ai_field_focused = None;
    }

    /// Where the AI conversation of the current project is kept.
    fn ai_history_path(&self) -> PathBuf {
        let root = self.project_path.clone().unwrap_or_else(projects_root);
        root.join(".tc-ide-ai").join("history.conf")
    }

    /// Load the conversation saved with the project, if there is one.
    fn load_ai_history(&mut self) {
        self.ai_messages = match fs::read_to_string(self.ai_history_path()) {
            Ok(content) => ai_core::Conversation::parse(&content),
            Err(_) => ai_core::Conversation::new(),
        };
    }

    /// Save the conversation with the project.
    fn save_ai_history(&self) {
        let path = self.ai_history_path();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::write(&path, self.ai_messages.serialize());
    }

    /// Write the request body next to the project, the way the other backend
    /// does, because the key travels through the environment and not the file.
    fn write_ai_body(&self, body: &str) -> Option<PathBuf> {
        let root = self.project_path.clone().unwrap_or_else(projects_root);
        let directory = root.join(".tc-ide-ai");
        fs::create_dir_all(&directory).ok()?;
        let path = directory.join("request.json");
        fs::write(&path, body).ok()?;
        Some(path)
    }

    /// Send the prompt in the panel to the configured endpoint.
    fn send_ai_prompt(&mut self) {
        let prompt = self.ai_prompt.trim().to_string();
        if prompt.is_empty() {
            return;
        }
        if self.ai_key.is_empty() {
            self.ai_error = Some(
                self.tr(
                    "Set an API key for this session before sending",
                    "发送前请先为本会话设置 API Key",
                )
                .into(),
            );
            return;
        }
        let settings = ai_core::AiSettings {
            endpoint: self.ai_endpoint.clone(),
            model: self.ai_model.clone(),
        };
        // The conversation so far goes with the prompt, so the answer has the
        // context of the project rather than only this line.
        let turns = self.ai_messages.turns().to_vec();
        let history = turns
            .iter()
            .map(|(from_user, text)| (if *from_user { "user" } else { "assistant" }, text.as_str()))
            .collect::<Vec<_>>();
        let body = ai_core::chat_body_with_history(&settings.model, &history, true);
        let Some(path) = self.write_ai_body(&body) else {
            self.ai_error = Some(
                self.tr("Could not write the request body", "无法写入请求体")
                    .into(),
            );
            return;
        };
        match ai_core::ChatSession::start(
            &settings,
            &self.ai_key,
            &prompt,
            &path,
            ai_core::DEFAULT_TIMEOUT_SECONDS,
        ) {
            Ok(session) => {
                self.ai_messages.push_user(&prompt);
                self.ai_prompt.clear();
                self.ai_error = None;
                self.ai_session = Some(session);
                self.status_message = self
                    .tr(
                        "AI request running; Esc cancels it",
                        "AI 请求进行中，Esc 可取消",
                    )
                    .into();
            }
            Err(error) => self.ai_error = Some(error.message()),
        }
    }

    /// Collect the streamed answer of a running request.
    fn poll_ai_session(&mut self) {
        let Some(mut session) = self.ai_session.take() else {
            return;
        };
        let prompt = session.prompt().to_string();
        let events = session.poll();
        let mut finished = false;
        let mut failure = None;
        for event in events {
            match event {
                ai_core::ChatEvent::Delta(text) => self.ai_messages.append_answer(&text),
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
                self.ai_error = Some(error.clone());
                self.status_message = error;
            } else {
                self.status_message = self.tr("AI answer received", "已收到 AI 回答").into();
                self.save_ai_history();
            }
        } else {
            self.ai_session = Some(session);
        }
    }

    /// Stop a running request.
    fn cancel_ai_request(&mut self) {
        if let Some(mut session) = self.ai_session.take() {
            session.cancel();
        }
        self.status_message = self
            .tr(
                "AI request cancelled; the partial answer is kept",
                "已取消 AI 请求，已收到的部分会保留",
            )
            .into();
    }

    /// Rectangle of the AI panel, on the right.
    fn ai_bounds(&self) -> (i32, i32, i32, i32) {
        let s = |v: i32| (v as f32 * self.scale()).round() as i32;
        let width = s(430);
        let left = self.width - width - s(16);
        (left, s(52), width, s(430))
    }

    /// Draw the AI panel: the conversation and the prompt line.
    ///
    /// # Safety
    /// Requires a live Cairo context.
    unsafe fn draw_ai_panel(&self) {
        if !self.ai_open {
            return;
        }
        let s = |v| self.s(v);
        let (x, y, w, h) = self.ai_bounds();
        self.rect(self.colors.panel, x, y, w, h);
        self.outline(self.colors.line, x, y, w, h);
        self.text(
            self.colors.muted,
            x + s(12),
            y + s(20),
            &format!("{}  {}", self.tr("AI", "AI"), self.ai_model),
        );
        if let Some(error) = self.ai_error.as_ref() {
            self.text(self.colors.preprocessor, x + s(12), y + s(42), error);
        }

        // The conversation, newest last, scrolled to the end.
        let turns = self.ai_messages.turns();
        let rows = ((h - s(110)) / s(20)).max(1) as usize;
        let start = turns.len().saturating_sub(rows);
        for (index, (from_user, text)) in turns.iter().skip(start).enumerate() {
            let row_y = y + s(58 + index as i32 * 20);
            let (label, color) = if *from_user {
                (self.tr("you", "你"), self.colors.accent)
            } else {
                (self.tr("ai", "AI"), self.colors.literal)
            };
            self.text(color, x + s(12), row_y, label);
            // One turn is one row here: the panel is narrow, and the full
            // answer is in the conversation that gets saved.
            let short: String = text.chars().take(48).collect();
            self.text(self.colors.text, x + s(46), row_y, &short);
        }
        if turns.is_empty() {
            self.text(
                self.colors.muted,
                x + s(12),
                y + s(58),
                self.tr("Ask about this project", "就这个项目提问"),
            );
        }

        // The prompt line.
        let field_y = y + h - s(34);
        self.rect(
            self.colors.panel_2,
            x + s(12),
            field_y - s(18),
            w - s(24),
            s(26),
        );
        self.outline(
            if self.ai_focused {
                self.colors.accent
            } else {
                self.colors.line
            },
            x + s(12),
            field_y - s(18),
            w - s(24),
            s(26),
        );
        let value = if self.ai_prompt.is_empty() {
            self.tr("Ask the model...", "向模型提问…")
        } else {
            self.ai_prompt.as_str()
        };
        self.text(
            if self.ai_prompt.is_empty() {
                self.colors.muted
            } else {
                self.colors.bright
            },
            x + s(20),
            field_y,
            value,
        );
        if self.ai_focused && blink_phase(self.started) {
            let caret = x + s(20) + self.pango_text_size(&self.ai_prompt, false).0;
            self.line(
                self.colors.bright,
                1,
                (caret, field_y - s(14)),
                (caret, field_y + s(4)),
            );
        }
    }

    /// The range buttons, in the order the shared model lists them, with the
    /// automatic range first.
    fn graph_scope_labels(&self) -> Vec<&'static str> {
        let mut labels = vec![self.tr("AUTO", "跟随")];
        labels.extend(
            graph_core::GraphScope::ALL
                .into_iter()
                .map(|scope| match scope {
                    graph_core::GraphScope::Project => self.tr("PROJECT", "整个项目"),
                    graph_core::GraphScope::Folder => self.tr("FOLDER", "文件夹"),
                    graph_core::GraphScope::File => self.tr("FILE", "当前文件"),
                    graph_core::GraphScope::Symbol => self.tr("SYMBOL", "当前符号"),
                }),
        );
        labels
    }

    /// Index of the button the current range is on, automatic included.
    fn graph_scope_index(&self) -> usize {
        match self.graph_scope {
            None => 0,
            Some(scope) => scope.index() + 1,
        }
    }

    /// Pick a range by its button.
    fn set_graph_scope_from_button(&mut self, index: usize) -> bool {
        let total = graph_core::GraphScope::ALL.len() + 1;
        if index >= total {
            return false;
        }
        self.graph_scope = if index == 0 {
            None
        } else {
            Some(graph_core::GraphScope::from_index(index - 1))
        };
        self.graph_scroll = 0;
        true
    }

    /// Rows the debug panel would show right now.
    fn debug_panel_rows(&self) -> Vec<dap_core::DebugRow> {
        dap_core::debug_rows(&self.debug_state, self.debug_frame)
    }

    /// Rectangle of the debug panel, on the left above the project tree.
    fn debug_panel_bounds(&self) -> (i32, i32, i32, i32) {
        let s = |v: i32| (v as f32 * self.scale()).round() as i32;
        let width = (self.width as f32 * 0.17) as i32 - s(8);
        let top = s(52);
        let height = s(240);
        (s(4), top, width, height)
    }

    /// Show or hide the debug panel.
    fn toggle_debug_panel(&mut self) {
        self.debug_panel_open = !self.debug_panel_open;
        self.status_message = if self.debug_panel_open {
            self.tr("Debug panel shown", "已显示调试面板").into()
        } else {
            self.tr("Debug panel hidden", "已隐藏调试面板").into()
        };
    }

    /// Move the selected stack frame, which the panel highlights.
    fn step_debug_frame(&mut self, delta: i32) {
        let count = self.debug_state.frames.len() as i32;
        if count == 0 {
            return;
        }
        self.debug_frame = (self.debug_frame as i32 + delta).clamp(0, count - 1) as usize;
        if let Some(frame) = self.debug_state.frames.get(self.debug_frame) {
            self.status_message = format!("{}:{}", frame.name, frame.line);
        }
    }

    /// Draw the call stack, the scopes and the variables of the current stop.
    ///
    /// The rows come from the shared model, so the panel shows the same thing
    /// on every backend and the layout is covered by tests that do not need a
    /// window.
    unsafe fn draw_debug_panel(&self) {
        if !self.debug_panel_open {
            return;
        }
        let s = |v| self.s(v);
        let (x, y, w, h) = self.debug_panel_bounds();
        self.rect(self.colors.panel, x, y, w, h);
        self.outline(self.colors.line, x, y, w, h);
        self.text(
            self.colors.muted,
            x + s(10),
            y + s(18),
            self.tr("DEBUG", "调试"),
        );

        let rows = self.debug_panel_rows();
        let capacity = ((h - s(34)) / s(20)).max(1) as usize;
        let (shown, hidden) = dap_core::visible_debug_rows(&rows, capacity);
        for (index, row) in shown.iter().enumerate() {
            let row_y = y + s(40 + index as i32 * 20);
            match row {
                dap_core::DebugRow::Heading(label) => {
                    self.text(self.colors.muted, x + s(10), row_y, label);
                }
                dap_core::DebugRow::Frame {
                    label,
                    detail,
                    selected,
                    ..
                } => {
                    if *selected {
                        self.rect(self.colors.accent, x + s(4), row_y - s(14), w - s(8), s(19));
                    }
                    self.text(
                        if *selected {
                            self.colors.bright
                        } else {
                            self.colors.text
                        },
                        x + s(14),
                        row_y,
                        label,
                    );
                    self.text(self.colors.muted, x + w - s(90), row_y, detail);
                }
                dap_core::DebugRow::Scope { label, detail } => {
                    self.text(self.colors.text, x + s(18), row_y, label);
                    self.text(self.colors.muted, x + w - s(80), row_y, detail);
                }
                dap_core::DebugRow::Variable { name, value } => {
                    self.text(self.colors.keyword, x + s(24), row_y, name);
                    self.text(self.colors.literal, x + s(110), row_y, value);
                }
                dap_core::DebugRow::Empty(message) => {
                    self.text(self.colors.muted, x + s(14), row_y, message);
                }
            }
        }
        if hidden > 0 {
            self.text(
                self.colors.muted,
                x + s(10),
                y + h - s(8),
                &format!("{} +{hidden}", self.tr("More", "更多")),
            );
        }
    }

    fn graph_bounds(&self) -> (i32, i32, i32, i32) {
        let left = self.graph_left();
        (
            left,
            self.s(82),
            self.width - left,
            self.height - self.s(112),
        )
    }

    /// Real installation state of the selected language's tooling.
    fn language_tooling_summary(&self) -> String {
        let language = self
            .languages
            .get(self.selected_language.unwrap_or(0))
            .map(|pack| pack.name)
            .unwrap_or("Plain Text");
        let language = match language {
            "Web" => "TypeScript / JavaScript",
            other => other,
        };
        extension_core::language_tools(language, |tool| {
            std::process::Command::new(tool)
                .arg("--version")
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .map(|status| status.success())
                .unwrap_or(false)
        })
        .summary()
    }

    /// Summary of the extension runtime cache, for the extensions panel.
    fn runtime_cache_summary(&self) -> String {
        let summary = format!(
            "{} {} · {}",
            self.runtime_cache.entries().len(),
            if self.ui_chinese {
                "个已缓存扩展"
            } else {
                "cached extension(s)"
            },
            extension_core::format_bytes(self.runtime_cache.total_bytes())
        );
        if self.extension_upgrades == 0 {
            return summary;
        }
        // The shared update plan says which cached extensions the project
        // catalog has a newer version of.
        summary
            + &format!(
                " · {}{}",
                self.extension_upgrades,
                if self.ui_chinese {
                    "个可升级"
                } else {
                    " can be upgraded"
                }
            )
    }

    /// Record the project and the open documents, then persist the session.
    fn remember_session(&mut self) {
        if let Some(path) = self.project_path.clone() {
            self.session.set_project(Some(path.clone()));
            self.session.remember_project(&path);
        }
        let documents = self
            .files
            .iter()
            .filter(|file| file.path.is_file())
            .map(|file| file.path.clone())
            .collect::<Vec<_>>();
        for path in documents {
            let name = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            self.session.open(Some(path), name);
        }
        self.session.prune_recent_projects(|path| path.is_dir());
        save_session(&self.session);
    }

    /// Ask the desktop for a file and open it in the editor.
    fn open_file_dialog(&mut self) {
        let output = Command::new("zenity")
            .args([
                "--file-selection",
                "--title=TC-IDE Open File",
                "--file-filter=Sources | *.rs *.c *.h *.cpp *.hpp *.py *.js *.ts *.go *.java *.json *.md *.toml *.txt",
                "--file-filter=All files | *",
            ])
            .output();
        match output {
            Ok(result) if result.status.success() => {
                let path = String::from_utf8_lossy(&result.stdout).trim().to_string();
                if !path.is_empty() {
                    self.open_path(PathBuf::from(path));
                }
            }
            Ok(_) => {}
            Err(_) => {
                self.status_message = self
                    .tr(
                        "Install zenity to open a file",
                        "需要安装 zenity 才能打开文件",
                    )
                    .into();
            }
        }
    }

    /// Ask the desktop for a folder and open it as the project.
    fn open_folder_dialog(&mut self) {
        let output = Command::new("zenity")
            .args([
                "--file-selection",
                "--directory",
                "--title=TC-IDE Open Folder",
            ])
            .output();
        match output {
            Ok(result) if result.status.success() => {
                let path = String::from_utf8_lossy(&result.stdout).trim().to_string();
                if !path.is_empty() {
                    self.open_project_at(PathBuf::from(path));
                }
            }
            Ok(_) => {}
            Err(_) => {
                self.status_message = self
                    .tr(
                        "Install zenity to open a folder",
                        "需要安装 zenity 才能打开文件夹",
                    )
                    .into();
            }
        }
    }

    /// Open one file, in a new tab when one is already open.
    fn open_path(&mut self, path: PathBuf) {
        if !path.is_file() {
            self.status_message = self
                .tr("That file no longer exists", "该文件已不存在")
                .into();
            return;
        }
        match fs::read_to_string(&path) {
            Ok(content) => {
                let name = path
                    .file_name()
                    .map(|value| value.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "untitled".into());
                let mut lines = content.lines().map(str::to_string).collect::<Vec<_>>();
                if lines.is_empty() {
                    lines.push(String::new());
                }
                self.files.push(FileBuffer::new(name, path, lines));
                self.selected_file = self.files.len() - 1;
                self.caret_line = 0;
                self.caret_column = 0;
                self.remember_session();
                let opened = self.files[self.selected_file].name.clone();
                self.status_message = format!("{} {opened}", self.tr("Opened", "已打开"));
                // A server for the file that was just opened, if one is
                // installed; a missing one only says so.
                self.ensure_language_server();
            }
            Err(error) => {
                self.status_message = format!(
                    "{}: {error}",
                    self.tr("Could not read the file", "无法读取文件")
                );
            }
        }
    }

    /// Make a folder the project and remember it.
    fn open_project_at(&mut self, folder: PathBuf) {
        if !folder.is_dir() {
            self.status_message = self
                .tr("That folder no longer exists", "该文件夹已不存在")
                .into();
            return;
        }
        let name = folder
            .file_name()
            .map(|value| value.to_string_lossy().into_owned())
            .unwrap_or_else(|| "TC-IDE".into());
        // A server belongs to the project it indexed, so a new project gets a
        // new one rather than answers about the old tree.
        self.shutdown_language_server();
        self.project_path = Some(folder.clone());
        self.project_name = name.clone();
        self.session.open(Some(folder), name.clone());
        self.remember_session();
        self.status_message = format!("{} {name}", self.tr("Opened project", "已打开项目"));
    }

    /// Rows of the File menu, from the shared recent project list.
    fn file_menu_rows(&self) -> Vec<FileMenuRow> {
        file_menu_rows(self.session.recent_projects())
    }

    /// Vertical centre of one File menu row.
    fn file_menu_row_y(&self, index: usize) -> i32 {
        let s = |v: i32| (v as f32 * self.scale()).round() as i32;
        let offset = self.overlay_offset(self.new_menu_motion);
        s(43) + offset + s(24 + index as i32 * FILE_MENU_ROW)
    }

    /// Restore the previous session: project first, then its documents.
    fn restore_session(&mut self) {
        let session = load_session();
        self.session = session;
        if let Some(project) = self.session.project().cloned() {
            if project.is_dir() {
                self.project_path = Some(project.clone());
                self.project_name = project
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "TC-IDE".into());
            }
        }
        let restored = self.session.restorable_documents();
        let mut files = Vec::new();
        for path in restored {
            let Ok(content) = fs::read_to_string(&path) else {
                continue;
            };
            let mut lines = content.lines().map(str::to_string).collect::<Vec<_>>();
            if lines.is_empty() {
                lines.push(String::new());
            }
            files.push(FileBuffer::new(
                path.file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                path,
                lines,
            ));
        }
        if !files.is_empty() {
            self.files = files;
            self.selected_file = 0;
            self.caret_line = 0;
            self.caret_column = 0;
            self.status_message = format!(
                "Restored {} document(s) from the previous session",
                self.files.len()
            );
        }
        self.rebuild_project_index();
    }

    /// Relation graph of the current editor state, built by the shared model.
    ///
    /// The symbols come from the incremental index rather than a fresh scan:
    /// this runs once per frame while the panel is open, and scanning a three
    /// hundred line file measured about seven milliseconds on its own.
    fn graph(&self) -> graph_core::Graph {
        let file = &self.files[self.selected_file];
        // The index keeps the symbols with the file they came from, which the
        // graph does not need here, so they are unwrapped into the plain form.
        let symbols = match self.project_index.file_symbols(&file.name) {
            Some(indexed) => indexed
                .iter()
                .map(|entry| entry.symbol.clone())
                .collect::<Vec<_>>(),
            // The index has not seen this file yet, so fall back to a scan
            // rather than drawing an empty graph for one frame.
            None => editor_core::symbol_occurrences(&file.lines, &file.name),
        };
        self.graph_with(&file.lines, &symbols)
    }

    fn graph_with(
        &self,
        lines: &[String],
        symbols: &[editor_core::SymbolOccurrence],
    ) -> graph_core::Graph {
        let current_file = self.files[self.selected_file].name.as_str();
        let folded = self.graph_folded.iter().cloned().collect::<Vec<_>>();
        let pinned = self.graph_scope;
        // Declarations come from the incremental index rather than a fresh
        // scan of every open file, because this runs once per frame.
        let declarations = self.project_index.declarations();
        let inputs = graph_core::GraphInputs {
            project_name: &self.project_name,
            current_file,
            current_folder: graph_core::folder_of(current_file),
            hovered: self.hovered_symbol.as_ref(),
            file_lines: lines,
            file_symbols: symbols,
            project_symbols: &self.project_symbols,
            project_declarations: &declarations,
            folded: &folded,
            relation: self.graph_relation,
        };
        let scope =
            graph_core::auto_scope(self.hovered_symbol.as_ref(), &self.graph_search, pinned);
        graph_core::build_graph(&inputs, scope, &self.graph_search)
    }

    /// Index the project once so the graph can show cross file relations.
    fn rebuild_project_index(&mut self) {
        let mut sources: Vec<(String, Vec<String>)> = Vec::new();
        if let Some(root) = self.project_path.clone() {
            collect_project_sources(&root, &mut sources);
        }
        if sources.is_empty() {
            for file in &self.files {
                sources.push((file.name.clone(), file.lines.clone()));
            }
        }
        self.project_symbols = editor_core::project_symbol_occurrences(&sources);
        self.graph_folded.clear();
        self.hovered = None;
        self.hovered_symbol = None;
        self.graph_scroll = 0;
    }

    /// Rows the graph panel can show.
    fn graph_rows(&self, graph: &graph_core::Graph) -> usize {
        let (_, _, _, h) = self.graph_bounds();
        let _ = graph;
        ((h - self.s(158)) / self.s(54).max(1)).max(1) as usize
    }

    /// Card position of a node index inside the graph panel.
    fn node_position(&self, graph: &graph_core::Graph, index: usize) -> (i32, i32) {
        let (x, y, _, _) = self.graph_bounds();
        let rows = self.graph_rows(graph);
        let first = graph.clamped_scroll(self.graph_scroll, rows);
        let row = index.saturating_sub(first);
        let depth = graph.nodes.get(index).map(|node| node.depth).unwrap_or(0);
        (
            x + self.s(28 + depth as i32 * 28),
            y + self.s(121) + (self.s(54) as f32 * self.graph_zoom * row as f32) as i32,
        )
    }

    /// Node indices currently on screen.
    fn graph_visible(&self, graph: &graph_core::Graph) -> Vec<usize> {
        let rows = self.graph_rows(graph);
        let first = graph.clamped_scroll(self.graph_scroll, rows);
        let last = graph.len().min(first.saturating_add(rows));
        (first..last).collect()
    }

    /// Focus a graph target: select the matching open file and place the caret.
    fn focus_graph_target(&mut self, target: &graph_core::GraphTarget) -> bool {
        let name = graph_core::file_name_of(&target.file);
        let Some(index) = self
            .files
            .iter()
            .position(|file| file.name == target.file || file.name.ends_with(name))
        else {
            return false;
        };
        self.selected_file = index;
        self.remember_session();
        self.graph_scroll = 0;
        self.caret_line = target
            .line
            .min(self.files[index].lines.len().saturating_sub(1));
        self.caret_column = target.column;
        self.selection_anchor = None;
        self.selection_head = None;
        true
    }

    /// Focus the symbol a graph node represents.
    fn focus_graph_node(&mut self, node: &graph_core::GraphNode) {
        let Some(target) = node.target.as_ref() else {
            self.hovered_symbol = None;
            return;
        };
        let name = node.label.trim_end_matches("()").to_string();
        if name.is_empty() || node.edge.is_none() {
            self.hovered_symbol = None;
            return;
        }
        self.hovered_symbol = Some(editor_core::SymbolOccurrence {
            name: name.clone(),
            line: target.line,
            start: target.column,
            end: target.column + name.len(),
            role: if node.edge == Some(graph_core::GraphEdge::Declaration) {
                editor_core::SymbolRole::Declaration
            } else {
                editor_core::SymbolRole::Reference
            },
        });
    }

    unsafe fn draw(&self) {
        self.rect(self.colors.bg, 0, 0, self.width, self.height);
        self.draw_header();
        self.draw_workspace();
        self.draw_editor();
        self.draw_debug_panel();
        if self.graph_motion.value > 0.08 {
            self.draw_graph();
        }
        if self.hovered_from_code {
            self.draw_code_relations();
        }
        if self.workspace_context_menu.is_some() {
            self.draw_workspace_context_menu();
        }
        if self.new_menu_motion.value > 0.01 {
            self.draw_new_menu();
        }
        if self.edit_menu_motion.value > 0.01 {
            self.draw_edit_menu();
        }
        if self.view_menu_motion.value > 0.01 {
            self.draw_view_menu();
        }
        if self.settings_motion.value > 0.01 {
            self.draw_settings_menu();
        }
        if self.package_motion.value > 0.01 {
            self.draw_package_menu();
        }
        if self.extensions_motion.value > 0.01 {
            self.draw_extensions();
        }
        self.draw_ai_panel();
        self.draw_hover();
        self.draw_language_overlays();
        self.draw_goto_prompt();
        self.draw_condition_prompt();
        if self.create_kind.is_some() && self.create_motion.value > 0.01 {
            self.draw_create_dialog();
        }
        if self.build_started.is_some() {
            self.draw_build_animation();
        }
        self.draw_status();
        self.draw_diagnostic_status();
        cairo_surface_flush(self.cairo_surface);
        XCopyArea(
            self.display,
            self.canvas,
            self.window,
            self.gc,
            0,
            0,
            self.width as u32,
            self.height as u32,
            0,
            0,
        );
        XFlush(self.display);
    }

    unsafe fn draw_header(&self) {
        let s = |v| self.s(v);
        let header = s(46);
        self.rect(self.colors.panel_2, 0, 0, self.width, header);
        self.line(
            self.colors.line,
            1,
            (0, header - 1),
            (self.width, header - 1),
        );
        if !self.logo_surface.is_null() {
            let image_w = cairo_image_surface_get_width(self.logo_surface).max(1) as f64;
            let image_h = cairo_image_surface_get_height(self.logo_surface).max(1) as f64;
            let target_h = s(54) as f64;
            let scale = target_h / image_h;
            cairo_save(self.cairo);
            cairo_translate(self.cairo, s(7) as f64, -s(4) as f64);
            cairo_scale(self.cairo, scale, scale);
            cairo_set_source_surface(self.cairo, self.logo_surface, 0.0, 0.0);
            cairo_paint_with_alpha(self.cairo, 1.0);
            cairo_restore(self.cairo);
            let _ = image_w;
        } else {
            self.text(self.colors.bright, s(15), s(29), "TC");
            self.text(self.colors.accent, s(42), s(29), "IDE");
        }
        self.outline(
            if self.new_menu_open {
                self.colors.accent
            } else {
                self.colors.line
            },
            s(112),
            s(10),
            s(88),
            s(27),
        );
        self.text(self.colors.text, s(124), s(29), self.tr("File", "文件"));
        self.chevron(s(181), s(24), self.new_menu_open);
        self.outline(
            if self.edit_menu_open {
                self.colors.accent
            } else {
                self.colors.line
            },
            s(207),
            s(10),
            s(88),
            s(27),
        );
        self.text(self.colors.text, s(219), s(29), self.tr("Edit", "编辑"));
        self.outline(
            if self.view_menu_open {
                self.colors.accent
            } else {
                self.colors.line
            },
            s(302),
            s(10),
            s(88),
            s(27),
        );
        self.text(self.colors.text, s(314), s(29), self.tr("View", "查看"));
        let path = format!(
            "{}  /  {}",
            self.project_name, self.files[self.selected_file].name
        );
        let path_x = self.width / 2 - self.pango_text_size(&path, false).0 / 2;
        self.text(self.colors.muted, path_x.max(s(405)), s(29), &path);
        self.outline(
            if self.settings_open {
                self.colors.accent
            } else {
                self.colors.line
            },
            self.width - s(410),
            s(10),
            s(120),
            s(27),
        );
        self.text(
            self.colors.text,
            self.width - s(394),
            s(29),
            self.tr("Settings", "设置"),
        );
        self.outline(
            if self.extensions_open {
                self.colors.accent
            } else {
                self.colors.line
            },
            self.width - s(275),
            s(10),
            s(126),
            s(27),
        );
        self.text(
            self.colors.text,
            self.width - s(260),
            s(29),
            self.tr("Extensions", "扩展"),
        );
        self.outline(
            if self.package_open {
                self.colors.accent
            } else {
                self.colors.line
            },
            self.width - s(136),
            s(10),
            s(118),
            s(27),
        );
        self.text(
            self.colors.text,
            self.width - s(119),
            s(29),
            self.tr("Package", "打包"),
        );
        self.chevron(self.width - s(39), s(24), self.package_open);
    }

    unsafe fn draw_workspace(&self) {
        let s = |v| self.s(v);
        let width = (self.width as f32 * 0.17) as i32;
        self.rect(self.colors.panel, 0, s(46), width, self.height - s(70));
        self.line(
            self.colors.line,
            1,
            (width, s(46)),
            (width, self.height - s(24)),
        );
        self.rect(self.colors.bg, s(10), s(58), width - s(20), s(34));
        self.outline(
            if self.workspace_search_focused {
                self.colors.accent
            } else {
                self.colors.line
            },
            s(10),
            s(58),
            width - s(20),
            s(34),
        );
        self.text(
            self.colors.muted,
            s(22),
            s(81),
            if self.workspace_search.is_empty() {
                self.tr("Search files", "搜索文件")
            } else {
                &self.workspace_search
            },
        );
        self.text(self.colors.muted, s(14), s(116), self.tr("PROJECT", "项目"));
        self.text(self.colors.bright, s(34), s(145), &self.project_name);
        self.chevron_progress(s(17), s(140), self.project_tree_amount);
        self.text(
            self.colors.muted,
            width - s(72),
            s(145),
            if self.project_tree_expanded {
                self.tr("HIDE", "收起")
            } else {
                self.tr("SHOW", "展开")
            },
        );
        let root_amount = if self.workspace_search.is_empty() {
            self.project_tree_amount.clamp(0.0, 1.0)
        } else {
            1.0
        };
        if root_amount > 0.01 || !self.workspace_search.is_empty() {
            let mut row_y = 148.0_f32;
            for folder in &self.folders {
                if !self.workspace_search.is_empty()
                    && !folder
                        .to_ascii_lowercase()
                        .contains(&self.workspace_search.to_ascii_lowercase())
                    && !self.files.iter().any(|file| {
                        let path = self.file_tree_path(file);
                        path.starts_with(&format!("{folder}/"))
                            && path
                                .to_ascii_lowercase()
                                .contains(&self.workspace_search.to_ascii_lowercase())
                    })
                {
                    continue;
                }
                let reveal = if self.workspace_search.is_empty() {
                    self.folder_reveal_amount(folder)
                } else {
                    1.0
                };
                row_y += 28.0 * reveal;
                if reveal < 0.08 {
                    continue;
                }
                let expanded = !self.collapsed_folders.contains(folder);
                let arrow_progress = self
                    .folder_tree_motion
                    .get(folder)
                    .map(|motion| motion.0)
                    .unwrap_or(if expanded { 1.0 } else { 0.0 });
                self.chevron_progress(s(18), s(row_y as i32 - 5), arrow_progress);
                self.text(self.colors.text, s(34), s(row_y as i32), folder);
            }
            for (index, file) in self.files.iter().enumerate().filter(|(_, file)| {
                self.workspace_search.is_empty()
                    || self
                        .file_tree_path(file)
                        .to_ascii_lowercase()
                        .contains(&self.workspace_search.to_ascii_lowercase())
            }) {
                let reveal = if self.workspace_search.is_empty() {
                    self.file_reveal_amount(file)
                } else {
                    1.0
                };
                row_y += 29.0 * reveal;
                if reveal < 0.08 {
                    continue;
                }
                let y = row_y as i32;
                if s(y) > self.height - s(65) {
                    break;
                }
                if index == self.selected_file {
                    self.rect(self.colors.panel_2, s(7), s(y - 21), width - s(14), s(27));
                }
                let color = if index == self.selected_file {
                    self.colors.bright
                } else {
                    self.colors.muted
                };
                self.text(color, s(45), s(y), &self.file_tree_path(file));
            }
        }
    }

    unsafe fn draw_workspace_context_menu(&self) {
        let Some((x, y)) = self.workspace_context_menu else {
            return;
        };
        let s = |v| self.s(v);
        let w = s(190);
        let h = s(92);
        self.floating_panel(x, y, w, h);
        self.text(
            self.colors.muted,
            x + s(14),
            y + s(22),
            self.tr("CREATE HERE", "在此新建"),
        );
        self.text(
            self.colors.text,
            x + s(16),
            y + s(51),
            self.tr("New File", "新建文件"),
        );
        self.text(
            self.colors.text,
            x + s(16),
            y + s(79),
            self.tr("New Folder", "新建文件夹"),
        );
    }

    unsafe fn draw_settings_menu(&self) {
        let s = |v| self.s(v);
        let x = self.width - s(410);
        let y = s(43) + self.overlay_offset(self.settings_motion);
        let w = s(390);
        let h = s(830);
        self.floating_panel(x, y, w, h);
        self.text(
            self.colors.bright,
            x + s(16),
            y + s(27),
            self.tr("SETTINGS", "设置"),
        );
        self.text(
            self.colors.muted,
            x + s(16),
            y + s(58),
            self.tr("Interface language", "界面语言"),
        );
        unsafe {
            self.draw_settings_control(
                SettingsControl::LanguageEnglish,
                "English",
                !self.ui_chinese,
            );
            self.draw_settings_control(
                SettingsControl::LanguageChinese,
                "简体中文",
                self.ui_chinese,
            );
        }
        self.text(
            self.colors.muted,
            x + s(16),
            y + s(127),
            self.tr("Theme", "主题"),
        );
        unsafe {
            self.draw_settings_control(
                SettingsControl::ThemeDark,
                self.tr("Dark", "深色"),
                !self.light_theme,
            );
            self.draw_settings_control(
                SettingsControl::ThemeLight,
                self.tr("Light", "浅色"),
                self.light_theme,
            );
        }
        self.line(
            self.colors.line,
            1,
            (x + s(12), y + s(153)),
            (x + w - s(12), y + s(153)),
        );
        self.text(
            self.colors.muted,
            x + s(16),
            y + s(187),
            self.tr("Editor font family", "编辑器字体"),
        );
        self.rect(self.colors.bg, x + s(180), y + s(163), s(192), s(34));
        self.outline(
            if self.font_search_focused {
                self.colors.accent
            } else {
                self.colors.line
            },
            x + s(180),
            y + s(163),
            s(192),
            s(34),
        );
        self.text(
            self.colors.muted,
            x + s(194),
            y + s(187),
            if self.font_search.is_empty() {
                self.tr("Search fonts", "搜索字体")
            } else {
                &self.font_search
            },
        );
        for (visible, index) in self.filtered_font_indices().into_iter().take(5).enumerate() {
            let family = CODE_FONT_FAMILIES[index];
            let row = y + s(226 + visible as i32 * 30);
            if index == self.code_font_family {
                self.rect(
                    self.colors.project,
                    x + s(10),
                    row - s(20),
                    w - s(20),
                    s(26),
                );
            }
            self.text(
                if index == self.code_font_family {
                    self.colors.bright
                } else {
                    self.colors.text
                },
                x + s(20),
                row,
                family,
            );
            if index == self.code_font_family {
                self.text(self.colors.accent, x + w - s(28), row, "o");
            }
        }
        self.line(
            self.colors.line,
            1,
            (x + s(12), y + s(376)),
            (x + w - s(12), y + s(376)),
        );
        self.text(
            self.colors.muted,
            x + s(16),
            y + s(408),
            self.tr("Editor font size", "编辑器字号"),
        );
        self.outline(self.colors.line, x + s(180), y + s(386), s(34), s(28));
        self.text(self.colors.text, x + s(192), y + s(407), "-");
        self.outline(self.colors.line, x + s(300), y + s(386), s(34), s(28));
        self.text(self.colors.text, x + s(312), y + s(407), "+");
        self.text(
            self.colors.bright,
            x + s(246),
            y + s(407),
            &(self.code_font_base + self.code_zoom * 2)
                .clamp(10, 36)
                .to_string(),
        );
        self.text(
            self.colors.muted,
            x + s(16),
            y + s(444),
            self.tr("Ctrl+wheel also changes size", "Ctrl+滚轮也可调整字号"),
        );
        self.line(
            self.colors.line,
            1,
            (x + s(12), y + s(458)),
            (x + w - s(12), y + s(458)),
        );
        self.text(
            self.colors.muted,
            x + s(16),
            y + s(489),
            self.tr("Editor background", "编辑器背景"),
        );
        unsafe {
            self.draw_settings_control(
                SettingsControl::BackgroundChoose,
                self.tr("CHOOSE PNG", "选择 PNG"),
                false,
            );
            self.draw_settings_control(
                SettingsControl::BackgroundClear,
                self.tr("CLEAR", "清除"),
                false,
            );
        }
        self.text(
            self.colors.muted,
            x + s(16),
            y + s(529),
            self.tr("Opacity", "不透明度"),
        );
        unsafe {
            self.draw_settings_control(SettingsControl::OpacityDown, "-", false);
            self.draw_settings_control(SettingsControl::OpacityUp, "+", false);
        }
        self.text(
            self.colors.bright,
            x + s(218),
            y + s(529),
            &format!("{}%", (self.background_opacity * 100.0).round() as i32),
        );
        self.text(
            self.colors.muted,
            x + s(16),
            y + s(569),
            self.tr("Crop position", "取景位置"),
        );
        unsafe {
            for (control, label) in [
                (SettingsControl::CropLeft, "←"),
                (SettingsControl::CropRight, "→"),
                (SettingsControl::CropUp, "↑"),
                (SettingsControl::CropDown, "↓"),
            ] {
                self.draw_settings_control(control, label, false);
            }
        }
        self.text(
            self.colors.muted,
            x + s(16),
            y + s(609),
            self.tr("Image zoom", "图片缩放"),
        );
        unsafe {
            self.draw_settings_control(SettingsControl::ZoomDown, "-", false);
            self.draw_settings_control(SettingsControl::ZoomUp, "+", false);
        }
        self.text(
            self.colors.bright,
            x + s(218),
            y + s(609),
            &format!("{:.1}x", self.background_zoom),
        );
        let background_name = self
            .background_path
            .as_ref()
            .and_then(|path| path.file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.tr("No image selected", "未选择图片").to_string());
        self.text(self.colors.muted, x + s(16), y + s(648), &background_name);
        if let Some(image) = self.background_image_size() {
            unsafe { self.draw_background_preview(image) };
        } else {
            let (px, py, width, height) = self.background_preview_rect();
            self.outline(self.colors.line, px, py, width, height);
            self.text(
                self.colors.muted,
                px + s(12),
                py + s(24),
                self.tr("Choose a PNG to crop it here", "选择 PNG 后可在这里取景"),
            );
        }
    }

    unsafe fn draw_new_menu(&self) {
        let s = |v| self.s(v);
        let x = s(112);
        let y = s(43) + self.overlay_offset(self.new_menu_motion);
        let w = s(300);
        let rows = self.file_menu_rows();
        let height = s(FILE_MENU_ROW * rows.len() as i32 + 12);
        self.floating_panel(x, y, w, height);
        for (index, row) in rows.iter().enumerate() {
            let row_y = self.file_menu_row_y(index);
            match row {
                FileMenuRow::Heading(english, chinese) => {
                    let label = if self.ui_chinese { *chinese } else { *english };
                    self.text(self.colors.muted, x + s(15), row_y, label);
                }
                FileMenuRow::Create(kind) => {
                    let label = match (kind, self.ui_chinese) {
                        (CreateKind::Project, true) => "新建项目",
                        (CreateKind::Project, false) => "New Project",
                        (CreateKind::File, true) => "新建文件",
                        (CreateKind::File, false) => "New File",
                        (CreateKind::Folder, true) => "新建文件夹",
                        (CreateKind::Folder, false) => "New Folder",
                    };
                    self.text(self.colors.text, x + s(18), row_y, label);
                    self.text(self.colors.muted, x + w - s(28), row_y, "+");
                }
                FileMenuRow::OpenFile => {
                    self.text(
                        self.colors.text,
                        x + s(18),
                        row_y,
                        self.tr("Open File…", "打开文件…"),
                    );
                }
                FileMenuRow::OpenFolder => {
                    self.text(
                        self.colors.text,
                        x + s(18),
                        row_y,
                        self.tr("Open Folder…", "打开文件夹…"),
                    );
                }
                FileMenuRow::Recent(path) => {
                    let label = path
                        .file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                        .unwrap_or_else(|| path.display().to_string());
                    self.text(self.colors.text, x + s(18), row_y, &label);
                    let parent = path
                        .parent()
                        .map(|value| value.display().to_string())
                        .unwrap_or_default();
                    let count = parent.chars().count();
                    let short = if count > 22 {
                        let tail: String = parent.chars().skip(count - 21).collect();
                        format!("…{tail}")
                    } else {
                        parent
                    };
                    self.text(self.colors.muted, x + s(130), row_y, &short);
                }
            }
        }
    }

    unsafe fn draw_edit_menu(&self) {
        let s = |v| self.s(v);
        let x = s(207);
        let y = s(43) + self.overlay_offset(self.edit_menu_motion);
        let w = s(260);
        let h = s(244);
        self.floating_panel(x, y, w, h);
        let labels = if self.ui_chinese {
            [
                "撤销",
                "重做",
                "保存",
                "全选",
                "复制",
                "粘贴",
                "增大代码字号",
                "减小代码字号",
            ]
        } else {
            [
                "Undo",
                "Redo",
                "Save",
                "Select All",
                "Copy",
                "Paste",
                "Larger Code",
                "Smaller Code",
            ]
        };
        for (index, (label, key)) in labels
            .iter()
            .zip([
                "Ctrl+Z", "Ctrl+Y", "Ctrl+S", "Ctrl+A", "Ctrl+C", "Ctrl+V", "Ctrl++", "Ctrl+-",
            ])
            .enumerate()
        {
            let row = y + s(30 + index as i32 * 27);
            self.text(self.colors.text, x + s(16), row, label);
            self.text(self.colors.muted, x + w - s(80), row, key);
        }
    }

    unsafe fn draw_view_menu(&self) {
        let s = |v| self.s(v);
        let x = s(302);
        let y = s(43) + self.overlay_offset(self.view_menu_motion);
        let w = s(280);
        // One row per panel, plus the heading and the padding.
        let h = s(51 + 36 * 3);
        self.floating_panel(x, y, w, h);
        self.text(
            self.colors.muted,
            x + s(15),
            y + s(23),
            self.tr("PANELS", "栏位"),
        );
        for (index, (label, enabled)) in [
            (self.tr("Call hierarchy", "调用层级"), self.graph_visible),
            (self.tr("Terminal", "终端"), self.terminal_open),
            (self.tr("Debug panel", "调试面板"), self.debug_panel_open),
        ]
        .iter()
        .enumerate()
        {
            let cy = y + s(51 + index as i32 * 36);
            self.text(self.colors.text, x + s(18), cy + s(5), label);
            let sx = x + w - s(64);
            self.rect(
                if *enabled {
                    self.colors.accent
                } else {
                    self.colors.line
                },
                sx,
                cy - s(13),
                s(46),
                s(24),
            );
            let progress = if index == 0 {
                self.graph_motion.value
            } else {
                self.terminal_motion.value
            }
            .clamp(0.0, 1.0);
            let thumb_x = sx + s(12) + (s(22) as f32 * progress) as i32;
            let stretch = ((progress * std::f32::consts::PI).sin() * s(3) as f32) as i32;
            self.rounded_rect(
                self.colors.bright,
                thumb_x - s(8) - stretch,
                cy - s(9),
                s(16) + stretch * 2,
                s(16),
                s(8),
            );
        }
    }

    unsafe fn draw_editor(&self) {
        let s = |v| self.s(v);
        let left = (self.width as f32 * 0.17) as i32;
        let right = self.graph_left();
        self.rect(
            self.colors.bg,
            left + 1,
            s(46),
            right - left - 1,
            self.height - s(70),
        );
        self.draw_background_region(left + 1, s(84), right - left - 1, self.height - s(154));
        self.rect(
            self.colors.panel_2,
            left + 1,
            s(46),
            right - left - 1,
            s(38),
        );
        self.line(self.colors.line, 1, (left, s(84)), (right, s(84)));
        self.text(
            self.colors.bright,
            left + s(18),
            s(71),
            &self.files[self.selected_file].name,
        );
        let debug_x = left + s(174);
        for (index, label) in ["●  BP", "▶  F5", "→  F10", "■  STOP"].iter().enumerate() {
            let bx = debug_x + index as i32 * s(78);
            self.outline(self.colors.line, bx, s(52), s(72), s(25));
            self.text(
                if (index == 1 && self.debug_session.is_some())
                    || (index == 0
                        && self
                            .breakpoints
                            .lines(&self.active_breakpoint_path())
                            .contains(&self.caret_line))
                {
                    self.colors.accent
                } else {
                    self.colors.muted
                },
                bx + s(7),
                s(70),
                label,
            );
        }
        let language = self.language_for_file(&self.files[self.selected_file].name);
        self.text(self.colors.muted, right - s(150), s(71), &language);
        XSetFont(self.display, self.gc, (*self.code_font).fid);
        let detected = self.detected_language_for_file(&self.files[self.selected_file].name);
        let base_y = s(119);
        let step = self.code_line_step();
        let show_bubbles = !(self.mouse_x >= 0 && self.mouse_x < left);
        // A folded block takes one row on screen, so the row is counted
        // separately from the line in the file.
        let hidden = self.folded_lines();
        let fold_starts = self
            .fold_regions()
            .into_iter()
            .map(|region| region.start_line)
            .collect::<Vec<_>>();
        let mut row = 0i32;
        for (index, line) in self.files[self.selected_file].lines.iter().enumerate() {
            if hidden.contains(&index) {
                continue;
            }
            let y = base_y + row * step;
            row += 1;
            if y > self.height - s(65) {
                break;
            }
            self.text(self.colors.line, left + s(15), y, &(index + 1).to_string());
            // A block that can be folded says so beside its number, and a
            // closed one shows the arrow pointing the other way.
            if fold_starts.contains(&index) {
                let closed = self.folded.contains(&index);
                let marker = if closed { ">" } else { "v" };
                self.text(self.colors.muted, left + s(4), y, marker);
            }
            // A marker in the gutter for a line the server complained about,
            // in the colour of the worst severity on it.
            if self.diagnostics_on_line(index) > 0 {
                let colour = match self.worst_severity_on_line(index) {
                    Some(lsp_core::DiagnosticSeverity::Error) => self.colors.preprocessor,
                    Some(lsp_core::DiagnosticSeverity::Warning) => self.colors.literal,
                    _ => self.colors.muted,
                };
                self.rect(colour, left + s(4), y - s(14), s(4), s(14));
            }
            if let Some(conditional) = self.breakpoint_mark(index) {
                // A conditional breakpoint may not stop at all, so it is drawn
                // in a different colour from one that always stops, with a
                // second ring around it.
                let dot = if conditional {
                    self.colors.keyword
                } else {
                    self.colors.preprocessor
                };
                self.circle(dot, dot, left + s(42), y - s(6), s(5));
                if conditional {
                    self.circle(
                        self.colors.keyword,
                        self.colors.keyword,
                        left + s(42),
                        y - s(6),
                        s(9),
                    );
                }
            }
            let color = if line.contains("fn ") {
                self.colors.accent
            } else if line.contains("struct ") || line.starts_with("impl") {
                self.colors.bright
            } else {
                self.colors.text
            };
            if let (Some(a), Some(b)) = (self.selection_anchor, self.selection_head) {
                let (start, end) = if a <= b { (a, b) } else { (b, a) };
                if index >= start.0 && index <= end.0 {
                    let from = if index == start.0 { start.1 } else { 0 };
                    let to = if index == end.0 { end.1 } else { line.len() };
                    let sx = left + s(52) + self.code_text_width(&line[..from]);
                    let sw = self.code_text_width(&line[from..to]).max(s(4));
                    self.rect(self.colors.project, sx, y - s(20), sw, s(25));
                }
            }
            if let Some((hover_line, start, end)) = self.hovered_code_span {
                if hover_line == index {
                    let (bx, by, bw, bh) = self.code_symbol_bounds(index, start, end);
                    self.rect(self.colors.project, bx, by, bw, bh);
                    self.outline(self.colors.bright, bx, by, bw, bh);
                }
            }
            // The bracket under the caret and the one it pairs with, drawn as
            // an outline so a reader can see where a block ends without
            // counting braces.
            if let Some(pair) = self.matched_brackets {
                for (at_line, at_column, matched) in [
                    (pair.open.0, pair.open.1, pair.caret_on_open),
                    (pair.close.0, pair.close.1, !pair.caret_on_open),
                ] {
                    if at_line != index {
                        continue;
                    }
                    let text_line = &self.files[self.selected_file].lines[at_line];
                    let before = char_to_byte_index(text_line, at_column);
                    let width = text_line[before..]
                        .chars()
                        .next()
                        .map(|character| character.len_utf8())
                        .unwrap_or(1);
                    let bx = left + s(52) + self.code_text_width(&text_line[..before]);
                    let bw = self
                        .code_text_width(&text_line[before..before + width])
                        .max(s(4));
                    // The bracket the caret is on is filled; its partner is
                    // only outlined, so which is which stays readable.
                    if matched {
                        self.rect(self.colors.accent, bx - s(1), y - s(18), bw + s(2), s(22));
                    } else {
                        self.outline(self.colors.bright, bx - s(1), y - s(18), bw + s(2), s(22));
                    }
                }
            }
            if let Some(comment) = self.comment_parts(line) {
                self.highlighted_text(left + s(52), y, comment.code, detected);
                let raw_anchor = left + s(58) + self.code_text_width(comment.code);
                if show_bubbles {
                    let expanded = self
                        .expanded_comments
                        .contains(&(self.selected_file, index));
                    let editing = self.editor_focused
                        && self.caret_line == index
                        && self.caret_column >= comment.content_column;
                    self.comment_bubble(
                        raw_anchor,
                        y,
                        comment.text,
                        expanded,
                        editing,
                        right - s(18),
                    );
                } else {
                    self.text(self.colors.accent, raw_anchor, y, "[comment]");
                }
            } else {
                let _ = color;
                if let Some(pulse) = self.edit_pulses.on_line(index).filter(|pulse| {
                    pulse.start <= pulse.end
                        && pulse.end <= line.len()
                        && line.is_char_boundary(pulse.start)
                        && line.is_char_boundary(pulse.end)
                }) {
                    let eased = pulse.eased();
                    let code_x = left + s(52);
                    let before = &line[..pulse.start];
                    let inserted = &line[pulse.start..pulse.end];
                    let after = &line[pulse.end..];
                    self.highlighted_text(code_x, y, before, detected);
                    let insert_x = code_x + self.code_text_width(before);
                    let lift = (eased * s(9) as f32) as i32;
                    self.pango_text(self.colors.bright, insert_x, y + lift, inserted, true);
                    self.highlighted_text(
                        insert_x + self.code_text_width(inserted),
                        y,
                        after,
                        detected,
                    );
                    if pulse.removes() {
                        // A deletion leaves nothing to lift, so the gap itself
                        // is marked while the highlight fades.
                        let height = (eased * s(16) as f32) as i32;
                        self.line(
                            self.colors.accent,
                            2,
                            (insert_x, y - height),
                            (insert_x, y + s(2)),
                        );
                    } else {
                        let trail = (eased * self.code_text_width(inserted) as f32) as i32;
                        if trail > 1 {
                            self.line(
                                self.colors.accent,
                                1,
                                (insert_x, y + s(7)),
                                (insert_x + trail, y + s(7)),
                            );
                        }
                    }
                } else {
                    self.highlighted_text(left + s(52), y, line, detected);
                }
            }
            if self.editor_focused && index == self.caret_line && blink_phase(self.started) {
                let caret_x = left
                    + s(52)
                    + self.code_text_width(
                        &self.files[self.selected_file].lines[index][..self.caret_column],
                    );
                self.line(
                    self.colors.bright,
                    1,
                    (caret_x, y - s(18)),
                    (caret_x, y + s(4)),
                );
            }
        }
        XSetFont(self.display, self.gc, (*self.font).fid);
        self.draw_debug_values(left, right, base_y, step);
        let hint = if self.editor_focused {
            self.tr(
                "EDITOR ACTIVE - type, Backspace, Enter, arrows",
                "编辑器已激活 - 支持输入、退格、回车和方向键",
            )
        } else {
            self.tr(
                "Click editor to type - hover symbols to trace",
                "点击编辑器开始输入；悬停符号可查看关系",
            )
        };
        self.text(self.colors.muted, left + s(20), self.height - s(50), hint);
        if self.editor_focused {
            let completions = self.completions();
            if !completions.is_empty() {
                let popup_x = left
                    + s(52)
                    + self.code_text_width(
                        &self.files[self.selected_file].lines[self.caret_line][..self.caret_column],
                    );
                let popup_y =
                    (base_y + (self.caret_line as i32 + 1) * step).min(self.height - s(180));
                let popup_w = s(210);
                let popup_h = s(30) * completions.len() as i32 + self.s(10);
                self.rect(self.colors.panel_2, popup_x, popup_y, popup_w, popup_h);
                self.outline(self.colors.line, popup_x, popup_y, popup_w, popup_h);
                for (index, item) in completions.iter().enumerate() {
                    if index == 0 {
                        self.rect(
                            self.colors.project,
                            popup_x + s(3),
                            popup_y + s(3),
                            popup_w - s(6),
                            s(27),
                        );
                    }
                    self.text(
                        if index == 0 {
                            self.colors.bright
                        } else {
                            self.colors.text
                        },
                        popup_x + s(12),
                        popup_y + s(23) + index as i32 * s(30),
                        item,
                    );
                }
                self.text(
                    self.colors.muted,
                    popup_x + popup_w - s(44),
                    popup_y + s(23),
                    "TAB",
                );
            }
        }
    }

    unsafe fn draw_debug_values(&self, left: i32, right: i32, base_y: i32, step: i32) {
        let Some(frame) = self.debug_state.frames.first() else {
            return;
        };
        let line_index = frame
            .line
            .min(self.files[self.selected_file].lines.len().saturating_sub(1));
        let line = &self.files[self.selected_file].lines[line_index];
        let y = base_y + line_index as i32 * step;
        let mut lane = 0;
        for variable in self.debug_state.variables.iter().take(6) {
            let Some((start, end, _)) = editor_core::identifier_ranges(line)
                .into_iter()
                .find(|(_, _, name)| name == &variable.name)
            else {
                continue;
            };
            let anchor_x = left + self.s(52) + self.code_text_width(&line[..start]);
            let anchor_w = self.code_text_width(&line[start..end]);
            let label = format!("{} = {}", variable.name, variable.value);
            let label_w = (self.pango_text_size(&label, false).0 + self.s(20)).min(self.s(250));
            let label_x = (anchor_x + anchor_w / 2 - label_w / 2)
                .clamp(left + self.s(48), right - label_w - self.s(12));
            let label_y = y - self.s(35 + lane * 27);
            self.rect(
                self.colors.panel_2,
                label_x,
                label_y - self.s(18),
                label_w,
                self.s(23),
            );
            self.outline(
                self.colors.function,
                label_x,
                label_y - self.s(18),
                label_w,
                self.s(23),
            );
            self.text(self.colors.bright, label_x + self.s(9), label_y, &label);
            self.line(
                self.colors.function,
                1,
                (label_x + label_w / 2, label_y + self.s(5)),
                (anchor_x + anchor_w / 2, y - self.s(19)),
            );
            lane = (lane + 1).min(2);
        }
        let gutter_x = left + self.s(43);
        self.line(
            self.colors.accent,
            2,
            (gutter_x, base_y - self.s(17)),
            (gutter_x, y - self.s(6)),
        );
        self.circle(
            self.colors.bright,
            self.colors.accent,
            gutter_x,
            y - self.s(6),
            self.s(5),
        );
    }

    unsafe fn draw_graph(&self) {
        let s = |v| self.s(v);
        let (x, y, w, h) = self.graph_bounds();
        self.rect(self.colors.panel, x, s(46), w, self.height - s(70));
        self.line(self.colors.line, 1, (x, s(46)), (x, self.height - s(24)));
        self.line(self.colors.line, 1, (x, s(84)), (self.width, s(84)));
        self.disclosure(
            x + s(10),
            s(58),
            self.graph_visible,
            self.graph_toggle_hovered(),
            1.0,
        );
        self.text(
            self.colors.text,
            x + s(38),
            s(71),
            self.tr("Call Hierarchy", "调用层级"),
        );
        let mode_labels = [
            self.tr("AUTO", "跟随"),
            self.tr("CALLERS", "调用者"),
            self.tr("CALLEES", "被调用"),
        ];
        let mode_w = s(73);
        let modes_x = self.width - mode_w * 3 - s(12);
        for (index, label) in mode_labels.iter().enumerate() {
            let bx = modes_x + index as i32 * mode_w;
            if self.graph_relation.index() == index {
                self.rect(self.colors.project, bx, s(52), mode_w - s(4), s(25));
            }
            self.outline(self.colors.line, bx, s(52), mode_w - s(4), s(25));
            self.text(self.colors.muted, bx + s(8), s(70), label);
        }
        if !self.graph_current {
            let cx = x + w / 2;
            let cy = y + h / 2;
            self.outline(self.colors.accent, cx - s(120), cy - s(38), s(240), s(76));
            self.text(
                self.colors.bright,
                cx - s(96),
                cy - s(6),
                &self.project_name,
            );
            self.text(
                self.colors.muted,
                cx - s(96),
                cy + s(19),
                self.tr(
                    "new project - semantic index is empty",
                    "新项目 - 语义索引为空",
                ),
            );
            self.text(
                self.colors.muted,
                x + s(14),
                y + h - s(10),
                self.tr(
                    "Graph cleared when the active project changed",
                    "切换项目后已清空旧关系图",
                ),
            );
            return;
        }
        let graph = self.graph();
        let focus_label = self
            .hovered_symbol
            .as_ref()
            .map(|symbol| symbol.name.clone())
            .unwrap_or_else(|| self.files[self.selected_file].name.clone());
        let scope_labels = self.graph_scope_labels();
        let count = scope_labels.len() as i32;
        let scope_w = ((w - s(36)) / count.max(1)).max(s(56));
        for (index, label) in scope_labels.iter().enumerate() {
            let bx = x + s(14) + index as i32 * scope_w;
            let selected = index == self.graph_scope_index();
            if selected {
                self.rect(self.colors.project, bx, y + s(8), scope_w - s(5), s(27));
            }
            self.outline(self.colors.line, bx, y + s(8), scope_w - s(5), s(27));
            self.text(self.colors.text, bx + s(7), y + s(27), label);
        }
        self.rect(self.colors.bg, x + s(14), y + s(43), w - s(28), s(30));
        self.outline(
            if self.graph_search_focused {
                self.colors.accent
            } else {
                self.colors.line
            },
            x + s(14),
            y + s(43),
            w - s(28),
            s(30),
        );
        self.text(
            self.colors.muted,
            x + s(26),
            y + s(64),
            if self.graph_search.is_empty() {
                self.tr("Search symbols in this scope", "搜索当前范围内的符号")
            } else {
                &self.graph_search
            },
        );
        self.text(
            self.colors.muted,
            x + s(18),
            y + s(98),
            &format!(
                "{}  /  {}  /  {}",
                self.project_name, self.files[self.selected_file].name, focus_label
            ),
        );
        let visible = self.graph_visible(&graph);
        let first_y = y + s(121);
        if let Some(last) = visible.last() {
            let (_, last_y) = self.node_position(&graph, *last);
            self.line(
                self.colors.accent,
                2,
                (x + s(28), first_y),
                (x + s(28), last_y),
            );
        }
        for index in visible {
            let Some(node) = graph.nodes.get(index) else {
                continue;
            };
            let (nx, ny) = self.node_position(&graph, index);
            let active = self.hovered == Some(index);
            let related = self
                .hovered_symbol
                .as_ref()
                .map(|symbol| node.label.trim_end_matches("()") == symbol.name)
                .unwrap_or(true)
                || node.edge.is_none();
            let card_x = nx + s(12);
            let card_w = (self.width - card_x - s(16)).max(s(120));
            if active {
                self.rect(self.colors.project, card_x, ny - s(19), card_w, s(39));
            }
            self.outline(
                if active {
                    self.colors.accent
                } else {
                    self.colors.line
                },
                card_x,
                ny - s(19),
                card_w,
                s(39),
            );
            self.rect(
                if related {
                    self.colors.accent
                } else {
                    self.colors.line
                },
                nx - s(7),
                ny - s(8),
                s(19),
                s(17),
            );
            let marker = match node.level {
                graph_core::GraphLevel::Project => "P",
                graph_core::GraphLevel::Folder => "D",
                graph_core::GraphLevel::File => "F",
                graph_core::GraphLevel::Class => "C",
                _ => "fn",
            };
            self.text(self.colors.bg, nx - s(4), ny + s(5), marker);
            let label_color = if related {
                self.colors.bright
            } else {
                self.colors.muted
            };
            self.text(label_color, card_x + s(12), ny + s(5), &node.caption());
            if node.has_children() {
                self.text(
                    self.colors.accent,
                    card_x + s(2),
                    ny + s(5),
                    graph_core::marker_glyph(node),
                );
            }
            let detail_width = self.pango_text_size(&node.detail, false).0;
            self.text(
                self.colors.muted,
                (self.width - detail_width - s(26)).max(card_x + s(160)),
                ny + s(5),
                &node.detail,
            );
        }
        self.text(
            self.colors.muted,
            x + s(14),
            y + h - s(10),
            &format!(
                "{}  /  {}",
                graph.footer(self.graph_scroll, self.graph_rows(&graph)),
                self.tr(
                    "hover a symbol: focus  |  click marker: fold  |  enter: jump",
                    "悬停符号：聚焦  |  点击标记：折叠  |  回车：跳转",
                )
            ),
        );
    }

    unsafe fn draw_code_relations(&self) {
        let Some((line, start_byte, end_byte)) = self.hovered_code_span else {
            return;
        };
        let s = |v| self.s(v);
        let editor_left = (self.width as f32 * 0.17) as i32;
        let symbol = &self.files[self.selected_file].lines[line][start_byte..end_byte];
        if symbol.len() < 2 {
            return;
        }
        let (box_x, box_y, _box_w, box_h) = self.code_symbol_bounds(line, start_byte, end_byte);
        let start = (box_x, box_y + box_h / 2);
        let gutter = editor_left + s(27);
        self.circle(
            self.colors.panel_2,
            self.colors.bright,
            start.0,
            start.1,
            s(4),
        );
        self.line(self.colors.accent, 2, start, (gutter, start.1));
        let mut drawn = 0;
        for (file_index, file) in self.files.iter().enumerate() {
            let mut file_has_relation = false;
            for (target_line, value) in file.lines.iter().enumerate() {
                for (target_start, _) in value.match_indices(symbol) {
                    let target_end = target_start + symbol.len();
                    if !identifier_match_is_exact(value, target_start, target_end) {
                        continue;
                    }
                    if file_index == self.selected_file
                        && target_line == line
                        && target_start == start_byte
                    {
                        continue;
                    }
                    file_has_relation = true;
                    if file_index != self.selected_file || drawn >= 12 {
                        continue;
                    }
                    let (target_x, target_y, target_w, target_h) =
                        self.code_symbol_bounds(target_line, target_start, target_end);
                    let end = (target_x, target_y + target_h / 2);
                    self.line(self.colors.accent, 1, (gutter, start.1), (gutter, end.1));
                    self.line(
                        self.colors.accent,
                        1,
                        (gutter, end.1),
                        (end.0 - s(7), end.1),
                    );
                    self.outline(self.colors.accent, target_x, target_y, target_w, target_h);
                    let role = symbol_role(value, symbol);
                    self.text(
                        self.colors.muted,
                        target_x + target_w + s(8),
                        target_y + s(19),
                        self.tr(role.0, role.1),
                    );
                    let mut arrow = [
                        XPoint {
                            x: end.0 as i16,
                            y: end.1 as i16,
                        },
                        XPoint {
                            x: (end.0 - s(8)) as i16,
                            y: (end.1 - s(4)) as i16,
                        },
                        XPoint {
                            x: (end.0 - s(8)) as i16,
                            y: (end.1 + s(4)) as i16,
                        },
                    ];
                    self.set_color(self.colors.accent);
                    XFillPolygon(
                        self.display,
                        self.canvas,
                        self.gc,
                        arrow.as_mut_ptr(),
                        3,
                        0,
                        0,
                    );
                    drawn += 1;
                }
            }
            if file_index == self.selected_file || !file_has_relation {
                continue;
            }
            let Some(tree_y) = self.file_tree_row(file_index) else {
                continue;
            };
            let tree_right = editor_left - s(9);
            self.line(
                self.colors.accent,
                1,
                (gutter, start.1),
                (editor_left, start.1),
            );
            self.line(
                self.colors.accent,
                1,
                (editor_left, start.1),
                (editor_left, tree_y),
            );
            self.line(
                self.colors.accent,
                1,
                (editor_left, tree_y),
                (tree_right, tree_y),
            );
            self.outline(
                self.colors.accent,
                s(8),
                tree_y - s(20),
                editor_left - s(17),
                s(26),
            );
            let mut arrow = [
                XPoint {
                    x: tree_right as i16,
                    y: tree_y as i16,
                },
                XPoint {
                    x: (tree_right + s(8)) as i16,
                    y: (tree_y - s(4)) as i16,
                },
                XPoint {
                    x: (tree_right + s(8)) as i16,
                    y: (tree_y + s(4)) as i16,
                },
            ];
            self.set_color(self.colors.accent);
            XFillPolygon(
                self.display,
                self.canvas,
                self.gc,
                arrow.as_mut_ptr(),
                3,
                0,
                0,
            );
        }
        let travel = ((self.started.elapsed().as_secs_f32() * 1.6) % 1.0).clamp(0.0, 1.0);
        let pulse_x = start.0 + ((gutter - start.0) as f32 * travel) as i32;
        self.circle(
            self.colors.bright,
            self.colors.accent,
            pulse_x,
            start.1,
            s(3),
        );
    }

    /// The condition prompt: one line over the editor, with the caret in it.
    ///
    /// # Safety
    /// Requires a live Cairo context.
    unsafe fn draw_condition_prompt(&self) {
        let Some(prompt) = self.breakpoint_prompt.as_ref() else {
            return;
        };
        let s = |v| self.s(v);
        let width = (self.width as f32 * 0.4) as i32;
        let x = (self.width - width) / 2;
        let y = s(96);
        self.rect(self.colors.panel_2, x, y, width, s(46));
        self.outline(self.colors.accent, x, y, width, s(46));
        self.text(
            self.colors.muted,
            x + s(12),
            y + s(19),
            &format!(
                "{} {}",
                self.tr("Breakpoint condition on line", "该行断点条件"),
                self.caret_line + 1
            ),
        );
        let field_x = x + s(12);
        let field_y = y + s(40);
        let value = if prompt.is_empty() {
            self.tr("for example count > 10", "例如 count > 10")
        } else {
            prompt.as_str()
        };
        self.text(
            if prompt.is_empty() {
                self.colors.muted
            } else {
                self.colors.bright
            },
            field_x,
            field_y,
            value,
        );
        if blink_phase(self.started) {
            let prefix: String = prompt.chars().take(self.condition_caret).collect();
            let caret = field_x + self.pango_text_size(&prefix, false).0;
            self.line(
                self.colors.bright,
                1,
                (caret, field_y - s(14)),
                (caret, field_y + s(4)),
            );
        }
    }

    unsafe fn draw_status(&self) {
        let s = |v| self.s(v);
        let terminal_progress = self.terminal_motion.value.clamp(0.0, 1.0);
        let expanded = self.terminal_height.clamp(s(150), self.height - s(150));
        let panel_h = s(28) + ((expanded - s(28)) as f32 * terminal_progress) as i32;
        let top = self.height - panel_h;
        self.rect(self.colors.panel_2, 0, top, self.width, panel_h);
        self.line(self.colors.line, 1, (0, top), (self.width, top));
        // A real disclosure button, instead of a `>` typed into the title.
        self.disclosure(
            s(12),
            top + s(4),
            terminal_progress > 0.5,
            self.terminal_toggle_hovered(),
            terminal_progress,
        );
        self.text(
            self.colors.bright,
            s(40),
            top + s(20),
            self.tr("TERMINAL", "终端"),
        );
        self.text(
            self.colors.muted,
            s(130),
            top + s(20),
            self.project_path
                .as_ref()
                .map(|path| path.to_string_lossy())
                .as_deref()
                .unwrap_or(self.tr("No active project", "没有活动项目")),
        );
        if terminal_progress > 0.08 {
            let list_x = self.width - s(190);
            self.rect(
                self.colors.panel,
                list_x,
                top + s(30),
                s(190),
                panel_h - s(30),
            );
            self.line(
                self.colors.line,
                1,
                (list_x, top + s(30)),
                (list_x, self.height),
            );
            self.outline(self.colors.line, list_x + s(10), top + s(38), s(76), s(27));
            self.text(
                self.colors.text,
                list_x + s(22),
                top + s(58),
                self.tr("+ NEW", "+ 新建"),
            );
            self.outline(self.colors.line, list_x + s(94), top + s(38), s(84), s(27));
            self.text(
                self.colors.text,
                list_x + s(105),
                top + s(58),
                self.tr("POP OUT", "弹出"),
            );
            for (index, terminal) in self.terminals.iter().take(4).enumerate() {
                let row = top + s(79 + index as i32 * 34);
                if index == self.terminal_active {
                    self.rect(
                        self.colors.project,
                        list_x + s(10),
                        row - s(22),
                        s(168),
                        s(29),
                    );
                }
                self.text(
                    if index == self.terminal_active {
                        self.colors.bright
                    } else {
                        self.colors.muted
                    },
                    list_x + s(24),
                    row,
                    &terminal.name,
                );
            }
            if self.terminals.len() > 4 {
                let more_y = top + s(79 + 4 * 34);
                self.outline(
                    self.colors.line,
                    list_x + s(10),
                    more_y - s(22),
                    s(168),
                    s(29),
                );
                self.text(
                    self.colors.text,
                    list_x + s(24),
                    more_y,
                    &format!("{} +{}", self.tr("More", "更多"), self.terminals.len() - 4),
                );
                if self.terminal_menu_open {
                    let menu_h = s(29) * self.terminals.len() as i32;
                    let menu_x = list_x - s(184);
                    self.rect(self.colors.panel, menu_x, top + s(38), s(180), menu_h);
                    self.outline(self.colors.line, menu_x, top + s(38), s(180), menu_h);
                    for (index, item) in self.terminals.iter().enumerate() {
                        self.text(
                            if index == self.terminal_active {
                                self.colors.accent
                            } else {
                                self.colors.text
                            },
                            menu_x + s(12),
                            top + s(60 + index as i32 * 29),
                            &item.name,
                        );
                    }
                }
            }
            let terminal = &self.terminals[self.terminal_active.min(self.terminals.len() - 1)];
            let lines = terminal
                .output
                .lock()
                .map(|lines| lines.clone())
                .unwrap_or_default();
            let rows = self.terminal_visible_rows();
            let first_line = lines.len().saturating_sub(rows);
            let visible = lines.iter().rev().take(6).collect::<Vec<_>>();
            for (offset, line) in visible.iter().rev().enumerate() {
                let max_chars = ((list_x - s(52)) / s(8).max(1)).max(8) as usize;
                let display_line = line.chars().take(max_chars).collect::<String>();
                // A selected cell is filled behind the glyphs, so the choice is
                // visible the way it is in the editor.
                if let Some(selection) = self.terminal_selection.as_ref() {
                    let index = first_line + offset;
                    let (from, to) = selection.ordered();
                    if index >= from.line && index <= to.line {
                        let start = if index == from.line { from.column } else { 0 };
                        let end = if index == to.line {
                            to.column + 1
                        } else {
                            display_line.chars().count()
                        };
                        if end > start {
                            let cell = s(8).max(1);
                            // The same fill the editor uses for a selection.
                            self.rect(
                                self.colors.project,
                                s(24) + start as i32 * cell,
                                top + s(45 + offset as i32 * 24),
                                ((end - start) as i32 * cell).max(2),
                                s(18),
                            );
                        }
                    }
                }
                self.text(
                    self.colors.text,
                    s(24),
                    top + s(55 + offset as i32 * 24),
                    &display_line,
                );
            }
            if self.terminal_selection.is_some() {
                let selected = self
                    .terminal_selection
                    .as_ref()
                    .and_then(|selection| terminal_core::selected_text(&lines, selection))
                    .map(|text| text.chars().count())
                    .unwrap_or(0);
                if selected > 0 {
                    self.text(
                        self.colors.accent,
                        s(24),
                        self.height - s(18),
                        &format!("{} {selected}", self.tr("selected", "已选中")),
                    );
                }
            }
            self.text(self.colors.accent, s(24), self.height - s(18), "$");
            self.text(
                self.colors.bright,
                s(44),
                self.height - s(18),
                &terminal.input,
            );
            if self.terminal_focused && blink_phase(self.started) {
                let x = s(44) + self.pango_text_size(&terminal.input, false).0;
                self.line(
                    self.colors.bright,
                    1,
                    (x, self.height - s(36)),
                    (x, self.height - s(12)),
                );
            }
        }
    }

    unsafe fn draw_package_menu(&self) {
        let s = |v| self.s(v);
        let x = self.width - s(270);
        let y = s(48) + self.overlay_offset(self.package_motion);
        let w = s(250);
        let h = s(207);
        self.floating_panel(x, y, w, h);
        self.text(
            self.colors.muted,
            x + s(15),
            y + s(25),
            self.tr("BUILD TARGETS", "打包目标"),
        );
        let labels = [
            "Windows  .exe",
            "Linux    .deb",
            "Linux    .AppImage",
            "macOS    .dmg",
        ];
        for (index, label) in labels.iter().enumerate() {
            let row_y = y + s(51 + index as i32 * 31);
            self.outline(
                if self.package_targets[index] {
                    self.colors.accent
                } else {
                    self.colors.line
                },
                x + s(15),
                row_y - s(15),
                s(16),
                s(16),
            );
            if self.package_targets[index] {
                self.rect(self.colors.accent, x + s(19), row_y - s(11), s(8), s(8));
            }
            self.text(self.colors.text, x + s(42), row_y, label);
        }
        self.rect(
            self.colors.bright,
            x + s(15),
            y + h - s(37),
            w - s(30),
            s(25),
        );
        self.text(
            self.colors.bg,
            x + s(72),
            y + h - s(19),
            self.tr("BUILD SELECTED", "打包已选项"),
        );
    }

    unsafe fn draw_extensions(&self) {
        let s = |v| self.s(v);
        let w = s(950);
        let h = s(680);
        let x = self.width - w - s(20);
        let y = s(52) + self.overlay_offset(self.extensions_motion);
        self.floating_panel(x, y, w, h);
        self.text(
            self.colors.bright,
            x + s(22),
            y + s(34),
            self.tr("EXTENSIONS", "扩展"),
        );
        self.text(
            self.colors.muted,
            x + s(22),
            y + s(64),
            self.tr("CATEGORY", "分类"),
        );
        let categories = [
            self.tr("Languages", "编程语言"),
            self.tr("Themes", "主题"),
            self.tr("Debuggers", "调试器"),
            self.tr("Formatters", "格式化"),
            self.tr("AI / Agents", "AI / 智能体"),
        ];
        for (index, label) in categories.iter().enumerate() {
            let row = y + s(101 + index as i32 * 43);
            if index == self.extension_category {
                self.rect(self.colors.project, x + s(14), row - s(26), s(150), s(36));
            }
            self.text(
                if index == self.extension_category {
                    self.colors.bright
                } else {
                    self.colors.text
                },
                x + s(25),
                row,
                label,
            );
            self.text(self.colors.muted, x + s(145), row, ">");
        }
        self.line(
            self.colors.line,
            1,
            (x + s(175), y + s(52)),
            (x + s(175), y + h - s(24)),
        );
        if self.extension_category == 0 {
            let list_x = x + s(195);
            self.text(
                self.colors.accent,
                list_x,
                y + s(64),
                self.tr("LANGUAGES  /  EXTENSIONS", "编程语言 / 扩展"),
            );
            self.rect(self.colors.bg, list_x, y + s(82), s(310), s(38));
            self.outline(
                if self.extension_search_focused {
                    self.colors.accent
                } else {
                    self.colors.line
                },
                list_x,
                y + s(82),
                s(310),
                s(38),
            );
            self.text(
                self.colors.text,
                list_x + s(14),
                y + s(108),
                if self.extension_search.is_empty() {
                    self.tr("Search language or suffix", "搜索语言或后缀")
                } else {
                    &self.extension_search
                },
            );
            let filtered = self.filtered_language_indices();
            let visible_count = 10;
            let start = self.extension_scroll.min(filtered.len().saturating_sub(1));
            for (visible, index) in filtered.iter().skip(start).take(visible_count).enumerate() {
                let row = y + s(153 + visible as i32 * 47);
                let selected = self.selected_language == Some(*index);
                if selected {
                    self.rect(self.colors.project, list_x, row - s(27), s(310), s(39));
                }
                self.text(
                    if selected {
                        self.colors.bright
                    } else {
                        self.colors.text
                    },
                    list_x + s(12),
                    row,
                    self.languages[*index].name,
                );
                if self.languages[*index].installed {
                    self.text(
                        self.colors.accent,
                        list_x + s(225),
                        row,
                        self.tr("INSTALLED", "已安装"),
                    );
                }
            }
            let track_x = x + s(511);
            let track_y = y + s(135);
            let track_h = s(472);
            self.rect(self.colors.line, track_x, track_y, s(3), track_h);
            if !filtered.is_empty() {
                let thumb_h = (track_h * visible_count as i32
                    / filtered.len().max(visible_count) as i32)
                    .max(s(34));
                let max_scroll = filtered.len().saturating_sub(visible_count).max(1) as i32;
                let thumb_y = track_y
                    + (track_h - thumb_h) * self.extension_scroll.min(max_scroll as usize) as i32
                        / max_scroll;
                self.rect(self.colors.accent, track_x - s(2), thumb_y, s(7), thumb_h);
            }
            self.line(
                self.colors.line,
                1,
                (x + s(530), y + s(52)),
                (x + s(530), y + h - s(24)),
            );
            if let Some(index) = self.selected_language {
                let pack = &self.languages[index];
                let dx = x + s(555);
                self.text(
                    self.colors.muted,
                    dx,
                    y + s(64),
                    self.tr("LANGUAGE EXTENSION", "语言扩展"),
                );
                self.text(self.colors.bright, dx, y + s(112), pack.name);
                self.text(
                    self.colors.muted,
                    dx,
                    y + s(145),
                    &format!(
                        "{}: {}",
                        self.tr("Files", "文件"),
                        pack.extensions
                            .iter()
                            .map(|ext| format!(".{ext}"))
                            .collect::<Vec<_>>()
                            .join("  ")
                    ),
                );
                self.text(
                    self.colors.muted,
                    dx,
                    y + s(190),
                    self.tr("VERSION", "版本"),
                );
                self.rect(self.colors.bg, dx, y + s(207), s(355), s(40));
                self.outline(
                    if self.version_menu_open {
                        self.colors.accent
                    } else {
                        self.colors.line
                    },
                    dx,
                    y + s(207),
                    s(355),
                    s(40),
                );
                self.text(
                    self.colors.text,
                    dx + s(14),
                    y + s(234),
                    pack.versions[pack.selected_version],
                );
                self.chevron(dx + s(326), y + s(226), self.version_menu_open);
                let downloading = self
                    .downloading
                    .map(|(active, _)| active == index)
                    .unwrap_or(false);
                let action = if downloading {
                    self.tr("DOWNLOADING...", "正在下载……")
                } else if pack.installed {
                    self.tr("REMOVE", "卸载")
                } else {
                    self.tr("DOWNLOAD", "下载")
                };
                self.rect(self.colors.bright, dx, y + s(282), s(355), s(38));
                self.text(self.colors.bg, dx + s(118), y + s(308), action);
                let state = if pack.installed {
                    format!(
                        "{}: {} {}",
                        self.tr("Installed", "已安装"),
                        pack.name,
                        pack.versions[pack.selected_version]
                    )
                } else {
                    format!(
                        "{} {}",
                        self.tr(
                            "Built-in detection active; install tooling for",
                            "已启用内置识别；可安装工具扩展："
                        ),
                        pack.name
                    )
                };
                self.text(self.colors.muted, dx, y + s(353), &state);
                self.text(
                    self.colors.muted,
                    dx,
                    y + s(395),
                    self.tr("Extension layers:", "扩展层级："),
                );
                self.text(
                    self.colors.text,
                    dx + s(16),
                    y + s(425),
                    self.tr(
                        "- suffix detection + precise keywords",
                        "- 后缀识别 + 精确关键字",
                    ),
                );
                self.text(
                    self.colors.text,
                    dx + s(16),
                    y + s(455),
                    self.tr("- project symbol completion", "- 项目符号补全"),
                );
                self.text(
                    self.colors.muted,
                    dx,
                    y + s(485),
                    &format!(
                        "{} {}",
                        self.tr("Runtime cache:", "运行时缓存："),
                        self.runtime_cache_summary()
                    ),
                );
                self.text(
                    self.colors.muted,
                    dx,
                    y + s(515),
                    &format!(
                        "{} {}",
                        self.tr("Tooling:", "工具链："),
                        self.language_tooling_summary()
                    ),
                );
                self.text(
                    self.colors.text,
                    dx + s(16),
                    y + s(545),
                    self.tr(
                        "- LSP / formatter / debugger adapter",
                        "- LSP / 格式化 / 调试器适配",
                    ),
                );
                if self.version_menu_open {
                    self.rect(
                        self.colors.panel,
                        dx,
                        y + s(250),
                        s(355),
                        s(34) * pack.versions.len() as i32,
                    );
                    for (version, value) in pack.versions.iter().enumerate() {
                        let vy = y + s(274 + version as i32 * 34);
                        self.text(
                            if version == pack.selected_version {
                                self.colors.accent
                            } else {
                                self.colors.text
                            },
                            dx + s(14),
                            vy,
                            value,
                        );
                    }
                }
            }
        } else {
            self.text(
                self.colors.accent,
                x + s(195),
                y + s(64),
                categories[self.extension_category],
            );
            if self.extension_category == 1 {
                for (index, label) in [self.tr("Dark", "深色主题"), self.tr("Light", "浅色主题")]
                    .iter()
                    .enumerate()
                {
                    let row = y + s(116 + index as i32 * 52);
                    let active =
                        (index == 0 && !self.light_theme) || (index == 1 && self.light_theme);
                    if active {
                        self.rect(self.colors.project, x + s(195), row - s(29), s(315), s(41));
                    }
                    self.outline(self.colors.line, x + s(195), row - s(29), s(315), s(41));
                    self.text(self.colors.text, x + s(212), row, label);
                    if active {
                        self.text(
                            self.colors.accent,
                            x + s(465),
                            row,
                            self.tr("ACTIVE", "使用中"),
                        );
                    }
                }
                self.text(
                    self.colors.muted,
                    x + s(560),
                    y + s(116),
                    self.tr("Theme changes apply immediately.", "主题切换会立即生效。"),
                );
            } else if self.extension_category == 4 {
                let dx = x + s(195);
                self.text(self.colors.bright, dx, y + s(108), "OpenAI-compatible API");
                for (index, (label, value)) in [
                    (self.tr("ENDPOINT", "接口地址"), self.ai_endpoint.as_str()),
                    (self.tr("MODEL", "模型"), self.ai_model.as_str()),
                    (
                        self.tr("API KEY (session only)", "API KEY（仅本次运行）"),
                        if self.ai_key.is_empty() {
                            ""
                        } else {
                            "****************"
                        },
                    ),
                ]
                .iter()
                .enumerate()
                {
                    let fy = y + s(145 + index as i32 * 92);
                    self.text(self.colors.muted, dx, fy, label);
                    self.rect(self.colors.bg, dx, fy + s(14), s(600), s(42));
                    self.outline(
                        if self.ai_field_focused == Some(index) {
                            self.colors.accent
                        } else {
                            self.colors.line
                        },
                        dx,
                        fy + s(14),
                        s(600),
                        s(42),
                    );
                    self.text(self.colors.text, dx + s(14), fy + s(42), value);
                }
                self.rect(self.colors.accent, dx, y + s(440), s(210), s(40));
                self.text(
                    self.colors.bg,
                    dx + s(43),
                    y + s(467),
                    self.tr("SAVE API SETTINGS", "保存 API 设置"),
                );
                self.text(
                    self.colors.muted,
                    dx,
                    y + s(520),
                    self.tr(
                        "Endpoint and model are saved. API keys are never written to disk.",
                        "接口地址和模型会保存；API Key 不会写入磁盘。",
                    ),
                );
            } else {
                let items = if self.extension_category == 2 {
                    ["Native LLDB", "GDB Adapter", "DAP Bridge"]
                } else {
                    ["Biome", "clang-format", "Black / Ruff"]
                };
                for (index, item) in items.iter().enumerate() {
                    let row = y + s(116 + index as i32 * 52);
                    self.outline(self.colors.line, x + s(195), row - s(29), s(315), s(41));
                    self.text(self.colors.text, x + s(212), row, item);
                    self.text(
                        self.colors.muted,
                        x + s(420),
                        row,
                        self.tr("NOT CONNECTED", "未接入"),
                    );
                }
                self.text(
                    self.colors.bright,
                    x + s(560),
                    y + s(116),
                    self.tr("Adapter status", "适配器状态"),
                );
                self.text(
                    self.colors.muted,
                    x + s(560),
                    y + s(154),
                    self.tr(
                        "These entries are not functional yet. Clicking one now reports its real status.",
                        "这些适配器目前尚未实现；点击后会明确显示真实状态。",
                    ),
                );
            }
        }
        self.text(
            self.colors.muted,
            x + s(22),
            y + h - s(18),
            self.tr(
                "Extensions  >  category  >  item  >  version / configuration",
                "扩展 > 分类 > 项目 > 版本 / 配置",
            ),
        );
    }

    unsafe fn draw_create_dialog(&self) {
        let s = |v| self.s(v);
        let project = self.create_kind == Some(CreateKind::Project);
        let w = s(if project { 680 } else { 520 });
        let h = s(if project { 500 } else { 230 });
        let x = (self.width - w) / 2;
        let y = (self.height - h) / 2 + self.overlay_offset(self.create_motion);
        self.floating_panel(x, y, w, h);
        self.line(self.colors.accent, 2, (x + s(18), y), (x + w - s(18), y));
        let title = match (self.create_kind.unwrap(), self.ui_chinese) {
            (CreateKind::Project, true) => "新建项目",
            (CreateKind::Folder, true) => "新建文件夹",
            (CreateKind::File, true) => "新建文件",
            (CreateKind::Project, false) => "CREATE NEW PROJECT",
            (CreateKind::Folder, false) => "CREATE NEW FOLDER",
            (CreateKind::File, false) => "CREATE NEW FILE",
        };
        self.text(self.colors.bright, x + s(22), y + s(36), title);
        self.text(
            self.colors.muted,
            x + s(22),
            y + s(66),
            if project {
                self.tr(
                    "Choose a language and optional environment",
                    "选择编程语言和可选环境",
                )
            } else if self.create_kind == Some(CreateKind::File) {
                self.tr(
                    "File suffix selects its language automatically",
                    "文件后缀会自动识别编程语言",
                )
            } else {
                self.tr(
                    "Folder will be created inside the active project",
                    "文件夹将在当前项目中创建",
                )
            },
        );
        self.rect(self.colors.bg, x + s(22), y + s(82), w - s(44), s(40));
        self.outline(self.colors.line, x + s(22), y + s(82), w - s(44), s(40));
        let placeholder = if project {
            self.tr("project-name", "项目名称")
        } else if self.create_kind == Some(CreateKind::File) {
            self.tr("filename.ext", "文件名.后缀")
        } else {
            self.tr("folder-name", "文件夹名称")
        };
        self.text(
            self.colors.text,
            x + s(35),
            y + s(109),
            if self.new_file_name.is_empty() {
                placeholder
            } else {
                &self.new_file_name
            },
        );
        if !self.project_location_focused && blink_phase(self.started) {
            let prefix = &self.new_file_name[..self.create_caret.min(self.new_file_name.len())];
            let caret = x + s(35) + self.pango_text_size(prefix, false).0;
            self.line(
                self.colors.bright,
                1,
                (caret, y + s(91)),
                (caret, y + s(114)),
            );
        }
        if project {
            let pack = &self.languages[self.project_language];
            let envs = project_environments(pack.name);
            self.text(
                self.colors.muted,
                x + s(22),
                y + s(157),
                self.tr("PROGRAMMING LANGUAGE", "编程语言"),
            );
            self.rect(self.colors.bg, x + s(22), y + s(171), s(300), s(40));
            self.outline(
                if self.project_language_menu {
                    self.colors.accent
                } else {
                    self.colors.line
                },
                x + s(22),
                y + s(171),
                s(300),
                s(40),
            );
            self.text(
                self.colors.text,
                x + s(36),
                y + s(198),
                &format!("{}  {}", pack.name, pack.versions[pack.selected_version]),
            );
            self.chevron(x + s(294), y + s(190), self.project_language_menu);
            self.text(
                self.colors.muted,
                x + s(350),
                y + s(157),
                self.tr("ENVIRONMENT / TOOLCHAIN", "环境 / 工具链"),
            );
            self.rect(self.colors.bg, x + s(350), y + s(171), s(308), s(40));
            self.outline(
                if self.project_environment_menu {
                    self.colors.accent
                } else {
                    self.colors.line
                },
                x + s(350),
                y + s(171),
                s(308),
                s(40),
            );
            self.text(
                self.colors.text,
                x + s(364),
                y + s(198),
                envs[self.project_environment.min(envs.len() - 1)],
            );
            self.chevron(x + s(630), y + s(190), self.project_environment_menu);
            self.text(
                self.colors.muted,
                x + s(22),
                y + s(247),
                self.tr("PROJECT LOCATION", "项目位置"),
            );
            self.rect(self.colors.bg, x + s(22), y + s(260), s(500), s(40));
            self.outline(
                if self.project_location_focused {
                    self.colors.accent
                } else {
                    self.colors.line
                },
                x + s(22),
                y + s(260),
                s(500),
                s(40),
            );
            self.text(
                self.colors.text,
                x + s(36),
                y + s(287),
                &self.project_location,
            );
            if self.project_location_focused && blink_phase(self.started) {
                let caret = x + s(36) + self.pango_text_size(&self.project_location, false).0;
                self.line(
                    self.colors.bright,
                    1,
                    (caret, y + s(268)),
                    (caret, y + s(292)),
                );
            }
            self.outline(self.colors.line, x + s(532), y + s(260), s(126), s(40));
            self.text(
                self.colors.text,
                x + s(550),
                y + s(287),
                self.tr("BROWSE", "选择位置"),
            );
            self.text(
                self.colors.text,
                x + s(22),
                y + s(333),
                self.tr(
                    "None skips environments. Python runtimes are cached by version and reused.",
                    "选择 None 可跳过虚拟环境；Python 运行时按版本缓存并复用。",
                ),
            );
            let fallback = self.tr("project-name", "项目名称");
            self.text(
                self.colors.muted,
                x + s(22),
                y + s(362),
                &format!(
                    "{}: {}/{}",
                    self.tr("Final path", "最终路径"),
                    self.project_location.trim_end_matches('/'),
                    if self.new_file_name.is_empty() {
                        fallback
                    } else {
                        &self.new_file_name
                    }
                ),
            );
            if self.project_language_menu {
                self.rect(self.colors.panel, x + s(22), y + s(214), s(300), s(34) * 6);
                for (visible, index) in self.languages.iter().enumerate().take(6) {
                    self.text(
                        if visible == self.project_language {
                            self.colors.accent
                        } else {
                            self.colors.text
                        },
                        x + s(36),
                        y + s(238 + visible as i32 * 34),
                        index.name,
                    );
                }
            }
            if self.project_environment_menu {
                self.rect(
                    self.colors.panel,
                    x + s(350),
                    y + s(214),
                    s(308),
                    s(34) * envs.len() as i32,
                );
                for (index, env) in envs.iter().enumerate() {
                    self.text(
                        if index == self.project_environment {
                            self.colors.accent
                        } else {
                            self.colors.text
                        },
                        x + s(364),
                        y + s(238 + index as i32 * 34),
                        env,
                    );
                }
            }
        }
        let button_y = y + h - s(52);
        self.text(
            self.colors.muted,
            x + s(22),
            button_y - s(17),
            &self.status_message,
        );
        self.outline(self.colors.line, x + w - s(238), button_y, s(102), s(34));
        self.text(
            self.colors.text,
            x + w - s(211),
            button_y + s(24),
            self.tr("CANCEL", "取消"),
        );
        self.rect(self.colors.accent, x + w - s(126), button_y, s(104), s(34));
        self.text(
            self.colors.bg,
            x + w - s(101),
            button_y + s(24),
            self.tr("CREATE", "确定"),
        );
    }

    /// Put the app into a known state so a rendered frame shows a scene that
    /// otherwise needs a running build or a click to appear.
    ///
    /// Used by `--render-scene`, which lets the packaging animation be inspected
    /// (and smoke tested in CI) without waiting for a real build.
    fn prime_scene(&mut self, name: &str) -> bool {
        let lines = self.files.iter().map(|file| file.lines.len()).sum();
        let absorb = package_motion::absorb_seconds(lines, 3);
        let ago = |seconds: f32| {
            Instant::now()
                .checked_sub(Duration::from_secs_f32(seconds))
                .unwrap_or_else(Instant::now)
        };
        match name {
            "view" => {
                self.view_menu_open = true;
                self.view_menu_motion = UiMotion::new(1.0);
                self.terminal_open = true;
                self.terminal_motion = UiMotion::new(1.0);
                // Give the terminal output and a selection over part of it, so
                // the frame covers the highlight and the selected count.
                if let Some(terminal) = self.terminals.first() {
                    if let Ok(mut lines) = terminal.output.lock() {
                        *lines = (0..12)
                            .map(|index| format!("$ cargo test --target {index}"))
                            .collect();
                    }
                }
                let rows = self.terminal_visible_rows();
                let first = 12usize.saturating_sub(rows);
                self.terminal_selection = Some(terminal_core::TerminalSelection::new(
                    terminal_core::TerminalPoint {
                        line: first + 1,
                        column: 2,
                    },
                    terminal_core::TerminalPoint {
                        line: first + 3,
                        column: 12,
                    },
                ));
                true
            }
            "folding" => {
                // A file with a folded block, so the marker, the arrow and the
                // missing lines can all be seen at once.
                self.files = vec![FileBuffer::new(
                    "main.rs".into(),
                    PathBuf::from("/tmp/tc-ide-preview/main.rs"),
                    vec![
                        "struct Engine {".into(),
                        "    name: String,".into(),
                        "    power: u32,".into(),
                        "}".into(),
                        "".into(),
                        "impl Engine {".into(),
                        "    fn run(&self) {".into(),
                        "        let a = 1;".into(),
                        "        let b = 2;".into(),
                        "    }".into(),
                        "}".into(),
                        "".into(),
                        "fn main() {".into(),
                        "    println!(\"hi\");".into(),
                        "}".into(),
                    ],
                )];
                self.selected_file = 0;
                self.caret_line = 5;
                self.caret_column = 0;
                self.folded = vec![0, 5];
                self.goto_prompt = Some("42".into());
                self.goto_focused = true;
                true
            }
            "brackets" => {
                // The caret sits inside a nested block, so the pair it is
                // between is drawn and the mismatch is visible if it is wrong.
                self.files = vec![FileBuffer::new(
                    "main.rs".into(),
                    PathBuf::from("/tmp/tc-ide-preview/main.rs"),
                    vec![
                        "fn build() {".into(),
                        "    let items = [".into(),
                        "        one,".into(),
                        "    ];".into(),
                        "    compute(items)".into(),
                        "}".into(),
                    ],
                )];
                self.selected_file = 0;
                self.caret_line = 1;
                self.caret_column = 16;
                true
            }
            "diagnostics" => {
                // A file with what a server would have said about it, so the
                // gutter markers and the status line can be inspected without
                // a language server installed.
                self.files = vec![FileBuffer::new(
                    "main.rs".into(),
                    PathBuf::from("/tmp/tc-ide-preview/main.rs"),
                    vec![
                        "use std::collections::HashMap;".into(),
                        "fn main() {".into(),
                        "    let unused = 3;".into(),
                        "    let x: usize = \"text\";".into(),
                        "    println!(\"{}\", x);".into(),
                        "}".into(),
                    ],
                )];
                self.selected_file = 0;
                self.caret_line = 3;
                self.lsp_diagnostic_file = self.active_breakpoint_path();
                self.lsp_completions = vec![
                    "build  fn() -> Engine".to_string(),
                    "builder  struct Builder".to_string(),
                    "build_all  fn() -> Vec<Engine>".to_string(),
                ];
                self.lsp_completion_index = 1;
                self.lsp_hover =
                    Some("fn build(&self) -> Engine\n  builds the engine used by the graph".into());
                self.lsp_diagnostics = vec![
                    lsp_core::Diagnostic {
                        path: self.lsp_diagnostic_file.clone(),
                        line: 2,
                        column: 8,
                        severity: lsp_core::DiagnosticSeverity::Warning,
                        message: "unused variable: `unused`".into(),
                    },
                    lsp_core::Diagnostic {
                        path: self.lsp_diagnostic_file.clone(),
                        line: 3,
                        column: 17,
                        severity: lsp_core::DiagnosticSeverity::Error,
                        message: "mismatched types: expected `usize`, found `&str`".into(),
                    },
                ];
                true
            }
            "ai-panel" => {
                // A conversation and a prompt, so the panel can be inspected
                // without an endpoint to talk to.
                self.ai_open = true;
                self.ai_focused = true;
                self.ai_model = "gpt-5.1".to_string();
                self.ai_messages = ai_core::Conversation::new();
                self.ai_messages.push_user("where is the graph built?");
                self.ai_messages
                    .append_answer("The relation graph is built by graph_core::build_graph,");
                self.ai_messages
                    .append_answer("which the three backends all call.");
                self.ai_messages.push_user("and the index?");
                self.ai_messages
                    .append_answer("index_core keeps one fingerprint per file.");
                self.ai_prompt = "summarise this project".to_string();
                true
            }
            "file-menu" => {
                // The File menu with its New, Open and Recent sections, so the
                // menu can be inspected without a pointer.
                for name in ["tc-ide", "notes", "sandbox"] {
                    let path = PathBuf::from(format!("/tmp/tc-ide-preview/{name}"));
                    self.session.open(Some(path.clone()), name.to_string());
                    self.session.remember_project(&path);
                }
                self.new_menu_open = true;
                self.new_menu_motion = UiMotion::new(1.0);
                true
            }
            "debug" => {
                self.files = vec![FileBuffer::new(
                    "main.cpp".into(),
                    PathBuf::from("/tmp/tc-ide-preview/main.cpp"),
                    vec![
                        "#include <iostream>".into(),
                        "constexpr int LIMIT = 42;".into(),
                        "int main() {".into(),
                        "    int count = LIMIT;".into(),
                        "    std::cout << count << std::endl;".into(),
                        "    return 0;".into(),
                        "}".into(),
                    ],
                )];
                self.selected_file = 0;
                self.caret_line = 4;
                self.breakpoints.toggle("/tmp/tc-ide-preview/main.cpp", 4);
                self.debug_state = dap_core::DebugState {
                    stopped: Some(dap_core::StopEvent {
                        reason: "breakpoint".into(),
                        thread_id: Some(1),
                        file: Some("/tmp/tc-ide-preview/main.cpp".into()),
                        line: Some(4),
                    }),
                    // A realistic stop: more than one frame, a scope and a
                    // long value, so the panel shows its sections and its
                    // shortening.
                    frames: vec![
                        dap_core::StackFrame {
                            id: 1,
                            name: "main".into(),
                            file: Some("/tmp/tc-ide-preview/main.cpp".into()),
                            line: 4,
                            column: 17,
                        },
                        dap_core::StackFrame {
                            id: 2,
                            name: "render".into(),
                            file: Some("/tmp/tc-ide-preview/render.cpp".into()),
                            line: 118,
                            column: 9,
                        },
                        dap_core::StackFrame {
                            id: 3,
                            name: "std::__invoke".into(),
                            file: None,
                            line: 0,
                            column: 0,
                        },
                    ],
                    scopes: vec![dap_core::Scope {
                        name: "Locals".into(),
                        reference: 7,
                        expensive: false,
                    }],
                    variables: vec![
                        dap_core::Variable {
                            name: "count".into(),
                            value: "42".into(),
                            kind: "int".into(),
                        },
                        dap_core::Variable {
                            name: "frame".into(),
                            value: "{width: 1440, height: 900, scale: 2.0, title: \"main window\", visible: true}"
                                .into(),
                            kind: "struct".into(),
                        },
                    ],
                    terminated: false,
                    output: vec![],
                };
                // The panel is part of this scene, so a frame covers both the
                // inline labels and the call stack panel.
                // One plain breakpoint and one with a condition, so the two
                // marks can be told apart in the frame, plus the prompt open.
                self.breakpoints.clear("/tmp/tc-ide-preview/main.cpp", 4);
                self.breakpoints.set(
                    "/tmp/tc-ide-preview/main.cpp",
                    dap_core::BreakpointSpec::new(4),
                );
                self.breakpoints.set(
                    "/tmp/tc-ide-preview/main.cpp",
                    dap_core::BreakpointSpec::conditional(5, "count > LIMIT"),
                );
                self.breakpoint_prompt = Some("count > LIMIT".to_string());
                self.condition_caret = "count > LIMIT".chars().count();
                self.condition_focused = true;
                self.debug_panel_open = true;
                self.debug_frame = 0;
                true
            }
            "packaging" => {
                self.package_targets = [true, true, true, true];
                self.build_started = Some(ago(absorb * 0.55));
                true
            }
            "packaging-emit" => {
                self.package_targets = [true, true, true, true];
                self.build_started = Some(ago(absorb + 0.9));
                true
            }
            "background" => {
                // Show the bundled icon as the background picture so the
                // placement can be inspected, and its crop box later on.
                if self.background_surface.is_null() {
                    // Loading a PNG goes through the Cairo FFI.
                    self.background_surface = unsafe {
                        bundled_asset_path("assets/tc-ide-icon.png")
                            .and_then(|path| CString::new(path.to_string_lossy().as_bytes()).ok())
                            .map(|path| cairo_image_surface_create_from_png(path.as_ptr()))
                            .unwrap_or(ptr::null_mut())
                    };
                }
                self.background_opacity = 0.55;
                self.background_zoom = 1.6;
                self.background_offset_x = 0.35;
                self.background_offset_y = -0.25;
                true
            }
            "graph-callees" => {
                // Focus the first method that calls a member, so the Callees
                // relation of the graph can be inspected as a frame.
                let focus = self.files.get(self.selected_file).and_then(|file| {
                    let scan = syntax_core::scan(&file.lines, &file.name);
                    let call = scan.calls.iter().find(|call| call.receiver.is_some())?;
                    let declaration = scan
                        .declarations
                        .iter()
                        .find(|declaration| {
                            scan.scope_at(call.line) == Some(declaration.scope)
                                || declaration.line == call.line
                        })
                        .or_else(|| scan.declarations.first())?;
                    Some(editor_core::SymbolOccurrence {
                        name: declaration.name.clone(),
                        line: declaration.line,
                        start: declaration.column,
                        end: declaration.column + declaration.name.len(),
                        role: editor_core::SymbolRole::Declaration,
                    })
                });
                let Some(focus) = focus else {
                    return false;
                };
                self.hovered_symbol = Some(focus);
                self.graph_relation = graph_core::GraphRelation::Callees;
                true
            }
            "settings-controls" | "settings-hover" => {
                // Settings open on a picture: one control hovered, one pressed,
                // one focused from the keyboard and several disabled.
                self.settings_open = true;
                self.settings_motion = UiMotion::new(1.0);
                self.background_surface = unsafe {
                    bundled_asset_path("assets/tc-ide-icon.png")
                        .and_then(|path| CString::new(path.to_string_lossy().as_bytes()).ok())
                        .map(|path| cairo_image_surface_create_from_png(path.as_ptr()))
                        .unwrap_or(ptr::null_mut())
                };
                self.background_opacity = 0.55;
                self.background_zoom = 1.6;
                self.background_offset_x = 0.35;
                self.background_offset_y = -0.25;
                self.settings_focus = Some(SettingsControl::BackgroundChoose);
                let target = SettingsControl::ThemeLight;
                let (x, y, width, height) = self.settings_control_rect(target);
                self.mouse_x = x + width / 2;
                self.mouse_y = y + height / 2;
                if name == "settings-controls" {
                    // A press always comes with the pointer already over the
                    // control, which is the state this frame shows.
                    self.settings_pressed = Some(target);
                }
                true
            }
            "background-crop" => {
                // The preview with the crop box, as if the settings panel were
                // open on a picture.
                self.settings_motion = UiMotion::new(1.0);
                self.background_surface = unsafe {
                    bundled_asset_path("assets/tc-ide-icon.png")
                        .and_then(|path| CString::new(path.to_string_lossy().as_bytes()).ok())
                        .map(|path| cairo_image_surface_create_from_png(path.as_ptr()))
                        .unwrap_or(ptr::null_mut())
                };
                self.background_opacity = 0.55;
                self.background_zoom = 1.6;
                self.background_offset_x = 0.35;
                self.background_offset_y = -0.25;
                true
            }
            "edit" => {
                // A paste highlight halfway through its fade, so the edit
                // animation can be inspected without typing.
                let end = self
                    .files
                    .get(self.selected_file)
                    .and_then(|file| file.lines.first())
                    .map(|line| line.len())
                    .unwrap_or(0);
                self.note_edit(motion_core::EditKind::Paste, 0, 0, end);
                self.edit_pulses
                    .step(motion_core::EditPulses::DURATION / 2.0);
                true
            }
            _ => false,
        }
    }

    unsafe fn draw_build_animation(&self) {
        let s = |v| self.s(v);
        let elapsed = self.build_started.unwrap().elapsed().as_secs_f32();
        let absorb_seconds = self.build_absorb_seconds();
        let cx = self.width / 2;
        let cy = self.height / 2;
        if elapsed < absorb_seconds {
            let t = (elapsed / absorb_seconds).clamp(0.0, 1.0);
            if t > 0.76 {
                let left = (self.width as f32 * 0.17) as i32;
                let right = self.graph_left();
                self.rect(
                    self.colors.bg,
                    left + s(1),
                    s(85),
                    right - left - s(1),
                    self.height - s(109),
                );
            }
            let sources: [(i32, i32, &str); 9] = [
                (s(250), s(160), "fn build_graph()"),
                (s(420), s(310), "resolve_symbols(files)"),
                (self.width - s(460), s(220), "GraphController"),
                (
                    self.width - s(330),
                    self.height - s(320),
                    "render_graph(nodes)",
                ),
                (s(310), self.height - s(260), "workspace.index()"),
                (s(510), s(205), "let package_target"),
                (self.width - s(540), s(360), "runtime_cache"),
                (s(360), self.height - s(190), "const BUILD_MODE"),
                (self.width - s(390), self.height - s(235), "artifact_path"),
            ];
            let centre = (cx as f32, cy as f32);
            let places = sources
                .iter()
                .map(|(sx, sy, _)| (*sx as f32, *sy as f32))
                .collect::<Vec<_>>();
            let max_distance = places
                .iter()
                .map(|(x, y)| ((x - centre.0).powi(2) + (y - centre.1).powi(2)).sqrt())
                .fold(0.0_f32, f32::max);
            for (index, (_, _, label)) in sources.iter().enumerate() {
                let travel = package_motion::absorbed(
                    places[index],
                    centre,
                    index,
                    places.len(),
                    max_distance,
                    t,
                );
                let px = travel.x.round() as i32;
                let py = travel.y.round() as i32;
                let color = if travel.progress > 0.9 {
                    self.colors.line
                } else {
                    self.colors.text
                };
                let stretch = (travel.trail * self.scale()).round() as i32;
                if stretch > s(package_motion::MIN_TRAIL as i32) {
                    self.line(
                        self.colors.muted,
                        1,
                        (px - stretch, py + s(4)),
                        (px - s(4), py + s(4)),
                    );
                }
                self.text(color, px, py, label);
            }
            self.abstract_vortex(cx, cy, elapsed, true);
            self.text(
                self.colors.muted,
                cx - s(104),
                cy + s(105),
                self.tr(
                    "CODE IS COLLAPSING INTO THE BUILD CORE",
                    "代码正被吸入打包核心",
                ),
            );
        } else {
            let t = ((elapsed - absorb_seconds) / 1.7).clamp(0.0, 1.0);
            self.abstract_vortex(cx, cy, elapsed, false);
            let labels = [".exe", ".deb", ".AppImage", ".dmg"];
            let selected = self
                .package_targets
                .iter()
                .take(labels.len())
                .filter(|enabled| **enabled)
                .count()
                .max(1);
            let slots = package_motion::emission_slots(selected, s(200) as f32, s(90) as f32);
            let centre = (cx as f32, cy as f32);
            let mut slot = 0usize;
            for (index, label) in labels.iter().enumerate() {
                if !self.package_targets.get(index).copied().unwrap_or(false) {
                    continue;
                }
                let place = slots[slot.min(slots.len().saturating_sub(1))];
                let emission = package_motion::emitted(centre, place, slot, selected, t);
                self.package_icon(
                    index,
                    emission.x.round() as i32,
                    emission.y.round() as i32,
                    label,
                );
                slot += 1;
            }
            let state = if t < 0.85 {
                self.tr("WHITE HOLE IS EMITTING PACKAGES", "白洞正在吐出安装包")
            } else {
                self.tr("BUILD COMPLETE", "打包完成")
            };
            self.text(self.colors.bright, cx - s(106), cy + s(150), state);
        }
    }

    unsafe fn resize(&mut self, width: i32, height: i32) {
        self.width = width.max(900);
        self.height = height.max(600);
        XFreePixmap(self.display, self.canvas);
        self.canvas = XCreatePixmap(
            self.display,
            self.window,
            self.width as u32,
            self.height as u32,
            self.depth as u32,
        );
        cairo_xlib_surface_set_drawable(self.cairo_surface, self.canvas, self.width, self.height);
        self.update_hover();
    }

    fn update_background(&mut self) {
        self.poll_debug_session();
        let now = Instant::now();
        let dt = now
            .duration_since(self.tree_motion_updated)
            .as_secs_f32()
            .min(0.04);
        self.tree_motion_updated = now;
        let root_target = if self.project_tree_expanded { 1.0 } else { 0.0 };
        Self::spring_step(
            &mut self.project_tree_amount,
            &mut self.project_tree_velocity,
            root_target,
            dt,
        );
        Self::step_ui_motion(&mut self.new_menu_motion, self.new_menu_open, dt);
        Self::step_ui_motion(&mut self.edit_menu_motion, self.edit_menu_open, dt);
        Self::step_ui_motion(&mut self.view_menu_motion, self.view_menu_open, dt);
        Self::step_ui_motion(&mut self.settings_motion, self.settings_open, dt);
        Self::step_ui_motion(&mut self.package_motion, self.package_open, dt);
        Self::step_ui_motion(&mut self.extensions_motion, self.extensions_open, dt);
        Self::step_ui_motion(&mut self.create_motion, self.create_kind.is_some(), dt);
        Self::step_ui_motion(&mut self.terminal_motion, self.terminal_open, dt);
        Self::step_ui_motion(&mut self.graph_motion, self.graph_visible, dt);
        for folder in self.folders.clone() {
            let target = if self.collapsed_folders.contains(&folder) {
                0.0
            } else {
                1.0
            };
            let motion = self.folder_tree_motion.entry(folder).or_insert((1.0, 0.0));
            Self::spring_step(&mut motion.0, &mut motion.1, target, dt);
        }
        self.edit_pulses.step(dt);
        if let Some((index, started)) = self.downloading {
            if started.elapsed() > Duration::from_millis(900) {
                self.languages[index].installed = true;
                self.status_message = format!(
                    "{} {} language pack installed",
                    self.languages[index].name,
                    self.languages[index].versions[self.languages[index].selected_version]
                );
                save_language_settings(&self.languages);
                self.downloading = None;
            }
        }
        if self
            .build_started
            .map(|started| {
                started.elapsed() > Duration::from_secs_f32(self.build_absorb_seconds() + 1.9)
            })
            .unwrap_or(false)
        {
            self.build_started = None;
            self.finish_build_preview();
        }
    }

    fn finish_build_preview(&mut self) {
        let output = self.active_root().join("dist");
        if let Err(error) = fs::create_dir_all(&output) {
            self.status_message = format!("Unable to create {}: {error}", output.display());
            return;
        }
        let labels = [
            "Windows .exe",
            "Linux .deb",
            "Linux .AppImage",
            "macOS .dmg",
        ];
        let selected = labels
            .iter()
            .enumerate()
            .filter(|(index, _)| self.package_targets[*index])
            .map(|(_, label)| *label)
            .collect::<Vec<_>>()
            .join(", ");
        let report = format!(
            "TC-IDE build preview\n\nRequested targets: {selected}\n\nThis development build currently demonstrates the packaging workflow and animation. Native installer generation is not implemented yet, so no .exe, .deb, .AppImage, or .dmg file has been fabricated. This directory is the reserved output location for the real packaging pipeline.\n"
        );
        if let Err(error) = fs::write(output.join("BUILD-RESULT.txt"), report) {
            self.status_message = format!("Unable to write build result: {error}");
            return;
        }
        let opened = Command::new("xdg-open")
            .arg(&output)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .is_ok()
            || Command::new("gio")
                .arg("open")
                .arg(&output)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .is_ok();
        self.status_message = if opened {
            format!(
                "{}: {}",
                self.tr(
                    "Build preview complete; opened output folder",
                    "打包预览完成；已打开输出目录"
                ),
                output.display()
            )
        } else {
            format!(
                "{}: {}",
                self.tr(
                    "Build preview complete; output folder",
                    "打包预览完成；输出目录"
                ),
                output.display()
            )
        };
    }

    fn active_root(&mut self) -> PathBuf {
        if let Some(path) = &self.project_path {
            return path.clone();
        }
        let base = projects_root().join("TC-IDE-Scratch");
        let _ = fs::create_dir_all(&base);
        self.project_path = Some(base.clone());
        base
    }

    fn save_current_file(&mut self) {
        if self.project_path.is_none() {
            self.status_message = "Create or open a project before saving the demo buffer".into();
            return;
        }
        let file = &self.files[self.selected_file];
        let content = file.lines.join("\n");
        match fs::write(&file.path, content) {
            Ok(_) => self.status_message = format!("Saved {}", file.path.display()),
            Err(error) => self.status_message = format!("Save failed: {error}"),
        }
    }

    fn reset_terminals_for_project(&mut self) {
        self.terminals = default_terminals();
        self.terminal_active = 0;
        self.terminal_focused = false;
        self.terminal_menu_open = false;
    }

    fn add_terminal(&mut self) {
        let number = self.terminals.len() + 1;
        self.terminals.push(TerminalSession {
            name: format!("Terminal {number}"),
            input: String::new(),
            output: Arc::new(Mutex::new(terminal_banner())),
        });
        self.terminal_active = self.terminals.len() - 1;
        self.terminal_focused = true;
    }

    fn pop_out_terminal(&mut self) {
        let cwd = self.project_path.clone().unwrap_or_else(projects_root);
        let script = "printf '\\n\\033[1;38;5;117m  [ T I A N C H U A N G Y A ]\\033[0m\\n\\033[38;5;44m  ===========================\\033[0m\\n  TC-IDE TERMINAL\\n  GitHub: https://github.com/tianchuangya/TC-IDE\\n  Native semantic workspace\\n\\n'; exec bash";
        let cwd_text = cwd.to_string_lossy().into_owned();
        let launched = Command::new("gnome-terminal")
            .args([
                "--title=TC-IDE Terminal",
                "--working-directory",
                &cwd_text,
                "--",
                "bash",
                "-lc",
                script,
            ])
            .spawn()
            .is_ok();
        if !launched {
            let _ = Command::new("x-terminal-emulator")
                .args(["-T", "TC-IDE Terminal", "-e", "bash", "-lc", script])
                .current_dir(&cwd)
                .spawn();
        }
        self.status_message = self
            .tr(
                "Terminal opened in a separate window",
                "终端已在独立窗口中打开",
            )
            .into();
    }

    fn save_project_package_targets(&self) {
        let Some(root) = &self.project_path else {
            return;
        };
        let _ = fs::write(
            root.join(workspace_core::PACKAGE_TARGETS_FILE),
            workspace_core::serialize_package_targets(&self.package_targets),
        );
    }

    fn build_absorb_seconds(&self) -> f32 {
        let lines = self.files.iter().map(|file| file.lines.len()).sum();
        let targets = self
            .package_targets
            .iter()
            .filter(|enabled| **enabled)
            .count();
        package_motion::absorb_seconds(lines, targets)
    }

    fn load_project_package_targets(&mut self) {
        self.package_targets = [false; 4];
        let Some(root) = &self.project_path else {
            return;
        };
        let Ok(value) = fs::read_to_string(root.join(workspace_core::PACKAGE_TARGETS_FILE)) else {
            return;
        };
        self.package_targets
            .copy_from_slice(&workspace_core::parse_package_targets(&value, 4));
    }

    fn run_terminal_command(&mut self) {
        if self.terminals.is_empty() {
            return;
        }
        let active = self.terminal_active.min(self.terminals.len() - 1);
        let command = self.terminals[active].input.trim().to_string();
        if command.is_empty() {
            return;
        }
        self.terminals[active].input.clear();
        self.run_in_terminal(&command);
    }

    /// Run one command in the visible terminal and show its output there.
    ///
    /// The command is run through the shell so the user sees exactly what was
    /// executed and what it printed, which is what an install needs to be
    /// trustworthy.
    fn run_in_terminal(&mut self, command: &str) {
        if self.terminals.is_empty() {
            return;
        }
        let active = self.terminal_active.min(self.terminals.len() - 1);
        let output = self.terminals[active].output.clone();
        if let Ok(mut lines) = output.lock() {
            lines.push(format!("$ {command}"));
        }
        let cwd = self.project_path.clone().unwrap_or_else(projects_root);
        // The thread outlives this call, so it owns the command.
        let owned = command.to_string();
        thread::spawn(move || {
            let result = Command::new("sh")
                .args(["-lc", &owned])
                .current_dir(&cwd)
                .output();
            if let Ok(mut lines) = output.lock() {
                match result {
                    Ok(result) => {
                        let text = format!(
                            "{}{}",
                            String::from_utf8_lossy(&result.stdout),
                            String::from_utf8_lossy(&result.stderr)
                        );
                        lines.extend(text.lines().map(str::to_string));
                        if text.is_empty() {
                            lines.push(format!("process exited with {}", result.status));
                        }
                    }
                    Err(error) => lines.push(format!("terminal error: {error}")),
                }
                if lines.len() > 200 {
                    let drain = lines.len() - 200;
                    lines.drain(0..drain);
                }
            }
        });
    }

    fn selected_text(&self) -> String {
        let (Some(a), Some(b)) = (self.selection_anchor, self.selection_head) else {
            return String::new();
        };
        let (start, end) = if a <= b { (a, b) } else { (b, a) };
        if start.0 == end.0 {
            return self.files[self.selected_file].lines[start.0][start.1..end.1].to_string();
        }
        let mut parts = Vec::new();
        parts.push(self.files[self.selected_file].lines[start.0][start.1..].to_string());
        for line in start.0 + 1..end.0 {
            parts.push(self.files[self.selected_file].lines[line].clone());
        }
        parts.push(self.files[self.selected_file].lines[end.0][..end.1].to_string());
        parts.join("\n")
    }

    /// Ask the server what could go here, and show the list.
    fn request_completions(&mut self) {
        if self.lsp_completions.is_empty() {
            self.request_language_feature(LspRequest::Completion);
        } else {
            // A second press walks the list instead of asking again.
            self.lsp_completion_index =
                (self.lsp_completion_index + 1) % self.lsp_completions.len();
        }
    }

    /// Insert the completion the language server offered.
    fn accept_server_completion(&mut self) {
        let Some(word) = self.lsp_completions.get(self.lsp_completion_index).cloned() else {
            return;
        };
        // Only the label goes in, not the detail that follows it.
        let label = word.split("  ").next().unwrap_or(&word).to_string();
        self.delete_selection();
        self.files[self.selected_file]
            .buffer
            .insert(self.caret_line, self.caret_column, &label);
        self.caret_column += label.len();
        self.files[self.selected_file].sync();
        self.note_edit(
            motion_core::EditKind::Insert,
            self.caret_line,
            self.caret_column - label.len(),
            self.caret_column,
        );
        self.lsp_completions.clear();
        self.lsp_completion_index = 0;
        self.status_message = format!("{} {label}", self.tr("Inserted", "已插入"));
    }

    /// Open the prompt that asks for the new name.
    fn begin_rename(&mut self) {
        if self.language_server.is_none() {
            self.status_message = self
                .tr(
                    "No language server is running for this file",
                    "此文件没有正在运行的语言服务器",
                )
                .into();
            return;
        }
        self.rename_prompt = Some(String::new());
        self.rename_focused = true;
        self.status_message = self
            .tr(
                "New name: type it, Enter renames, Esc cancels",
                "新名称：输入后回车重命名，Esc 取消",
            )
            .into();
    }

    /// Ask the server to rename the symbol at the caret.
    fn send_rename(&mut self) {
        let new_name = self.rename_prompt.take().unwrap_or_default();
        self.rename_focused = false;
        if new_name.trim().is_empty() {
            self.status_message = self.tr("Rename needs a name", "重命名需要名称").into();
            return;
        }
        let path = self.active_breakpoint_path();
        let line = self.caret_line as u32;
        let character = self.files[self.selected_file]
            .lines
            .get(self.caret_line)
            .map(|text| text[..self.caret_column.min(text.len())].chars().count() as u32)
            .unwrap_or(0);
        let Some(server) = self.language_server.as_mut() else {
            return;
        };
        match server.rename(&path, line, character, new_name.trim()) {
            Ok(id) => self.lsp_pending.push((id, LspRequest::Rename)),
            Err(error) => self.status_message = error,
        }
    }

    /// Apply the edits a rename produced.
    ///
    /// The edits are grouped by file: the open document is edited in place so
    /// its history and the caret survive, and the others are read, changed and
    /// written back. A file that cannot be written stops the rename and says
    /// so rather than leaving half the project renamed.
    fn apply_rename_edits(&mut self, edits: &[lsp_core::RenameEdit]) {
        let mut grouped: Vec<(String, Vec<lsp_core::RenameEdit>)> = Vec::new();
        for edit in edits {
            match grouped.iter_mut().find(|(path, _)| *path == edit.path) {
                Some((_, list)) => list.push(edit.clone()),
                None => grouped.push((edit.path.clone(), vec![edit.clone()])),
            }
        }
        let mut touched = 0usize;
        for (path, list) in grouped {
            let name = PathBuf::from(&path)
                .file_name()
                .map(|value| value.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.clone());
            let open = self.files.iter().position(|document| document.name == name);
            if let Some(index) = open {
                let text = self.files[index].lines.join("\n");
                let updated = lsp_core::apply_edits(&text, &list);
                let lines = updated.split('\n').map(str::to_string).collect::<Vec<_>>();
                self.files[index] = FileBuffer::new(name, PathBuf::from(&path), lines);
                touched += 1;
                continue;
            }
            let Ok(text) = fs::read_to_string(&path) else {
                self.status_message = format!(
                    "{} {path}",
                    self.tr("Rename could not read", "重命名无法读取")
                );
                return;
            };
            let updated = lsp_core::apply_edits(&text, &list);
            if fs::write(&path, updated).is_err() {
                self.status_message = format!(
                    "{} {path}",
                    self.tr("Rename could not write", "重命名无法写入")
                );
                return;
            }
            touched += 1;
        }
        self.status_message = format!(
            "{} {touched} {}",
            self.tr("Renamed in", "已重命名的文件数："),
            self.tr("file(s)", "个文件")
        );
    }

    /// Ask the server something about the symbol at the caret.
    fn request_language_feature(&mut self, request: LspRequest) {
        // Where the caret is comes from the editor, so it is read before the
        // server is borrowed to ask the question.
        let path = self.active_breakpoint_path();
        let line = self.caret_line as u32;
        // The protocol counts characters, the editor counts bytes.
        let character = self.files[self.selected_file]
            .lines
            .get(self.caret_line)
            .map(|text| text[..self.caret_column.min(text.len())].chars().count() as u32)
            .unwrap_or(0);
        let Some(server) = self.language_server.as_mut() else {
            self.status_message = self
                .tr(
                    "No language server is running for this file",
                    "此文件没有正在运行的语言服务器",
                )
                .into();
            return;
        };
        let method = match request {
            LspRequest::Completion => "textDocument/completion",
            LspRequest::Hover => "textDocument/hover",
            LspRequest::Definition => "textDocument/definition",
            LspRequest::References => "textDocument/references",
            // A rename goes through `rename`, which carries the new name.
            LspRequest::Rename => return,
        };
        match server.position_request(method, &path, line, character) {
            Ok(id) => self.lsp_pending.push((id, request)),
            Err(error) => self.status_message = error,
        }
    }

    /// Show what the server said about the symbol at the caret.
    fn show_hover(&mut self) {
        if self.lsp_hover.is_some() {
            // A second press puts it away rather than asking again.
            self.lsp_hover = None;
            return;
        }
        self.request_language_feature(LspRequest::Hover);
    }

    /// Jump to the definition of the symbol at the caret.
    fn go_to_definition(&mut self) {
        self.request_language_feature(LspRequest::Definition);
    }

    /// Jump to where the symbol at the caret is used.
    fn go_to_references(&mut self) {
        self.request_language_feature(LspRequest::References);
    }

    /// Move the caret to a location the server named.
    ///
    /// A location in another file is opened as a document first, because the
    /// editor can only show what it has.
    fn jump_to_location(&mut self, file: String, line: u32, column: u32) {
        let name = PathBuf::from(&file)
            .file_name()
            .map(|value| value.to_string_lossy().into_owned())
            .unwrap_or_else(|| file.clone());
        let open = self.files.iter().position(|document| document.name == name);
        if let Some(index) = open {
            self.selected_file = index;
        } else {
            let path = PathBuf::from(&file);
            match fs::read_to_string(&path) {
                Ok(content) => {
                    let mut lines = content.lines().map(str::to_string).collect::<Vec<_>>();
                    if lines.is_empty() {
                        lines.push(String::new());
                    }
                    self.files.push(FileBuffer::new(name.clone(), path, lines));
                    self.selected_file = self.files.len() - 1;
                }
                Err(error) => {
                    self.status_message = format!("{file}: {error}");
                    return;
                }
            }
        }
        let last = self.files[self.selected_file].lines.len().saturating_sub(1);
        self.caret_line = (line as usize).min(last);
        // The server counts characters, the editor counts bytes.
        let text = self.files[self.selected_file].lines[self.caret_line].clone();
        self.caret_column = text
            .char_indices()
            .nth(column as usize)
            .map(|(index, _)| index)
            .unwrap_or(text.len());
        self.selection_anchor = None;
        self.selection_head = None;
        self.status_message = format!("{} {name}:{}", self.tr("Jumped to", "已跳转到"), line + 1);
    }

    /// Answer what the server sent back for a request we made.
    fn handle_language_reply(&mut self, id: u64, message: &str) {
        let Some(position) = self.lsp_pending.iter().position(|(known, _)| *known == id) else {
            return;
        };
        let (_, request) = self.lsp_pending.remove(position);
        match request {
            LspRequest::Completion => {
                self.lsp_completions = lsp_core::parse_completions(message)
                    .into_iter()
                    .map(|(label, detail)| {
                        if detail.is_empty() {
                            label
                        } else {
                            format!("{label}  {detail}")
                        }
                    })
                    .collect();
                self.lsp_completion_index = 0;
                if self.lsp_completions.is_empty() {
                    self.status_message = self
                        .tr("No completions from the server", "服务器未给出补全")
                        .into();
                }
            }
            LspRequest::Rename => {
                let edits = lsp_core::parse_rename_edits(message);
                if edits.is_empty() {
                    self.status_message = self
                        .tr("The server refused the rename", "服务器拒绝了重命名")
                        .into();
                } else {
                    self.apply_rename_edits(&edits);
                }
            }
            LspRequest::Hover => {
                self.lsp_hover = lsp_core::parse_hover(message);
                if self.lsp_hover.is_none() {
                    self.status_message = self
                        .tr("The server had nothing to say", "服务器没有给出信息")
                        .into();
                }
            }
            LspRequest::Definition => {
                let locations = lsp_core::parse_locations(message);
                match locations.first() {
                    Some((file, line, column)) => {
                        self.jump_to_location(file.clone(), *line, *column)
                    }
                    None => {
                        self.status_message = self
                            .tr("No definition from the server", "服务器未给出定义位置")
                            .into()
                    }
                }
            }
            LspRequest::References => {
                let locations = lsp_core::parse_locations(message);
                self.lsp_locations = locations;
                self.lsp_reference_index = 0;
                if self.lsp_locations.is_empty() {
                    self.status_message = self
                        .tr("No references from the server", "服务器未给出引用位置")
                        .into();
                } else {
                    let (file, line, column) = self.lsp_locations[0].clone();
                    self.jump_to_location(file, line, column);
                    self.status_message = format!(
                        "{} 1/{}",
                        self.tr("Reference", "引用"),
                        self.lsp_locations.len()
                    );
                }
            }
        }
    }

    /// Walk to the next reference the server listed.
    fn next_reference(&mut self) {
        if self.lsp_locations.is_empty() {
            return;
        }
        self.lsp_reference_index = (self.lsp_reference_index + 1) % self.lsp_locations.len();
        let (file, line, column) = self.lsp_locations[self.lsp_reference_index].clone();
        let position = self.lsp_reference_index + 1;
        let total = self.lsp_locations.len();
        self.jump_to_location(file, line, column);
        self.status_message = format!("{} {position}/{total}", self.tr("Reference", "引用"));
    }

    /// Start the language server for the active file, when one is installed.
    ///
    /// The server is started lazily: opening a project should not spawn a rust
    /// analyzer for a file nobody has touched.
    fn ensure_language_server(&mut self) {
        let name = self.files[self.selected_file].name.clone();
        let Some(spec) = lsp_core::server_for_file(&name) else {
            self.shutdown_language_server();
            return;
        };
        // Already running for this file and language: only tell it what changed.
        let path = self.active_breakpoint_path();
        let text = self.document_text();
        if let Some(server) = self.language_server.as_mut() {
            if server.spec().program == spec.program {
                let _ = server.change_document(&path, &text);
                return;
            }
        }
        self.shutdown_language_server();
        if !lsp_core::executable_available(&spec.program) {
            self.lsp_diagnostics.clear();
            self.status_message = format!("{} is not installed for {name}", spec.program);
            return;
        }
        let root = self.project_path.clone().unwrap_or_else(projects_root);
        match lsp_core::LanguageServer::start(&spec, &root) {
            Ok(mut server) => {
                if server.initialize().is_err() || server.notify_initialized().is_err() {
                    self.status_message = format!("{} did not answer initialize", spec.program);
                    return;
                }
                if let Err(error) = server.open_document(&path, &text) {
                    self.status_message = error;
                    return;
                }
                self.status_message = format!("{} started for {name}", spec.program);
                self.language_server = Some(server);
                self.lsp_diagnostics.clear();
            }
            Err(error) => self.status_message = error,
        }
    }

    /// Stop the server and forget what it told us.
    fn shutdown_language_server(&mut self) {
        let path = self.active_breakpoint_path();
        if let Some(server) = self.language_server.as_mut() {
            let _ = server.close_document(&path);
        }
        self.language_server = None;
        self.lsp_diagnostics.clear();
    }

    /// Collect what the server has said since the last frame.
    ///
    /// The server sends diagnostics on its own schedule, so they are read once
    /// a frame rather than waited for.
    fn poll_language_server(&mut self) {
        // The file is read before the server is borrowed, because the events
        // arrive from the server and the path comes from the editor.
        let path = self.active_breakpoint_path();
        let Some(server) = self.language_server.as_mut() else {
            return;
        };
        let mut published = None;
        let mut replies = Vec::new();
        while let Some(message) = server.try_message() {
            // A server asks questions of its own; answering keeps it working.
            if server.answer_automatically(&message) {
                continue;
            }
            if lsp_core::message_method(&message).as_deref()
                == Some("textDocument/publishDiagnostics")
            {
                published = Some(lsp_core::parse_diagnostics(&message));
            } else if let Some(id) = lsp_core::message_id(&message) {
                replies.push((id, message.clone()));
            }
        }
        if let Some(diagnostics) = published {
            self.lsp_diagnostics = diagnostics;
            self.lsp_diagnostic_file = path;
        }
        // A reply to something we asked is handed to the feature that asked.
        for (id, message) in replies {
            self.handle_language_reply(id, &message);
        }
    }

    /// Tell the server the document changed, a moment after typing stops.
    ///
    /// Sending every keystroke would make a real server re-analyse on every
    /// character, so the change waits until the typing pauses.
    fn sync_language_server(&mut self) {
        if self.language_server.is_none() {
            return;
        }
        let now = Instant::now();
        let due = self
            .lsp_last_change
            .map(|last| now.duration_since(last) >= Duration::from_millis(400))
            .unwrap_or(true);
        if !due {
            return;
        }
        self.lsp_last_change = Some(now);
        let path = self.active_breakpoint_path();
        let text = self.document_text();
        if let Some(server) = self.language_server.as_mut() {
            let _ = server.change_document(&path, &text);
        }
    }

    /// Diagnostics of the open file that are worth showing.
    fn visible_diagnostics(&self) -> Vec<lsp_core::Diagnostic> {
        if self.lsp_diagnostic_file != self.active_breakpoint_path() {
            return Vec::new();
        }
        lsp_core::filter_diagnostics(&self.lsp_diagnostics, self.lsp_severity)
    }

    /// Diagnostics on one line, for the marker beside it.
    fn diagnostics_on_line(&self, line: usize) -> usize {
        self.visible_diagnostics()
            .iter()
            .filter(|diagnostic| diagnostic.line as usize == line)
            .count()
    }

    /// The worst severity reported on one line, which is the colour the marker
    /// beside it takes.
    fn worst_severity_on_line(&self, line: usize) -> Option<lsp_core::DiagnosticSeverity> {
        let mut worst = None;
        for diagnostic in self.visible_diagnostics() {
            if diagnostic.line as usize != line {
                continue;
            }
            let rank = |severity: lsp_core::DiagnosticSeverity| match severity {
                lsp_core::DiagnosticSeverity::Error => 3,
                lsp_core::DiagnosticSeverity::Warning => 2,
                _ => 1,
            };
            if worst.map(|current| rank(diagnostic.severity) > rank(current)) != Some(false) {
                worst = Some(diagnostic.severity);
            }
        }
        worst
    }

    /// The diagnostic on the caret line, for the status bar.
    fn diagnostic_at_caret(&self) -> Option<lsp_core::Diagnostic> {
        self.visible_diagnostics()
            .into_iter()
            .find(|diagnostic| diagnostic.line as usize == self.caret_line)
    }

    /// Regions of the active file that can be folded.
    fn fold_regions(&self) -> Vec<syntax_core::FoldRegion> {
        let file = &self.files[self.selected_file];
        let scan = syntax_core::scan(&file.lines, &file.name);
        syntax_core::fold_regions(&scan)
    }

    /// Lines hidden by the folds that are closed.
    fn folded_lines(&self) -> Vec<usize> {
        syntax_core::folded_lines(&self.fold_regions(), &self.folded)
    }

    /// Open or close the fold that starts on a line.
    fn toggle_fold(&mut self, line: usize) -> bool {
        let Some(region) = syntax_core::fold_region_at(
            &syntax_core::scan(
                &self.files[self.selected_file].lines,
                &self.files[self.selected_file].name,
            ),
            line,
        ) else {
            return false;
        };
        if let Some(index) = self.folded.iter().position(|known| *known == line) {
            self.folded.remove(index);
            self.status_message = format!(
                "{} {}",
                self.tr("Expanded the block at line", "已展开第"),
                line + 1
            );
        } else {
            self.folded.push(line);
            self.folded.sort_unstable();
            // The caret cannot sit inside a block that is folded away.
            let hidden = self.folded_lines();
            if hidden.contains(&self.caret_line) {
                self.caret_line = region.start_line;
                self.caret_column = 0;
            }
            self.status_message = format!(
                "{} {} {}",
                self.tr("Folded", "已折叠"),
                region.hidden_lines(),
                self.tr("line(s)", "行")
            );
        }
        true
    }

    /// Fold or unfold every region of the file at once.
    fn toggle_fold_all(&mut self) {
        let regions = self.fold_regions();
        if regions.is_empty() {
            self.status_message = self
                .tr("This file has nothing to fold", "此文件没有可折叠的区块")
                .into();
            return;
        }
        if self.folded.is_empty() {
            // Only the outermost regions: folding a nested one as well would
            // hide lines that are already hidden.
            self.folded = {
                let mut lines = regions
                    .iter()
                    .filter(|region| region.depth == 0)
                    .map(|region| region.start_line)
                    .collect::<Vec<_>>();
                lines.sort_unstable();
                lines
            };
            self.status_message = format!(
                "{} {}",
                self.tr("Folded every block;", "已折叠全部区块，共"),
                self.folded.len()
            );
        } else {
            self.folded.clear();
            self.status_message = self.tr("Expanded every block", "已展开全部区块").into();
        }
    }

    /// Open the prompt that asks which line to go to.
    fn begin_goto_line(&mut self) {
        self.goto_prompt = Some(String::new());
        self.goto_focused = true;
        self.status_message = self
            .tr(
                "Line number: type it, Enter jumps, Esc cancels",
                "行号：输入后回车跳转，Esc 取消",
            )
            .into();
    }

    /// Jump to the line the prompt names.
    fn apply_goto_line(&mut self) {
        let input = self.goto_prompt.take().unwrap_or_default();
        self.goto_focused = false;
        let count = self.files[self.selected_file].lines.len();
        let Some(target) = editor_core::parse_line_target(&input, count) else {
            self.status_message = format!(
                "{} {input}",
                self.tr("Not a line in this file:", "不是本文件中的行：")
            );
            return;
        };
        let lines = self.files[self.selected_file].lines.clone();
        let Some((line, column)) = editor_core::line_target_position(&lines, target) else {
            self.status_message = self
                .tr("That line is not in this file", "该行不在本文件中")
                .into();
            return;
        };
        self.caret_line = line;
        self.caret_column = column;
        self.selection_anchor = None;
        self.selection_head = None;
        // A line inside a closed fold is opened, so the jump is visible.
        let regions = self.fold_regions();
        self.folded.retain(|start| {
            !regions
                .iter()
                .any(|region| region.start_line == *start && region.hides(line))
        });
        self.status_message = format!("{} {}", self.tr("Line", "第"), target.line);
    }

    /// The line prompt, over the editor.
    ///
    /// # Safety
    /// Requires a live Cairo context.
    unsafe fn draw_goto_prompt(&self) {
        let Some(prompt) = self.goto_prompt.as_ref() else {
            return;
        };
        let s = |v| self.s(v);
        let width = (self.width as f32 * 0.3) as i32;
        let left = (self.width - width) / 2;
        let top = s(96);
        self.rect(self.colors.panel_2, left, top, width, s(46));
        self.outline(self.colors.accent, left, top, width, s(46));
        self.text(
            self.colors.muted,
            left + s(12),
            top + s(19),
            self.tr("Go to line", "跳转到行"),
        );
        self.text(
            if prompt.is_empty() {
                self.colors.muted
            } else {
                self.colors.bright
            },
            left + s(12),
            top + s(40),
            if prompt.is_empty() {
                self.tr("line or line:column", "行号或 行号:列号")
            } else {
                prompt.as_str()
            },
        );
    }

    /// Recompute which brackets the caret is between.
    ///
    /// Named for what it does rather than when it runs: the pair is worked out
    /// once a frame from the caret, and the shared function does the matching.
    fn refresh_matched_brackets(&mut self) {
        let file = &self.files[self.selected_file];
        let lines = &file.lines;
        let name = &file.name;
        // Comments and strings are blanked first, so a bracket inside one is
        // not matched against code. The mask keeps the same columns.
        let masked = lines
            .iter()
            .map(|line| syntax_core::mask_line(line, name))
            .collect::<Vec<_>>();
        self.matched_brackets =
            editor_core::bracket_pair_at(lines, &masked, self.caret_line, self.caret_column);
    }

    /// The active document as one string, which is what a server is told.
    fn document_text(&self) -> String {
        self.files[self.selected_file].lines.join("\n")
    }

    /// Show the diagnostic on the caret line, if the server reported one.
    ///
    /// # Safety
    /// Requires a live Cairo context.
    unsafe fn draw_diagnostic_status(&self) {
        let Some(diagnostic) = self.diagnostic_at_caret() else {
            return;
        };
        let s = |v| self.s(v);
        let colour = match diagnostic.severity {
            lsp_core::DiagnosticSeverity::Error => self.colors.preprocessor,
            lsp_core::DiagnosticSeverity::Warning => self.colors.literal,
            _ => self.colors.muted,
        };
        let label = format!(
            "{} {}",
            match diagnostic.severity {
                lsp_core::DiagnosticSeverity::Error => self.tr("error:", "错误："),
                lsp_core::DiagnosticSeverity::Warning => self.tr("warning:", "警告："),
                _ => self.tr("note:", "提示："),
            },
            diagnostic.message
        );
        let short: String = label.chars().take(120).collect();
        self.text(colour, s(420), self.height - s(10), &short);
    }

    /// Show what the server said about the symbol under the caret.
    ///
    /// # Safety
    /// Requires a live Cairo context.
    unsafe fn draw_hover(&self) {
        let Some(text) = self.lsp_hover.as_ref() else {
            return;
        };
        let s = |v| self.s(v);
        // The panel sits under the caret's line, kept inside the editor.
        let left = (self.width as f32 * 0.17) as i32 + s(60);
        let y = s(119) + self.caret_line as i32 * self.code_line_step() + s(24);
        let width = (text.chars().count() as i32 * s(8) + s(24)).min(self.width - left - s(24));
        let lines = ((width - s(20)) / s(8).max(1)).max(1) as usize;
        let wrapped = text.chars().count().div_ceil(lines).max(1) as i32;
        let height = wrapped * s(20) + s(12);
        self.rect(self.colors.panel_2, left, y, width, height);
        self.outline(self.colors.accent, left, y, width, height);
        let short: String = text.chars().take(lines * wrapped as usize).collect();
        self.text(self.colors.text, left + s(12), y + s(20), &short);
    }

    /// The completion list and the rename prompt, over the editor.
    ///
    /// # Safety
    /// Requires a live Cairo context.
    unsafe fn draw_language_overlays(&self) {
        let s = |v| self.s(v);
        if !self.lsp_completions.is_empty() {
            let left = (self.width as f32 * 0.17) as i32 + s(60);
            let top = s(119) + self.caret_line as i32 * self.code_line_step() + s(24);
            let rows = self.lsp_completions.len().min(8) as i32;
            let width = s(340);
            let height = rows * s(20) + s(10);
            self.rect(self.colors.panel_2, left, top, width, height);
            self.outline(self.colors.line, left, top, width, height);
            for (index, entry) in self.lsp_completions.iter().take(8).enumerate() {
                let row = top + s(16 + index as i32 * 20);
                if index == self.lsp_completion_index {
                    self.rect(
                        self.colors.project,
                        left + s(2),
                        row - s(13),
                        width - s(4),
                        s(19),
                    );
                }
                let short: String = entry.chars().take(44).collect();
                self.text(self.colors.text, left + s(10), row, &short);
            }
        }
        let Some(prompt) = self.rename_prompt.as_ref() else {
            return;
        };
        let width = (self.width as f32 * 0.36) as i32;
        let left = (self.width - width) / 2;
        let top = s(96);
        self.rect(self.colors.panel_2, left, top, width, s(46));
        self.outline(self.colors.accent, left, top, width, s(46));
        self.text(
            self.colors.muted,
            left + s(12),
            top + s(19),
            self.tr("Rename symbol to", "将符号重命名为"),
        );
        self.text(
            if prompt.is_empty() {
                self.colors.muted
            } else {
                self.colors.bright
            },
            left + s(12),
            top + s(40),
            if prompt.is_empty() {
                self.tr("new name", "新名称")
            } else {
                prompt.as_str()
            },
        );
    }

    /// The catalog the project supplies, if it has one.
    ///
    /// Real downloads need a real url and checksum, so the catalog is data the
    /// user writes at `<project>/.tc-ide-catalog.conf`; the project ships none
    /// of its own.
    fn project_catalog(&self) -> Result<Vec<extension_core::ExtensionSource>, String> {
        let Some(project) = self.project_path.as_ref() else {
            return Err(self
                .tr(
                    "Open a project before installing extensions",
                    "安装扩展前请先打开项目",
                )
                .into());
        };
        let path = project.join(CATALOG_FILE);
        let content = fs::read_to_string(&path).map_err(|_| {
            format!(
                "{} {}",
                self.tr("No catalog at", "没有扩展目录文件："),
                path.display()
            )
        })?;
        extension_core::parse_catalog(&content)
    }

    /// The shell commands are written for.
    fn shell_kind(&self) -> extension_core::Shell {
        extension_core::Shell::Unix
    }

    /// Where installed extensions live.
    fn extension_cache_root(&self) -> PathBuf {
        runtime_cache_root()
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."))
            .join("extensions")
    }

    /// Install an extension by planning the real download and unpack steps.
    ///
    /// The commands are typed into the terminal rather than run behind the
    /// user's back, so the download, the checksum check and the unpacking are
    /// all visible and any error they print stays on screen. A version that is
    /// already cached is reused instead of downloaded again.
    fn run_extension_install(&mut self, source: &extension_core::ExtensionSource) {
        if let Some(entry) = self.runtime_cache.reuse(&source.id, &source.version) {
            self.status_message = format!(
                "{} {} {} {}",
                source.name,
                entry.version,
                self.tr("is already cached in", "已缓存在"),
                entry.directory
            );
            return;
        }
        let root = self.extension_cache_root();
        let plan = match extension_core::install_plan(
            &root.to_string_lossy(),
            source,
            self.shell_kind(),
        ) {
            Ok(plan) => plan,
            Err(error) => {
                self.status_message = error;
                return;
            }
        };
        // The commands need a terminal to run in; one is opened if needed.
        self.terminal_open = true;
        self.terminal_focused = true;
        if self.terminals.is_empty() {
            self.status_message = self
                .tr("The terminal could not be started", "终端无法启动")
                .into();
            return;
        }
        for command in &plan.commands {
            self.run_in_terminal(command);
        }
        self.runtime_cache.record(extension_core::CacheEntry {
            id: source.id.clone(),
            version: source.version.clone(),
            sha256: source.sha256.clone(),
            bytes: 0,
            directory: plan.directory.clone(),
        });
        self.save_runtime_cache();
        self.status_message = format!(
            "{} {} {}",
            self.tr("Installing", "正在安装"),
            source.name,
            source.version
        );
    }

    /// Remove an installed extension and forget it.
    fn uninstall_extension(&mut self, id: &str) {
        let Some(entry) = self
            .runtime_cache
            .entries()
            .iter()
            .find(|entry| entry.id == id)
            .cloned()
        else {
            self.status_message = self
                .tr("That extension is not installed", "该扩展尚未安装")
                .into();
            return;
        };
        let root = self.extension_cache_root();
        for command in
            extension_core::uninstall_commands(&root.to_string_lossy(), &entry, self.shell_kind())
        {
            self.run_in_terminal(&command);
        }
        let remaining = self
            .runtime_cache
            .entries()
            .iter()
            .filter(|known| known.id != id)
            .cloned()
            .collect::<Vec<_>>();
        self.runtime_cache = extension_core::RuntimeCache::default();
        for kept in remaining {
            self.runtime_cache.record(kept);
        }
        self.save_runtime_cache();
        self.status_message = format!("{} {id}", self.tr("Uninstalled", "已卸载"));
    }

    /// Remember what is installed.
    fn save_runtime_cache(&self) {
        let path = runtime_cache_path();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::write(&path, self.runtime_cache.serialize());
    }

    /// Undo the last edit of the active document.
    fn undo_edit(&mut self) {
        let caret = (self.caret_line, self.caret_column);
        if !self.files[self.selected_file].buffer.undo() {
            self.status_message = self.tr("Nothing to undo", "没有可撤销的编辑").into();
            return;
        }
        self.files[self.selected_file].sync();
        self.clamp_caret_to_document(caret);
        self.note_edit(motion_core::EditKind::Undo, self.caret_line, 0, 0);
        self.status_message = self.tr("Undone", "已撤销").into();
    }

    /// Redo the last undone edit.
    fn redo_edit(&mut self) {
        let caret = (self.caret_line, self.caret_column);
        if !self.files[self.selected_file].buffer.redo() {
            self.status_message = self.tr("Nothing to redo", "没有可重做的编辑").into();
            return;
        }
        self.files[self.selected_file].sync();
        self.clamp_caret_to_document(caret);
        self.note_edit(motion_core::EditKind::Redo, self.caret_line, 0, 0);
        self.status_message = self.tr("Redone", "已重做").into();
    }

    /// Keep the caret inside the document after the lines changed.
    fn clamp_caret_to_document(&mut self, caret: (usize, usize)) {
        let last = self.files[self.selected_file].lines.len().saturating_sub(1);
        self.caret_line = caret.0.min(last);
        let length = self.files[self.selected_file].lines[self.caret_line].len();
        self.caret_column = caret.1.min(length);
        self.selection_anchor = None;
        self.selection_head = None;
    }

    fn delete_selection(&mut self) -> bool {
        let (Some(a), Some(b)) = (self.selection_anchor, self.selection_head) else {
            return false;
        };
        if a == b {
            return false;
        }
        let (mut start, mut end) = if a <= b { (a, b) } else { (b, a) };
        if let Some((comment_start, comment_end)) = self.active_comment_edit_bounds() {
            if start.0 != self.caret_line || end.0 != self.caret_line {
                self.selection_anchor = None;
                self.selection_head = None;
                self.status_message = self
                    .tr(
                        "Comment editing cannot delete code outside the bubble",
                        "注释编辑不能删除气泡之外的代码",
                    )
                    .into();
                return false;
            }
            start.1 = start.1.clamp(comment_start, comment_end);
            end.1 = end.1.clamp(comment_start, comment_end);
            if start == end {
                self.selection_anchor = None;
                self.selection_head = None;
                return false;
            }
        }
        // Deleting a selection is one undoable edit in the piece table.
        self.files[self.selected_file].buffer.delete(start, end);
        self.files[self.selected_file].sync();
        self.caret_line = start.0;
        self.caret_column = start.1;
        self.selection_anchor = None;
        self.selection_head = None;
        self.note_edit(
            motion_core::EditKind::Delete,
            self.caret_line,
            self.caret_column,
            self.caret_column,
        );
        true
    }

    fn insert_clipboard(&mut self) {
        if self.clipboard.is_empty() {
            return;
        }
        self.delete_selection();
        let comment_bounds = self.active_comment_edit_bounds();
        let parts = self
            .clipboard
            .split('\n')
            .map(str::to_string)
            .collect::<Vec<_>>();
        if comment_bounds.is_some() && parts.len() > 1 {
            self.status_message = self
                .tr(
                    "Multiline paste is disabled while editing one comment bubble",
                    "编辑单个注释气泡时不允许粘贴多行内容",
                )
                .into();
            return;
        }
        if parts.len() == 1 {
            let start = self.caret_column;
            self.files[self.selected_file].buffer.insert(
                self.caret_line,
                self.caret_column,
                &parts[0],
            );
            self.caret_column += parts[0].len();
            self.files[self.selected_file].sync();
            self.note_edit(
                motion_core::EditKind::Paste,
                self.caret_line,
                start,
                self.caret_column,
            );
        } else {
            // A multiline paste is one edit, so undo takes it back in one step.
            self.files[self.selected_file].buffer.insert(
                self.caret_line,
                self.caret_column,
                &self.clipboard.clone(),
            );
            self.caret_line += parts.len() - 1;
            self.caret_column = parts.last().map(|part| part.len()).unwrap_or(0);
            self.files[self.selected_file].sync();
            self.note_edit(
                motion_core::EditKind::Paste,
                self.caret_line,
                0,
                self.caret_column,
            );
        }
        self.status_message = self.tr("Pasted from clipboard", "已从剪贴板粘贴").into();
    }

    fn copy_selection(&mut self) {
        self.clipboard = self.selected_text();
        if self.clipboard.is_empty() {
            self.status_message = self.tr("Nothing is selected", "尚未选择任何内容").into();
            return;
        }
        let mut exported = false;
        for (program, arguments) in [
            ("xsel", &["--clipboard", "--input"][..]),
            ("xclip", &["-selection", "clipboard", "-in"][..]),
        ] {
            if let Ok(mut child) = Command::new(program)
                .args(arguments)
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
            {
                if let Some(mut stdin) = child.stdin.take() {
                    exported = stdin.write_all(self.clipboard.as_bytes()).is_ok();
                }
                if exported {
                    break;
                }
            }
        }
        self.status_message = if exported {
            format!(
                "{} ({} chars)",
                self.tr("Copied to system clipboard", "已复制到系统剪贴板"),
                self.clipboard.chars().count()
            )
        } else {
            self.tr(
                "Copied inside TC-IDE; system clipboard helper unavailable",
                "已复制到 TC-IDE 内部；系统剪贴板工具不可用",
            )
            .into()
        };
    }

    fn paste_clipboard(&mut self) {
        for (program, arguments) in [
            ("xsel", &["--clipboard", "--output"][..]),
            ("xclip", &["-selection", "clipboard", "-out"][..]),
        ] {
            if let Ok(output) = Command::new(program).args(arguments).output() {
                if output.status.success() && !output.stdout.is_empty() {
                    self.clipboard = String::from_utf8_lossy(&output.stdout).into_owned();
                    break;
                }
            }
        }
        if self.clipboard.is_empty() {
            self.status_message = self.tr("The clipboard is empty", "剪贴板为空").into();
            return;
        }
        self.insert_clipboard();
    }

    fn create_requested_item(&mut self) {
        let name = self.new_file_name.trim().to_string();
        if name.is_empty()
            || name.contains('/')
            || name.contains('\\')
            || name == "."
            || name == ".."
        {
            self.status_message = "Use a safe non-empty name".into();
            return;
        }
        let created = match self.create_kind.unwrap() {
            CreateKind::File => {
                let root = self.active_root();
                if let Err(error) = fs::create_dir_all(&root) {
                    self.status_message = format!("Could not prepare project folder: {error}");
                    return;
                }
                let path = root.join(&name);
                if path.exists() || self.files.iter().any(|file| file.path == path) {
                    self.status_message = self
                        .tr("A file already exists at this path", "该路径已存在同名文件")
                        .into();
                    return;
                }
                if let Err(error) = fs::write(&path, "") {
                    self.status_message = format!("Could not create file: {error}");
                    return;
                }
                let pack_index = self.language_pack_index_for_file(&name);
                self.files
                    .push(FileBuffer::new(name.clone(), path, vec![String::new()]));
                self.selected_file = self.files.len() - 1;
                self.caret_line = 0;
                self.caret_column = 0;
                self.editor_focused = true;
                self.remember_session();
                if let Some(index) = pack_index {
                    let language = self.languages[index].name;
                    if self.languages[index].installed {
                        self.status_message = format!(
                            "Created {name} - selected {language} {}",
                            self.languages[index].versions[self.languages[index].selected_version]
                        );
                    } else {
                        self.status_message=format!("Detected {language}; built-in completion active, tooling extension optional");
                    }
                } else {
                    self.status_message = format!("Created {name} - Plain Text fallback");
                }
                true
            }
            CreateKind::Folder => {
                let root = self.active_root();
                let path = root.join(&name);
                if path.exists() || self.folders.contains(&name) {
                    self.status_message = self
                        .tr(
                            "A folder already exists at this path",
                            "该路径已存在同名文件夹",
                        )
                        .into();
                    false
                } else {
                    match fs::create_dir_all(&path) {
                        Ok(_) => {
                            self.folders.push(name.clone());
                            self.status_message = format!("Created folder {name}");
                            true
                        }
                        Err(error) => {
                            self.status_message = format!("Could not create folder: {error}");
                            false
                        }
                    }
                }
            }
            CreateKind::Project => self.create_project(&name),
        };
        if created {
            self.create_kind = None;
            self.project_language_menu = false;
            self.project_environment_menu = false;
        }
    }

    fn create_project(&mut self, name: &str) -> bool {
        let root = expand_home(&self.project_location).join(name);
        if root.exists() {
            self.status_message = if self.ui_chinese {
                format!("该路径已有项目：{}", root.display())
            } else {
                format!("A project already exists at {}", root.display())
            };
            return false;
        }
        if let Err(error) = fs::create_dir_all(&root) {
            self.status_message = format!("Could not create project: {error}");
            return false;
        }
        let language = self.languages[self.project_language].name;
        let envs = project_environments(language);
        let environment = envs[self.project_environment.min(envs.len() - 1)];
        let version = self.languages[self.project_language].versions
            [self.languages[self.project_language].selected_version];
        let cache = if language == "Python" {
            Some(ensure_python_runtime_cache(version))
        } else {
            None
        };
        let manifest=format!("name={name}\nlanguage={language}\nversion={version}\nenvironment={environment}\nruntime_cache={}\n",cache.as_ref().map(|path|path.display().to_string()).unwrap_or_default());
        let _ = fs::write(root.join(".tc-ide-project"), manifest);
        if language == "Python" && environment == "venv" {
            let interpreter = python_interpreter(version);
            let _ = Command::new(interpreter)
                .args(["-m", "venv", ".venv"])
                .current_dir(&root)
                .spawn();
        }
        let template = workspace_core::project_template(name, language, version, environment);
        let mut folders = Vec::new();
        for file in &template.files {
            let path = root.join(&file.path);
            if let Some(parent) = path.parent().filter(|parent| *parent != root) {
                let _ = fs::create_dir_all(parent);
                if let Ok(relative) = parent.strip_prefix(&root) {
                    let relative = relative.to_string_lossy().to_string();
                    if !relative.is_empty() && !folders.contains(&relative) {
                        folders.push(relative);
                    }
                }
            }
            if let Err(error) = fs::write(&path, &file.contents) {
                self.status_message = format!("Could not write project template: {error}");
                return false;
            }
        }
        if language == "Python" && environment == "venv" {
            folders.push(".venv".to_string());
        }
        let primary = template
            .files
            .iter()
            .rev()
            .find(|file| !matches!(file.path.as_str(), "Cargo.toml" | "go.mod" | ".gitignore"))
            .or_else(|| template.files.first())
            .expect("project templates always contain a file");
        let file_path = root.join(&primary.path);
        let file_name = primary.path.clone();
        let content = primary.contents.clone();
        self.project_name = name.to_string();
        self.project_path = Some(root.clone());
        self.folders = folders;
        self.project_tree_expanded = true;
        self.project_tree_amount = 1.0;
        self.project_tree_velocity = 0.0;
        self.folder_tree_motion.clear();
        self.collapsed_folders.clear();
        self.workspace_search.clear();
        self.files = vec![FileBuffer::new(
            file_name,
            file_path,
            content
                .lines()
                .map(str::to_string)
                .chain(std::iter::once(String::new()))
                .collect(),
        )];
        self.selected_file = 0;
        self.caret_line = 0;
        self.caret_column = 0;
        self.graph_current = true;
        self.rebuild_project_index();
        self.remember_session();
        self.load_project_package_targets();
        self.reset_terminals_for_project();
        self.status_message = format!(
            "Project {name} created at {} with {language} {version} / {environment}",
            root.display()
        );
        true
    }

    /// Extend a terminal selection while the pointer is down.
    fn drag_terminal_selection(&mut self, x: i32, y: i32) {
        if !self.terminal_selecting {
            return;
        }
        let Some(point) = self.terminal_point_at(x, y) else {
            return;
        };
        if let Some(selection) = self.terminal_selection.as_ref() {
            self.terminal_selection = Some(selection.extended_to(point));
        }
    }

    /// The pointer was released: the selection stays, dragging stops.
    fn end_terminal_selection(&mut self) {
        self.terminal_selecting = false;
    }

    fn handle_click(&mut self, x: i32, y: i32, button: u32, state: c_uint) {
        let scale = self.scale();
        let s = move |v: i32| (v as f32 * scale).round() as i32;
        let graph_left = self.graph_left();
        if let Some((menu_x, menu_y)) = self.workspace_context_menu {
            if button == 1
                && x >= menu_x
                && x <= menu_x + s(190)
                && y >= menu_y + s(29)
                && y <= menu_y + s(92)
            {
                let kind = if y < menu_y + s(66) {
                    CreateKind::File
                } else {
                    CreateKind::Folder
                };
                self.create_kind = Some(kind);
                self.new_file_name.clear();
                self.create_caret = 0;
                self.status_message = self.tr("Enter a name", "请输入名称").into();
                self.workspace_context_menu = None;
                return;
            }
            if button != 3 {
                self.workspace_context_menu = None;
            }
        }
        if self.settings_open {
            if let Some(image) = self.background_image_size() {
                if let Some(handle) = self.background_handle_at(x, y, image) {
                    self.background_drag = Some((handle, x, y));
                    return;
                }
            }
        }
        if let Some(kind) = self.create_kind {
            let project = kind == CreateKind::Project;
            let w = s(if project { 680 } else { 520 });
            let h = s(if project { 500 } else { 230 });
            let dx = (self.width - w) / 2;
            let dy = (self.height - h) / 2;
            let button_y = dy + h - s(52);
            if button == 1 && y >= button_y && y <= button_y + s(34) {
                if x >= dx + w - s(238) && x <= dx + w - s(136) {
                    self.create_kind = None;
                    self.project_language_menu = false;
                    self.project_environment_menu = false;
                    return;
                }
                if x >= dx + w - s(126) && x <= dx + w - s(22) {
                    self.create_requested_item();
                    return;
                }
            }
            if x >= dx + s(22) && x <= dx + w - s(22) && y >= dy + s(82) && y <= dy + s(122) {
                self.project_location_focused = false;
                self.create_caret =
                    self.byte_index_at_ui_x(&self.new_file_name, (x - dx - s(35)).max(0));
                return;
            }
            if project {
                if self.project_language_menu {
                    for visible in 0..6.min(self.languages.len()) {
                        let row = dy + s(238 + visible as i32 * 34);
                        if (y - row).abs() < s(16) {
                            self.project_language = visible;
                            self.project_environment = 0;
                            self.project_language_menu = false;
                            return;
                        }
                    }
                }
                if self.project_environment_menu {
                    let envs = project_environments(self.languages[self.project_language].name);
                    for index in 0..envs.len() {
                        let row = dy + s(238 + index as i32 * 34);
                        if (y - row).abs() < s(16) {
                            self.project_environment = index;
                            self.project_environment_menu = false;
                            return;
                        }
                    }
                }
                if x >= dx + s(22) && x <= dx + s(322) && y >= dy + s(171) && y <= dy + s(211) {
                    self.project_language_menu = !self.project_language_menu;
                    self.project_environment_menu = false;
                    return;
                }
                if x >= dx + s(350) && x <= dx + s(658) && y >= dy + s(171) && y <= dy + s(211) {
                    self.project_environment_menu = !self.project_environment_menu;
                    self.project_language_menu = false;
                    return;
                }
                if x >= dx + s(22) && x <= dx + s(522) && y >= dy + s(260) && y <= dy + s(300) {
                    self.project_location_focused = true;
                    return;
                }
                if x >= dx + s(532) && x <= dx + s(658) && y >= dy + s(260) && y <= dy + s(300) {
                    self.choose_project_location();
                    return;
                }
            }
            return;
        }
        if self.extensions_open && self.extension_category == 0 && (button == 4 || button == 5) {
            let filtered = self.filtered_language_indices();
            let visible = 10;
            let maximum = filtered.len().saturating_sub(visible);
            if button == 4 {
                self.extension_scroll = self.extension_scroll.saturating_sub(1);
            } else {
                self.extension_scroll = (self.extension_scroll + 1).min(maximum);
            }
            return;
        }
        if state & CONTROL_MASK != 0 && (button == 4 || button == 5) {
            if button == 4 {
                self.code_zoom = (self.code_zoom + 1).min(8);
            } else {
                self.code_zoom = (self.code_zoom - 1).max(-2);
            }
            self.reload_code_font();
            return;
        }
        let workspace_right = (self.width as f32 * 0.17) as i32;
        let debug_x = workspace_right + s(174);
        if button == 1 && y >= s(52) && y <= s(77) && x >= debug_x && x < debug_x + s(312) {
            match ((x - debug_x) / s(78).max(1)).clamp(0, 3) {
                0 => self.toggle_breakpoint(),
                1 => self.start_or_continue_debugging(),
                2 => self.step_debugging(),
                _ => self.stop_debugging(),
            }
            return;
        }
        if button == 1 && self.terminal_open && (y - self.terminal_top()).abs() <= s(7) {
            self.panel_drag = Some(PanelDrag::Terminal);
            self.editor_focused = false;
            return;
        }
        if button == 1
            && self.graph_visible
            && (x - graph_left).abs() <= s(7)
            && y >= s(46)
            && y < self.terminal_top()
        {
            self.panel_drag = Some(PanelDrag::CallHierarchy);
            self.editor_focused = false;
            return;
        }
        if button == 1 && x >= graph_left && y >= s(52) && y <= s(77) {
            let mode_w = s(73);
            let modes_x = self.width - mode_w * 3 - s(12);
            if x >= modes_x {
                self.graph_relation = graph_core::GraphRelation::ALL
                    [((x - modes_x) / mode_w.max(1)).clamp(0, 2) as usize];
                self.status_message = match self.graph_relation.index() {
                    1 => self.tr(
                        "Showing callers of the focused symbol",
                        "正在显示焦点符号的调用者",
                    ),
                    2 => self.tr(
                        "Showing callees of the focused symbol",
                        "正在显示焦点符号调用的内容",
                    ),
                    _ => self.tr(
                        "Graph follows the symbol under the pointer",
                        "关系图自动跟随鼠标所指符号",
                    ),
                }
                .into();
                return;
            }
        }
        let (_, graph_y, _, _) = self.graph_bounds();
        if button == 1 && x >= graph_left && y >= graph_y + s(8) && y <= graph_y + s(35) {
            // The buttons are AUTO plus the four ranges the shared model
            // lists, so the count comes from there rather than a literal.
            let count = (graph_core::GraphScope::ALL.len() + 1) as i32;
            let scope_w = ((self.width - graph_left - s(36)) / count).max(s(56));
            let index = ((x - graph_left - s(14)) / scope_w.max(1)).max(0) as usize;
            let labels = self.graph_scope_labels();
            let tapped = labels.get(index).copied().unwrap_or("");
            if !self.set_graph_scope_from_button(index) {
                return;
            }
            self.status_message = format!("{} {tapped}", self.tr("Range:", "范围："));
            return;
        }
        if button == 1 && x >= graph_left + s(14) && y >= graph_y + s(43) && y <= graph_y + s(73) {
            self.graph_search_focused = true;
            self.editor_focused = false;
            return;
        }
        if button == 1 && x >= graph_left && y >= graph_y + s(102) {
            let graph = self.graph();
            let mut action: Option<(usize, bool)> = None;
            for index in self.graph_visible(&graph) {
                let (nx, ny) = self.node_position(&graph, index);
                if (y - ny).abs() <= s(20) {
                    let marker_hit = (x - (nx + s(24))).abs() <= s(16);
                    action = Some((index, marker_hit));
                    break;
                }
            }
            if let Some((index, marker_hit)) = action {
                self.hovered = Some(index);
                if let Some(node) = graph.nodes.get(index) {
                    if node.has_children()
                        && (marker_hit || node.level == graph_core::GraphLevel::File)
                    {
                        let key = node.key.clone();
                        let label = node.label.clone();
                        if !self.graph_folded.remove(&key) {
                            self.graph_folded.insert(key);
                            self.status_message =
                                format!("{} {}", self.tr("Collapsed", "已折叠"), label);
                        } else {
                            self.status_message =
                                format!("{} {}", self.tr("Expanded", "已展开"), label);
                        }
                        self.graph_scroll = 0;
                    } else if let Some(target) = node.target.clone() {
                        let opened = self.focus_graph_target(&target);
                        self.status_message = if opened {
                            format!(
                                "{} {}:{}",
                                self.tr("Jumped to relation at", "已跳转到关系位置"),
                                target.file,
                                target.line + 1
                            )
                        } else {
                            format!(
                                "{} {}",
                                self.tr("File is not open:", "文件未打开："),
                                target.file
                            )
                        };
                    }
                    self.focus_graph_node(node);
                }
            }
            return;
        }
        if x >= graph_left && button == 4 {
            self.graph_scroll = self.graph_scroll.saturating_sub(1);
            return;
        }
        if x >= graph_left && button == 5 {
            let graph = self.graph();
            let rows = self.graph_rows(&graph);
            self.graph_scroll = (self.graph_scroll + 1).min(graph.max_scroll(rows));
            return;
        }
        {
            let (gx, gy, gw, gh) = self.graph_toggle_rect();
            if self.graph_visible && x >= gx && x <= gx + gw && y >= gy && y <= gy + gh {
                self.graph_visible = false;
                self.status_message = self.tr("Call hierarchy hidden", "已隐藏调用层级").into();
                return;
            }
        }
        if x >= s(112) && x <= s(200) && y < s(46) {
            self.new_menu_open = !self.new_menu_open;
            self.edit_menu_open = false;
            self.settings_open = false;
            self.extensions_open = false;
            self.package_open = false;
            self.editor_focused = false;
            return;
        }
        if x >= s(207) && x <= s(295) && y < s(46) {
            self.edit_menu_open = !self.edit_menu_open;
            self.new_menu_open = false;
            self.view_menu_open = false;
            self.settings_open = false;
            self.extensions_open = false;
            self.package_open = false;
            return;
        }
        if x >= s(302) && x <= s(390) && y < s(46) {
            self.view_menu_open = !self.view_menu_open;
            self.new_menu_open = false;
            self.edit_menu_open = false;
            self.settings_open = false;
            self.extensions_open = false;
            self.package_open = false;
            self.editor_focused = false;
            return;
        }
        if self.view_menu_open {
            let menu_x = s(302);
            let menu_y = s(43);
            if button == 1
                && x >= menu_x
                && x <= menu_x + s(280)
                && y >= menu_y + s(29)
                && y <= menu_y + s(51 + 36 * 3) - s(11)
            {
                let index = ((y - menu_y - s(29)) / s(36).max(1)).clamp(0, 2);
                if index == 0 {
                    self.graph_visible = !self.graph_visible;
                    self.status_message = self
                        .tr(
                            if self.graph_visible {
                                "Call hierarchy shown"
                            } else {
                                "Call hierarchy hidden"
                            },
                            if self.graph_visible {
                                "已显示调用层级"
                            } else {
                                "已隐藏调用层级"
                            },
                        )
                        .into();
                } else if index == 1 {
                    self.terminal_open = !self.terminal_open;
                    self.terminal_focused = self.terminal_open;
                } else {
                    self.toggle_debug_panel();
                }
                return;
            }
            self.view_menu_open = false;
        }
        if self.edit_menu_open {
            let menu_x = s(207);
            let menu_y = s(43);
            // Eight rows now, so the region has to match the panel it opens.
            if x >= menu_x && x <= menu_x + s(260) && y >= menu_y && y <= menu_y + s(244) {
                // The rows are the labels the menu draws, in the same order.
                let index = ((y - menu_y - s(17)) / s(27).max(1)).clamp(0, 7);
                match index {
                    0 => self.undo_edit(),
                    1 => self.redo_edit(),
                    2 => self.save_current_file(),
                    3 => {
                        let last = self.files[self.selected_file].lines.len() - 1;
                        self.selection_anchor = Some((0, 0));
                        self.selection_head =
                            Some((last, self.files[self.selected_file].lines[last].len()));
                    }
                    4 => self.copy_selection(),
                    5 => self.paste_clipboard(),
                    6 => {
                        self.code_zoom = (self.code_zoom + 1).min(8);
                        self.reload_code_font();
                    }
                    _ => {
                        self.code_zoom = (self.code_zoom - 1).max(-2);
                        self.reload_code_font();
                    }
                }
                self.edit_menu_open = false;
                return;
            }
            self.edit_menu_open = false;
        }
        if self.new_menu_open {
            let menu_x = s(112);
            let menu_w = s(300);
            let rows = self.file_menu_rows();
            for (index, row) in rows.iter().enumerate() {
                if !row.is_action() {
                    continue;
                }
                let row_y = self.file_menu_row_y(index);
                if x >= menu_x && x <= menu_x + menu_w && (y - row_y).abs() <= s(14) {
                    self.new_menu_open = false;
                    match row {
                        FileMenuRow::Create(kind) => {
                            self.create_kind = Some(*kind);
                            self.new_file_name.clear();
                            self.create_caret = 0;
                            self.project_location_focused = false;
                            self.status_message = self.tr("Enter a name", "请输入名称").into();
                        }
                        FileMenuRow::OpenFile => self.open_file_dialog(),
                        FileMenuRow::OpenFolder => self.open_folder_dialog(),
                        FileMenuRow::Recent(path) => self.open_project_at(path.clone()),
                        FileMenuRow::Heading(..) => {}
                    }
                    return;
                }
            }
            self.new_menu_open = false;
        }
        if x >= self.width - s(410) && x <= self.width - s(290) && y < s(46) {
            self.settings_open = !self.settings_open;
            self.new_menu_open = false;
            self.edit_menu_open = false;
            self.extensions_open = false;
            self.package_open = false;
            self.editor_focused = false;
            return;
        }
        if self.settings_open {
            let menu_x = self.width - s(410);
            let menu_y = s(43);
            if button == 1 && x >= menu_x && x <= menu_x + s(390) {
                // Every button of this panel is one control with one rectangle,
                // so the pressed state and the click cannot disagree.
                if let Some(control) = self.settings_control_at(x, y) {
                    self.settings_pressed = Some(control);
                    self.settings_focus = Some(control);
                    self.activate_settings_control(control);
                    return;
                }
                if x >= menu_x + s(180)
                    && x <= menu_x + s(372)
                    && y >= menu_y + s(163)
                    && y <= menu_y + s(197)
                {
                    self.font_search_focused = true;
                    return;
                }
                for (visible, index) in self.filtered_font_indices().into_iter().take(5).enumerate()
                {
                    let row = menu_y + s(226 + visible as i32 * 30);
                    if y >= row - s(20) && y <= row + s(7) {
                        self.code_font_family = index;
                        self.reload_code_font();
                        self.status_message = format!(
                            "{}: {}",
                            self.tr("Editor font", "编辑器字体"),
                            CODE_FONT_FAMILIES[index]
                        );
                        return;
                    }
                }
                if y >= menu_y + s(386) && y <= menu_y + s(414) {
                    if x >= menu_x + s(180) && x <= menu_x + s(214) {
                        self.code_zoom = (self.code_zoom - 1).max(-2);
                        self.reload_code_font();
                        return;
                    }
                    if x >= menu_x + s(300) && x <= menu_x + s(334) {
                        self.code_zoom = (self.code_zoom + 1).min(8);
                        self.reload_code_font();
                        return;
                    }
                }
            }
            self.settings_pressed = None;
            self.font_search_focused = false;
            self.settings_open = false;
        }
        if x > self.width - s(290) && x < self.width - s(145) && y < s(46) {
            self.extensions_open = !self.extensions_open;
            self.new_menu_open = false;
            self.edit_menu_open = false;
            self.settings_open = false;
            self.package_open = false;
            self.editor_focused = false;
            return;
        }
        if x > self.width - s(150) && y < s(46) {
            self.package_open = !self.package_open;
            self.new_menu_open = false;
            self.edit_menu_open = false;
            self.settings_open = false;
            self.extensions_open = false;
            self.editor_focused = false;
            return;
        }
        if self.extensions_open {
            let w = s(950);
            let h = s(680);
            let menu_x = self.width - w - s(20);
            let menu_y = s(52);
            if x >= menu_x && x <= menu_x + w && y >= menu_y && y <= menu_y + h {
                for category in 0..5 {
                    let row = menu_y + s(101 + category as i32 * 43);
                    if x <= menu_x + s(175) && (y - row).abs() < s(19) {
                        self.extension_category = category;
                        self.extension_search_focused = false;
                        self.ai_field_focused = None;
                        self.version_menu_open = false;
                        return;
                    }
                }
                if self.extension_category != 0 {
                    if self.extension_category == 1 {
                        for index in 0..2 {
                            let row = menu_y + s(116 + index * 52);
                            if x >= menu_x + s(195)
                                && x <= menu_x + s(510)
                                && (y - row).abs() < s(22)
                            {
                                self.set_light_theme(index == 1);
                                return;
                            }
                        }
                    } else if self.extension_category == 4 {
                        let field_x = menu_x + s(195);
                        for index in 0..3 {
                            let field_y = menu_y + s(159 + index as i32 * 92);
                            if x >= field_x
                                && x <= field_x + s(600)
                                && y >= field_y
                                && y <= field_y + s(42)
                            {
                                self.ai_field_focused = Some(index);
                                return;
                            }
                        }
                        if x >= field_x
                            && x <= field_x + s(210)
                            && y >= menu_y + s(440)
                            && y <= menu_y + s(480)
                        {
                            if self.ai_endpoint.trim().is_empty() || self.ai_model.trim().is_empty()
                            {
                                self.status_message = self
                                    .tr("Endpoint and model are required", "接口地址和模型不能为空")
                                    .into();
                            } else {
                                save_ai_preferences(&self.ai_endpoint, &self.ai_model);
                                self.status_message = self
                                    .tr(
                                        "AI API settings saved; API key kept in memory for this session",
                                        "AI API 设置已保存；API Key 仅保留在本次运行内存中",
                                    )
                                    .into();
                            }
                            return;
                        }
                    } else {
                        for index in 0..3 {
                            let row = menu_y + s(116 + index * 52);
                            if x >= menu_x + s(195)
                                && x <= menu_x + s(510)
                                && (y - row).abs() < s(22)
                            {
                                self.status_message = self
                                    .tr(
                                        "This adapter is not connected yet",
                                        "该适配器目前尚未接入，不能使用",
                                    )
                                    .into();
                                return;
                            }
                        }
                    }
                    return;
                }
                let list_x = menu_x + s(195);
                if x >= list_x
                    && x <= list_x + s(310)
                    && y >= menu_y + s(82)
                    && y <= menu_y + s(120)
                {
                    self.extension_search_focused = true;
                    return;
                }
                let filtered = self.filtered_language_indices();
                for visible in 0..10 {
                    let position = self.extension_scroll + visible;
                    if position >= filtered.len() {
                        break;
                    }
                    let index = filtered[position];
                    let row = menu_y + s(153 + visible as i32 * 47);
                    if x >= list_x && x <= list_x + s(310) && (y - row).abs() < s(20) {
                        self.selected_language = Some(index);
                        self.version_menu_open = false;
                        self.extension_search_focused = false;
                        return;
                    }
                }
                if let Some(index) = self.selected_language {
                    let dx = menu_x + s(555);
                    if self.version_menu_open {
                        let versions = self.languages[index].versions;
                        for version in 0..versions.len() {
                            let row = menu_y + s(274 + version as i32 * 34);
                            if x >= dx && x <= dx + s(355) && (y - row).abs() < s(16) {
                                if self.languages[index].selected_version != version
                                    && self.languages[index].installed
                                {
                                    self.languages[index].installed = false;
                                    save_language_settings(&self.languages);
                                    self.status_message =
                                        "Version changed; download the selected language extension"
                                            .into();
                                }
                                self.languages[index].selected_version = version;
                                self.version_menu_open = false;
                                return;
                            }
                        }
                    }
                    if x >= dx && x <= dx + s(355) && y >= menu_y + s(207) && y <= menu_y + s(247) {
                        self.version_menu_open = !self.version_menu_open;
                        return;
                    }
                    if x >= dx && x <= dx + s(355) && y >= menu_y + s(282) && y <= menu_y + s(320) {
                        if self.languages[index].installed {
                            // Removing means both the panel state and the
                            // files that were installed for it.
                            let name = self.languages[index].name.to_string();
                            self.uninstall_extension(&name);
                            self.languages[index].installed = false;
                            self.status_message = format!(
                                "{} {name}",
                                self.tr(
                                    "Language extension removed; built-in completion remains active for",
                                    "已移除语言扩展；以下语言仍使用内置补全："
                                )
                            );
                            save_language_settings(&self.languages);
                        } else {
                            // A real install needs a real url and checksum, so
                            // the catalog is the project's to supply; without
                            // one the button says so rather than pretending.
                            let name = self.languages[index].name.to_string();
                            match self.project_catalog() {
                                Ok(catalog) => {
                                    // A catalog entry names the language it
                                    // belongs to, so the row picks its entry.
                                    let wanted =
                                        catalog.iter().find(|source| source.name == name).cloned();
                                    match wanted {
                                        Some(source) => self.run_extension_install(&source),
                                        None => {
                                            self.status_message = format!(
                                                "{} {name}",
                                                self.tr(
                                                    "The project catalog has no entry for",
                                                    "项目目录中没有该语言的条目："
                                                )
                                            )
                                        }
                                    }
                                }
                                Err(message) => self.status_message = message,
                            }
                        }
                        return;
                    }
                }
                return;
            }
            self.extensions_open = false;
            self.extension_search_focused = false;
        }
        if self.package_open {
            let menu_x = self.width - s(270);
            let menu_y = s(48);
            if x >= menu_x && x <= self.width - s(20) && y >= menu_y {
                for index in 0..4 {
                    let center = menu_y + s(51 + index as i32 * 31);
                    if (y - center).abs() < s(15) {
                        self.package_targets[index] = !self.package_targets[index];
                        self.save_project_package_targets();
                        return;
                    }
                }
                if y > menu_y + s(170) && self.package_targets.iter().any(|v| *v) {
                    self.build_started = Some(Instant::now());
                    self.status_message = "Building selected targets...".into();
                    self.package_open = false;
                    return;
                }
            }
            self.package_open = false;
        }
        let terminal_top = self.terminal_top();
        if y >= terminal_top {
            if !self.terminal_open {
                self.terminal_open = true;
                self.terminal_focused = true;
                return;
            }
            // A press in the body starts a selection; the header controls are
            // handled below, so they still work.
            if button == 1 && y > terminal_top + s(30) {
                if let Some(point) = self.terminal_point_at(x, y) {
                    self.terminal_selection =
                        Some(terminal_core::TerminalSelection::new(point, point));
                    self.terminal_selecting = true;
                    self.terminal_focused = true;
                    return;
                }
            }
            let (tx, ty, tw, th) = self.terminal_toggle_rect();
            if x >= tx && x <= tx + tw && y >= ty && y <= ty + th {
                self.terminal_open = false;
                self.terminal_focused = false;
                return;
            }
            if y <= terminal_top + s(30) && x < s(160) {
                self.terminal_open = false;
                self.terminal_focused = false;
                return;
            }
            let list_x = self.width - s(190);
            if x >= list_x {
                if y >= terminal_top + s(38) && y <= terminal_top + s(65) {
                    if x <= list_x + s(86) {
                        self.add_terminal();
                    } else {
                        self.pop_out_terminal();
                    }
                    return;
                }
                for index in 0..self.terminals.len().min(4) {
                    let row = terminal_top + s(79 + index as i32 * 34);
                    if (y - row).abs() <= s(16) {
                        self.terminal_active = index;
                        self.terminal_focused = true;
                        return;
                    }
                }
                if self.terminals.len() > 4 {
                    let row = terminal_top + s(79 + 4 * 34);
                    if (y - row).abs() <= s(16) {
                        self.terminal_menu_open = !self.terminal_menu_open;
                        return;
                    }
                }
                return;
            }
            if self.terminal_menu_open {
                let menu_x = list_x - s(184);
                for index in 0..self.terminals.len() {
                    let row = terminal_top + s(60 + index as i32 * 29);
                    if x >= menu_x && x <= menu_x + s(180) && (y - row).abs() < s(14) {
                        self.terminal_active = index;
                        self.terminal_menu_open = false;
                        self.terminal_focused = true;
                        return;
                    }
                }
            }
            self.terminal_focused = true;
            self.editor_focused = false;
            return;
        }
        if x < workspace_right && button == 3 && y >= s(145) && y < terminal_top {
            let menu_w = s(190);
            let menu_h = s(92);
            self.workspace_context_menu = Some((
                x.min(workspace_right - menu_w - s(4)).max(s(4)),
                y.min(terminal_top - menu_h - s(4)).max(s(48)),
            ));
            self.editor_focused = false;
            return;
        }
        if x >= workspace_right && x < graph_left && button == 1 {
            let base = s(119);
            let step = self.code_line_step();
            for index in 0..self.files[self.selected_file].lines.len() {
                let y_line = base + index as i32 * step;
                let Some(comment) =
                    self.comment_parts(&self.files[self.selected_file].lines[index])
                else {
                    continue;
                };
                let text = comment.text.to_string();
                let content_column = comment.content_column;
                let anchor = workspace_right + s(58) + self.code_text_width(comment.code);
                let expanded = self
                    .expanded_comments
                    .contains(&(self.selected_file, index));
                let layout =
                    self.comment_bubble_layout(anchor, y_line, &text, expanded, graph_left - s(18));
                if x >= layout.left
                    && x <= layout.left + layout.width
                    && y >= layout.top
                    && y <= layout.top + layout.height
                {
                    if y <= layout.top + s(27) && x <= layout.left + s(28) {
                        let key = (self.selected_file, index);
                        if self.expanded_comments.remove(&key) {
                            self.editor_focused = false;
                            self.selection_anchor = None;
                            self.selection_head = None;
                        } else {
                            self.expanded_comments.insert(key);
                        }
                        return;
                    }
                    self.expanded_comments.insert((self.selected_file, index));
                    self.editor_focused = true;
                    self.caret_line = index;
                    if y > layout.top + s(27) {
                        let visual_line = ((y - layout.top - s(38)) / s(22).max(1)).max(0) as usize;
                        let visual_column =
                            ((x - layout.left - s(12)) / s(11).max(1)).max(0) as usize;
                        let char_offset = (visual_line * layout.chars_per_line + visual_column)
                            .min(text.chars().count());
                        self.caret_column = content_column + char_to_byte_index(&text, char_offset);
                    } else {
                        self.caret_column = content_column + text.len();
                    }
                    self.status_message =
                        "Comment editing active - click text to place the caret".into();
                    return;
                }
            }
        }
        if x < workspace_right {
            if button == 1 && y >= s(102) && y <= s(154) {
                self.project_tree_expanded = !self.project_tree_expanded;
                self.status_message = if self.project_tree_expanded {
                    self.tr("Project file list expanded", "项目文件列表已展开")
                } else {
                    self.tr("Project file list collapsed", "项目文件列表已收起")
                }
                .into();
                return;
            }
            if y >= s(58) && y <= s(92) {
                self.workspace_search_focused = true;
                self.editor_focused = false;
                return;
            }
            if !self.project_tree_expanded && self.workspace_search.is_empty() {
                return;
            }
            let needle = self.workspace_search.to_ascii_lowercase();
            let mut center = 176;
            for folder in self.folders.clone() {
                let has_match = needle.is_empty()
                    || folder.to_ascii_lowercase().contains(&needle)
                    || self.files.iter().any(|file| {
                        let path = self.file_tree_path(file);
                        path.starts_with(&format!("{folder}/"))
                            && path.to_ascii_lowercase().contains(&needle)
                    });
                if !has_match {
                    continue;
                }
                if (y - s(center)).abs() < s(14) {
                    if !self.collapsed_folders.remove(&folder) {
                        self.collapsed_folders.insert(folder);
                    }
                    return;
                }
                center += 28;
            }
            for (index, file) in self.files.iter().enumerate() {
                if !self.file_visible_in_tree(file) {
                    continue;
                }
                if (y - s(center)).abs() < s(15) {
                    self.selected_file = index;
                    self.graph_scroll = 0;
                    self.caret_line = 0;
                    self.caret_column = 0;
                    self.editor_focused = false;
                    self.selection_anchor = None;
                    self.selection_head = None;
                    self.hovered = if self.graph_current && index < 3 {
                        Some(index + 1)
                    } else {
                        None
                    };
                    self.hovered_from_code = false;
                    return;
                }
                center += 29;
            }
        }
        if x >= workspace_right && x < graph_left && y > s(85) && y < self.height - s(60) {
            self.editor_focused = true;
            self.terminal_focused = false;
            self.workspace_search_focused = false;
            self.graph_search_focused = false;
            self.extension_search_focused = false;
            self.font_search_focused = false;
            self.ai_field_focused = None;
            let base = s(119);
            let step = s(28);
            let line = ((y - base + step / 2) / step).max(0) as usize;
            self.caret_line =
                line.min(self.files[self.selected_file].lines.len().saturating_sub(1));
            let text_x = workspace_right + s(52);
            self.caret_column = self.byte_index_at_code_x(
                &self.files[self.selected_file].lines[self.caret_line],
                (x - text_x).max(0),
            );
            self.selection_anchor = Some((self.caret_line, self.caret_column));
            self.selection_head = Some((self.caret_line, self.caret_column));
            return;
        }
        self.editor_focused = false;
    }

    unsafe fn handle_key(&mut self, event: &mut XKeyEvent) -> bool {
        let mut keysym = 0;
        let mut buffer = [0_i8; 32];
        let count = XLookupString(
            event,
            buffer.as_mut_ptr(),
            buffer.len() as i32,
            &mut keysym,
            ptr::null_mut(),
        );
        let bytes = buffer[..count.max(0) as usize]
            .iter()
            .map(|v| *v as u8)
            .collect::<Vec<_>>();
        let input = std::str::from_utf8(&bytes).unwrap_or("");
        let control = event.state & CONTROL_MASK != 0;
        // Ctrl+Shift+C copies the terminal selection; Ctrl+C is left alone so
        // it still interrupts the running program.
        if control
            && event.state & SHIFT_MASK != 0
            && keysym == ('C' as c_ulong | 0x20)
            && self.terminal_focused
            && self.copy_terminal_selection()
        {
            return true;
        }
        // Ctrl+G asks for a line, Ctrl+Shift+[ folds the block at the caret
        // and Ctrl+Shift+] opens every block, the keys the editor has room for.
        if control && keysym == ('G' as c_ulong | 0x20) {
            self.begin_goto_line();
            return true;
        }
        if control && keysym == ('[' as c_ulong) {
            if !self.toggle_fold(self.caret_line) {
                self.status_message = self
                    .tr("Nothing to fold on this line", "此行没有可折叠的区块")
                    .into();
            }
            return true;
        }
        if control && keysym == (']' as c_ulong) {
            self.toggle_fold_all();
            return true;
        }
        // F12 goes to the definition, Ctrl+I shows what the symbol is and
        // Ctrl+R walks the places that use it, the keys Windows uses.
        if self.editor_focused {
            // F2 renames the symbol at the caret through the server.
            if keysym == F2_KEYSYM {
                self.begin_rename();
                return true;
            }
            if keysym == F12_KEYSYM {
                self.go_to_definition();
                return true;
            }
            if control && keysym == ('I' as c_ulong | 0x20) {
                self.show_hover();
                return true;
            }
            if control && keysym == ('R' as c_ulong | 0x20) {
                if self.lsp_locations.is_empty() {
                    self.go_to_references();
                } else {
                    self.next_reference();
                }
                return true;
            }
            // Ctrl+Space asks the server what could go here, and Tab takes
            // what the list is on while it is open.
            if control && keysym == SPACE_KEYSYM {
                self.request_completions();
                return true;
            }
            if keysym == TAB_KEYSYM && !self.lsp_completions.is_empty() {
                self.accept_server_completion();
                return true;
            }
        }
        // Ctrl+Z undoes and Ctrl+Y redoes, the keys the other backends use.
        if control && self.editor_focused {
            if keysym == ('Z' as c_ulong | 0x20) {
                if event.state & SHIFT_MASK != 0 {
                    self.redo_edit();
                } else {
                    self.undo_edit();
                }
                return true;
            }
            if keysym == ('Y' as c_ulong | 0x20) {
                self.redo_edit();
                return true;
            }
        }
        if control && event.state & SHIFT_MASK != 0 && keysym == ('I' as c_ulong | 0x20) {
            self.toggle_ai();
            if self.ai_open {
                self.load_ai_history();
            }
            return true;
        }
        if keysym == F9_KEYSYM {
            // Shift+F9 conditions the breakpoint instead of toggling it, which
            // is the key Windows uses for the same thing.
            if event.state & SHIFT_MASK != 0 {
                self.open_breakpoint_condition();
            } else {
                self.toggle_breakpoint();
            }
            return true;
        }
        if keysym == F5_KEYSYM {
            if event.state & SHIFT_MASK != 0 {
                self.stop_debugging();
            } else {
                self.start_or_continue_debugging();
            }
            return true;
        }
        if keysym == F10_KEYSYM {
            self.step_debugging();
            return true;
        }
        if self.terminal_focused {
            if keysym == ESCAPE_KEYSYM {
                self.terminal_focused = false;
                return true;
            }
            if keysym == BACKSPACE_KEYSYM {
                self.terminals[self.terminal_active].input.pop();
                return true;
            }
            if keysym == RETURN_KEYSYM {
                self.run_terminal_command();
                return true;
            }
            if count > 0 && !input.chars().any(char::is_control) {
                self.terminals[self.terminal_active].input.push_str(input);
            }
            return true;
        }
        if self.goto_focused {
            if keysym == RETURN_KEYSYM {
                self.apply_goto_line();
                return true;
            }
            if keysym == ESCAPE_KEYSYM {
                self.goto_prompt = None;
                self.goto_focused = false;
                return true;
            }
            if keysym == BACKSPACE_KEYSYM {
                if let Some(prompt) = self.goto_prompt.as_mut() {
                    prompt.pop();
                }
                return true;
            }
            if count > 0 && !input.chars().any(|character| character.is_control()) {
                if let Some(prompt) = self.goto_prompt.as_mut() {
                    prompt.push_str(input);
                }
            }
            return true;
        }
        if self.rename_focused {
            if keysym == RETURN_KEYSYM {
                self.send_rename();
                return true;
            }
            if keysym == ESCAPE_KEYSYM {
                self.rename_prompt = None;
                self.rename_focused = false;
                self.status_message = self.tr("Rename cancelled", "已取消重命名").into();
                return true;
            }
            if keysym == BACKSPACE_KEYSYM {
                if let Some(prompt) = self.rename_prompt.as_mut() {
                    prompt.pop();
                }
                return true;
            }
            if count > 0 && !input.chars().any(char::is_control) {
                if let Some(prompt) = self.rename_prompt.as_mut() {
                    prompt.push_str(input);
                }
            }
            return true;
        }
        if self.ai_focused && self.ai_open {
            // Enter sends, Esc cancels a running request or closes, and the
            // rest of the typing goes into the prompt.
            if keysym == RETURN_KEYSYM {
                self.send_ai_prompt();
                return true;
            }
            if keysym == ESCAPE_KEYSYM {
                if self.ai_session.is_some() {
                    self.cancel_ai_request();
                } else {
                    self.ai_open = false;
                    self.ai_focused = false;
                }
                return true;
            }
            if keysym == BACKSPACE_KEYSYM {
                self.ai_prompt.pop();
                return true;
            }
            if count > 0 && !input.chars().any(char::is_control) {
                self.ai_prompt.push_str(input);
            }
            return true;
        }
        if self.condition_focused {
            // Enter applies, Esc clears the condition and closes.
            if keysym == RETURN_KEYSYM {
                self.apply_breakpoint_condition();
                return true;
            }
            if keysym == ESCAPE_KEYSYM {
                if let Some(prompt) = self.breakpoint_prompt.as_mut() {
                    prompt.clear();
                }
                self.apply_breakpoint_condition();
                return true;
            }
            if keysym == BACKSPACE_KEYSYM {
                if let Some(prompt) = self.breakpoint_prompt.as_mut() {
                    prompt.pop();
                }
                return true;
            }
            if count > 0 && !input.chars().any(char::is_control) {
                if let Some(prompt) = self.breakpoint_prompt.as_mut() {
                    prompt.push_str(input);
                }
            }
            return true;
        }
        if self.font_search_focused {
            if keysym == ESCAPE_KEYSYM || keysym == RETURN_KEYSYM {
                self.font_search_focused = false;
                return true;
            }
            if keysym == BACKSPACE_KEYSYM {
                self.font_search.pop();
                return true;
            }
            if count > 0 && !input.chars().any(char::is_control) {
                self.font_search.push_str(input);
            }
            return true;
        }
        if self.graph_search_focused {
            if keysym == ESCAPE_KEYSYM || keysym == RETURN_KEYSYM {
                self.graph_search_focused = false;
                return true;
            }
            if keysym == BACKSPACE_KEYSYM {
                self.graph_search.pop();
                self.graph_scroll = 0;
                return true;
            }
            if count > 0 && !input.chars().any(char::is_control) {
                self.graph_search.push_str(input);
                self.graph_scroll = 0;
            }
            return true;
        }
        if let Some(field) = self.ai_field_focused {
            if keysym == ESCAPE_KEYSYM || keysym == RETURN_KEYSYM {
                self.ai_field_focused = None;
                return true;
            }
            let value = match field {
                0 => &mut self.ai_endpoint,
                1 => &mut self.ai_model,
                _ => &mut self.ai_key,
            };
            if keysym == BACKSPACE_KEYSYM {
                value.pop();
                return true;
            }
            if count > 0 && !input.chars().any(char::is_control) {
                value.push_str(input);
            }
            return true;
        }
        if self.workspace_search_focused {
            if keysym == ESCAPE_KEYSYM || keysym == RETURN_KEYSYM {
                self.workspace_search_focused = false;
                return true;
            }
            if keysym == BACKSPACE_KEYSYM {
                self.workspace_search.pop();
                return true;
            }
            if count > 0 && !input.chars().any(char::is_control) {
                self.workspace_search.push_str(input);
            }
            return true;
        }
        if self.extension_search_focused {
            if keysym == ESCAPE_KEYSYM {
                self.extension_search_focused = false;
                return true;
            }
            if keysym == BACKSPACE_KEYSYM {
                self.extension_search.pop();
                self.extension_scroll = 0;
                return true;
            }
            if keysym == RETURN_KEYSYM {
                self.extension_search_focused = false;
                return true;
            }
            if count > 0 {
                let bytes = buffer[..count as usize]
                    .iter()
                    .map(|v| *v as u8)
                    .collect::<Vec<_>>();
                if let Ok(input) = std::str::from_utf8(&bytes) {
                    if !input.chars().any(char::is_control) {
                        self.extension_search.push_str(input);
                        self.extension_scroll = 0;
                    }
                }
            }
            return true;
        }
        if self.create_kind.is_some() {
            if keysym == ESCAPE_KEYSYM {
                if self.project_language_menu || self.project_environment_menu {
                    self.project_language_menu = false;
                    self.project_environment_menu = false;
                } else {
                    self.create_kind = None;
                }
                return true;
            }
            if self.project_location_focused {
                if keysym == BACKSPACE_KEYSYM {
                    self.project_location.pop();
                    return true;
                }
                if keysym == TAB_KEYSYM || keysym == RETURN_KEYSYM {
                    self.project_location_focused = false;
                    return true;
                }
                if count > 0 && !input.chars().any(char::is_control) {
                    self.project_location.push_str(input);
                }
                return true;
            }
            if keysym == LEFT_KEYSYM {
                self.create_caret = previous_char_boundary(&self.new_file_name, self.create_caret);
                return true;
            }
            if keysym == RIGHT_KEYSYM {
                self.create_caret = next_char_boundary(&self.new_file_name, self.create_caret);
                return true;
            }
            if keysym == BACKSPACE_KEYSYM {
                if self.create_caret > 0 {
                    let previous = previous_char_boundary(&self.new_file_name, self.create_caret);
                    self.new_file_name
                        .replace_range(previous..self.create_caret, "");
                    self.create_caret = previous;
                }
                return true;
            }
            if keysym == RETURN_KEYSYM {
                self.create_requested_item();
                return true;
            }
            if count > 0 && !input.chars().any(char::is_control) {
                self.new_file_name.insert_str(self.create_caret, input);
                self.create_caret += input.len();
            }
            return true;
        }
        if self.settings_open && !self.font_search_focused {
            // The panel is a control grid: Tab walks it, Enter and space use the
            // control the keyboard is on, and Esc leaves.
            let enabled = SettingsControl::ALL
                .into_iter()
                .filter(|control| self.settings_control_enabled(*control))
                .collect::<Vec<_>>();
            if keysym == TAB_KEYSYM && !enabled.is_empty() {
                let step = if event.state & SHIFT_MASK != 0 { -1 } else { 1 };
                let position = self
                    .settings_focus
                    .and_then(|focused| enabled.iter().position(|control| *control == focused));
                let next = match position {
                    Some(index) => (index as i32 + step).rem_euclid(enabled.len() as i32) as usize,
                    None => 0,
                };
                self.settings_focus = Some(enabled[next]);
                let label = format!("{:?}", enabled[next]);
                self.status_message =
                    format!("{} {label}", self.tr("Settings control:", "设置控件："));
                return true;
            }
            if (keysym == RETURN_KEYSYM || keysym == SPACE_KEYSYM) && self.settings_focus.is_some()
            {
                let focused = self
                    .settings_focus
                    .unwrap_or(SettingsControl::LanguageEnglish);
                self.activate_settings_control(focused);
                return true;
            }
            if self.debug_panel_open
                && self.debug_state.stopped.is_some()
                && (keysym == UP_KEYSYM || keysym == DOWN_KEYSYM)
            {
                self.step_debug_frame(if keysym == UP_KEYSYM { -1 } else { 1 });
                return true;
            }
            if keysym == ESCAPE_KEYSYM && self.settings_focus.is_some() {
                self.settings_focus = None;
                self.status_message = self
                    .tr("Left the settings controls", "已退出设置控件")
                    .into();
                return true;
            }
        }
        if keysym == ESCAPE_KEYSYM && self.lsp_hover.is_some() {
            self.lsp_hover = None;
            return true;
        }
        if keysym == ESCAPE_KEYSYM {
            if self.extensions_open
                || self.new_menu_open
                || self.edit_menu_open
                || self.settings_open
                || self.workspace_context_menu.is_some()
                || self.package_open
                || self.build_started.is_some()
            {
                self.extensions_open = false;
                self.new_menu_open = false;
                self.edit_menu_open = false;
                self.settings_open = false;
                self.workspace_context_menu = None;
                self.version_menu_open = false;
                self.package_open = false;
                self.build_started = None;
                return true;
            }
            return false;
        }
        if control {
            let shortcut = shortcut_from_keysym(keysym).unwrap_or('\0');
            match shortcut {
                's' => self.save_current_file(),
                'a' => {
                    if let Some((start, end)) = self.active_comment_edit_bounds() {
                        self.selection_anchor = Some((self.caret_line, start));
                        self.selection_head = Some((self.caret_line, end));
                    } else {
                        let last = self.files[self.selected_file].lines.len() - 1;
                        self.selection_anchor = Some((0, 0));
                        self.selection_head =
                            Some((last, self.files[self.selected_file].lines[last].len()));
                    }
                }
                'c' => self.copy_selection(),
                'x' => {
                    self.copy_selection();
                    self.delete_selection();
                }
                'v' => self.paste_clipboard(),
                '+' | '=' => {
                    self.code_zoom = (self.code_zoom + 1).min(8);
                    self.reload_code_font();
                    self.status_message = format!(
                        "{}: {}",
                        self.tr("Code font size", "代码字号"),
                        (self.code_font_base + self.code_zoom * 2).clamp(10, 36)
                    );
                }
                '-' => {
                    self.code_zoom = (self.code_zoom - 1).max(-2);
                    self.reload_code_font();
                    self.status_message = format!(
                        "{}: {}",
                        self.tr("Code font size", "代码字号"),
                        (self.code_font_base + self.code_zoom * 2).clamp(10, 36)
                    );
                }
                _ => {}
            }
            return true;
        }
        if !self.editor_focused {
            return true;
        }
        match keysym {
            TAB_KEYSYM => self.accept_completion(),
            BACKSPACE_KEYSYM => {
                if !self.delete_selection() {
                    let lower_bound = self
                        .active_comment_edit_bounds()
                        .map(|(start, _)| start)
                        .unwrap_or(0);
                    if self.caret_column > lower_bound {
                        let line = self.files[self.selected_file].lines[self.caret_line].clone();
                        let previous = previous_char_boundary(&line, self.caret_column);
                        self.files[self.selected_file].buffer.delete(
                            (self.caret_line, previous),
                            (self.caret_line, self.caret_column),
                        );
                        self.caret_column = previous;
                        self.files[self.selected_file].sync();
                    } else if lower_bound == 0 && self.caret_line > 0 {
                        // Joining with the line above is one undoable edit.
                        let previous_line_length =
                            self.files[self.selected_file].lines[self.caret_line - 1].len();
                        self.files[self.selected_file].buffer.delete(
                            (self.caret_line - 1, previous_line_length),
                            (self.caret_line, 0),
                        );
                        self.caret_line -= 1;
                        self.caret_column = previous_line_length;
                        self.files[self.selected_file].sync();
                    }
                    self.note_edit(
                        motion_core::EditKind::Delete,
                        self.caret_line,
                        self.caret_column,
                        self.caret_column,
                    );
                }
            }
            DELETE_KEYSYM => {
                if !self.delete_selection() {
                    let len = self.files[self.selected_file].lines[self.caret_line].len();
                    let upper_bound = self
                        .active_comment_edit_bounds()
                        .map(|(_, end)| end)
                        .unwrap_or(len);
                    if self.caret_column < upper_bound {
                        let next = next_char_boundary(
                            &self.files[self.selected_file].lines[self.caret_line],
                            self.caret_column,
                        );
                        self.files[self.selected_file].lines[self.caret_line]
                            .replace_range(self.caret_column..next, "");
                    } else if upper_bound == len
                        && self.caret_line + 1 < self.files[self.selected_file].lines.len()
                    {
                        let next = self.files[self.selected_file]
                            .lines
                            .remove(self.caret_line + 1);
                        self.files[self.selected_file].lines[self.caret_line].push_str(&next);
                    }
                    self.note_edit(
                        motion_core::EditKind::Delete,
                        self.caret_line,
                        self.caret_column,
                        self.caret_column,
                    );
                }
            }
            RETURN_KEYSYM => {
                if self.active_comment_edit_bounds().is_some() {
                    self.status_message = self
                        .tr(
                            "Use a block comment for multiline bubble content",
                            "单行注释气泡不能换行，多行内容请使用块注释",
                        )
                        .into();
                    return true;
                }
                self.delete_selection();
                self.files[self.selected_file].buffer.insert(
                    self.caret_line,
                    self.caret_column,
                    "\n",
                );
                self.caret_line += 1;
                self.caret_column = 0;
                self.files[self.selected_file].sync();
            }
            LEFT_KEYSYM => {
                let lower_bound = self
                    .active_comment_edit_bounds()
                    .map(|(start, _)| start)
                    .unwrap_or(0);
                if self.caret_column > lower_bound {
                    self.caret_column = previous_char_boundary(
                        &self.files[self.selected_file].lines[self.caret_line],
                        self.caret_column,
                    );
                } else if lower_bound == 0 && self.caret_line > 0 {
                    self.caret_line -= 1;
                    self.caret_column = self.files[self.selected_file].lines[self.caret_line].len();
                }
                self.selection_anchor = None;
                self.selection_head = None;
            }
            RIGHT_KEYSYM => {
                let len = self.files[self.selected_file].lines[self.caret_line].len();
                let upper_bound = self
                    .active_comment_edit_bounds()
                    .map(|(_, end)| end)
                    .unwrap_or(len);
                if self.caret_column < upper_bound {
                    self.caret_column = next_char_boundary(
                        &self.files[self.selected_file].lines[self.caret_line],
                        self.caret_column,
                    );
                } else if upper_bound == len
                    && self.caret_line + 1 < self.files[self.selected_file].lines.len()
                {
                    self.caret_line += 1;
                    self.caret_column = 0;
                }
                self.selection_anchor = None;
                self.selection_head = None;
            }
            UP_KEYSYM => {
                if self.active_comment_edit_bounds().is_some() {
                    return true;
                }
                let visual = self.files[self.selected_file].lines[self.caret_line]
                    [..self.caret_column]
                    .chars()
                    .count();
                self.caret_line = self.caret_line.saturating_sub(1);
                self.caret_column = char_to_byte_index(
                    &self.files[self.selected_file].lines[self.caret_line],
                    visual,
                );
                self.selection_anchor = None;
                self.selection_head = None;
            }
            DOWN_KEYSYM => {
                if self.active_comment_edit_bounds().is_some() {
                    return true;
                }
                let visual = self.files[self.selected_file].lines[self.caret_line]
                    [..self.caret_column]
                    .chars()
                    .count();
                self.caret_line =
                    (self.caret_line + 1).min(self.files[self.selected_file].lines.len() - 1);
                self.caret_column = char_to_byte_index(
                    &self.files[self.selected_file].lines[self.caret_line],
                    visual,
                );
                self.selection_anchor = None;
                self.selection_head = None;
            }
            _ => {
                if count > 0 && !input.chars().any(char::is_control) {
                    self.delete_selection();
                    let line = self.caret_line;
                    let start = self.caret_column;
                    // The edit goes into the piece table so it can be undone,
                    // and the lines are read back out of it.
                    self.files[self.selected_file].buffer.insert(
                        self.caret_line,
                        self.caret_column,
                        input,
                    );
                    self.caret_column += input.len();
                    self.files[self.selected_file].sync();
                    let kind = if input.chars().count() > 1 {
                        motion_core::EditKind::Paste
                    } else {
                        motion_core::EditKind::Insert
                    };
                    self.note_edit(kind, line, start, self.caret_column);
                }
            }
        }
        true
    }

    fn update_drag_selection(&mut self, x: i32, y: i32, state: c_uint) {
        if state & BUTTON1_MASK == 0 || self.selection_anchor.is_none() {
            return;
        }
        let s = |v| self.s(v);
        let left = (self.width as f32 * 0.17) as i32;
        let graph = self.graph_left();
        if x < left || x >= graph || y < s(85) {
            return;
        }
        let comment_bounds = self.active_comment_edit_bounds();
        let line = (((y - s(119) + s(14)) / s(28).max(1)).max(0) as usize)
            .min(self.files[self.selected_file].lines.len() - 1);
        let column = self.byte_index_at_code_x(
            &self.files[self.selected_file].lines[line],
            (x - left - s(52)).max(0),
        );
        if let Some((start, end)) = comment_bounds {
            let column = if line < self.caret_line {
                start
            } else if line > self.caret_line {
                end
            } else {
                column.clamp(start, end)
            };
            self.selection_head = Some((self.caret_line, column));
            self.caret_column = column;
        } else {
            self.selection_head = Some((line, column));
            self.caret_line = line;
            self.caret_column = column;
        }
    }

    fn update_panel_drag(&mut self, x: i32, y: i32, state: c_uint) {
        let Some(panel) = self.panel_drag else {
            return;
        };
        if state & BUTTON1_MASK == 0 {
            self.panel_drag = None;
            return;
        }
        match panel {
            PanelDrag::CallHierarchy => {
                let minimum = self.s(260) as f32 / self.width.max(1) as f32;
                let maximum = 1.0
                    - ((self.width as f32 * 0.17) + self.s(360) as f32) / self.width.max(1) as f32;
                self.graph_fraction = (1.0 - x as f32 / self.width.max(1) as f32)
                    .clamp(minimum, maximum.max(minimum));
                self.status_message = self
                    .tr("Resizing call hierarchy", "正在调整调用层级宽度")
                    .into();
            }
            PanelDrag::Terminal => {
                self.terminal_height =
                    (self.height - y).clamp(self.s(150), self.height - self.s(150));
                self.status_message = self.tr("Resizing terminal", "正在调整终端高度").into();
            }
        }
    }

    fn update_hover(&mut self) {
        let scale = self.scale();
        let s = move |v: i32| (v as f32 * scale).round() as i32;
        let left = (self.width as f32 * 0.17) as i32;
        let graph = self.graph_left();
        if self.mouse_x >= left && self.mouse_x < graph && self.mouse_y > s(90) {
            let row = ((self.mouse_y - s(119) + self.code_line_step() / 2)
                / self.code_line_step().max(1))
            .max(0) as usize;
            if let Some(line) = self.files[self.selected_file].lines.get(row) {
                let byte = self.byte_index_at_code_x(line, (self.mouse_x - left - s(52)).max(0));
                if let Some((start, end)) = identifier_span_at(line, byte) {
                    let symbol_x = left + s(52) + self.code_text_width(&line[..start]);
                    let symbol_right = symbol_x + self.code_text_width(&line[start..end]);
                    if self.mouse_x >= symbol_x - s(3) && self.mouse_x <= symbol_right + s(3) {
                        let symbol = &line[start..end];
                        self.hovered_symbol = editor_core::symbol_at(
                            &self.files[self.selected_file].lines,
                            &self.files[self.selected_file].name,
                            row,
                            start,
                        );
                        let _ = symbol;
                        self.hovered = None;
                        self.hovered_from_code = true;
                        self.hovered_code_span = Some((row, start, end));
                        return;
                    }
                }
            }
        }
        self.hovered_from_code = false;
        self.hovered_code_span = None;
        if self.mouse_x < graph {
            self.hovered = None;
            self.hovered_symbol = None;
            return;
        }
        let graph_model = self.graph();
        let mut best = None;
        let mut distance = self.s(40) as i64 * self.s(40) as i64;
        for index in self.graph_visible(&graph_model) {
            let (x, y) = self.node_position(&graph_model, index);
            let dx = (self.mouse_x - x) as i64;
            let dy = (self.mouse_y - y) as i64;
            let candidate = dx * dx + dy * dy;
            if candidate < distance {
                distance = candidate;
                best = Some(index);
            }
        }
        self.hovered = best;
        if let Some(node) = best.and_then(|index| graph_model.nodes.get(index)) {
            self.focus_graph_node(node);
        }
    }
}

unsafe fn color(display: *mut Display, map: Colormap, value: &str) -> c_ulong {
    let name = CString::new(value).unwrap();
    let mut screen = XColor {
        pixel: 0,
        red: 0,
        green: 0,
        blue: 0,
        flags: 0,
        pad: 0,
    };
    let mut exact = screen;
    if XAllocNamedColor(display, map, name.as_ptr(), &mut screen, &mut exact) == 0 {
        0
    } else {
        screen.pixel
    }
}

/// Collect project sources for the semantic index, bounded like the Windows backend.
fn collect_project_sources(root: &PathBuf, sources: &mut Vec<(String, Vec<String>)>) {
    fn walk(root: &PathBuf, path: &PathBuf, sources: &mut Vec<(String, Vec<String>)>) {
        if sources.len() >= 300 {
            return;
        }
        let Ok(entries) = fs::read_dir(path) else {
            return;
        };
        for entry in entries.filter_map(Result::ok) {
            if sources.len() >= 300 {
                return;
            }
            let path = entry.path();
            if path.is_dir() {
                let ignored = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .map(|name| matches!(name, ".git" | "target" | "node_modules" | ".venv"))
                    .unwrap_or(false);
                if !ignored {
                    walk(root, &path, sources);
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
    walk(root, root, sources);
}

/// Blinking caret phase. `u128::is_multiple_of` is newer than the toolchain this
/// project still builds with locally, so keep the modulo form and allow the lint.
#[allow(unknown_lints, clippy::manual_is_multiple_of)]
fn blink_phase(started: Instant) -> bool {
    (started.elapsed().as_millis() / 500) % 2 == 0
}

fn initial_files() -> Vec<FileBuffer> {
    let sources = [
        (
            "graph.rs",
            [
                "use tc_graph::{Project, Relation};",
                "",
                "// AI maps project relations in real time",
                "pub struct GraphController {",
                "    project: Project,",
                "}",
                "",
                "impl GraphController {",
                "    pub fn build_graph(&mut self) {",
                "        let files = analyze_workspace(&self.project);",
                "        let symbols = resolve_symbols(files);",
                "        render_graph(self.project.root(), symbols);",
                "    }",
                "}",
            ]
            .as_slice(),
        ),
        (
            "workspace.rs",
            [
                "use crate::workspace::Project;",
                "",
                "pub fn analyze_workspace(project: &Project) {",
                "    project.index_files();",
                "}",
                "",
                "pub fn resolve_symbols(files: Vec<File>) {",
                "    SemanticIndex::from(files);",
                "}",
            ]
            .as_slice(),
        ),
        (
            "render.rs",
            [
                "use crate::graph::SymbolNode;",
                "",
                "pub struct GraphRenderer;",
                "",
                "impl GraphRenderer {",
                "    pub fn render_graph(nodes: &[SymbolNode]) {",
                "        scene.draw(nodes);",
                "    }",
                "}",
            ]
            .as_slice(),
        ),
    ];
    sources
        .iter()
        .map(|(name, lines)| {
            FileBuffer::new(
                (*name).into(),
                PathBuf::from(name),
                lines.iter().map(|line| line.to_string()).collect(),
            )
        })
        .collect()
}

fn language_settings_path() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("tc-ide/languages.conf")
}

fn ui_settings_path() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("tc-ide/interface.conf")
}

fn background_settings_path() -> PathBuf {
    ui_settings_path().with_file_name("background.conf")
}

fn session_settings_path() -> PathBuf {
    ui_settings_path().with_file_name(workspace_core::SESSION_FILE)
}

/// Runtime cache manifest of the user level extension cache.
fn runtime_cache_path() -> PathBuf {
    ui_settings_path().with_file_name("runtimes.conf")
}

/// Name of the file a project keeps its extension catalog in.
const CATALOG_FILE: &str = ".tc-ide-catalog.conf";

/// How many cached extensions the project catalog offers a newer version of.
///
/// The catalog is user supplied data (`<project>/.tc-ide-catalog.conf`), so a
/// project without one simply has nothing to upgrade.
fn extension_upgrade_count(
    cache: &extension_core::RuntimeCache,
    project: &Option<PathBuf>,
) -> usize {
    let Some(project) = project.as_ref() else {
        return 0;
    };
    // The same file the panel installs from.
    let Ok(content) = fs::read_to_string(project.join(CATALOG_FILE)) else {
        return 0;
    };
    let Ok(catalog) = extension_core::parse_catalog(&content) else {
        return 0;
    };
    extension_core::update_plan(cache, &catalog)
        .iter()
        .filter(|plan| plan.state == extension_core::UpdateState::Upgrade)
        .count()
}

fn load_runtime_cache() -> extension_core::RuntimeCache {
    match fs::read_to_string(runtime_cache_path()) {
        Ok(content) => extension_core::RuntimeCache::parse(&content),
        Err(_) => extension_core::RuntimeCache::default(),
    }
}

/// Read the previous session, if any.
fn load_session() -> session_core::Session {
    match fs::read_to_string(session_settings_path()) {
        Ok(content) => session_core::Session::parse(&content),
        Err(_) => session_core::Session::default(),
    }
}

/// Persist the session: project, open documents and recent projects.
fn save_session(session: &session_core::Session) {
    let path = session_settings_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(path, session.serialize());
}

fn ai_settings_path() -> PathBuf {
    ui_settings_path().with_file_name("ai-api.conf")
}

fn load_ai_preferences() -> (String, String) {
    let content = fs::read_to_string(ai_settings_path()).unwrap_or_default();
    let preferences = workspace_core::parse_preferences(&content);
    (preferences.ai_endpoint, preferences.ai_model)
}

fn save_ai_preferences(endpoint: &str, model: &str) {
    let path = ai_settings_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let endpoint = endpoint.replace(['\n', '\r'], "");
    let model = model.replace(['\n', '\r'], "");
    let _ = fs::write(path, format!("endpoint={endpoint}\nmodel={model}\n"));
}

struct BackgroundPreferences {
    path: Option<PathBuf>,
    opacity: f64,
    offset_x: f64,
    offset_y: f64,
    zoom: f64,
}

fn load_background_preferences() -> BackgroundPreferences {
    let content = fs::read_to_string(background_settings_path()).unwrap_or_default();
    let preferences = workspace_core::parse_preferences(&content);
    BackgroundPreferences {
        path: env::var_os("TC_IDE_BACKGROUND")
            .map(PathBuf::from)
            .or_else(|| preferences.background_path.map(PathBuf::from)),
        opacity: preferences.background_opacity as f64,
        offset_x: preferences.background_offset_x as f64,
        offset_y: preferences.background_offset_y as f64,
        zoom: preferences.background_zoom as f64,
    }
}

fn save_background_preferences(
    path: Option<&std::path::Path>,
    opacity: f64,
    offset_x: f64,
    offset_y: f64,
    zoom: f64,
) {
    let config = background_settings_path();
    if let Some(parent) = config.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let path = path
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_default();
    let _ = fs::write(
        config,
        format!(
            "path={path}\nopacity={}\noffset_x={}\noffset_y={}\nzoom={}\n",
            opacity.clamp(0.0, 1.0),
            offset_x.clamp(-1.0, 1.0),
            offset_y.clamp(-1.0, 1.0),
            zoom.clamp(1.0, 3.0)
        ),
    );
}

struct UiPreferences {
    chinese: bool,
    font_family: usize,
    code_zoom: i32,
    light_theme: bool,
}

fn load_ui_preferences() -> UiPreferences {
    let content = fs::read_to_string(ui_settings_path()).unwrap_or_default();
    let preferences = workspace_core::parse_preferences(&content);
    let chinese = match env::var("TC_IDE_LANGUAGE").ok().as_deref() {
        Some("zh-CN") | Some("zh") => true,
        Some("en") | Some("en-US") => false,
        _ => preferences.ui_chinese,
    };
    UiPreferences {
        chinese,
        font_family: preferences
            .code_font_family
            .min(CODE_FONT_FAMILIES.len() - 1),
        code_zoom: preferences.code_zoom,
        light_theme: preferences.light_theme,
    }
}

fn save_ui_preferences(chinese: bool, font_family: usize, code_zoom: i32, light_theme: bool) {
    let path = ui_settings_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let language = if chinese { "zh-CN" } else { "en" };
    let _ = fs::write(
        path,
        format!(
            "language={language}\nfont_family={}\ncode_zoom={}\ntheme={}\n",
            font_family.min(CODE_FONT_FAMILIES.len() - 1),
            code_zoom.clamp(-2, 8),
            if light_theme { "light" } else { "dark" }
        ),
    );
}

fn bundled_font_path() -> Option<PathBuf> {
    let relative = PathBuf::from("assets/fonts/JetBrainsMono-Regular.ttf");
    let installed = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(|parent| parent.join(&relative)));
    installed.filter(|path| path.exists()).or_else(|| {
        let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative);
        source.exists().then_some(source)
    })
}

fn bundled_asset_path(relative: &str) -> Option<PathBuf> {
    let relative = PathBuf::from(relative);
    let installed = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(|parent| parent.join(&relative)));
    installed.filter(|path| path.exists()).or_else(|| {
        let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative);
        source.exists().then_some(source)
    })
}

unsafe fn register_bundled_fonts() {
    let Some(path) = bundled_font_path() else {
        return;
    };
    let Ok(path) = CString::new(path.to_string_lossy().as_bytes()) else {
        return;
    };
    let config = FcConfigGetCurrent();
    if !config.is_null() && FcConfigAppFontAddFile(config, path.as_ptr().cast()) != 0 {
        FcConfigBuildFonts(config);
    }
}

fn projects_root() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("TC-IDE-Projects")
}

fn default_terminals() -> Vec<TerminalSession> {
    ["Build", "Run", "Output"]
        .iter()
        .map(|name| TerminalSession {
            name: (*name).into(),
            input: String::new(),
            output: Arc::new(Mutex::new(terminal_banner())),
        })
        .collect()
}

fn terminal_banner() -> Vec<String> {
    [
        "",
        "  [ T I A N C H U A N G Y A ]",
        "  ===========================",
        "  TC-IDE TERMINAL",
        "  GitHub: https://github.com/tianchuangya/TC-IDE",
        "  Native semantic workspace",
        "",
    ]
    .iter()
    .map(|line| (*line).to_string())
    .collect()
}

fn expand_home(value: &str) -> PathBuf {
    if value == "~" {
        return std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
    }
    if let Some(rest) = value.strip_prefix("~/") {
        return std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."))
            .join(rest);
    }
    PathBuf::from(value)
}

fn runtime_cache_root() -> PathBuf {
    std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("tc-ide/runtimes")
}

fn python_interpreter(version: &str) -> String {
    let candidate = format!("python{version}");
    if Command::new(&candidate)
        .arg("--version")
        .output()
        .map(|result| result.status.success())
        .unwrap_or(false)
    {
        candidate
    } else {
        "python3".into()
    }
}

fn ensure_python_runtime_cache(version: &str) -> PathBuf {
    let root = runtime_cache_root().join("python").join(version);
    let _ = fs::create_dir_all(&root);
    let interpreter = python_interpreter(version);
    let _ = fs::write(
        root.join("runtime.conf"),
        format!("version={version}\ninterpreter={interpreter}\n"),
    );
    root
}

fn comment_markers_for_file(name: &str) -> &'static [&'static str] {
    let extension = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    match extension.as_str() {
        "py" | "pyw" | "rb" | "sh" | "bash" | "zsh" | "fish" | "r" => &["#"],
        "sql" | "lua" | "hs" | "lhs" => &["--"],
        "html" | "htm" | "md" | "mdx" => &["<!--"],
        "css" | "scss" | "sass" => &["/*"],
        "c" | "h" | "cc" | "cpp" | "cxx" | "hpp" | "rs" | "java" | "kt" | "kts" | "swift"
        | "go" | "cs" | "ts" | "tsx" | "js" | "jsx" | "mjs" | "dart" | "php" | "scala" | "vue"
        | "svelte" => &["//", "/*"],
        _ => &[],
    }
}

fn comment_content_bounds(line: &str, file: &str) -> Option<(usize, usize, usize)> {
    let (marker_start, marker) = comment_markers_for_file(file)
        .iter()
        .filter_map(|marker| line.find(marker).map(|index| (index, *marker)))
        .min_by_key(|(index, _)| *index)?;
    if marker == "#" && marker_start == 0 && line.starts_with("#!") {
        return None;
    }
    let marker_end = marker_start + marker.len();
    let after = &line[marker_end..];
    let leading = after.len() - after.trim_start().len();
    let content_start = marker_end + leading;
    let closing = match marker {
        "/*" => "*/",
        "<!--" => "-->",
        _ => "",
    };
    let raw_end = if closing.is_empty() {
        line.len()
    } else {
        after
            .find(closing)
            .map(|offset| marker_end + offset)
            .unwrap_or(line.len())
    };
    let content_end = content_start + line[content_start..raw_end].trim_end().len();
    Some((marker_start, content_start, content_end))
}

fn char_to_byte_index(value: &str, character: usize) -> usize {
    value
        .char_indices()
        .nth(character)
        .map(|(index, _)| index)
        .unwrap_or(value.len())
}

fn previous_char_boundary(value: &str, index: usize) -> usize {
    value[..index.min(value.len())]
        .char_indices()
        .last()
        .map(|(position, _)| position)
        .unwrap_or(0)
}

fn next_char_boundary(value: &str, index: usize) -> usize {
    if index >= value.len() {
        return value.len();
    }
    value[index..]
        .char_indices()
        .nth(1)
        .map(|(offset, _)| index + offset)
        .unwrap_or(value.len())
}

fn identifiers_in(value: &str) -> Vec<String> {
    value
        .split(|character: char| !(character.is_ascii_alphanumeric() || character == '_'))
        .filter(|token| {
            token.len() > 1
                && token
                    .chars()
                    .next()
                    .map(|first| first.is_ascii_alphabetic() || first == '_')
                    .unwrap_or(false)
        })
        .map(str::to_string)
        .collect()
}

fn identifier_span_at(value: &str, byte: usize) -> Option<(usize, usize)> {
    let bytes = value.as_bytes();
    if bytes.is_empty() {
        return None;
    }
    let mut cursor = byte.min(bytes.len().saturating_sub(1));
    if !bytes[cursor].is_ascii_alphanumeric() && bytes[cursor] != b'_' {
        if cursor == 0 || (!bytes[cursor - 1].is_ascii_alphanumeric() && bytes[cursor - 1] != b'_')
        {
            return None;
        }
        cursor -= 1;
    }
    let mut start = cursor;
    while start > 0 && (bytes[start - 1].is_ascii_alphanumeric() || bytes[start - 1] == b'_') {
        start -= 1;
    }
    let mut end = cursor + 1;
    while end < bytes.len() && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_') {
        end += 1;
    }
    (end > start + 1).then_some((start, end))
}

fn identifier_match_is_exact(value: &str, start: usize, end: usize) -> bool {
    let bytes = value.as_bytes();
    let valid = |byte: u8| byte.is_ascii_alphanumeric() || byte == b'_';
    (start == 0 || !valid(bytes[start - 1])) && (end == bytes.len() || !valid(bytes[end]))
}

fn symbol_role(line: &str, symbol: &str) -> (&'static str, &'static str) {
    let trimmed = line.trim_start();
    if trimmed.contains(&format!("fn {symbol}"))
        || trimmed.contains(&format!("def {symbol}"))
        || trimmed.contains(&format!("function {symbol}"))
        || trimmed.contains(&format!("class {symbol}"))
        || trimmed.contains(&format!("struct {symbol}"))
    {
        ("declaration", "声明")
    } else {
        ("call / reference", "调用 / 引用")
    }
}

/// The shared palette colour an X11 pixel stands for.
///
/// Text is handed its colour as an X11 pixel value, and drawing has to turn that
/// back into the palette entry it came from. Every colour the tokenizer can
/// produce has to be listed: a missing one silently becomes the fallback, which
/// is how keywords, types and constants once ended up drawn in the selection
/// colour and were almost invisible in the dark theme.
fn palette_colour_of(colors: &Palette, palette: &theme_core::Palette, color: c_ulong) -> u32 {
    if color == colors.bg {
        palette.background
    } else if color == colors.panel {
        palette.panel
    } else if color == colors.panel_2 {
        palette.raised
    } else if color == colors.line {
        palette.border
    } else if color == colors.muted {
        palette.muted
    } else if color == colors.text {
        palette.text
    } else if color == colors.bright {
        palette.bright
    } else if color == colors.accent {
        palette.accent
    } else if color == colors.shadow {
        palette.shadow
    } else if color == colors.keyword {
        palette.keyword
    } else if color == colors.syntax_type {
        palette.syntax_type
    } else if color == colors.function {
        palette.function
    } else if color == colors.constant {
        palette.constant
    } else if color == colors.variable {
        palette.variable
    } else if color == colors.parameter {
        palette.parameter
    } else if color == colors.property {
        palette.property
    } else if color == colors.macro_name {
        palette.macro_name
    } else if color == colors.module {
        palette.module
    } else if color == colors.preprocessor {
        palette.preprocessor
    } else if color == colors.literal {
        palette.literal
    } else if color == colors.comment {
        palette.comment
    } else if color == colors.project {
        palette.accent
    } else {
        palette.text
    }
}

fn extension_for_language(language: &str) -> &'static str {
    match language {
        "C" => "c",
        "C++" => "cpp",
        "Python" => "py",
        "Rust" => "rs",
        "Java" => "java",
        "Go" => "go",
        "TypeScript" => "ts",
        "JavaScript" => "js",
        "HTML" => "html",
        "CSS" => "css",
        _ => "txt",
    }
}

fn completion_words_for_language(language: &str) -> &'static [&'static str] {
    match language {
        "C" => &[
            "auto", "break", "case", "char", "const", "continue", "default", "define", "do",
            "double", "else", "enum", "extern", "float", "for", "goto", "if", "include", "inline",
            "int", "long", "NULL", "register", "restrict", "return", "short", "signed", "sizeof",
            "size_t", "static", "struct", "switch", "typedef", "union", "unsigned", "void",
            "volatile", "while",
        ],
        "C++" => &[
            "alignas",
            "alignof",
            "auto",
            "bool",
            "break",
            "case",
            "catch",
            "char",
            "class",
            "concept",
            "const",
            "constexpr",
            "define",
            "delete",
            "do",
            "double",
            "else",
            "enum",
            "explicit",
            "export",
            "extern",
            "false",
            "float",
            "for",
            "friend",
            "if",
            "include",
            "inline",
            "int",
            "namespace",
            "new",
            "nullptr",
            "operator",
            "private",
            "protected",
            "public",
            "requires",
            "return",
            "short",
            "signed",
            "sizeof",
            "static",
            "std",
            "string",
            "struct",
            "switch",
            "template",
            "this",
            "throw",
            "true",
            "try",
            "typename",
            "union",
            "unsigned",
            "using",
            "vector",
            "virtual",
            "void",
            "volatile",
            "while",
        ],
        "Rust" => &[
            "async", "await", "const", "enum", "fn", "impl", "let", "match", "mod", "pub",
            "Result", "self", "struct", "trait", "use",
        ],
        "Python" => &[
            "async", "await", "class", "def", "elif", "else", "from", "import", "lambda", "None",
            "return", "self", "try", "with",
        ],
        "TypeScript / JavaScript" | "Web / HTML / CSS" => &[
            "async",
            "await",
            "class",
            "const",
            "export",
            "extends",
            "function",
            "import",
            "interface",
            "return",
            "type",
        ],
        "Go" => &[
            "chan",
            "defer",
            "func",
            "go",
            "import",
            "interface",
            "map",
            "package",
            "range",
            "return",
            "struct",
        ],
        "Java" | "Kotlin" => &[
            "class",
            "extends",
            "fun",
            "import",
            "interface",
            "new",
            "package",
            "private",
            "public",
            "return",
            "static",
        ],
        _ => &[
            "class", "const", "function", "import", "return", "struct", "type", "value",
        ],
    }
}

fn wrap_bubble_text(value: &str, max_chars: usize, max_lines: usize) -> Vec<String> {
    let source = if value.trim().is_empty() {
        "comment"
    } else {
        value.trim()
    };
    let mut lines = Vec::new();
    for paragraph in source.lines() {
        let chars = paragraph.chars().collect::<Vec<_>>();
        if chars.is_empty() {
            lines.push(String::new());
            continue;
        }
        for chunk in chars.chunks(max_chars.max(1)) {
            lines.push(chunk.iter().collect());
        }
    }
    if lines.len() > max_lines {
        lines.truncate(max_lines);
        if let Some(last) = lines.last_mut() {
            if last.chars().count() >= 3 {
                let keep = last
                    .chars()
                    .take(max_chars.saturating_sub(3))
                    .collect::<String>();
                *last = format!("{keep}...");
            }
        }
    }
    lines
}

fn project_environments(language: &str) -> &'static [&'static str] {
    match language {
        "Python" => &["None", "venv", "uv", "conda", "poetry"],
        "C" => &["None", "GCC", "Clang", "CMake"],
        "C++" => &["None", "CMake + GCC", "CMake + Clang", "Meson", "Conan"],
        "Java" => &["None", "JDK 25", "JDK 21 LTS", "Gradle", "Maven"],
        "Rust" => &["None", "cargo stable", "cargo nightly", "Nix shell"],
        "TypeScript / JavaScript" => &["None", "pnpm", "npm", "yarn", "bun"],
        "Go" => &["None", "Go modules", "GVM", "container"],
        _ => &["None", "system toolchain", "container", "custom"],
    }
}

fn initial_languages() -> Vec<LanguagePack> {
    let installed =
        fs::read_to_string(language_settings_path()).unwrap_or_else(|_| "Rust\n".into());
    let specs = [
        ("C", &["c", "h"][..], &["C23", "C17", "C11"][..]),
        (
            "Python",
            &["py", "pyw"][..],
            &["3.13", "3.12", "3.11", "3.10", "3.9"][..],
        ),
        (
            "C++",
            &["cc", "cpp", "cxx", "hpp"][..],
            &["C++23", "C++20", "C++17", "C++14"][..],
        ),
        (
            "Java",
            &["java"][..],
            &["25", "21 LTS", "17 LTS", "11 LTS"][..],
        ),
        ("Rust", &["rs"][..], &["stable", "beta", "nightly"][..]),
        (
            "TypeScript / JavaScript",
            &["ts", "tsx", "js", "jsx", "mjs"][..],
            &["ES2025", "ES2024", "ES2023"][..],
        ),
        ("Go", &["go"][..], &["1.25", "1.24", "1.23"][..]),
        (
            "C#",
            &["cs"][..],
            &["14 / .NET 10", "13 / .NET 9", "12 / .NET 8"][..],
        ),
        ("Kotlin", &["kt", "kts"][..], &["2.2", "2.1", "2.0"][..]),
        ("Swift", &["swift"][..], &["6.2", "6.1", "6.0"][..]),
        ("Zig", &["zig"][..], &["0.15", "0.14", "master"][..]),
        ("Lua", &["lua"][..], &["5.4", "5.3", "LuaJIT"][..]),
        ("Ruby", &["rb"][..], &["3.4", "3.3", "3.2"][..]),
        (
            "Web / HTML / CSS",
            &["html", "htm", "css", "scss", "sass"][..],
            &["Living Standard", "HTML5"][..],
        ),
        ("PHP", &["php"][..], &["8.4", "8.3", "8.2"][..]),
        ("Dart", &["dart"][..], &["3.9", "3.8", "3.7"][..]),
        (
            "Shell",
            &["sh", "bash", "zsh", "fish"][..],
            &["Bash 5", "POSIX sh", "Zsh"][..],
        ),
        ("SQL", &["sql"][..], &["PostgreSQL", "SQLite", "MySQL"][..]),
        ("R", &["r"][..], &["4.5", "4.4", "4.3"][..]),
        ("Elixir", &["ex", "exs"][..], &["1.19", "1.18", "1.17"][..]),
        (
            "Haskell",
            &["hs", "lhs"][..],
            &["GHC 9.12", "GHC 9.10", "GHC 9.8"][..],
        ),
        ("OCaml", &["ml", "mli"][..], &["5.3", "5.2", "5.1"][..]),
        ("Scala", &["scala", "sc"][..], &["3.7", "3.6", "2.13"][..]),
        (
            "Clojure",
            &["clj", "cljs", "cljc"][..],
            &["1.12", "1.11"][..],
        ),
        ("Julia", &["jl"][..], &["1.11", "1.10 LTS"][..]),
        ("Perl", &["pl", "pm"][..], &["5.40", "5.38"][..]),
        (
            "Objective-C",
            &["m", "mm"][..],
            &["Clang 20", "Clang 19"][..],
        ),
        (
            "Markdown",
            &["md", "mdx"][..],
            &["CommonMark", "GFM", "MDX"][..],
        ),
        (
            "Data / Config",
            &["json", "yaml", "yml", "toml", "xml"][..],
            &["Current", "Legacy"][..],
        ),
        (
            "Vue / Svelte",
            &["vue", "svelte"][..],
            &["Vue 3 / Svelte 5", "Vue 2 / Svelte 4"][..],
        ),
    ];
    specs
        .iter()
        .map(|(name, extensions, versions)| {
            let saved = installed.lines().find_map(|line| {
                let (saved_name, version) = line.split_once('|').unwrap_or((line, ""));
                (saved_name == *name).then_some(version)
            });
            let selected_version = saved
                .and_then(|value| versions.iter().position(|candidate| *candidate == value))
                .unwrap_or(0);
            LanguagePack {
                name,
                extensions,
                versions,
                selected_version,
                installed: saved.is_some(),
            }
        })
        .collect()
}

fn save_language_settings(languages: &[LanguagePack]) {
    let path = language_settings_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let content = languages
        .iter()
        .filter(|pack| pack.installed)
        .map(|pack| format!("{}|{}", pack.name, pack.versions[pack.selected_version]))
        .collect::<Vec<_>>()
        .join("\n");
    let _ = fs::write(path, format!("{content}\n"));
}

/// `--render-frame <path>` renders one frame to a PNG without mapping a window.
fn frame_export_path() -> Option<String> {
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        if argument == "--render-frame" {
            return arguments.next();
        }
        if let Some(value) = argument.strip_prefix("--render-frame=") {
            return Some(value.to_string());
        }
    }
    None
}

/// `--render-scene <name>` picks the state a rendered frame shows, for example
/// `packaging` or `packaging-emit`.
fn frame_scene() -> Option<String> {
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        if argument == "--render-scene" {
            return arguments.next();
        }
        if let Some(value) = argument.strip_prefix("--render-scene=") {
            return Some(value.to_string());
        }
    }
    None
}

/// Write the current pixmap frame to a PNG so CI and local runs can inspect it.
unsafe fn export_frame(app: &App, path: &str) -> bool {
    const CAIRO_FORMAT_ARGB32: c_int = 0;
    let Ok(path) = CString::new(path) else {
        return false;
    };
    let surface = unsafe { cairo_image_surface_create(CAIRO_FORMAT_ARGB32, app.width, app.height) };
    if surface.is_null() {
        return false;
    }
    let context = unsafe { cairo_create(surface) };
    if context.is_null() {
        unsafe { cairo_surface_destroy(surface) };
        return false;
    }
    unsafe {
        cairo_surface_flush(app.cairo_surface);
        cairo_set_source_surface(context, app.cairo_surface, 0.0, 0.0);
        cairo_paint(context);
        let status = cairo_surface_write_to_png(surface, path.as_ptr());
        cairo_destroy(context);
        cairo_surface_destroy(surface);
        status == 0
    }
}

pub fn run() {
    unsafe {
        let locale = CString::new("").unwrap();
        setlocale(LC_ALL, locale.as_ptr());
        register_bundled_fonts();
        let display = XOpenDisplay(ptr::null());
        if display.is_null() {
            eprintln!("TC-IDE: cannot open X11 display. Run inside a desktop session.");
            std::process::exit(1);
        }
        let screen = XDefaultScreen(display);
        let screen_width = XDisplayWidth(display, screen);
        let screen_height = XDisplayHeight(display, screen);
        let root = XRootWindow(display, screen);
        let map = XDefaultColormap(display, screen);
        let bg = color(display, map, "#0d0f11");
        let width = (screen_width as f32 * 0.94) as i32;
        let height = (screen_height as f32 * 0.90) as i32;
        let window = XCreateSimpleWindow(
            display,
            root,
            (screen_width - width) / 2,
            (screen_height - height) / 2,
            width as u32,
            height as u32,
            0,
            bg,
            bg,
        );
        let title = CString::new("TC-IDE - Native Semantic Workspace").unwrap();
        XStoreName(display, window, title.as_ptr());
        XSelectInput(
            display,
            window,
            KEY_PRESS_MASK
                | BUTTON_PRESS_MASK
                | POINTER_MOTION_MASK
                | EXPOSURE_MASK
                | STRUCTURE_NOTIFY_MASK,
        );
        let frame_export = frame_export_path();
        if frame_export.is_none() {
            let delete_name = CString::new("WM_DELETE_WINDOW").unwrap();
            let mut delete_atom = XInternAtom(display, delete_name.as_ptr(), 0);
            XSetWMProtocols(display, window, &mut delete_atom, 1);
            XMapWindow(display, window);
        }
        let gc = XCreateGC(display, window, 0, ptr::null_mut());
        let font_pattern = if width > 2200 {
            "-misc-fixed-medium-r-normal--24-*-*-*-*-*-iso10646-1"
        } else {
            "-misc-fixed-medium-r-normal--18-*-*-*-*-*-iso10646-1"
        };
        let font_name = CString::new(font_pattern).unwrap();
        let mut font = XLoadQueryFont(display, font_name.as_ptr());
        let fallback_font = CString::new("fixed").unwrap();
        if font.is_null() {
            font = XLoadQueryFont(display, fallback_font.as_ptr());
        }
        let mut code_font = XLoadQueryFont(display, font_name.as_ptr());
        if code_font.is_null() {
            code_font = XLoadQueryFont(display, fallback_font.as_ptr());
        }
        let font_set_pattern = CString::new("-*-sans-medium-r-normal--18-*-*-*-*-*-*-*").unwrap();
        let mut missing: *mut *mut c_char = ptr::null_mut();
        let mut missing_count = 0;
        let mut default_string: *mut c_char = ptr::null_mut();
        let font_set = XCreateFontSet(
            display,
            font_set_pattern.as_ptr(),
            &mut missing,
            &mut missing_count,
            &mut default_string,
        );
        if !font.is_null() {
            XSetFont(display, gc, (*font).fid);
        }
        let depth = XDefaultDepth(display, screen);
        let canvas = XCreatePixmap(display, window, width as u32, height as u32, depth as u32);
        let visual = XDefaultVisual(display, screen);
        let cairo_surface = cairo_xlib_surface_create(display, canvas, visual, width, height);
        let cairo = cairo_create(cairo_surface);
        // The palette is shared with the Windows backend, so the themes stay one
        // design and its contrast is asserted in theme_core's tests.
        let share = |palette: theme_core::Palette| Palette {
            bg: color(display, map, &theme_core::Palette::hex(palette.background)),
            panel: color(display, map, &theme_core::Palette::hex(palette.panel)),
            panel_2: color(display, map, &theme_core::Palette::hex(palette.raised)),
            line: color(display, map, &theme_core::Palette::hex(palette.border)),
            muted: color(display, map, &theme_core::Palette::hex(palette.muted)),
            text: color(display, map, &theme_core::Palette::hex(palette.text)),
            bright: color(display, map, &theme_core::Palette::hex(palette.bright)),
            accent: color(display, map, &theme_core::Palette::hex(palette.accent)),
            keyword: color(display, map, &theme_core::Palette::hex(palette.keyword)),
            syntax_type: color(display, map, &theme_core::Palette::hex(palette.syntax_type)),
            function: color(display, map, &theme_core::Palette::hex(palette.function)),
            constant: color(display, map, &theme_core::Palette::hex(palette.constant)),
            variable: color(display, map, &theme_core::Palette::hex(palette.variable)),
            parameter: color(display, map, &theme_core::Palette::hex(palette.parameter)),
            property: color(display, map, &theme_core::Palette::hex(palette.property)),
            macro_name: color(display, map, &theme_core::Palette::hex(palette.macro_name)),
            module: color(display, map, &theme_core::Palette::hex(palette.module)),
            preprocessor: color(
                display,
                map,
                &theme_core::Palette::hex(palette.preprocessor),
            ),
            literal: color(display, map, &theme_core::Palette::hex(palette.literal)),
            comment: color(display, map, &theme_core::Palette::hex(palette.comment)),
            project: color(display, map, &theme_core::Palette::hex(palette.selection)),
            shadow: color(display, map, &theme_core::Palette::hex(palette.shadow)),
        };
        let dark_colors = share(theme_core::Palette::dark());
        let light_colors = share(theme_core::Palette::light());
        let preferences = load_ui_preferences();
        let colors = if preferences.light_theme {
            light_colors
        } else {
            dark_colors
        };
        let background_preferences = load_background_preferences();
        let (ai_endpoint, ai_model) = load_ai_preferences();
        let background_surface = background_preferences
            .path
            .as_ref()
            .and_then(|path| CString::new(path.to_string_lossy().as_bytes()).ok())
            .map(|path| cairo_image_surface_create_from_png(path.as_ptr()))
            .filter(|surface| {
                !surface.is_null()
                    && cairo_image_surface_get_width(*surface) > 0
                    && cairo_image_surface_get_height(*surface) > 0
            })
            .unwrap_or(ptr::null_mut());
        let logo_surface = bundled_asset_path("assets/tc-ide-icon.png")
            .and_then(|path| CString::new(path.to_string_lossy().as_bytes()).ok())
            .map(|path| cairo_image_surface_create_from_png(path.as_ptr()))
            .unwrap_or(ptr::null_mut());
        let mut app = App {
            display,
            window,
            gc,
            canvas,
            cairo_surface,
            cairo,
            depth,
            font,
            font_set,
            code_font,
            code_font_base: if width > 2200 { 18 } else { 14 },
            code_font_family: preferences.font_family,
            background_surface,
            background_path: background_preferences.path,
            background_opacity: background_preferences.opacity,
            background_offset_x: background_preferences.offset_x,
            background_offset_y: background_preferences.offset_y,
            background_zoom: background_preferences.zoom,
            logo_surface,
            colors,
            dark_colors,
            light_colors,
            light_theme: preferences.light_theme,
            width,
            height,
            mouse_x: -100,
            mouse_y: -100,
            hovered: None,
            settings_pressed: None,
            settings_focus: None,
            hovered_from_code: false,
            hovered_code_span: None,
            selected_file: 0,
            files: initial_files(),
            folders: vec!["src".into(), "src/editor".into()],
            project_name: "TC-IDE".into(),
            project_path: None,
            project_location: "~/TC-IDE-Projects".into(),
            project_location_focused: false,
            create_caret: 0,
            editor_focused: false,
            caret_line: 0,
            caret_column: 0,
            selection_anchor: None,
            selection_head: None,
            clipboard: String::new(),
            code_zoom: preferences.code_zoom,
            font_search: String::new(),
            font_search_focused: false,
            edit_menu_open: false,
            view_menu_open: false,
            workspace_search: String::new(),
            workspace_search_focused: false,
            project_tree_expanded: true,
            project_tree_amount: 1.0,
            project_tree_velocity: 0.0,
            folder_tree_motion: BTreeMap::new(),
            tree_motion_updated: Instant::now(),
            new_menu_motion: UiMotion::new(0.0),
            edit_menu_motion: UiMotion::new(0.0),
            view_menu_motion: UiMotion::new(0.0),
            settings_motion: UiMotion::new(0.0),
            package_motion: UiMotion::new(0.0),
            extensions_motion: UiMotion::new(0.0),
            create_motion: UiMotion::new(0.0),
            terminal_motion: UiMotion::new(0.0),
            graph_motion: UiMotion::new(1.0),
            collapsed_folders: BTreeSet::new(),
            terminal_open: false,
            terminal_height: (230.0 * (width as f32 / 1440.0).clamp(0.85, 2.0)).round() as i32,
            terminal_focused: false,
            terminal_active: 0,
            terminals: default_terminals(),
            terminal_menu_open: false,
            graph_current: true,
            graph_visible: true,
            graph_fraction: 0.39,
            panel_drag: None,
            breakpoints: dap_core::Breakpoints::new(),
            project_index: index_core::IncrementalIndex::new(),
            terminal_selection: None,
            terminal_selecting: false,
            breakpoint_prompt: None,
            condition_caret: 0,
            condition_focused: false,
            debug_session: None,
            debug_state: dap_core::DebugState::default(),
            debug_panel_open: false,
            debug_frame: 0,
            debug_stop_seen: String::new(),
            graph_zoom: 1.0,
            session: session_core::Session::default(),
            runtime_cache: extension_core::RuntimeCache::default(),
            extension_upgrades: 0,
            background_drag: None,
            graph_relation: graph_core::GraphRelation::Follow,
            folded: Vec::new(),
            goto_prompt: None,
            goto_focused: false,
            matched_brackets: None,
            language_server: None,
            lsp_diagnostics: Vec::new(),
            lsp_diagnostic_file: String::new(),
            lsp_severity: lsp_core::SeverityFilter::All,
            lsp_last_change: None,
            lsp_pending: Vec::new(),
            lsp_hover: None,
            lsp_locations: Vec::new(),
            lsp_reference_index: 0,
            lsp_completions: Vec::new(),
            lsp_completion_index: 0,
            rename_prompt: None,
            rename_focused: false,
            ai_open: false,
            ai_focused: false,
            ai_prompt: String::new(),
            ai_messages: ai_core::Conversation::new(),
            ai_session: None,
            ai_error: None,
            graph_scope: None,
            graph_scroll: 0,
            graph_search: String::new(),
            graph_search_focused: false,
            graph_folded: BTreeSet::new(),
            project_symbols: Vec::new(),
            hovered_symbol: None,
            package_open: false,
            package_targets: [false, false, false, false],
            build_started: None,
            new_menu_open: false,
            workspace_context_menu: None,
            extensions_open: false,
            settings_open: false,
            ui_chinese: preferences.chinese,
            languages: initial_languages(),
            downloading: None,
            extension_scroll: 0,
            extension_category: 0,
            extension_search: String::new(),
            extension_search_focused: false,
            selected_language: Some(1),
            version_menu_open: false,
            ai_endpoint,
            ai_model,
            ai_key: String::new(),
            ai_field_focused: None,
            create_kind: None,
            new_file_name: String::new(),
            project_language: 1,
            project_environment: 0,
            project_language_menu: false,
            project_environment_menu: false,
            expanded_comments: BTreeSet::new(),
            edit_pulses: motion_core::EditPulses::new(),
            status_message: "Ready".into(),
            started: Instant::now(),
        };
        app.rebuild_project_index();
        app.restore_session();
        app.runtime_cache = load_runtime_cache();
        app.extension_upgrades = extension_upgrade_count(&app.runtime_cache, &app.project_path);
        let frame = Duration::from_micros(16_667);
        if let Some(path) = frame_export {
            if let Some(scene) = frame_scene() {
                if !app.prime_scene(&scene) {
                    eprintln!("TC-IDE: unknown render scene {scene}, drawing the editor");
                }
            }
            app.refresh_index();
            app.poll_ai_session();
            app.refresh_matched_brackets();
            app.poll_language_server();
            app.sync_language_server();
            app.update_background();
            app.draw();
            let written = export_frame(&app, &path);
            if written {
                println!("TC-IDE frame written to {path}");
            } else {
                eprintln!("TC-IDE: could not write frame {path}");
            }
            if !app.background_surface.is_null() {
                cairo_surface_destroy(app.background_surface);
            }
            if !app.logo_surface.is_null() {
                cairo_surface_destroy(app.logo_surface);
            }
            cairo_destroy(app.cairo);
            cairo_surface_destroy(app.cairo_surface);
            XFreePixmap(display, app.canvas);
            if !app.font.is_null() {
                XFreeFont(display, app.font);
            }
            if !app.font_set.is_null() {
                XFreeFontSet(display, app.font_set);
            }
            if !app.code_font.is_null() {
                XFreeFont(display, app.code_font);
            }
            XFreeGC(display, gc);
            XDestroyWindow(display, window);
            XCloseDisplay(display);
            if !written {
                std::process::exit(1);
            }
            return;
        }
        let mut running = true;
        while running {
            let began = Instant::now();
            while XPending(display) > 0 {
                let mut event = XEvent { pad: [0; 24] };
                XNextEvent(display, &mut event);
                match event.type_ {
                    KEY_PRESS => {
                        let mut key = event.key;
                        running = app.handle_key(&mut key);
                    }
                    BUTTON_PRESS => {
                        let button = event.button;
                        app.handle_click(button.x, button.y, button.button, button.state);
                    }
                    MOTION_NOTIFY => {
                        let motion = event.motion;
                        app.mouse_x = motion.x;
                        app.mouse_y = motion.y;
                        app.update_panel_drag(motion.x, motion.y, motion.state);
                        app.update_drag_selection(motion.x, motion.y, motion.state);
                        app.update_background_drag(motion.x, motion.y, motion.state);
                        if motion.state & BUTTON1_MASK != 0 {
                            app.drag_terminal_selection(motion.x, motion.y);
                        }
                        if motion.state & BUTTON1_MASK == 0 {
                            app.settings_pressed = None;
                            // The button came up, so any drag in progress is
                            // over; the selection itself stays.
                            app.end_terminal_selection();
                        }
                        app.update_hover();
                    }
                    CONFIGURE_NOTIFY => {
                        let config = event.configure;
                        if config.width != app.width || config.height != app.height {
                            app.resize(config.width, config.height);
                        }
                    }
                    CLIENT_MESSAGE => running = false,
                    EXPOSE => {}
                    _ => {}
                }
            }
            app.refresh_index();
            app.update_background();
            app.draw();
            let used = began.elapsed();
            if used < frame {
                thread::sleep(frame - used);
            }
        }
        if !app.background_surface.is_null() {
            cairo_surface_destroy(app.background_surface);
        }
        if !app.logo_surface.is_null() {
            cairo_surface_destroy(app.logo_surface);
        }
        cairo_destroy(app.cairo);
        cairo_surface_destroy(app.cairo_surface);
        XFreePixmap(display, app.canvas);
        if !app.font.is_null() {
            XFreeFont(display, app.font);
        }
        if !app.font_set.is_null() {
            XFreeFontSet(display, app.font_set);
        }
        if !app.code_font.is_null() {
            XFreeFont(display, app.code_font);
        }
        XFreeGC(display, gc);
        XDestroyWindow(display, window);
        XCloseDisplay(display);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_shortcuts_use_keysyms_instead_of_control_text() {
        assert_eq!(shortcut_from_keysym('C' as c_ulong), Some('c'));
        assert_eq!(shortcut_from_keysym('v' as c_ulong), Some('v'));
        assert_eq!(shortcut_from_keysym('S' as c_ulong), Some('s'));
    }

    #[test]
    fn tree_spring_moves_toward_both_states() {
        let (mut value, mut velocity) = (1.0, 0.0);
        for _ in 0..90 {
            App::spring_step(&mut value, &mut velocity, 0.0, 1.0 / 60.0);
        }
        assert!(value.abs() < 0.01);
        for _ in 0..90 {
            App::spring_step(&mut value, &mut velocity, 1.0, 1.0 / 60.0);
        }
        assert!((value - 1.0).abs() < 0.01);
    }

    fn demo_sources() -> Vec<(String, Vec<String>)> {
        initial_files()
            .iter()
            .map(|file| (file.name.clone(), file.lines.clone()))
            .collect()
    }

    fn demo_graph(scope: graph_core::GraphScope, folded: &[String]) -> graph_core::Graph {
        let files = initial_files();
        let sources = demo_sources();
        let project = editor_core::project_symbol_occurrences(&sources);
        let file = &files[0];
        let symbols = editor_core::symbol_occurrences(&file.lines, &file.name);
        let declarations = sources
            .iter()
            .flat_map(|(name, lines)| {
                syntax_core::declaration_refs(&syntax_core::scan(lines, name), name)
            })
            .collect::<Vec<_>>();
        let inputs = graph_core::GraphInputs {
            project_name: "TC-IDE",
            current_file: &file.name,
            current_folder: graph_core::folder_of(&file.name),
            hovered: None,
            file_lines: &file.lines,
            file_symbols: &symbols,
            project_symbols: &project,
            project_declarations: &declarations,
            folded,
            relation: graph_core::GraphRelation::Follow,
        };
        graph_core::build_graph(&inputs, scope, "")
    }

    #[test]
    fn graph_builds_a_real_hierarchy_from_the_open_files() {
        let graph = demo_graph(graph_core::GraphScope::File, &[]);
        let roots = graph
            .nodes
            .iter()
            .filter(|node| node.level == graph_core::GraphLevel::Project)
            .count();
        assert_eq!(roots, 1);
        assert_eq!(graph.nodes[0].depth, 0);
        assert!(graph.nodes[0].children > 0);
        assert!(graph
            .nodes
            .iter()
            .any(|node| node.level == graph_core::GraphLevel::Class));
        assert!(graph
            .nodes
            .iter()
            .any(|node| node.caption().contains("build_graph")));
        assert!(graph
            .nodes
            .iter()
            .any(|node| node.caption().contains("analyze_workspace") || node.edge.is_some()));
    }

    #[test]
    fn graph_nodes_carry_targets_that_open_open_files() {
        let graph = demo_graph(graph_core::GraphScope::File, &[]);
        let with_target = graph
            .nodes
            .iter()
            .filter(|node| node.target.is_some())
            .count();
        assert!(with_target >= 2, "{:?}", graph.nodes);
        assert!(graph.nodes.iter().all(|node| !node.key.is_empty()));
    }

    #[test]
    fn folding_a_file_hides_its_symbols() {
        let folded = vec![graph_core::file_key("graph.rs")];
        let graph = demo_graph(graph_core::GraphScope::File, &folded);
        assert_eq!(graph.len(), 2);
        assert!(graph.nodes[1].folded);
        assert!(graph.nodes[1].children > 0);
    }

    #[test]
    fn project_scope_groups_the_demo_files() {
        let graph = demo_graph(graph_core::GraphScope::Project, &[]);
        let files = graph
            .nodes
            .iter()
            .filter(|node| node.level == graph_core::GraphLevel::File)
            .map(|node| node.label.clone())
            .collect::<Vec<_>>();
        assert!(files.contains(&"graph.rs".to_string()));
        assert!(files.contains(&"workspace.rs".to_string()));
        assert!(files.contains(&"render.rs".to_string()));
    }

    #[test]
    fn language_registry_has_common_and_fallback_ready_formats() {
        let languages = initial_languages();
        assert!(languages.len() >= 25);
        assert!(languages
            .iter()
            .any(|pack| pack.name == "Python" && pack.extensions.contains(&"py")));
        assert!(languages
            .iter()
            .any(|pack| pack.name == "Data / Config" && pack.extensions.contains(&"json")));
    }

    #[test]
    fn primary_language_layer_and_versions_are_available() {
        let languages = initial_languages();
        assert_eq!(
            languages
                .iter()
                .take(4)
                .map(|pack| pack.name)
                .collect::<Vec<_>>(),
            vec!["C", "Python", "C++", "Java"]
        );
        let python = languages.iter().find(|pack| pack.name == "Python").unwrap();
        assert!(python.versions.contains(&"3.13"));
        assert!(python.versions.contains(&"3.12"));
    }

    #[test]
    fn python_projects_offer_optional_and_real_venv_modes() {
        assert_eq!(project_environments("Python").first(), Some(&"None"));
        assert!(project_environments("Python").contains(&"venv"));
    }

    #[test]
    fn comment_bubbles_wrap_and_keep_explicit_lines() {
        assert_eq!(
            wrap_bubble_text("first line\nsecond line", 30, 4),
            vec!["first line", "second line"]
        );
        assert_eq!(
            wrap_bubble_text("123456789", 4, 4),
            vec!["1234", "5678", "9"]
        );
        assert!(wrap_bubble_text(&"x".repeat(100), 10, 3)
            .last()
            .unwrap()
            .ends_with("..."));
    }

    #[test]
    fn comment_syntax_depends_on_file_language() {
        assert_eq!(comment_markers_for_file("main.c"), &["//", "/*"]);
        assert!(!comment_markers_for_file("main.c").contains(&"#"));
        assert_eq!(comment_markers_for_file("main.py"), &["#"]);
        assert_eq!(comment_markers_for_file("query.sql"), &["--"]);
        assert!(comment_markers_for_file("data.json").is_empty());
        assert_eq!(
            comment_content_bounds("let n = 1; // keep", "main.c"),
            Some((11, 14, 18))
        );
        assert_eq!(
            comment_content_bounds("/* editable */ tail", "main.c"),
            Some((0, 3, 11))
        );
        assert_eq!(comment_content_bounds("#include <stdio.h>", "main.c"), None);
    }

    #[test]
    fn unicode_caret_helpers_stay_on_character_boundaries() {
        let value = "a注b";
        assert_eq!(char_to_byte_index(value, 2), 4);
        assert_eq!(previous_char_boundary(value, 4), 1);
        assert_eq!(next_char_boundary(value, 1), 4);
    }

    #[test]
    fn c_and_cpp_have_distinct_suffixes_and_completions() {
        let languages = initial_languages();
        let detect = |suffix: &str| {
            languages
                .iter()
                .find(|pack| pack.extensions.contains(&suffix))
                .map(|pack| pack.name)
        };
        assert_eq!(detect("c"), Some("C"));
        assert_eq!(detect("cpp"), Some("C++"));
        assert!(!completion_words_for_language("C").contains(&"class"));
        assert!(completion_words_for_language("C++").contains(&"class"));
        assert!(completion_words_for_language("C++").contains(&"constexpr"));
    }

    #[test]
    fn project_symbols_can_feed_completion_candidates() {
        let symbols = identifiers_in("Widget build_widget(const WidgetConfig& config);");
        assert!(symbols.contains(&"Widget".to_string()));
        assert!(symbols.contains(&"build_widget".to_string()));
        assert!(symbols.contains(&"WidgetConfig".to_string()));
    }

    #[test]
    fn identifier_relations_use_exact_symbol_boundaries() {
        let line = "let graph = build_graph(graph_data);";
        let start = line.find("build_graph").unwrap();
        assert_eq!(
            identifier_span_at(line, start + 4),
            Some((start, start + 11))
        );
        assert!(identifier_match_is_exact(line, start, start + 11));
        let graph = line.find("graph").unwrap();
        assert!(identifier_match_is_exact(line, graph, graph + 5));
        assert!(!identifier_match_is_exact("build_graph_more", 0, 11));
    }

    #[test]
    fn symbol_roles_distinguish_declarations_from_calls() {
        assert_eq!(
            symbol_role("pub fn build_graph() {", "build_graph").0,
            "declaration"
        );
        assert_eq!(
            symbol_role("build_graph();", "build_graph").0,
            "call / reference"
        );
    }

    #[test]
    fn syntax_highlighting_classifies_cpp_keywords_and_strings() {
        let tokens = editor_core::syntax_tokens("constexpr auto name = \"TCI\";", "main.cpp");
        assert!(tokens.iter().any(
            |token| token.text == "constexpr" && token.kind == editor_core::TokenKind::Keyword
        ));
        assert!(tokens
            .iter()
            .any(|token| token.text == "\"TCI\"" && token.kind == editor_core::TokenKind::Literal));
    }

    #[test]
    fn terminal_banner_is_readable_plain_ascii() {
        let banner = terminal_banner();
        assert!(banner.iter().all(|line| line.is_ascii()));
        assert!(banner
            .iter()
            .any(|line| line.contains("T I A N C H U A N G Y A")));
        assert!(banner.iter().all(|line| line.len() <= 64));
    }

    #[test]
    fn the_pixel_colour_maps_back_to_the_palette_it_came_from() {
        // The renderer passes X11 pixel values around and has to turn them back
        // into palette entries. A colour missing from the mapping used to fall
        // through to the selection colour, which drew keywords, types and
        // constants in a dark blue that was nearly invisible on the dark theme.
        use theme_core::Palette as SharedPalette;
        for palette in [SharedPalette::dark(), SharedPalette::light()] {
            let colors = Palette {
                bg: 1,
                panel: 2,
                panel_2: 3,
                line: 4,
                muted: 5,
                text: 6,
                bright: 7,
                accent: 8,
                keyword: 9,
                syntax_type: 10,
                function: 11,
                constant: 12,
                variable: 13,
                parameter: 14,
                property: 15,
                macro_name: 16,
                module: 17,
                preprocessor: 18,
                literal: 19,
                comment: 20,
                project: 21,
                shadow: 22,
            };
            let cases = [
                (colors.keyword, palette.keyword, "keyword"),
                (colors.syntax_type, palette.syntax_type, "type"),
                (colors.function, palette.function, "function"),
                (colors.constant, palette.constant, "constant"),
                (colors.variable, palette.variable, "variable"),
                (colors.parameter, palette.parameter, "parameter"),
                (colors.property, palette.property, "property"),
                (colors.macro_name, palette.macro_name, "macro"),
                (colors.module, palette.module, "module"),
                (colors.preprocessor, palette.preprocessor, "preprocessor"),
                (colors.literal, palette.literal, "literal"),
                (colors.comment, palette.comment, "comment"),
            ];
            for (pixel, expected, name) in cases {
                assert_eq!(
                    palette_colour_of(&colors, &palette, pixel),
                    expected,
                    "{name} has to map back to its own palette colour"
                );
                assert_ne!(
                    palette_colour_of(&colors, &palette, pixel),
                    palette.selection,
                    "{name} must never fall back to the selection colour"
                );
            }
            // Every syntax colour is distinct, so the kinds can be told apart.
            let mut seen = std::collections::HashSet::new();
            for (_, expected, _) in cases {
                seen.insert(expected);
            }
            assert_eq!(seen.len(), cases.len());
            // An unknown pixel still produces something readable.
            assert_eq!(palette_colour_of(&colors, &palette, 9999), palette.text);
        }
    }

    #[test]
    fn the_file_menu_lists_new_open_and_recent_projects() {
        let recent = vec![
            PathBuf::from("/home/me/tc-ide"),
            PathBuf::from("/home/me/notes"),
        ];
        let rows = file_menu_rows(&recent);
        // The three creation entries stay where they were.
        assert!(rows.contains(&FileMenuRow::Create(CreateKind::Project)));
        assert!(rows.contains(&FileMenuRow::Create(CreateKind::File)));
        assert!(rows.contains(&FileMenuRow::Create(CreateKind::Folder)));
        // Opening is part of the same menu instead of a separate button.
        assert!(rows.contains(&FileMenuRow::OpenFile));
        assert!(rows.contains(&FileMenuRow::OpenFolder));
        // The recent projects are listed, newest first.
        let listed = rows
            .iter()
            .filter_map(|row| match row {
                FileMenuRow::Recent(path) => Some(path.clone()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(listed, recent);

        // Headings are drawn but never clicked, so a stray click between rows
        // cannot start an action.
        assert_eq!(rows.iter().filter(|row| !row.is_action()).count(), 3);

        // Without a recent list the section is left out entirely.
        let bare = file_menu_rows(&[]);
        assert!(!bare.iter().any(|row| matches!(row, FileMenuRow::Recent(_))));
        assert_eq!(bare.iter().filter(|row| !row.is_action()).count(), 2);
        assert!(bare.len() < rows.len());

        // A long list is capped so the menu cannot grow past the window.
        let many = (0..40)
            .map(|index| PathBuf::from(format!("/tmp/project-{index}")))
            .collect::<Vec<_>>();
        assert_eq!(
            file_menu_rows(&many)
                .iter()
                .filter(|row| matches!(row, FileMenuRow::Recent(_)))
                .count(),
            4
        );
    }
}
