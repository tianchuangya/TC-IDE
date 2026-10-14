#![allow(clippy::upper_case_acronyms)]

use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::ptr;

pub type Hwnd = *mut c_void;
type Bool = i32;
type Dword = u32;
type Handle = *mut c_void;
type Hmenu = *mut c_void;
type Lparam = isize;
type Uint = u32;
type Wparam = usize;

pub const COMMAND_OPEN_FILE: Wparam = 1001;
pub const COMMAND_COPY_DIAGNOSTIC: Wparam = 1002;
pub const COMMAND_PASTE: Wparam = 1003;
pub const COMMAND_OPEN_FOLDER: Wparam = 1004;
pub const COMMAND_OPEN_TERMINAL: Wparam = 1005;
pub const COMMAND_SELECT_ALL: Wparam = 1006;
pub const COMMAND_CUT: Wparam = 1007;
pub const COMMAND_UNDO: Wparam = 1008;
pub const COMMAND_REDO: Wparam = 1009;
pub const COMMAND_REVEAL_FOLDER: Wparam = 1010;
pub const COMMAND_THEME_DARK: Wparam = 1011;
pub const COMMAND_THEME_LIGHT: Wparam = 1012;
pub const COMMAND_LANGUAGE_TOGGLE: Wparam = 1013;
pub const COMMAND_FONT_INCREASE: Wparam = 1014;
pub const COMMAND_FONT_DECREASE: Wparam = 1015;
pub const COMMAND_NEW_PROJECT: Wparam = 1016;
pub const COMMAND_NEW_FILE: Wparam = 1017;
pub const COMMAND_NEW_FOLDER: Wparam = 1018;
pub const COMMAND_WORKSPACE_SEARCH: Wparam = 1019;
pub const COMMAND_SETTINGS: Wparam = 1020;
pub const COMMAND_EXTENSIONS: Wparam = 1021;
pub const COMMAND_PACKAGE: Wparam = 1022;
pub const COMMAND_REFRESH_PROJECT: Wparam = 1023;

const CF_UNICODETEXT: Uint = 13;
const GMEM_MOVEABLE: Uint = 0x0002;
const MF_POPUP: Uint = 0x0010;
const MF_STRING: Uint = 0x0000;
const OFN_EXPLORER: Dword = 0x0008_0000;
const OFN_FILEMUSTEXIST: Dword = 0x0000_1000;
const OFN_PATHMUSTEXIST: Dword = 0x0000_0800;
const SW_SHOWNORMAL: i32 = 1;
const SWP_NOACTIVATE: Uint = 0x0010;
const SWP_NOZORDER: Uint = 0x0004;
const VK_CONTROL: i32 = 0x11;
const VK_SHIFT: i32 = 0x10;

#[repr(C)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

#[repr(C)]
struct OpenFileName {
    struct_size: Dword,
    owner: Hwnd,
    instance: Handle,
    filter: *const u16,
    custom_filter: *mut u16,
    max_custom_filter: Dword,
    filter_index: Dword,
    file: *mut u16,
    max_file: Dword,
    file_title: *mut u16,
    max_file_title: Dword,
    initial_directory: *const u16,
    title: *const u16,
    flags: Dword,
    file_offset: u16,
    file_extension: u16,
    default_extension: *const u16,
    custom_data: Lparam,
    hook: *mut c_void,
    template_name: *const u16,
    reserved: *mut c_void,
    reserved_dword: Dword,
    flags_ex: Dword,
}

#[repr(C)]
struct BrowseInfo {
    owner: Hwnd,
    root: *const c_void,
    display_name: *mut u16,
    title: *const u16,
    flags: Uint,
    callback: *mut c_void,
    parameter: Lparam,
    image: i32,
}

#[repr(C)]
struct Point {
    x: i32,
    y: i32,
}

#[link(name = "kernel32")]
extern "system" {
    fn GlobalAlloc(flags: Uint, bytes: usize) -> Handle;
    fn GlobalFree(memory: Handle) -> Handle;
    fn GlobalLock(memory: Handle) -> *mut c_void;
    fn GlobalSize(memory: Handle) -> usize;
    fn GlobalUnlock(memory: Handle) -> Bool;
}

