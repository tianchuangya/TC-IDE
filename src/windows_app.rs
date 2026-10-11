#![allow(clippy::upper_case_acronyms)]

use std::ffi::c_void;
use std::ptr;
use std::sync::{Mutex, OnceLock};

#[path = "windows_platform.rs"]
mod platform;

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
const DT_CENTER: Uint = 0x0001;
const IDC_ARROW: *const u16 = 32512_usize as *const u16;
const SW_SHOW: i32 = 5;
const WM_COMMAND: Uint = 0x0111;
const WM_DESTROY: Uint = 0x0002;
const WM_DPICHANGED: Uint = 0x02e0;
const WM_KEYDOWN: Uint = 0x0100;
const WM_PAINT: Uint = 0x000f;
const WS_OVERLAPPEDWINDOW: Dword = 0x00cf0000;
const WS_VISIBLE: Dword = 0x10000000;
const TRANSPARENT: i32 = 1;

static STATUS: OnceLock<Mutex<String>> = OnceLock::new();

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
    fn DrawTextW(dc: Hdc, text: *const u16, length: i32, area: *mut Rect, format: Uint) -> i32;
    fn EndPaint(window: Hwnd, paint: *const PaintStruct) -> Bool;
    fn FillRect(dc: Hdc, area: *const Rect, brush: Hbrush) -> i32;
    fn GetClientRect(window: Hwnd, area: *mut Rect) -> Bool;
    fn GetMessageW(message: *mut Msg, window: Hwnd, minimum: Uint, maximum: Uint) -> Bool;
    fn InvalidateRect(window: Hwnd, area: *const Rect, erase: Bool) -> Bool;
    fn LoadCursorW(instance: Hinstance, cursor_name: *const u16) -> Hcursor;
    fn PostQuitMessage(exit_code: i32);
    fn RegisterClassW(class: *const WindowClass) -> u16;
    fn ShowWindow(window: Hwnd, command: i32) -> Bool;
    fn TranslateMessage(message: *const Msg) -> Bool;
    fn UpdateWindow(window: Hwnd) -> Bool;
}

#[link(name = "gdi32")]
extern "system" {
    fn CreateSolidBrush(color: Dword) -> Hbrush;
    fn DeleteObject(object: *mut c_void) -> Bool;
    fn SetBkMode(dc: Hdc, mode: i32) -> i32;
    fn SetTextColor(dc: Hdc, color: Dword) -> Dword;
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn status() -> &'static Mutex<String> {
    STATUS.get_or_init(|| Mutex::new("Windows platform services are ready".into()))
}

fn set_status(window: Hwnd, value: impl Into<String>) {
    if let Ok(mut status) = status().lock() {
        *status = value.into();
    }
    unsafe {
        InvalidateRect(window, ptr::null(), 1);
    }
}

