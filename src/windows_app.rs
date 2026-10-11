#![allow(clippy::upper_case_acronyms)]

use std::cell::RefCell;
use std::ffi::c_void;
use std::fs;
use std::path::PathBuf;
use std::ptr;
use std::time::Instant;

#[path = "windows_platform.rs"]
mod platform;
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
const WM_KEYDOWN: Uint = 0x0100;
const WM_LBUTTONDOWN: Uint = 0x0201;
const WM_PAINT: Uint = 0x000f;
const WM_SIZE: Uint = 0x0005;
const WM_TIMER: Uint = 0x0113;
const WS_OVERLAPPEDWINDOW: Dword = 0x00cf0000;
const WS_VISIBLE: Dword = 0x10000000;
const VK_BACK: u32 = 0x08;
const VK_DOWN: u32 = 0x28;
const VK_LEFT: u32 = 0x25;
const VK_RIGHT: u32 = 0x27;
const VK_UP: u32 = 0x26;

thread_local! {
    static APP: RefCell<WindowsAppState> = RefCell::new(WindowsAppState::new());
    static RENDERER: RefCell<Option<DirectEditorRenderer>> = const { RefCell::new(None) };
}

struct WindowsAppState {
    file_name: String,
    file_path: Option<PathBuf>,
    lines: Vec<String>,
    caret_line: usize,
    caret_column: usize,
    terminal_open: bool,
    terminal_motion: f32,
    terminal_velocity: f32,
    terminal: Option<ConPtySession>,
    terminal_input: String,
    status: String,
    started: Instant,
    last_frame: Instant,
    last_edit: Option<Instant>,
}