#[link(name = "user32")]
extern "system" {
    fn AppendMenuW(menu: Hmenu, flags: Uint, item: Wparam, text: *const u16) -> Bool;
    fn CloseClipboard() -> Bool;
    fn CreateMenu() -> Hmenu;
    fn CreatePopupMenu() -> Hmenu;
    fn ClientToScreen(window: Hwnd, point: *mut Point) -> Bool;
    fn DestroyMenu(menu: Hmenu) -> Bool;
    fn EmptyClipboard() -> Bool;
    fn GetClipboardData(format: Uint) -> Handle;
    fn GetDpiForWindow(window: Hwnd) -> Uint;
    fn GetKeyState(key: i32) -> i16;
    fn IsClipboardFormatAvailable(format: Uint) -> Bool;
    fn OpenClipboard(owner: Hwnd) -> Bool;
    fn SetClipboardData(format: Uint, memory: Handle) -> Handle;
    fn SetMenu(window: Hwnd, menu: Hmenu) -> Bool;
    fn SetProcessDpiAwarenessContext(context: Handle) -> Bool;
    fn SetWindowPos(
        window: Hwnd,
        insert_after: Hwnd,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        flags: Uint,
    ) -> Bool;
    fn TrackPopupMenu(
        menu: Hmenu,
        flags: Uint,
        x: i32,
        y: i32,
        reserved: i32,
        window: Hwnd,
        area: *const c_void,
    ) -> Uint;
}

#[link(name = "comdlg32")]
extern "system" {
    fn GetOpenFileNameW(file_name: *mut OpenFileName) -> Bool;
}

#[link(name = "shell32")]
extern "system" {
    fn ShellExecuteW(
        window: Hwnd,
        operation: *const u16,
        file: *const u16,
        parameters: *const u16,
        directory: *const u16,
        show: i32,
    ) -> Handle;
    fn SHBrowseForFolderW(info: *const BrowseInfo) -> *mut c_void;
    fn SHGetPathFromIDListW(item: *const c_void, path: *mut u16) -> Bool;
}

#[link(name = "ole32")]
extern "system" {
    fn CoTaskMemFree(memory: *const c_void);
}

pub fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn shell_execute(window: Hwnd, file: &str, parameters: Option<&str>) -> bool {
    let operation = wide("open");
    let file = wide(file);
    let parameters = parameters.map(wide);
    let result = unsafe {
        ShellExecuteW(
            window,
            operation.as_ptr(),
            file.as_ptr(),
            parameters
                .as_ref()
                .map(|value| value.as_ptr())
                .unwrap_or(ptr::null()),
            ptr::null(),
            SW_SHOWNORMAL,
        )
    };
    result as isize > 32
}

pub fn enable_per_monitor_dpi() {
    // DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2 is the documented -4 pseudo handle.
    unsafe {
        SetProcessDpiAwarenessContext((-4_isize) as Handle);
    }
}

pub fn dpi_for_window(window: Hwnd) -> u32 {
    unsafe { GetDpiForWindow(window).max(96) }
}

pub fn apply_dpi_change(window: Hwnd, l_param: Lparam) {
    if l_param == 0 {
        return;
    }
    let suggested = unsafe { &*(l_param as *const Rect) };
    unsafe {
        SetWindowPos(
            window,
            ptr::null_mut(),
            suggested.left,
            suggested.top,
            suggested.right - suggested.left,
            suggested.bottom - suggested.top,
            SWP_NOACTIVATE | SWP_NOZORDER,
        );
    }
}

pub fn control_pressed() -> bool {
    unsafe { GetKeyState(VK_CONTROL) < 0 }
}

pub fn shift_pressed() -> bool {
    unsafe { GetKeyState(VK_SHIFT) < 0 }
}