fn perform_command(window: Hwnd, command: Wparam) {
    match command {
        platform::COMMAND_OPEN_FILE => {
            if let Some(path) = platform::choose_file(window) {
                set_status(
                    window,
                    format!("Selected with GetOpenFileNameW: {}", path.display()),
                );
            } else {
                set_status(window, "File selection cancelled");
            }
        }
        platform::COMMAND_COPY_DIAGNOSTIC => {
            let message = "TC-IDE Windows Clipboard API is connected";
            match platform::write_clipboard(window, message) {
                Ok(()) => set_status(window, "Copied Unicode text through Windows Clipboard API"),
                Err(error) => set_status(window, format!("Clipboard write failed: {error}")),
            }
        }
        platform::COMMAND_PASTE => match platform::read_clipboard(window) {
            Ok(value) => {
                let preview = value
                    .chars()
                    .take(96)
                    .collect::<String>()
                    .replace('\r', " ")
                    .replace('\n', " ");
                set_status(
                    window,
                    format!(
                        "Read {} characters from Windows Clipboard: {preview}",
                        value.chars().count()
                    ),
                );
            }
            Err(error) => set_status(window, format!("Clipboard read failed: {error}")),
        },
        platform::COMMAND_OPEN_FOLDER => match std::env::current_dir() {
            Ok(path) if platform::open_in_explorer(window, &path) => {
                set_status(window, format!("Opened Explorer at {}", path.display()));
            }
            Ok(path) => set_status(
                window,
                format!("Could not open Explorer at {}", path.display()),
            ),
            Err(error) => set_status(
                window,
                format!("Could not resolve project directory: {error}"),
            ),
        },
        platform::COMMAND_OPEN_TERMINAL => match std::env::current_dir() {
            Ok(path) if platform::open_terminal(window, &path) => {
                set_status(
                    window,
                    format!("Opened Windows terminal at {}", path.display()),
                );
            }
            Ok(path) => set_status(
                window,
                format!("Could not open terminal at {}", path.display()),
            ),
            Err(error) => set_status(
                window,
                format!("Could not resolve project directory: {error}"),
            ),
        },
        _ => {}
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
            let mut paint: PaintStruct = std::mem::zeroed();
            let dc = BeginPaint(window, &mut paint);
            let mut area: Rect = std::mem::zeroed();
            GetClientRect(window, &mut area);
            let background = CreateSolidBrush(0x0018_1410);
            FillRect(dc, &area, background);
            DeleteObject(background);
            SetBkMode(dc, TRANSPARENT);
            SetTextColor(dc, 0x00ec_e7dc);
            area.left += 72;
            area.right -= 72;
            area.top += 110;
            area.bottom -= 90;
            let status = status()
                .lock()
                .map(|value| value.clone())
                .unwrap_or_else(|_| "Status unavailable".into());
            let text = wide(&format!(
                "TC-IDE Windows native backend\n\nWin32 window + per-monitor DPI: {} DPI\nWindows Clipboard: connected\nNative file dialog: connected\nExplorer / ShellExecute: connected\nWindows Terminal / PowerShell: connected\n\nCtrl+O  Open file    Ctrl+C  Copy diagnostic\nCtrl+V  Read clipboard    Ctrl+T  Open terminal\n\n{status}",
                platform::dpi_for_window(window)
            ));
            DrawTextW(dc, text.as_ptr(), -1, &mut area, DT_CENTER);
            EndPaint(window, &paint);
            0
        }
        WM_COMMAND => {
            perform_command(window, w_param & 0xffff);
            0
        }
        WM_KEYDOWN => {
            if platform::control_pressed() {
                let command = match w_param as u32 {
                    0x43 => platform::COMMAND_COPY_DIAGNOSTIC,
                    0x4f => platform::COMMAND_OPEN_FILE,
                    0x54 => platform::COMMAND_OPEN_TERMINAL,
                    0x56 => platform::COMMAND_PASTE,
                    _ => 0,
                };
                if command != 0 {
                    perform_command(window, command);
                    return 0;
                }
            }
            DefWindowProcW(window, message, w_param, l_param)
        }
        WM_DPICHANGED => {
            platform::apply_dpi_change(window, l_param);
            set_status(
                window,
                format!("DPI changed to {}", platform::dpi_for_window(window)),
            );
            0
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(window, message, w_param, l_param),
    }
}

pub fn run() {
    unsafe {
        platform::enable_per_monitor_dpi();
        let instance = GetModuleHandleW(ptr::null());
        let class_name = wide("TC_IDE_WINDOWS_BACKEND");
        let title = wide("TC-IDE — Windows native backend");
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
            1280,
            800,
            ptr::null_mut(),
            ptr::null_mut(),
            instance,
            ptr::null_mut(),
        );
        if window.is_null() {
            return;
        }
        platform::install_native_menu(window);
        ShowWindow(window, SW_SHOW);
        UpdateWindow(window);
        let mut message: Msg = std::mem::zeroed();
        while GetMessageW(&mut message, ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}