impl WindowsAppState {
    fn new() -> Self {
        Self {
            file_name: "main.rs".into(),
            file_path: None,
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
            terminal_open: false,
            terminal_motion: 0.0,
            terminal_velocity: 0.0,
            terminal: None,
            terminal_input: String::new(),
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
        self.lines[self.caret_line]
            .char_indices()
            .nth(self.caret_column)
            .map(|(index, _)| index)
            .unwrap_or_else(|| self.lines[self.caret_line].len())
    }

    fn insert_char(&mut self, value: char) {
        let byte = self.byte_at_caret();
        self.lines[self.caret_line].insert(byte, value);
        self.caret_column += 1;
        self.last_edit = Some(Instant::now());
        self.status = "Edited with DirectWrite caret mapping".into();
    }

    fn newline(&mut self) {
        let byte = self.byte_at_caret();
        let tail = self.lines[self.caret_line].split_off(byte);
        self.caret_line += 1;
        self.lines.insert(self.caret_line, tail);
        self.caret_column = 0;
        self.last_edit = Some(Instant::now());
    }

    fn backspace(&mut self) {
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

    fn save(&mut self) {
        let Some(path) = &self.file_path else {
            self.status = "Open a file before saving".into();
            return;
        };
        match fs::write(path, self.lines.join("\r\n")) {
            Ok(()) => self.status = format!("Saved {}", path.display()),
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
                self.caret_line = 0;
                self.caret_column = 0;
                self.started = Instant::now();
                self.status = format!("Opened {}", path.display());
            }
            Err(error) => self.status = format!("Open failed: {error}"),
        }
    }

    fn ensure_terminal(&mut self) {
        if self.terminal.is_some() {
            return;
        }
        let cwd = self
            .file_path
            .as_ref()
            .and_then(|path| path.parent())
            .map(PathBuf::from)
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| PathBuf::from("."));
        match unsafe { ConPtySession::spawn(&cwd, 120, 30) } {
            Ok(session) => {
                self.terminal = Some(session);
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
                let selected = app.lines.get(app.caret_line).cloned().unwrap_or_default();
                match platform::write_clipboard(window, &selected) {
                    Ok(()) => app.status = "Copied current line as Unicode text".into(),
                    Err(error) => app.status = format!("Clipboard write failed: {error}"),
                }
            }
            platform::COMMAND_PASTE => match platform::read_clipboard(window) {
                Ok(value) => {
                    for value in value.replace("\r\n", "\n").chars() {
                        if value == '\n' {
                            app.newline();
                        } else if !value.is_control() {
                            app.insert_char(value);
                        }
                    }
                }
                Err(error) => app.status = format!("Clipboard read failed: {error}"),
            },
            platform::COMMAND_OPEN_FOLDER => match std::env::current_dir() {
                Ok(path) if platform::open_in_explorer(window, &path) => {
                    app.status = format!("Opened Explorer at {}", path.display());
                }
                Ok(path) => app.status = format!("Could not open Explorer at {}", path.display()),
                Err(error) => app.status = format!("Could not resolve project directory: {error}"),
            },
            platform::COMMAND_OPEN_TERMINAL => app.toggle_terminal(),
            _ => {}
        }
    });
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
            let terminal_lines = app
                .terminal
                .as_ref()
                .and_then(|session| session.lines.lock().ok().map(|lines| lines.clone()))
                .unwrap_or_default();
            let scene = WindowsScene {
                file_name: &app.file_name,
                lines: &app.lines,
                caret_line: app.caret_line,
                caret_column: app.caret_column,
                status: &app.status,
                terminal_lines: &terminal_lines,
                terminal_input: &app.terminal_input,
                terminal_open: app.terminal_open,
                terminal_progress: app.terminal_motion,
                animation_seconds: app.started.elapsed().as_secs_f32(),
                edit_age: app.last_edit.map(|started| started.elapsed().as_secs_f32()),
            };
            let mut renderer = renderer_cell.borrow_mut();
            if renderer.is_none() {
                *renderer = unsafe { DirectEditorRenderer::new(dpi) }.ok();
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
        if app.terminal_open {
            match value {
                '\r' | '\n' => {
                    if let Some(session) = &app.terminal {
                        let _ = session.write("\r");
                    }
                    app.terminal_input.clear();
                }
                '\u{8}' => {
                    app.terminal_input.pop();
                    if let Some(session) = &app.terminal {
                        let _ = session.write("\u{7f}");
                    }
                }
                ch if !ch.is_control() => {
                    app.terminal_input.push(ch);
                    if let Some(session) = &app.terminal {
                        let _ = session.write(&ch.to_string());
                    }
                }
                _ => {}
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
        WM_KEYDOWN => {
            if platform::control_pressed() {
                let command = match w_param as u32 {
                    0x43 => platform::COMMAND_COPY_DIAGNOSTIC,
                    0x4f => platform::COMMAND_OPEN_FILE,
                    0x54 => platform::COMMAND_OPEN_TERMINAL,
                    0x56 => platform::COMMAND_PASTE,
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
                if !app.terminal_open {
                    match w_param as u32 {
                        VK_BACK => app.backspace(),
                        VK_LEFT => app.caret_column = app.caret_column.saturating_sub(1),
                        VK_RIGHT => app.caret_column += 1,
                        VK_UP => app.caret_line = app.caret_line.saturating_sub(1),
                        VK_DOWN => app.caret_line += 1,
                        _ => return,
                    }
                    app.clamp_caret();
                }
            });
            invalidate(window);
            0
        }
        WM_LBUTTONDOWN => {
            let y = ((l_param >> 16) as u16) as i32;
            let mut area = unsafe { std::mem::zeroed::<Rect>() };
            unsafe { GetClientRect(window, &mut area) };
            let scale = platform::dpi_for_window(window) as f32 / 96.0;
            let terminal_top = APP.with(|cell| {
                let motion = cell.borrow().terminal_motion.clamp(0.0, 1.0);
                area.bottom - ((28.0 + 214.0 * motion) * scale) as i32
            });
            if y >= terminal_top && y <= terminal_top + (34.0 * scale) as i32 {
                APP.with(|cell| cell.borrow_mut().toggle_terminal());
                invalidate(window);
            }
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
                if let Some(terminal) = &app.terminal {
                    terminal.resize((width / 9).min(i16::MAX as u32) as i16, 30);
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
            APP.with(|cell| cell.borrow_mut().terminal = None);
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