pub fn install_native_menu(window: Hwnd) {
    unsafe {
        let menu = CreateMenu();
        let file_menu = CreatePopupMenu();
        let edit_menu = CreatePopupMenu();
        let view_menu = CreatePopupMenu();
        let platform_menu = CreatePopupMenu();
        let open_file = wide("Open file...\tCtrl+O");
        let new_project = wide("New project...");
        let new_file = wide("New file...");
        let new_folder = wide("New folder...");
        let open_folder = wide("Open project folder...");
        let reveal_folder = wide("Reveal current project in Explorer");
        let open_terminal = wide("Toggle integrated ConPTY terminal\tCtrl+T");
        let undo = wide("Undo\tCtrl+Z");
        let redo = wide("Redo\tCtrl+Y");
        let select_all = wide("Select all\tCtrl+A");
        let copy = wide("Copy\tCtrl+C");
        let cut = wide("Cut\tCtrl+X");
        let paste = wide("Paste\tCtrl+V");
        let dark = wide("Dark theme");
        let light = wide("Light theme");
        let language = wide("English / 中文");
        let font_increase = wide("Increase code size\tCtrl++");
        let font_decrease = wide("Decrease code size\tCtrl+-");
        let workspace_search = wide("Search project files\tCtrl+P");
        let settings = wide("Settings...");
        let extensions = wide("Extensions...");
        let package = wide("Package project...");
        AppendMenuW(
            file_menu,
            MF_STRING,
            COMMAND_NEW_PROJECT,
            new_project.as_ptr(),
        );
        AppendMenuW(view_menu, MF_STRING, COMMAND_SETTINGS, settings.as_ptr());
        AppendMenuW(
            view_menu,
            MF_STRING,
            COMMAND_EXTENSIONS,
            extensions.as_ptr(),
        );
        AppendMenuW(file_menu, MF_STRING, COMMAND_NEW_FILE, new_file.as_ptr());
        AppendMenuW(
            file_menu,
            MF_STRING,
            COMMAND_NEW_FOLDER,
            new_folder.as_ptr(),
        );
        AppendMenuW(file_menu, MF_STRING, COMMAND_OPEN_FILE, open_file.as_ptr());
        AppendMenuW(file_menu, MF_STRING, COMMAND_PACKAGE, package.as_ptr());
        AppendMenuW(
            file_menu,
            MF_STRING,
            COMMAND_OPEN_FOLDER,
            open_folder.as_ptr(),
        );
        AppendMenuW(
            file_menu,
            MF_STRING,
            COMMAND_REVEAL_FOLDER,
            reveal_folder.as_ptr(),
        );
        AppendMenuW(
            platform_menu,
            MF_STRING,
            COMMAND_OPEN_TERMINAL,
            open_terminal.as_ptr(),
        );
        AppendMenuW(edit_menu, MF_STRING, COMMAND_UNDO, undo.as_ptr());
        AppendMenuW(edit_menu, MF_STRING, COMMAND_REDO, redo.as_ptr());
        AppendMenuW(
            edit_menu,
            MF_STRING,
            COMMAND_SELECT_ALL,
            select_all.as_ptr(),
        );
        AppendMenuW(edit_menu, MF_STRING, COMMAND_COPY_DIAGNOSTIC, copy.as_ptr());
        AppendMenuW(edit_menu, MF_STRING, COMMAND_CUT, cut.as_ptr());
        AppendMenuW(edit_menu, MF_STRING, COMMAND_PASTE, paste.as_ptr());
        AppendMenuW(view_menu, MF_STRING, COMMAND_THEME_DARK, dark.as_ptr());
        AppendMenuW(view_menu, MF_STRING, COMMAND_THEME_LIGHT, light.as_ptr());
        AppendMenuW(
            view_menu,
            MF_STRING,
            COMMAND_LANGUAGE_TOGGLE,
            language.as_ptr(),
        );
        AppendMenuW(
            view_menu,
            MF_STRING,
            COMMAND_FONT_INCREASE,
            font_increase.as_ptr(),
        );
        AppendMenuW(
            view_menu,
            MF_STRING,
            COMMAND_FONT_DECREASE,
            font_decrease.as_ptr(),
        );
        AppendMenuW(
            view_menu,
            MF_STRING,
            COMMAND_WORKSPACE_SEARCH,
            workspace_search.as_ptr(),
        );
        let file_label = wide("File");
        let edit_label = wide("Edit");
        let view_label = wide("View");
        let platform_label = wide("Windows API");
        AppendMenuW(menu, MF_POPUP, file_menu as Wparam, file_label.as_ptr());
        AppendMenuW(menu, MF_POPUP, edit_menu as Wparam, edit_label.as_ptr());
        AppendMenuW(menu, MF_POPUP, view_menu as Wparam, view_label.as_ptr());
        AppendMenuW(
            menu,
            MF_POPUP,
            platform_menu as Wparam,
            platform_label.as_ptr(),
        );
        SetMenu(window, menu);
    }
}

pub fn workspace_context_menu(window: Hwnd, x: i32, y: i32) -> Option<Wparam> {
    unsafe {
        let menu = CreatePopupMenu();
        if menu.is_null() {
            return None;
        }
        let project = wide("New project...");
        let file = wide("New file...");
        let folder = wide("New folder...");
        let refresh = wide("Refresh project tree");
        AppendMenuW(menu, MF_STRING, COMMAND_NEW_PROJECT, project.as_ptr());
        AppendMenuW(menu, MF_STRING, COMMAND_NEW_FILE, file.as_ptr());
        AppendMenuW(menu, MF_STRING, COMMAND_NEW_FOLDER, folder.as_ptr());
        AppendMenuW(menu, MF_STRING, COMMAND_REFRESH_PROJECT, refresh.as_ptr());
        let mut point = Point { x, y };
        ClientToScreen(window, &mut point);
        let command = TrackPopupMenu(
            menu,
            0x0100 | 0x0002,
            point.x,
            point.y,
            0,
            window,
            ptr::null(),
        ) as Wparam;
        DestroyMenu(menu);
        (command != 0).then_some(command)
    }
}

pub fn choose_file(window: Hwnd) -> Option<PathBuf> {
    let mut file = [0_u16; 32_768];
    let title = wide("Open a file in TC-IDE");
    let filter = "Source files\0*.rs;*.c;*.h;*.cpp;*.hpp;*.py;*.java;*.js;*.ts\0All files\0*.*\0\0"
        .encode_utf16()
        .collect::<Vec<_>>();
    let mut dialog = OpenFileName {
        struct_size: std::mem::size_of::<OpenFileName>() as Dword,
        owner: window,
        instance: ptr::null_mut(),
        filter: filter.as_ptr(),
        custom_filter: ptr::null_mut(),
        max_custom_filter: 0,
        filter_index: 1,
        file: file.as_mut_ptr(),
        max_file: file.len() as Dword,
        file_title: ptr::null_mut(),
        max_file_title: 0,
        initial_directory: ptr::null(),
        title: title.as_ptr(),
        flags: OFN_EXPLORER | OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST,
        file_offset: 0,
        file_extension: 0,
        default_extension: ptr::null(),
        custom_data: 0,
        hook: ptr::null_mut(),
        template_name: ptr::null(),
        reserved: ptr::null_mut(),
        reserved_dword: 0,
        flags_ex: 0,
    };
    if unsafe { GetOpenFileNameW(&mut dialog) } == 0 {
        return None;
    }
    let length = file
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(file.len());
    Some(PathBuf::from(String::from_utf16_lossy(&file[..length])))
}

pub fn choose_background(window: Hwnd) -> Option<PathBuf> {
    let mut file = [0_u16; 32_768];
    let title = wide("Choose a TC-IDE background image");
    let filter = "Images\0*.png;*.jpg;*.jpeg;*.bmp;*.tif;*.tiff\0All files\0*.*\0\0"
        .encode_utf16()
        .collect::<Vec<_>>();
    let mut dialog = OpenFileName {
        struct_size: std::mem::size_of::<OpenFileName>() as Dword,
        owner: window,
        instance: ptr::null_mut(),
        filter: filter.as_ptr(),
        custom_filter: ptr::null_mut(),
        max_custom_filter: 0,
        filter_index: 1,
        file: file.as_mut_ptr(),
        max_file: file.len() as Dword,
        file_title: ptr::null_mut(),
        max_file_title: 0,
        initial_directory: ptr::null(),
        title: title.as_ptr(),
        flags: OFN_EXPLORER | OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST,
        file_offset: 0,
        file_extension: 0,
        default_extension: ptr::null(),
        custom_data: 0,
        hook: ptr::null_mut(),
        template_name: ptr::null(),
        reserved: ptr::null_mut(),
        reserved_dword: 0,
        flags_ex: 0,
    };
    if unsafe { GetOpenFileNameW(&mut dialog) } == 0 {
        return None;
    }
    let length = file
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(file.len());
    Some(PathBuf::from(String::from_utf16_lossy(&file[..length])))
}

pub fn choose_folder(window: Hwnd) -> Option<PathBuf> {
    let title = wide("Open a project folder in TC-IDE");
    let mut display_name = [0u16; 260];
    let info = BrowseInfo {
        owner: window,
        root: ptr::null(),
        display_name: display_name.as_mut_ptr(),
        title: title.as_ptr(),
        // BIF_RETURNONLYFSDIRS | BIF_NEWDIALOGSTYLE | BIF_EDITBOX
        flags: 0x0001 | 0x0040 | 0x0010,
        callback: ptr::null_mut(),
        parameter: 0,
        image: 0,
    };
    let item = unsafe { SHBrowseForFolderW(&info) };
    if item.is_null() {
        return None;
    }
    let mut path = [0u16; 32_768];
    let ok = unsafe { SHGetPathFromIDListW(item, path.as_mut_ptr()) != 0 };
    unsafe { CoTaskMemFree(item) };
    if !ok {
        return None;
    }
    let length = path
        .iter()
        .position(|value| *value == 0)
        .unwrap_or(path.len());
    Some(PathBuf::from(String::from_utf16_lossy(&path[..length])))
}

pub fn write_clipboard(window: Hwnd, value: &str) -> Result<(), &'static str> {
    let contents = wide(value);
    unsafe {
        if OpenClipboard(window) == 0 {
            return Err("OpenClipboard failed");
        }
        if EmptyClipboard() == 0 {
            CloseClipboard();
            return Err("EmptyClipboard failed");
        }
        let bytes = contents.len() * std::mem::size_of::<u16>();
        let memory = GlobalAlloc(GMEM_MOVEABLE, bytes);
        if memory.is_null() {
            CloseClipboard();
            return Err("GlobalAlloc failed");
        }
        let target = GlobalLock(memory) as *mut u16;
        if target.is_null() {
            GlobalFree(memory);
            CloseClipboard();
            return Err("GlobalLock failed");
        }
        ptr::copy_nonoverlapping(contents.as_ptr(), target, contents.len());
        GlobalUnlock(memory);
        if SetClipboardData(CF_UNICODETEXT, memory).is_null() {
            GlobalFree(memory);
            CloseClipboard();
            return Err("SetClipboardData failed");
        }
        // Ownership of memory transfers to the system after SetClipboardData succeeds.
        CloseClipboard();
    }
    Ok(())
}

pub fn read_clipboard(window: Hwnd) -> Result<String, &'static str> {
    unsafe {
        if IsClipboardFormatAvailable(CF_UNICODETEXT) == 0 {
            return Err("CF_UNICODETEXT is unavailable");
        }
        if OpenClipboard(window) == 0 {
            return Err("OpenClipboard failed");
        }
        let memory = GetClipboardData(CF_UNICODETEXT);
        if memory.is_null() {
            CloseClipboard();
            return Err("GetClipboardData failed");
        }
        let source = GlobalLock(memory) as *const u16;
        if source.is_null() {
            CloseClipboard();
            return Err("GlobalLock failed");
        }
        let maximum = GlobalSize(memory) / std::mem::size_of::<u16>();
        let mut length = 0;
        while length < maximum && *source.add(length) != 0 {
            length += 1;
        }
        let value = String::from_utf16_lossy(std::slice::from_raw_parts(source, length));
        GlobalUnlock(memory);
        CloseClipboard();
        Ok(value)
    }
}

pub fn open_in_explorer(window: Hwnd, path: &Path) -> bool {
    shell_execute(window, &path.to_string_lossy(), None)
}

pub fn open_external_terminal(window: Hwnd, path: &Path) -> bool {
    let display = path.to_string_lossy();
    let quoted = format!("-d \"{}\"", display.replace('"', "\\\""));
    if shell_execute(window, "wt.exe", Some(&quoted)) {
        return true;
    }
    let escaped = display.replace('\'', "''");
    shell_execute(
        window,
        "powershell.exe",
        Some(&format!(
            "-NoExit -Command \"Set-Location -LiteralPath '{}'\"",
            escaped
        )),
    )
}
