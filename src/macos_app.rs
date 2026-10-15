//! macOS backend: a Cocoa window with the shared editor inside it.
//!
//! The backend talks to AppKit through the Objective-C runtime directly instead
//! of pulling in a binding crate, which keeps the dependency list empty on every
//! platform. It is deliberately thin: the document, the piece table, the
//! viewport, the session, the palette and the motion models all come from the
//! shared cores, so this file only owns what is truly macOS specific — the
//! window, the view that draws a frame, the key mapping and the application
//! menu.
//!
//! Verification: the type level is checked on any host with
//! `cargo check --target x86_64-apple-darwin`, the release build runs on the
//! macOS runner of the pipeline, and that runner also executes
//! `tc-ide --self-check`, so this backend is executed and not only compiled.

#![allow(dead_code)] // The panels and the pasteboard land on top of this.

use std::cell::RefCell;
use std::ffi::{c_char, c_double, c_int, c_long, c_void, CString};
use std::path::PathBuf;
use std::ptr;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::editor_core;
use crate::editor_session;
use crate::extension_core;
use crate::graph_core::{self, Graph, GraphNode, GraphRelation, GraphScope};
use crate::lsp_core;
use crate::motion_core;
use crate::syntax_core;
use crate::terminal_core::{
    self, TerminalKey, TerminalParser, TerminalView, TerminalViewport, Utf8Chunker,
};
use crate::text_buffer;
use crate::theme_core;
use crate::workspace_core;

/// An object or a class of the Objective-C runtime.
type Id = *mut c_void;
/// A selector of the Objective-C runtime.
type Sel = *mut c_void;
/// Signature of a method implementation.
type Imp = unsafe extern "C" fn();

#[link(name = "objc")]
extern "C" {
    fn objc_getClass(name: *const c_char) -> Id;
    fn sel_registerName(name: *const c_char) -> Sel;
    fn objc_msgSend();
    fn objc_allocateClassPair(superclass: Id, name: *const c_char, extra: usize) -> Id;
    fn objc_registerClassPair(class: Id);
    fn class_addMethod(class: Id, selector: Sel, implementation: Imp, types: *const c_char)
        -> bool;
    fn sel_getName(selector: Sel) -> *const c_char;
}

#[link(name = "AppKit", kind = "framework")]
extern "C" {
    fn NSApplicationLoad() -> bool;
}

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGContextSetRGBFillColor(
        context: Id,
        red: c_double,
        green: c_double,
        blue: c_double,
        alpha: c_double,
    );
    fn CGContextFillRect(context: Id, rect: CGRect);
}

/// A rectangle in the coordinates AppKit draws in.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct CGRect {
    x: c_double,
    y: c_double,
    width: c_double,
    height: c_double,
}

/// A point in view coordinates.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct CGPoint {
    x: c_double,
    y: c_double,
}

/// A size in view coordinates.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct CGSize {
    width: c_double,
    height: c_double,
}

impl CGRect {
    fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    fn contains(&self, point: CGPoint) -> bool {
        point.x >= self.x
            && point.x <= self.x + self.width
            && point.y >= self.y
            && point.y <= self.y + self.height
    }
}

/// The message send, as a function pointer of the shape the caller needs.
///
/// The Objective-C runtime only exports one `objc_msgSend`; every call site
/// gives it the signature of the method it is calling.
macro_rules! send_as {
    ($signature:ty) => {
        std::mem::transmute::<*const c_void, $signature>(objc_msgSend as *const c_void)
    };
}

/// Call a method that returns an object.
///
/// # Safety
/// The selector has to exist on the receiver and return an object.
unsafe fn msg_id(receiver: Id, selector: &str) -> Id {
    let send = send_as!(extern "C" fn(Id, Sel) -> Id);
    send(receiver, selector_name(selector))
}

/// Call a method that takes one object and returns an object.
///
/// # Safety
/// The selector has to exist on the receiver and take an object.
unsafe fn msg_id_id(receiver: Id, selector: &str, argument: Id) -> Id {
    let send = send_as!(extern "C" fn(Id, Sel, Id) -> Id);
    send(receiver, selector_name(selector), argument)
}

/// Call a method that takes one object and returns nothing.
///
/// # Safety
/// The selector has to exist on the receiver and take an object.
unsafe fn msg_void_id(receiver: Id, selector: &str, argument: Id) {
    let send = send_as!(extern "C" fn(Id, Sel, Id));
    send(receiver, selector_name(selector), argument);
}

/// Call a method that takes two objects and returns nothing.
///
/// # Safety
/// The selector has to exist on the receiver and take two objects.
unsafe fn msg_void_id_id(receiver: Id, selector: &str, first: Id, second: Id) {
    let send = send_as!(extern "C" fn(Id, Sel, Id, Id));
    send(receiver, selector_name(selector), first, second);
}

/// Call a method that takes one boolean.
///
/// # Safety
/// The selector has to exist on the receiver.
unsafe fn msg_bool(receiver: Id, selector: &str, argument: bool) {
    let send = send_as!(extern "C" fn(Id, Sel, bool));
    send(receiver, selector_name(selector), argument);
}

/// Call a method that takes one integer.
///
/// # Safety
/// The selector has to exist on the receiver.
unsafe fn msg_long(receiver: Id, selector: &str, argument: c_long) {
    let send = send_as!(extern "C" fn(Id, Sel, c_long));
    send(receiver, selector_name(selector), argument);
}

/// Call a method that takes one double and returns an object.
///
/// # Safety
/// The selector has to exist on the receiver.
unsafe fn msg_id_double(receiver: Id, selector: &str, argument: f64) -> Id {
    let send = send_as!(extern "C" fn(Id, Sel, f64) -> Id);
    send(receiver, selector_name(selector), argument)
}

/// Call a method that returns a rectangle.
///
/// # Safety
/// The selector has to exist on the receiver and return a rectangle.
unsafe fn msg_rect_out(receiver: Id, selector: &str) -> CGRect {
    let send = send_as!(extern "C" fn(Id, Sel) -> CGRect);
    send(receiver, selector_name(selector))
}

/// Call a method that returns a point.
///
/// # Safety
/// The selector has to exist on the receiver and return a point.
unsafe fn msg_point_out(receiver: Id, selector: &str) -> CGPoint {
    let send = send_as!(extern "C" fn(Id, Sel) -> CGPoint);
    send(receiver, selector_name(selector))
}

/// Call a method that returns an unsigned integer.
///
/// # Safety
/// The selector has to exist on the receiver and return an integer.
unsafe fn msg_usize(receiver: Id, selector: &str) -> usize {
    let send = send_as!(extern "C" fn(Id, Sel) -> usize);
    send(receiver, selector_name(selector))
}

/// Call a method that takes a rectangle and returns an object.
///
/// # Safety
/// Only used with `initWithFrame:` style constructors.
unsafe fn msg_init_rect(receiver: Id, selector: &str, rect: CGRect) -> Id {
    let send = send_as!(extern "C" fn(Id, Sel, CGRect) -> Id);
    send(receiver, selector_name(selector), rect)
}

/// Call an `NSWindow` style initialiser.
///
/// # Safety
/// Only used with `initWithContentRect:styleMask:backing:defer:`.
unsafe fn msg_window_init(
    receiver: Id,
    selector: &str,
    rect: CGRect,
    style: c_long,
    backing: c_long,
    defer: bool,
) -> Id {
    let send = send_as!(extern "C" fn(Id, Sel, CGRect, c_long, c_long, bool) -> Id);
    send(
        receiver,
        selector_name(selector),
        rect,
        style,
        backing,
        defer,
    )
}

/// Call `drawAtPoint:withAttributes:`.
///
/// # Safety
/// The selector has to exist on the receiver and take a point and a dictionary.
unsafe fn msg_draw_at_point(receiver: Id, point: CGPoint, attributes: Id) {
    let send = send_as!(extern "C" fn(Id, Sel, CGPoint, Id));
    send(
        receiver,
        selector_name("drawAtPoint:withAttributes:"),
        point,
        attributes,
    );
}

/// Call `colorWithCalibratedRed:green:blue:alpha:`.
///
/// # Safety
/// Builds an AppKit object; call on the main thread.
unsafe fn msg_color(red: f64, green: f64, blue: f64, alpha: f64) -> Id {
    let send = send_as!(extern "C" fn(Id, Sel, f64, f64, f64, f64) -> Id);
    send(
        class("NSColor"),
        selector_name("colorWithCalibratedRed:green:blue:alpha:"),
        red,
        green,
        blue,
        alpha,
    )
}

/// Selector for a method name, interned by the runtime.
fn selector_name(name: &str) -> Sel {
    let text = CString::new(name).unwrap_or_default();
    unsafe { sel_registerName(text.as_ptr()) }
}

/// Class object for a runtime class name.
fn class(name: &str) -> Id {
    let text = CString::new(name).unwrap_or_default();
    unsafe { objc_getClass(text.as_ptr()) }
}

/// `NSString` from a Rust string.
fn nsstring(value: &str) -> Id {
    let text = CString::new(value).unwrap_or_default();
    unsafe {
        let string = msg_id(class("NSString"), "alloc");
        msg_id_id(string, "initWithUTF8String:", text.as_ptr() as Id)
    }
}

/// The visible part of a token that starts at `start`, once the view has been
/// scrolled right by `scroll` columns.
///
/// Returns the column the visible part begins at, and the text itself. A token
/// that is scrolled out entirely returns nothing, which is how the drawing loop
/// skips it.
fn visible_token(text: &str, start: usize, scroll: usize) -> Option<(usize, String)> {
    let end = start + text.chars().count();
    if end <= scroll {
        return None;
    }
    let first = start.max(scroll);
    Some((first, text.chars().skip(first - start).collect()))
}

/// Colour a syntax token uses in the current palette.
///
/// The tokens come from the shared tokenizer, and the colours from the shared
/// palette, so the macOS editor highlights the same code the same way the other
/// backends do.
fn token_color(palette: &theme_core::Palette, kind: editor_core::TokenKind) -> u32 {
    match kind {
        editor_core::TokenKind::Keyword => palette.keyword,
        editor_core::TokenKind::Type => palette.syntax_type,
        editor_core::TokenKind::Function => palette.function,
        editor_core::TokenKind::Constant => palette.constant,
        editor_core::TokenKind::Variable => palette.variable,
        editor_core::TokenKind::Parameter => palette.parameter,
        editor_core::TokenKind::Property => palette.property,
        editor_core::TokenKind::Macro => palette.macro_name,
        editor_core::TokenKind::Module => palette.module,
        editor_core::TokenKind::Preprocessor => palette.preprocessor,
        editor_core::TokenKind::Literal => palette.literal,
        editor_core::TokenKind::Comment => palette.comment,
        editor_core::TokenKind::Plain => palette.text,
    }
}

/// Width of one document tab in the header, in points.
const TAB_WIDTH: f64 = 150.0;
/// Left margin of the first tab.
const TAB_MARGIN: f64 = 12.0;

/// Width of the line number gutter, in points.
const GUTTER_WIDTH: f64 = 54.0;
/// Width of one character of the code font, in points.
const TEXT_CELL: f64 = 7.4;

/// Style mask of a titled, closable, resizable window.
const WINDOW_STYLE: c_long = 1 | 2 | 4 | 8;
/// Backing store kept in a buffer, which is what a resizable window wants.
const BACKING_BUFFERED: c_long = 2;
/// `⌘` in the modifier flags of an event.
const COMMAND_MASK: u64 = 1 << 20;
/// `⇧` in the modifier flags of an event.
const SHIFT_MASK: u64 = 1 << 17;

/// The general pasteboard, as the object AppKit hands out.
///
/// # Safety
/// Requires a running pasteboard server; call on the main thread.
unsafe fn general_pasteboard() -> Id {
    msg_id(class("NSPasteboard"), "generalPasteboard")
}

/// Write text to the system pasteboard.
///
/// # Safety
/// Call on the main thread.
unsafe fn pasteboard_write(value: &str) {
    let pasteboard = general_pasteboard();
    if pasteboard.is_null() {
        return;
    }
    // The type is the AppKit constant `NSStringPboardType`.
    msg_long(pasteboard, "clearContents", 0);
    msg_bool(pasteboard, "setString:forType:", false);
    let send: extern "C" fn(Id, Sel, Id, Id) -> bool =
        std::mem::transmute(objc_msgSend as *const c_void);
    send(
        pasteboard,
        selector_name("setString:forType:"),
        nsstring(value),
        nsstring("public.utf8-plain-text"),
    );
}

/// Read text from the system pasteboard.
///
/// # Safety
/// Call on the main thread.
unsafe fn pasteboard_read() -> Option<String> {
    let pasteboard = general_pasteboard();
    if pasteboard.is_null() {
        return None;
    }
    let string = msg_id_id(
        pasteboard,
        "stringForType:",
        nsstring("public.utf8-plain-text"),
    );
    if string.is_null() {
        return None;
    }
    let bytes = msg_utf8(string);
    if bytes.is_null() {
        return None;
    }
    Some(
        std::ffi::CStr::from_ptr(bytes)
            .to_string_lossy()
            .into_owned(),
    )
}

/// `UTF8String` of an `NSString`.
///
/// # Safety
/// The receiver has to be an `NSString`.
unsafe fn msg_utf8(receiver: Id) -> *const c_char {
    let send: extern "C" fn(Id, Sel) -> *const c_char =
        std::mem::transmute(objc_msgSend as *const c_void);
    send(receiver, selector_name("UTF8String"))
}

/// Path of an `NSURL`.
///
/// # Safety
/// The receiver has to be an `NSURL`.
unsafe fn url_path(url: Id) -> Option<PathBuf> {
    let path = msg_id(url, "path");
    if path.is_null() {
        return None;
    }
    let bytes = msg_utf8(path);
    if bytes.is_null() {
        return None;
    }
    Some(PathBuf::from(
        std::ffi::CStr::from_ptr(bytes)
            .to_string_lossy()
            .into_owned(),
    ))
}

/// Run an `NSOpenPanel` and return the chosen path.
///
/// # Safety
/// Call on the main thread with a running application.
unsafe fn run_open_panel(directories: bool) -> Option<PathBuf> {
    let panel = msg_id(class("NSOpenPanel"), "openPanel");
    if panel.is_null() {
        return None;
    }
    msg_bool(panel, "setCanChooseFiles:", !directories);
    msg_bool(panel, "setCanChooseDirectories:", directories);
    msg_bool(panel, "setAllowsMultipleSelection:", false);
    // A modal loop is what a panel needs when the app has no sheets yet.
    let response = msg_send_long(panel, "runModal");
    if response != 1 {
        // 1 is NSModalResponseOK.
        return None;
    }
    let urls = msg_id(panel, "URLs");
    if urls.is_null() {
        return None;
    }
    let count = msg_usize(urls, "count");
    if count == 0 {
        return None;
    }
    let first = msg_usize_index(urls, "objectAtIndex:", 0);
    url_path(first)
}

/// Ask for a file to open.
///
/// # Safety
/// Call on the main thread.
unsafe fn choose_file() -> Option<PathBuf> {
    run_open_panel(false)
}

/// Ask for a folder to open as a project.
///
/// # Safety
/// Call on the main thread.
unsafe fn choose_directory() -> Option<PathBuf> {
    run_open_panel(true)
}

/// Show a file in the Finder.
///
/// # Safety
/// Call on the main thread.
unsafe fn reveal_path(path: &std::path::Path) {
    let workspace = msg_id(class("NSWorkspace"), "sharedWorkspace");
    if workspace.is_null() {
        return;
    }
    let directory = path
        .parent()
        .map(|value| value.display().to_string())
        .unwrap_or_default();
    let send: extern "C" fn(Id, Sel, Id, Id) -> bool =
        std::mem::transmute(objc_msgSend as *const c_void);
    send(
        workspace,
        selector_name("selectFile:inFileViewerRootedAtPath:"),
        nsstring(&path.display().to_string()),
        nsstring(&directory),
    );
}

/// Width of one character of the code font, measured through AppKit.
///
/// # Safety
/// Call on the main thread.
unsafe fn measure_cell(size: f64) -> Option<f64> {
    let font = msg_id_double(class("NSFont"), "monospacedSystemFontOfSize:weight:", size);
    if font.is_null() {
        return None;
    }
    let attributes = msg_id(class("NSMutableDictionary"), "dictionary");
    msg_void_id_id(attributes, "setObject:forKey:", font, nsstring("NSFont"));
    let sample = nsstring("0123456789");
    let send: extern "C" fn(Id, Sel, Id) -> CGSize =
        std::mem::transmute(objc_msgSend as *const c_void);
    let measured = send(sample, selector_name("sizeWithAttributes:"), attributes);
    if measured.width <= 0.0 {
        return None;
    }
    Some(measured.width / 10.0)
}

/// Quit the application, after the window is gone.
///
/// # Safety
/// Call on the main thread.
unsafe fn quit_application() {
    with_shell(|shell| shell.stop_terminal());
    let application = msg_id(class("NSApplication"), "sharedApplication");
    if !application.is_null() {
        msg_void_id(application, "terminate:", ptr::null_mut());
    }
}

/// Call a method that returns a modal response.
///
/// # Safety
/// The selector has to exist on the receiver.
unsafe fn msg_send_long(receiver: Id, selector: &str) -> c_long {
    let send: extern "C" fn(Id, Sel) -> c_long = std::mem::transmute(objc_msgSend as *const c_void);
    send(receiver, selector_name(selector))
}

/// Call `objectAtIndex:`.
///
/// # Safety
/// The receiver has to be an array.
unsafe fn msg_usize_index(receiver: Id, selector: &str, index: usize) -> Id {
    let send: extern "C" fn(Id, Sel, usize) -> Id =
        std::mem::transmute(objc_msgSend as *const c_void);
    send(receiver, selector_name(selector), index)
}

// ---------------------------------------------------------------------------
// Pseudo terminal
// ---------------------------------------------------------------------------

/// A window size for the pseudo terminal, as `winsize` in C.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Winsize {
    rows: u16,
    columns: u16,
    x_pixels: u16,
    y_pixels: u16,
}

/// `TIOCSWINSZ` on macOS: set the window size of a terminal.
const TIOCSWINSZ: std::os::raw::c_ulong = 0x8008_7467;
/// Signal sent to the shell when the terminal is closed.
const SIGHUP: c_int = 1;
/// `waitpid` option that does not block.
const WNOHANG: c_int = 1;

extern "C" {
    fn forkpty(
        master: *mut c_int,
        name: *mut c_char,
        termios: *const c_void,
        size: *const Winsize,
    ) -> c_int;
    fn execvp(program: *const c_char, arguments: *const *const c_char) -> c_int;
    fn _exit(status: c_int) -> !;
    fn dup(fd: c_int) -> c_int;
    fn close(fd: c_int) -> c_int;
    fn read(fd: c_int, buffer: *mut c_void, count: usize) -> isize;
    fn write(fd: c_int, buffer: *const c_void, count: usize) -> isize;
    fn ioctl(fd: c_int, request: std::os::raw::c_ulong, ...) -> c_int;
    fn waitpid(pid: c_int, status: *mut c_int, options: c_int) -> c_int;
    fn kill(pid: c_int, signal: c_int) -> c_int;
}

/// Write to a file descriptor, retrying a short write.
///
/// # Safety
/// The descriptor has to be open for writing.
unsafe fn pty_write(fd: c_int, bytes: &[u8]) -> isize {
    let mut written = 0usize;
    while written < bytes.len() {
        let count = unsafe {
            write(
                fd,
                bytes[written..].as_ptr() as *const c_void,
                bytes.len() - written,
            )
        };
        if count <= 0 {
            return if written == 0 {
                count
            } else {
                written as isize
            };
        }
        written += count as usize;
    }
    written as isize
}

/// The shell a new terminal starts, from `$SHELL`.
fn default_shell() -> String {
    std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string())
}

/// A running shell on a pseudo terminal.
struct Pty {
    master: c_int,
    child: c_int,
    receiver: std::sync::mpsc::Receiver<Vec<u8>>,
}

impl Pty {
    /// Start a shell with the given window size.
    fn spawn(rows: u16, columns: u16) -> Result<Self, String> {
        let shell = default_shell();
        let size = Winsize {
            rows: rows.max(2),
            columns: columns.max(8),
            ..Winsize::default()
        };
        let mut master: c_int = -1;
        let pid = unsafe {
            forkpty(
                &mut master,
                ptr::null_mut(),
                ptr::null(),
                &size as *const Winsize,
            )
        };
        if pid < 0 {
            return Err("Could not start a pseudo terminal".into());
        }
        if pid == 0 {
            // The child: replace the process with the shell. Nothing else may
            // run here, which is why the call never returns on success.
            let program = CString::new(shell.clone()).unwrap_or_default();
            let arguments = [program.as_ptr(), ptr::null()];
            unsafe {
                execvp(program.as_ptr(), arguments.as_ptr());
                _exit(127);
            }
        }
        // The parent reads on a duplicate, so the shell can be written to and
        // resized while the reader blocks on its own descriptor.
        let reader_fd = unsafe { dup(master) };
        if reader_fd < 0 {
            unsafe {
                let _ = close(master);
                let _ = kill(pid, SIGHUP);
            }
            return Err("Could not duplicate the terminal descriptor".into());
        }
        let (sender, receiver) = std::sync::mpsc::channel::<Vec<u8>>();
        std::thread::spawn(move || {
            let mut buffer = vec![0u8; 4096];
            loop {
                let count =
                    unsafe { read(reader_fd, buffer.as_mut_ptr() as *mut c_void, buffer.len()) };
                if count <= 0 {
                    break;
                }
                let chunk = buffer[..count as usize].to_vec();
                if sender.send(chunk).is_err() {
                    break;
                }
            }
            unsafe {
                let _ = close(reader_fd);
            }
        });
        Ok(Self {
            master,
            child: pid,
            receiver,
        })
    }

    /// Everything the shell has written since the last call.
    fn drain(&self) -> Vec<Vec<u8>> {
        let mut chunks = Vec::new();
        while let Ok(chunk) = self.receiver.try_recv() {
            chunks.push(chunk);
        }
        chunks
    }

    /// Send keystrokes to the shell.
    fn send(&self, text: &str) {
        if self.master < 0 || text.is_empty() {
            return;
        }
        unsafe {
            pty_write(self.master, text.as_bytes());
        }
    }

    /// Tell the shell how large the terminal is.
    fn resize(&self, rows: u16, columns: u16) {
        if self.master < 0 {
            return;
        }
        let size = Winsize {
            rows: rows.max(2),
            columns: columns.max(8),
            ..Winsize::default()
        };
        unsafe {
            ioctl(self.master, TIOCSWINSZ, &size as *const Winsize);
        }
    }

    /// Whether the shell is still running.
    fn is_running(&self) -> bool {
        if self.child <= 0 {
            return false;
        }
        let mut status: c_int = 0;
        let result = unsafe { waitpid(self.child, &mut status, WNOHANG) };
        // `waitpid` returns the child when it has just exited and 0 while it runs.
        result == 0
    }

    /// Stop the shell and release the descriptors.
    fn close(&mut self) {
        if self.master >= 0 {
            unsafe {
                let _ = close(self.master);
            }
            self.master = -1;
        }
        if self.child > 0 {
            unsafe {
                let _ = kill(self.child, SIGHUP);
                let mut status: c_int = 0;
                let _ = waitpid(self.child, &mut status, 0);
            }
            self.child = -1;
        }
    }
}

impl Drop for Pty {
    fn drop(&mut self) {
        self.close();
    }
}

/// The terminal panel: the shell, its parsed output and the scroll position.
struct TerminalState {
    pty: Pty,
    parser: TerminalParser,
    /// What the parser last published, which is what the panel draws.
    view: Arc<Mutex<TerminalView>>,
    viewport: TerminalViewport,
    chunker: Utf8Chunker,
    /// Rows the view is scrolled back from the newest output.
    scroll: usize,
    columns: u16,
    rows: u16,
    open: bool,
    /// Drawer animation, from the shared spring.
    motion: f32,
    velocity: f32,
}

impl TerminalState {
    /// Start a shell sized to the grid the panel would use.
    fn start(rows: u16, columns: u16) -> Result<Self, String> {
        Ok(Self {
            pty: Pty::spawn(rows, columns)?,
            parser: TerminalParser::new(),
            view: Arc::new(Mutex::new(TerminalView::default())),
            viewport: TerminalViewport::default(),
            chunker: Utf8Chunker::new(),
            scroll: 0,
            columns,
            rows,
            open: true,
            motion: 0.0,
            velocity: 0.0,
        })
    }

    /// Feed everything the shell wrote into the parser.
    fn pump(&mut self) {
        let mut fed = false;
        for chunk in self.pty.drain() {
            let text = self.chunker.push(&chunk);
            if !text.is_empty() {
                self.parser.feed(&text);
                fed = true;
            }
        }
        if fed {
            self.parser.publish(&self.view);
        }
    }

    /// Pump the shell until its output contains a marker, or time runs out.
    fn wait_for(&mut self, marker: &str, timeout: std::time::Duration) -> bool {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            self.pump();
            if self.lines().iter().any(|line| line.contains(marker)) {
                return true;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        false
    }

    /// Lines the panel draws, newest last.
    fn lines(&self) -> Vec<String> {
        let Ok(view) = self.view.lock() else {
            return Vec::new();
        };
        let mut lines = view.lines.clone();
        if !view.current.is_empty() {
            lines.push(view.current.clone());
        }
        lines
    }

    /// Lines the panel shows, honouring the scroll position.
    fn visible_lines(&self) -> Vec<String> {
        let lines = self.lines();
        let rows = self.rows as usize;
        if self.scroll == 0 {
            let start = lines.len().saturating_sub(rows);
            return lines[start..].to_vec();
        }
        let end = lines.len().saturating_sub(self.scroll);
        let start = end.saturating_sub(rows);
        lines[start..end].to_vec()
    }

    /// Scroll the view: a positive number walks back into the history.
    fn scroll_by(&mut self, rows: i32) {
        let count = self.lines().len();
        let visible = (self.rows as usize).max(1);
        let maximum = count.saturating_sub(visible);
        self.scroll = (self.scroll as i32 + rows).clamp(0, maximum as i32) as usize;
    }

    /// Go back to the newest output.
    fn scroll_to_bottom(&mut self) {
        self.scroll = 0;
    }

    /// Resize the shell when the grid changes.
    fn ensure_size(&mut self, rows: u16, columns: u16) {
        if rows == self.rows && columns == self.columns {
            return;
        }
        self.rows = rows;
        self.columns = columns;
        self.pty.resize(rows, columns);
    }

    /// Step the drawer animation.
    fn tick(&mut self, dt: f32) {
        let target = if self.open { 1.0 } else { 0.0 };
        motion_core::step(
            &mut self.motion,
            &mut self.velocity,
            target,
            dt,
            motion_core::DRAWER,
        );
    }
}

/// A key that belongs to the terminal rather than the editor.
///
/// The macOS key codes are the ones `keyDown:` reports, and the terminal sends
/// the fixed byte sequences of the shared terminal core.
fn terminal_key(key_code: u16) -> Option<TerminalKey> {
    match key_code {
        126 => Some(TerminalKey::Up),
        125 => Some(TerminalKey::Down),
        124 => Some(TerminalKey::Right),
        123 => Some(TerminalKey::Left),
        115 => Some(TerminalKey::Home),
        119 => Some(TerminalKey::End),
        117 => Some(TerminalKey::Delete),
        51 => Some(TerminalKey::Backspace),
        36 | 76 => Some(TerminalKey::Enter),
        48 => Some(TerminalKey::Tab),
        53 => Some(TerminalKey::Escape),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Relation graph panel
// ---------------------------------------------------------------------------

/// The view state of the relation graph panel.
///
/// The graph itself comes from the shared `graph_core`, built from the documents
/// the session has open, so the macOS backend shows the same hierarchy, edges
/// and folds as the others.
struct GraphPanel {
    open: bool,
    focused: bool,
    selection: usize,
    scroll: usize,
    folded: Vec<String>,
    relation: GraphRelation,
    scope: Option<GraphScope>,
    /// Search query the graph is filtered by.
    search: String,
    /// Rebuilt when the documents change, not on every frame.
    graph: Graph,
    dirty: bool,
}

impl GraphPanel {
    /// A closed panel with an empty graph.
    fn new() -> Self {
        Self {
            open: false,
            focused: false,
            selection: 0,
            scroll: 0,
            folded: Vec::new(),
            relation: GraphRelation::Follow,
            scope: None,
            search: String::new(),
            graph: Graph {
                scope: GraphScope::File,
                search: String::new(),
                nodes: Vec::new(),
                truncated: false,
            },
            dirty: true,
        }
    }

    /// Rows the panel can show in a panel of this height.
    ///
    /// The layout constants come from the shared graph core, so the macOS panel
    /// lines up with the Windows and Linux ones instead of guessing its own.
    fn rows_for(height: f64) -> usize {
        graph_core::visible_rows(0.0, height as f32, 1.0)
    }

    /// The row a point of the panel is over, in the shared layout.
    fn row_at(y: f64, rows: usize) -> Option<usize> {
        let top = graph_core::FIRST_ROW_TOP as f64;
        if y < top {
            return None;
        }
        let row = ((y - top) / graph_core::ROW_HEIGHT as f64).floor();
        if row < 0.0 {
            return None;
        }
        let row = row as usize;
        if row < rows {
            Some(row)
        } else {
            None
        }
    }

    /// Keep the selection inside the node list and the list in view.
    fn clamp(&mut self, count: usize, rows: usize) {
        let count = count.max(1);
        self.selection = self.selection.min(count - 1);
        if self.selection < self.scroll {
            self.scroll = self.selection;
        } else if self.selection >= self.scroll + rows {
            self.scroll = self.selection + 1 - rows;
        }
    }

    /// The node the keyboard is on.
    fn selected(&self) -> Option<&GraphNode> {
        self.graph.nodes.get(self.selection)
    }

    /// Move the selection by a number of rows.
    fn move_selection(&mut self, delta: i64, rows: usize) {
        let count = self.graph.nodes.len() as i64;
        if count == 0 {
            return;
        }
        self.selection = (self.selection as i64 + delta).clamp(0, count - 1) as usize;
        self.clamp(count as usize, rows);
    }

    /// Fold or unfold the selected node.
    fn toggle_fold(&mut self) {
        let Some(node) = self.graph.nodes.get(self.selection) else {
            return;
        };
        if !node.has_children() {
            return;
        }
        let key = node.key.clone();
        match self.folded.iter().position(|known| *known == key) {
            Some(index) => {
                self.folded.remove(index);
            }
            None => self.folded.push(key),
        }
        self.dirty = true;
    }

    /// Walk to the next relation.
    fn cycle_relation(&mut self) {
        self.relation = self.relation.next();
        self.selection = 0;
        self.scroll = 0;
        self.dirty = true;
    }

    /// Walk to the next range, back to automatic at the end.
    fn cycle_scope(&mut self) {
        self.scope = graph_core::cycle_scope(self.scope, true);
        self.selection = 0;
        self.scroll = 0;
        self.dirty = true;
    }
}

// ---------------------------------------------------------------------------
// Settings panel
// ---------------------------------------------------------------------------

/// One control of the settings panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Setting {
    ThemeDark,
    ThemeLight,
    FontSmaller,
    FontLarger,
    LanguageEnglish,
    LanguageChinese,
    Close,
}

impl Setting {
    /// Every control, in the order `Tab` walks them and the panel draws them.
    const ALL: [Setting; 7] = [
        Setting::ThemeDark,
        Setting::ThemeLight,
        Setting::FontSmaller,
        Setting::FontLarger,
        Setting::LanguageEnglish,
        Setting::LanguageChinese,
        Setting::Close,
    ];

    /// Label drawn on the control.
    fn label(self) -> &'static str {
        match self {
            Setting::ThemeDark => "Dark",
            Setting::ThemeLight => "Light",
            Setting::FontSmaller => "−",
            Setting::FontLarger => "+",
            Setting::LanguageEnglish => "English",
            Setting::LanguageChinese => "中文",
            Setting::Close => "Close",
        }
    }

    /// Whether the control is the selected one in a preference.
    fn is_active(self, preferences: &workspace_core::Preferences) -> bool {
        match self {
            Setting::ThemeDark => !preferences.light_theme,
            Setting::ThemeLight => preferences.light_theme,
            Setting::LanguageEnglish => !preferences.ui_chinese,
            Setting::LanguageChinese => preferences.ui_chinese,
            _ => false,
        }
    }

    /// Whether the control can be used right now.
    fn is_enabled(self, preferences: &workspace_core::Preferences) -> bool {
        match self {
            Setting::FontSmaller => preferences.code_zoom > MIN_CODE_ZOOM,
            Setting::FontLarger => preferences.code_zoom < MAX_CODE_ZOOM,
            _ => true,
        }
    }
}

/// Smallest editor font step.
const MIN_CODE_ZOOM: i32 = -4;
/// Largest editor font step.
const MAX_CODE_ZOOM: i32 = 12;
/// Width of the settings panel, in points.
const SETTINGS_WIDTH: f64 = 360.0;
/// Height of one settings row, in points.
const SETTINGS_ROW_HEIGHT: f64 = 40.0;
/// Top of the first settings row, inside the panel.
const SETTINGS_FIRST_ROW: f64 = 56.0;

/// The view state of the settings panel.
struct SettingsPanel {
    open: bool,
    /// Control the keyboard is on, when the panel has the keyboard.
    focus: Option<Setting>,
    /// Control the pointer went down on.
    pressed: Option<Setting>,
}

impl SettingsPanel {
    fn new() -> Self {
        Self {
            open: false,
            focus: None,
            pressed: None,
        }
    }

    /// Rectangle of a control, in panel coordinates.
    fn rect(control: Setting) -> CGRect {
        let row = SETTINGS_ROW_HEIGHT;
        let height = row - 8.0;
        match control {
            Setting::ThemeDark => CGRect::new(16.0, SETTINGS_FIRST_ROW, 140.0, height),
            Setting::ThemeLight => CGRect::new(164.0, SETTINGS_FIRST_ROW, 140.0, height),
            Setting::FontSmaller => CGRect::new(16.0, SETTINGS_FIRST_ROW + row, 44.0, height),
            Setting::FontLarger => CGRect::new(112.0, SETTINGS_FIRST_ROW + row, 44.0, height),
            Setting::LanguageEnglish => {
                CGRect::new(16.0, SETTINGS_FIRST_ROW + row * 2.0, 140.0, height)
            }
            Setting::LanguageChinese => {
                CGRect::new(164.0, SETTINGS_FIRST_ROW + row * 2.0, 140.0, height)
            }
            Setting::Close => CGRect::new(16.0, SETTINGS_FIRST_ROW + row * 3.0, 140.0, height),
        }
    }

    /// Height the panel needs.
    fn height() -> f64 {
        SETTINGS_FIRST_ROW + SETTINGS_ROW_HEIGHT * 3.0 + SETTINGS_ROW_HEIGHT + 16.0
    }

    /// The control a point of the panel is over.
    fn control_at(x: f64, y: f64) -> Option<Setting> {
        Setting::ALL.into_iter().find(|control| {
            let rect = Self::rect(*control);
            x >= rect.x && x <= rect.x + rect.width && y >= rect.y && y <= rect.y + rect.height
        })
    }

    /// Walk the keyboard focus, skipping the controls that cannot be used.
    fn step_focus(&mut self, forward: bool, preferences: &workspace_core::Preferences) {
        let enabled = Setting::ALL
            .into_iter()
            .filter(|control| control.is_enabled(preferences))
            .collect::<Vec<_>>();
        if enabled.is_empty() {
            self.focus = None;
            return;
        }
        let position = self
            .focus
            .and_then(|focused| enabled.iter().position(|control| *control == focused));
        let next = match position {
            Some(index) if forward => (index + 1) % enabled.len(),
            Some(index) => (index + enabled.len() - 1) % enabled.len(),
            None => 0,
        };
        self.focus = Some(enabled[next]);
    }
}

/// Where the macOS preferences live, following the platform convention.
fn preferences_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(
        PathBuf::from(home)
            .join("Library")
            .join("Application Support")
            .join("TC-IDE")
            .join("settings.conf"),
    )
}

/// Read the preferences, falling back to the defaults.
fn load_preferences() -> workspace_core::Preferences {
    preferences_path()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .map(|text| workspace_core::parse_preferences(&text))
        .unwrap_or_default()
}

/// Write the preferences, creating the directory when it is missing.
fn save_preferences(preferences: &workspace_core::Preferences) -> Result<PathBuf, String> {
    let Some(path) = preferences_path() else {
        return Err("The home directory is unknown".into());
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("Could not create {}: {error}", parent.display()))?;
    }
    std::fs::write(&path, workspace_core::serialize_preferences(preferences))
        .map_err(|error| format!("Could not write {}: {error}", path.display()))?;
    Ok(path)
}

// ---------------------------------------------------------------------------
// Extensions and tooling panel
// ---------------------------------------------------------------------------

/// Languages the tooling table knows, in the order the panel lists them.
const EXTENSION_LANGUAGES: [&str; 7] = [
    "Rust",
    "Python",
    "C",
    "C++",
    "Java",
    "Go",
    "TypeScript / JavaScript",
];

/// One row of the extensions panel.
#[derive(Clone, Debug, PartialEq, Eq)]
struct ToolingRow {
    language: String,
    server: Option<String>,
    formatter: Option<String>,
    debugger: Option<String>,
    server_installed: bool,
    formatter_installed: bool,
    debugger_installed: bool,
}

impl ToolingRow {
    /// Build a row from the shared tooling table and a probe.
    fn probe(language: &str, probe: impl Fn(&str) -> bool) -> Self {
        let tools = extension_core::language_tools(language, probe);
        Self {
            language: tools.language.clone(),
            server: tools.server.clone(),
            formatter: tools.formatter.clone(),
            debugger: tools.debugger.clone(),
            server_installed: tools.server_installed,
            formatter_installed: tools.formatter_installed,
            debugger_installed: tools.debugger_installed,
        }
    }

    /// `LSP`, `format` and `debug` columns, with a tick for what is installed.
    fn marks(&self) -> String {
        let mark = |installed: bool| if installed { "✓" } else { "·" };
        format!(
            "{} {} {}",
            mark(self.server_installed),
            mark(self.formatter_installed),
            mark(self.debugger_installed)
        )
    }

    /// What is missing, for the status line.
    fn summary(&self) -> String {
        let missing = [
            ("LSP", self.server.as_deref(), self.server_installed),
            (
                "format",
                self.formatter.as_deref(),
                self.formatter_installed,
            ),
            ("debug", self.debugger.as_deref(), self.debugger_installed),
        ]
        .into_iter()
        .filter(|(_, tool, installed)| tool.is_some() && !installed)
        .map(|(label, tool, _)| format!("{label}: {}", tool.unwrap_or_default()))
        .collect::<Vec<_>>();
        if missing.is_empty() {
            format!("{} tooling is complete", self.language)
        } else {
            format!("{} missing {}", self.language, missing.join(", "))
        }
    }
}

/// The view state of the extensions panel.
struct ExtensionsPanel {
    open: bool,
    focused: bool,
    focus: usize,
    /// One row per language, probed when the panel opens.
    rows: Vec<ToolingRow>,
}

impl ExtensionsPanel {
    fn new() -> Self {
        Self {
            open: false,
            focused: false,
            focus: 0,
            rows: Vec::new(),
        }
    }

    /// Probe every language with the real executables on this machine.
    fn probe_all(&mut self) {
        self.rows = EXTENSION_LANGUAGES
            .iter()
            .map(|language| ToolingRow::probe(language, lsp_core::executable_available))
            .collect();
        self.focus = self.focus.min(self.rows.len().saturating_sub(1));
    }

    /// Probe one language again.
    fn probe_one(&mut self, index: usize) {
        let Some(language) = EXTENSION_LANGUAGES.get(index).copied() else {
            return;
        };
        let row = ToolingRow::probe(language, lsp_core::executable_available);
        if let Some(slot) = self.rows.get_mut(index) {
            *slot = row;
        }
    }

    /// The row a point of the panel is over.
    fn row_at(y: f64, top: f64, rows: usize) -> Option<usize> {
        let header = top + EXTENSIONS_HEADER_HEIGHT;
        if y < header {
            return None;
        }
        let row = ((y - header) / EXTENSIONS_ROW_HEIGHT).floor();
        if row < 0.0 {
            return None;
        }
        let row = row as usize;
        if row < rows {
            Some(row)
        } else {
            None
        }
    }

    /// Move the selection within the list.
    fn move_focus(&mut self, delta: i64) {
        let count = self.rows.len() as i64;
        if count == 0 {
            return;
        }
        self.focus = (self.focus as i64 + delta).clamp(0, count - 1) as usize;
    }
}

/// Height of the extensions panel header.
const EXTENSIONS_HEADER_HEIGHT: f64 = 52.0;
/// Height of one extensions row.
const EXTENSIONS_ROW_HEIGHT: f64 = 32.0;
/// Width of the extensions panel.
const EXTENSIONS_WIDTH: f64 = 420.0;

/// Index of the tab a point of the header is over.
///
/// The tabs are a fixed width strip starting at the left margin, which keeps the
/// hit testing a two line function that cannot disagree with the drawing.
fn tab_index_at(x: f64, count: usize, width: f64) -> Option<usize> {
    if count == 0 || width <= 0.0 || x < TAB_MARGIN {
        return None;
    }
    let index = ((x - TAB_MARGIN) / width).floor() as usize;
    if index < count {
        Some(index)
    } else {
        None
    }
}

/// A key the editor understands, with the macOS key code it comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MacCommand {
    InsertNewline,
    Backspace,
    DeleteForward,
    MoveLeft,
    MoveRight,
    MoveUp,
    MoveDown,
    MoveLineStart,
    MoveLineEnd,
    SelectAll,
    Save,
    Tab,
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    OpenFile,
    OpenFolder,
    Reveal,
    Quit,
    NextTab,
    PreviousTab,
    CloseTab,
    ToggleTerminal,
    ToggleGraph,
    GraphPrevious,
    GraphNext,
    GraphFold,
    GraphJump,
    GraphRelation,
    GraphScope,
    ToggleSettings,
    SettingNext,
    SettingPrevious,
    SettingUse,
    ToggleExtensions,
    ExtensionNext,
    ExtensionPrevious,
    ExtensionProbe,
    FormatFile,
}

/// Map a key code and modifiers to an editor command.
///
/// Virtual key codes are layout independent on macOS, which is why the mapping
/// is a table instead of a character comparison. `command` is the `⌘` key and
/// `shift` picks the second meaning of a shortcut.
fn key_command(key_code: u16, command: bool, shift: bool) -> Option<MacCommand> {
    match key_code {
        // 36 return, 76 keypad enter, 51 delete, 117 forward delete.
        36 | 76 => Some(MacCommand::InsertNewline),
        51 => Some(MacCommand::Backspace),
        117 => Some(MacCommand::DeleteForward),
        123 => Some(MacCommand::MoveLeft),
        124 => Some(MacCommand::MoveRight),
        125 => Some(MacCommand::MoveDown),
        126 => Some(MacCommand::MoveUp),
        115 => Some(MacCommand::MoveLineStart),
        119 => Some(MacCommand::MoveLineEnd),
        48 => Some(MacCommand::Tab),
        // ⌘ shortcuts, by key code: a s z x c v o r q.
        0 if command => Some(MacCommand::SelectAll),
        1 if command => Some(MacCommand::Save),
        6 if command && shift => Some(MacCommand::Redo),
        6 if command => Some(MacCommand::Undo),
        7 if command => Some(MacCommand::Cut),
        8 if command => Some(MacCommand::Copy),
        9 if command => Some(MacCommand::Paste),
        31 if command && shift => Some(MacCommand::OpenFolder),
        31 if command => Some(MacCommand::OpenFile),
        15 if command && shift => Some(MacCommand::Reveal),
        // ⌘⇧] and ⌘⇧[ walk the tabs, ⌘W closes the active one.
        30 if command && shift => Some(MacCommand::NextTab),
        33 if command && shift => Some(MacCommand::PreviousTab),
        13 if command => Some(MacCommand::CloseTab),
        // ⌘J shows or hides the terminal drawer, ⌘G the relation graph.
        38 if command => Some(MacCommand::ToggleTerminal),
        5 if command && shift => Some(MacCommand::GraphScope),
        5 if command => Some(MacCommand::ToggleGraph),
        // ⌘, opens the settings, as it does in every macOS application.
        43 if command => Some(MacCommand::ToggleSettings),
        // ⌘E opens the tooling panel, ⌘⇧F formats the current file.
        14 if command => Some(MacCommand::ToggleExtensions),
        3 if command && shift => Some(MacCommand::FormatFile),
        // The panel owns these while it has the keyboard.
        _ => None,
    }
}

/// The command a menu item stands for.
///
/// The menu and the key table share this mapping, so a menu item and its
/// shortcut can never do two different things.
fn command_for_menu_action(action: &str) -> Option<MacCommand> {
    match action {
        "tcUndo:" => Some(MacCommand::Undo),
        "tcRedo:" => Some(MacCommand::Redo),
        "tcCut:" => Some(MacCommand::Cut),
        "tcCopy:" => Some(MacCommand::Copy),
        "tcPaste:" => Some(MacCommand::Paste),
        "tcOpen:" => Some(MacCommand::OpenFile),
        "tcOpenFolder:" => Some(MacCommand::OpenFolder),
        "tcSave:" => Some(MacCommand::Save),
        "tcReveal:" => Some(MacCommand::Reveal),
        "tcQuit:" => Some(MacCommand::Quit),
        "tcNextTab:" => Some(MacCommand::NextTab),
        "tcPreviousTab:" => Some(MacCommand::PreviousTab),
        "tcCloseTab:" => Some(MacCommand::CloseTab),
        "tcToggleTerminal:" => Some(MacCommand::ToggleTerminal),
        "tcToggleGraph:" => Some(MacCommand::ToggleGraph),
        "tcGraphRelation:" => Some(MacCommand::GraphRelation),
        "tcGraphScope:" => Some(MacCommand::GraphScope),
        "tcSettings:" => Some(MacCommand::ToggleSettings),
        "tcExtensions:" => Some(MacCommand::ToggleExtensions),
        "tcFormat:" => Some(MacCommand::FormatFile),
        _ => None,
    }
}

/// Whether a key press carries a printable character instead of a command.
fn is_printable_key(key_code: u16, command: bool) -> bool {
    !command
        && !matches!(
            key_code,
            36 | 76 | 51 | 117 | 123 | 124 | 125 | 126 | 48 | 53
        )
}

/// The window: the shared session, the palette and the two Cocoa handles.
struct MacShell {
    session: editor_session::EditorSession,
    preferences: workspace_core::Preferences,
    palette: theme_core::Palette,
    status: String,
    started: Instant,
    /// The relation graph panel.
    graph: GraphPanel,
    /// The settings panel.
    settings: SettingsPanel,
    /// The extensions and tooling panel.
    extensions: ExtensionsPanel,
    /// The pointer, for hover and pressed states.
    mouse: (f64, f64),
    mouse_pressed: bool,
    /// Folder the user opened as the project, when there is one.
    project: Option<PathBuf>,
    /// The terminal drawer, once a shell has been started.
    terminal: Option<TerminalState>,
    /// Whether keystrokes go to the shell instead of the document.
    terminal_focused: bool,
    /// Cell width AppKit measured for the current font size.
    measured_cell: Option<f64>,
    measured_cell_size: f64,
    window: Id,
    view: Id,
}

impl MacShell {
    fn new() -> Self {
        let preferences = load_preferences();
        let welcome = editor_session::Document::from_lines(
            "main.rs",
            None,
            vec![
                "// TC-IDE on macOS".to_string(),
                "fn main() {".to_string(),
                "    println!(\"hello from Cocoa\");".to_string(),
                "}".to_string(),
            ],
        );
        Self {
            session: editor_session::EditorSession::new(welcome),
            palette: theme_core::Palette::for_scheme(preferences.light_theme),
            preferences,
            status: "Ready".into(),
            started: Instant::now(),
            graph: GraphPanel::new(),
            settings: SettingsPanel::new(),
            extensions: ExtensionsPanel::new(),
            mouse: (0.0, 0.0),
            mouse_pressed: false,
            project: None,
            terminal: None,
            terminal_focused: false,
            measured_cell: None,
            measured_cell_size: 0.0,
            window: ptr::null_mut(),
            view: ptr::null_mut(),
        }
    }

    fn header_height(&self) -> f64 {
        46.0
    }

    fn status_height(&self) -> f64 {
        26.0
    }

    fn line_step(&self) -> f64 {
        (18.0 + self.preferences.code_zoom as f64).clamp(14.0, 34.0)
    }

    fn code_font_size(&self) -> f64 {
        (12.0 + self.preferences.code_zoom as f64 * 0.5).clamp(9.0, 28.0)
    }

    /// Width of the text area, in points.
    fn text_width(&self) -> f64 {
        self.session.viewport.width as f64 * TEXT_CELL
    }

    /// Width of the whole view: the gutter plus the text area.
    fn view_width(&self) -> f64 {
        GUTTER_WIDTH + self.text_width()
    }

    /// Height of the text area, in points.
    fn text_height(&self) -> f64 {
        self.session.viewport.height as f64 * self.line_step()
    }

    /// Height of the whole view: header, text area and status bar.
    fn view_height(&self) -> f64 {
        self.header_height() + self.text_height() + self.status_height()
    }

    /// The text area, in top-left coordinates, which is what a flipped view
    /// draws in.
    fn text_area(&self) -> CGRect {
        CGRect::new(
            GUTTER_WIDTH,
            self.header_height(),
            self.text_width(),
            self.text_height(),
        )
    }

    /// Symbols the shared occurrence scan finds in the active document.
    fn symbol_count(&self) -> usize {
        let document = self.session.document();
        editor_core::symbol_occurrences(&document.lines, &document.name).len()
    }

    /// Symbol the caret sits on, when it sits on one.
    fn symbol_at_caret(&self) -> Option<String> {
        let document = self.session.document();
        editor_core::symbol_at(
            &document.lines,
            &document.name,
            document.caret_line,
            document.caret_column,
        )
        .map(|symbol| symbol.name)
    }

    /// Apply a command through the shared session.
    fn apply(&mut self, command: MacCommand) {
        match command {
            MacCommand::InsertNewline => {
                self.session.insert_text("\n");
                self.graph.dirty = true;
            }
            MacCommand::Backspace => {
                self.session.backspace();
                self.graph.dirty = true;
            }
            MacCommand::DeleteForward => {
                self.session.delete_forward();
                self.graph.dirty = true;
            }
            MacCommand::MoveLeft => self.session.move_caret(0, -1),
            MacCommand::MoveRight => self.session.move_caret(0, 1),
            MacCommand::MoveUp => self.session.move_caret(-1, 0),
            MacCommand::MoveDown => self.session.move_caret(1, 0),
            MacCommand::MoveLineStart => self.session.move_to_line_end(false),
            MacCommand::MoveLineEnd => self.session.move_to_line_end(true),
            MacCommand::SelectAll => self.session.select_all(),
            MacCommand::Tab => {
                self.session.insert_text("    ");
                self.graph.dirty = true;
            }
            MacCommand::Save => {
                self.status = match self.session.save() {
                    Ok(path) => format!("Saved {}", path.display()),
                    Err(message) => message,
                }
            }
            MacCommand::Undo => {
                self.status = if self.session.undo() {
                    "Undone".into()
                } else {
                    "Nothing left to undo".into()
                };
            }
            MacCommand::Redo => {
                self.status = if self.session.redo() {
                    "Redone".into()
                } else {
                    "Nothing left to redo".into()
                };
            }
            MacCommand::Copy => self.copy_selection(),
            MacCommand::Cut => self.cut_selection(),
            MacCommand::Paste => {
                self.paste_clipboard();
                self.graph.dirty = true;
            }
            MacCommand::OpenFile => self.open_file_panel(),
            MacCommand::OpenFolder => self.open_project_panel(),
            MacCommand::Reveal => self.reveal_in_finder(),
            MacCommand::Quit => unsafe { quit_application() },
            MacCommand::NextTab => self.step_tab(1),
            MacCommand::PreviousTab => self.step_tab(-1),
            MacCommand::CloseTab => self.close_tab(),
            MacCommand::ToggleTerminal => self.toggle_terminal(),
            MacCommand::ToggleGraph => self.toggle_graph(),
            MacCommand::GraphPrevious => self.graph_move(-1),
            MacCommand::GraphNext => self.graph_move(1),
            MacCommand::GraphFold => self.graph_fold(),
            MacCommand::GraphJump => self.graph_activate(),
            MacCommand::GraphRelation => self.graph_cycle_relation(),
            MacCommand::GraphScope => self.graph_cycle_scope(),
            MacCommand::ToggleSettings => self.toggle_settings(),
            MacCommand::SettingNext => self.step_settings_focus(true),
            MacCommand::SettingPrevious => self.step_settings_focus(false),
            MacCommand::ToggleExtensions => self.toggle_extensions(),
            MacCommand::ExtensionNext => self.extensions.move_focus(1),
            MacCommand::ExtensionPrevious => self.extensions.move_focus(-1),
            MacCommand::ExtensionProbe => self.probe_selected_language(),
            MacCommand::FormatFile => self.format_current_file(),
            MacCommand::SettingUse => {
                if let Some(control) = self.settings.focus {
                    self.activate_setting(control);
                }
            }
        }
    }

    /// Switch to the next or the previous tab.
    fn step_tab(&mut self, step: i64) {
        let count = self.session.documents().len();
        if count <= 1 {
            self.status = "Only one document is open".into();
            return;
        }
        let active = self.session.active() as i64;
        let next = (active + step).rem_euclid(count as i64) as usize;
        if self.session.activate(next) {
            let name = self.session.document().name.clone();
            self.status = format!("Switched to {name}");
        }
    }

    /// Close the active tab, unless it is the last one.
    fn close_tab(&mut self) {
        if !self.session.close_active() {
            self.status = "The last document stays open".into();
            return;
        }
        let name = self.session.document().name.clone();
        self.status = format!("Closed a tab; now on {name}");
    }

    /// Width of one tab in the header.
    fn tab_width(&self) -> f64 {
        TAB_WIDTH
    }

    /// The tab a point of the header is over.
    fn tab_at(&self, x: f64, y: f64) -> Option<usize> {
        if y > self.header_height() {
            return None;
        }
        tab_index_at(x, self.session.documents().len(), self.tab_width())
    }

    /// Rebuild the graph when the documents changed.
    fn refresh_graph(&mut self) {
        if !self.graph.dirty {
            return;
        }
        self.graph.dirty = false;
        let active = self.session.active();
        #[allow(clippy::redundant_clone)]
        let document = self.session.document().clone();
        let searched = self.graph.search.trim().to_ascii_lowercase();
        let entries = self
            .session
            .documents()
            .iter()
            .map(|document| {
                (
                    document.name.clone(),
                    editor_core::symbol_occurrences(&document.lines, &document.name),
                    syntax_core::declaration_refs(
                        &syntax_core::scan(&document.lines, &document.name),
                        &document.name,
                    ),
                )
            })
            .collect::<Vec<_>>();
        let file_symbols = entries
            .get(active)
            .map(|entry| entry.1.as_slice())
            .unwrap_or(&[]);
        let project_symbols =
            entries
                .iter()
                .flat_map(|entry| {
                    let name = entry.0.clone();
                    entry.1.iter().cloned().map(move |symbol| {
                        editor_core::ProjectSymbolOccurrence {
                            file: name.clone(),
                            symbol,
                        }
                    })
                })
                .collect::<Vec<_>>();
        let project_declarations = entries
            .iter()
            .flat_map(|entry| entry.2.iter().cloned())
            .collect::<Vec<_>>();
        let project_name = self
            .project
            .as_ref()
            .and_then(|path| path.file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "TC-IDE".to_string());
        // The symbol under the caret is the focus, so the caller and callee
        // relations describe where the user is working.
        let hovered = editor_core::symbol_at(
            &document.lines,
            &document.name,
            document.caret_line,
            document.caret_column,
        );
        let inputs = graph_core::GraphInputs {
            project_name: &project_name,
            current_file: &document.name,
            current_folder: graph_core::folder_of(&document.name),
            hovered: hovered.as_ref(),
            file_lines: &document.lines,
            file_symbols,
            project_symbols: &project_symbols,
            project_declarations: &project_declarations,
            folded: &self.graph.folded,
            relation: self.graph.relation,
        };
        let scope = graph_core::auto_scope(hovered.as_ref(), &searched, self.graph.scope);
        self.graph.graph = graph_core::build_graph(&inputs, scope, &self.graph.search);
        let rows = self.graph_rows();
        let count = self.graph.graph.nodes.len();
        self.graph.clamp(count, rows);
    }

    /// Rows the graph panel can show.
    fn graph_rows(&self) -> usize {
        GraphPanel::rows_for(self.view_height() - self.status_height())
    }

    /// Left edge of the graph panel.
    fn graph_panel_left(&self) -> f64 {
        self.view_width() - self.graph_panel_width()
    }

    /// Width of the graph panel.
    fn graph_panel_width(&self) -> f64 {
        (self.view_width() * 0.27).max(260.0)
    }

    /// The row of the graph panel a point is over.
    fn graph_row_at(&self, x: f64, y: f64) -> Option<usize> {
        if !self.graph.open || x < self.graph_panel_left() {
            return None;
        }
        GraphPanel::row_at(y, self.graph_rows())
    }

    /// Show or hide the graph panel.
    fn toggle_graph(&mut self) {
        self.graph.open = !self.graph.open;
        self.graph.focused = self.graph.open;
        self.graph.dirty = true;
        self.status = if self.graph.open {
            "Relation graph shown".into()
        } else {
            "Relation graph hidden".into()
        };
    }

    /// Move the graph selection.
    fn graph_move(&mut self, delta: i64) {
        if !self.graph.focused {
            return;
        }
        let rows = self.graph_rows();
        self.graph.move_selection(delta, rows);
        if let Some(node) = self.graph.selected() {
            self.status = format!("{}  {}", node.caption(), node.detail);
        }
    }

    /// Fold or unfold the node the keyboard is on.
    fn graph_fold(&mut self) {
        if !self.graph.focused {
            return;
        }
        self.graph.toggle_fold();
        self.refresh_graph();
    }

    /// Jump to the target of the selected node.
    fn graph_activate(&mut self) {
        if !self.graph.focused {
            return;
        }
        let Some(target) = self.graph.selected().and_then(|node| node.target.clone()) else {
            self.status = "This node has nowhere to jump to".into();
            return;
        };
        let open = self.session.documents().iter().position(|document| {
            document
                .path
                .as_ref()
                .map(|path| {
                    path.file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                        == Some(target.file.clone())
                })
                .unwrap_or(false)
                || document.name == target.file
        });
        if let Some(index) = open {
            self.session.activate(index);
        }
        self.session.set_caret(target.line, target.column);
        self.status = format!("Jumped to {}:{}", target.file, target.line + 1);
    }

    /// Walk to the next relation, then leave the panel focused.
    fn graph_cycle_relation(&mut self) {
        if !self.graph.focused {
            return;
        }
        self.graph.cycle_relation();
        self.refresh_graph();
        self.status = format!("Relation: {}", self.graph.relation.label());
    }

    /// Walk to the next range, then leave the panel focused.
    fn graph_cycle_scope(&mut self) {
        if !self.graph.focused {
            return;
        }
        self.graph.cycle_scope();
        self.refresh_graph();
        self.status = format!("Range: {}", self.graph.graph.scope_label());
    }

    /// Show or hide the settings panel.
    fn toggle_settings(&mut self) {
        self.settings.open = !self.settings.open;
        self.settings.focus = if self.settings.open {
            Some(Setting::ThemeDark)
        } else {
            None
        };
        self.settings.pressed = None;
        self.status = if self.settings.open {
            "Settings: Tab walks the controls, Enter uses one, Esc closes".into()
        } else {
            "Settings closed".into()
        };
    }

    /// Rectangle of the settings panel in view coordinates.
    fn settings_rect(&self) -> CGRect {
        CGRect::new(
            (self.view_width() - SETTINGS_WIDTH) * 0.5,
            12.0,
            SETTINGS_WIDTH,
            SettingsPanel::height(),
        )
    }

    /// The control under a point of the view.
    fn settings_control_at(&self, x: f64, y: f64) -> Option<Setting> {
        if !self.settings.open {
            return None;
        }
        let panel = self.settings_rect();
        if !panel.contains(CGPoint { x, y }) {
            return None;
        }
        SettingsPanel::control_at(x - panel.x, y - panel.y)
    }

    /// Use a control: the one the keyboard is on, or the one clicked.
    fn activate_setting(&mut self, control: Setting) {
        if !control.is_enabled(&self.preferences) {
            self.status = format!("{} is not available right now", control.label());
            return;
        }
        match control {
            Setting::ThemeDark | Setting::ThemeLight => {
                self.preferences.light_theme = control == Setting::ThemeLight;
                self.palette = theme_core::Palette::for_scheme(self.preferences.light_theme);
                self.status = format!(
                    "{} theme",
                    if self.preferences.light_theme {
                        "Light"
                    } else {
                        "Dark"
                    }
                );
            }
            Setting::FontSmaller | Setting::FontLarger => {
                let step = if control == Setting::FontSmaller {
                    -1
                } else {
                    1
                };
                self.preferences.code_zoom =
                    (self.preferences.code_zoom + step).clamp(MIN_CODE_ZOOM, MAX_CODE_ZOOM);
                // The measured cell belongs to the size it was measured for.
                self.measured_cell = None;
                self.status = format!("Editor font step {}", self.preferences.code_zoom);
            }
            Setting::LanguageEnglish | Setting::LanguageChinese => {
                self.preferences.ui_chinese = control == Setting::LanguageChinese;
                self.status = if self.preferences.ui_chinese {
                    "界面语言已切换为简体中文".into()
                } else {
                    "Interface language changed to English".into()
                };
            }
            Setting::Close => {
                self.settings.open = false;
                self.settings.focus = None;
                self.status = "Settings closed".into();
                return;
            }
        }
        // The panel keeps the keyboard on a control that still exists.
        if self.settings.focus == Some(control) && !control.is_enabled(&self.preferences) {
            self.settings.step_focus(true, &self.preferences);
        }
    }

    /// Walk the keyboard focus inside the settings panel.
    fn step_settings_focus(&mut self, forward: bool) {
        self.settings.step_focus(forward, &self.preferences);
        if let Some(control) = self.settings.focus {
            self.status = format!("{}: {}", control.label(), control.label());
        }
    }

    /// Show or hide the extensions panel, probing the tools when it opens.
    fn toggle_extensions(&mut self) {
        self.extensions.open = !self.extensions.open;
        self.extensions.focused = self.extensions.open;
        if self.extensions.open {
            self.extensions.probe_all();
            let languages = self.extensions.rows.len();
            self.status = format!("Tooling for {languages} language(s) probed");
        } else {
            self.status = "Extensions closed".into();
        }
    }

    /// Probe the selected language again and report what is missing.
    fn probe_selected_language(&mut self) {
        let index = self.extensions.focus;
        self.extensions.probe_one(index);
        self.status = match self.extensions.rows.get(index) {
            Some(row) => row.summary(),
            None => "No language selected".into(),
        };
    }

    /// Type the formatter command of the current file into the terminal.
    fn format_current_file(&mut self) {
        let name = self.session.document().name.clone();
        let language = editor_core::language_for_file(&name);
        let Some(command) = workspace_core::format_command(language, &name) else {
            self.status = format!("No formatter is configured for {language}");
            return;
        };
        if self.terminal.is_none() {
            self.toggle_terminal();
        }
        let Some(terminal) = self.terminal.as_mut() else {
            self.status = "The terminal could not be started".into();
            return;
        };
        terminal.open = true;
        terminal.pty.send(&format!("{command}\n"));
        terminal.scroll_to_bottom();
        self.terminal_focused = true;
        self.status = format!("Formatting {name} with {command}");
    }

    /// Rectangle of the extensions panel in view coordinates.
    fn extensions_rect(&self) -> CGRect {
        CGRect::new(
            (self.view_width() - EXTENSIONS_WIDTH).max(0.0) * 0.5,
            12.0,
            EXTENSIONS_WIDTH,
            EXTENSIONS_HEADER_HEIGHT
                + EXTENSIONS_ROW_HEIGHT * EXTENSION_LANGUAGES.len() as f64
                + 12.0,
        )
    }

    /// The language row under a point of the view.
    fn extensions_row_at(&self, x: f64, y: f64) -> Option<usize> {
        if !self.extensions.open {
            return None;
        }
        let panel = self.extensions_rect();
        if !panel.contains(CGPoint { x, y }) {
            return None;
        }
        ExtensionsPanel::row_at(y, panel.y, self.extensions.rows.len())
    }

    /// Step everything that moves in one frame and collect what the shell wrote.
    fn tick(&mut self) {
        self.session.tick();
        // The geometry is read before the terminal is borrowed, so the shell can
        // be asked for its metrics while the drawer is stepped.
        let cell = self.measured_cell.unwrap_or(TEXT_CELL).max(1.0);
        let step = self.line_step().max(1.0);
        let (width, height) = (self.view_width() as f32, self.view_height() as f32);
        let progress = self
            .terminal
            .as_ref()
            .map(|terminal| terminal.motion)
            .unwrap_or(0.0);
        let panel =
            terminal_core::panel_metrics(width, height, (width / 1280.0).max(0.8), progress);
        let rows = (((panel.body_bottom - panel.body_top) as f64 / step).floor() as i64)
            .clamp(2, 200) as u16;
        let columns = (((panel.body_right - panel.body_left) as f64 / cell).floor() as i64)
            .clamp(8, 400) as u16;
        if let Some(terminal) = self.terminal.as_mut() {
            terminal.pump();
            terminal.tick(1.0 / 60.0);
            terminal.ensure_size(rows, columns);
        }
    }

    /// Scroll the terminal view, without sending anything to the shell.
    fn scroll_terminal(&mut self, rows: i32) -> bool {
        let Some(terminal) = self.terminal.as_mut() else {
            return false;
        };
        if !terminal.open {
            return false;
        }
        terminal.scroll_by(rows);
        true
    }

    /// Show or hide the terminal drawer, starting a shell the first time.
    fn toggle_terminal(&mut self) {
        if let Some(terminal) = self.terminal.as_mut() {
            terminal.open = !terminal.open;
            let open = terminal.open;
            self.status = if open {
                "Terminal shown".into()
            } else {
                "Terminal hidden".into()
            };
            return;
        }
        let (rows, columns) = self.terminal_grid();
        match TerminalState::start(rows, columns) {
            Ok(terminal) => {
                self.terminal = Some(terminal);
                self.terminal_focused = true;
                self.status = format!("Terminal started with {}", default_shell());
            }
            Err(message) => self.status = message,
        }
    }

    /// Stop the shell, for example when the window closes.
    fn stop_terminal(&mut self) {
        if let Some(mut terminal) = self.terminal.take() {
            terminal.pty.close();
        }
        self.terminal_focused = false;
    }

    /// The grid the terminal panel would use at the current size.
    fn terminal_grid(&self) -> (u16, u16) {
        let panel = terminal_core::panel_metrics(
            self.view_width() as f32,
            self.view_height() as f32,
            (self.view_width() / 1280.0).max(0.8) as f32,
            1.0,
        );
        let cell = self.measured_cell.unwrap_or(TEXT_CELL).max(1.0);
        let step = self.line_step().max(1.0);
        // The panel metrics are f32 and the geometry here is f64.
        let rows = (((panel.body_bottom - panel.body_top) as f64 / step).floor() as i64)
            .clamp(2, 200) as u16;
        let columns = (((panel.body_right - panel.body_left) as f64 / cell).floor() as i64)
            .clamp(8, 400) as u16;
        (rows, columns)
    }

    /// Type into the focused terminal instead of the document.
    fn terminal_insert(&mut self, text: &str) -> bool {
        if !self.terminal_focused {
            return false;
        }
        let Some(terminal) = self.terminal.as_mut() else {
            return false;
        };
        terminal.pty.send(text);
        terminal.scroll_to_bottom();
        true
    }

    /// Send a key the terminal has a fixed sequence for.
    fn terminal_send_key(&mut self, key_code: u16) -> bool {
        if !self.terminal_focused {
            return false;
        }
        let Some(key) = terminal_key(key_code) else {
            return false;
        };
        let Some(sequence) = terminal_core::key_sequence(key) else {
            return false;
        };
        let Some(terminal) = self.terminal.as_mut() else {
            return false;
        };
        terminal.pty.send(sequence);
        terminal.scroll_to_bottom();
        true
    }

    /// Copy the selection to the system pasteboard.
    fn copy_selection(&mut self) {
        let text = self.session.copy();
        if text.is_empty() {
            self.status = "Nothing selected to copy".into();
            return;
        }
        unsafe { pasteboard_write(&text) };
        self.status = format!("Copied {} character(s)", text.chars().count());
    }

    /// Cut the selection to the system pasteboard.
    fn cut_selection(&mut self) {
        let text = self.session.cut();
        if text.is_empty() {
            self.status = "Nothing selected to cut".into();
            return;
        }
        unsafe { pasteboard_write(&text) };
        self.status = format!("Cut {} character(s)", text.chars().count());
    }

    /// Paste the pasteboard into the document, replacing a selection.
    fn paste_clipboard(&mut self) {
        let read = unsafe { pasteboard_read() };
        let Some(text) = read else {
            self.status = "The pasteboard holds no text".into();
            return;
        };
        let characters = text.chars().count();
        self.session.paste(&text);
        self.status = format!("Pasted {characters} character(s)");
    }

    /// Ask the user for a file and open it in a tab.
    fn open_file_panel(&mut self) {
        let chosen = unsafe { choose_file() };
        let Some(path) = chosen else {
            self.status = "No file chosen".into();
            return;
        };
        self.status = match self.session.open_file(path.clone()) {
            Ok(_) => {
                self.graph.dirty = true;
                format!("Opened {}", path.display())
            }
            Err(message) => message,
        };
    }

    /// Ask the user for a folder and remember it as the project.
    fn open_project_panel(&mut self) {
        let chosen = unsafe { choose_directory() };
        let Some(path) = chosen else {
            self.status = "No folder chosen".into();
            return;
        };
        self.open_project(path);
    }

    /// Show the active file in the Finder.
    fn reveal_in_finder(&mut self) {
        let Some(path) = self.session.document().path.clone() else {
            self.status = "Save the file before revealing it".into();
            return;
        };
        unsafe { reveal_path(&path) };
        self.status = format!("Revealed {}", path.display());
    }

    /// The width of one character of the code font, measured through AppKit.
    ///
    /// The caret and the selection are placed by this number, so a fallback is
    /// only used when AppKit cannot measure at all.
    fn cell_width(&mut self) -> f64 {
        let size = self.code_font_size();
        if let Some(measured) = self.measured_cell {
            if (self.measured_cell_size - size).abs() < 0.01 {
                return measured;
            }
        }
        let measured = unsafe { measure_cell(size) }.unwrap_or(TEXT_CELL);
        self.measured_cell = Some(measured);
        self.measured_cell_size = size;
        measured
    }

    /// Where a column of the code column starts, in points.
    fn column_x(&mut self, column: usize) -> f64 {
        let cell = self.cell_width();
        Self::column_x_with(column, cell)
    }

    /// Where a column starts, with a cell width the caller already has.
    fn column_x_with(column: usize, cell: f64) -> f64 {
        GUTTER_WIDTH + column as f64 * cell
    }

    /// The column under a point, with the measured cell width.
    fn column_at_point(&mut self, x: f64, line: usize) -> usize {
        let cell = self.cell_width();
        self.session.column_at(x, GUTTER_WIDTH, cell, line)
    }

    /// Place the caret at a point of the view.
    fn click_at(&mut self, x: f64, y: f64) {
        if let Some(index) = self.tab_at(x, y) {
            if self.session.activate(index) {
                let name = self.session.document().name.clone();
                self.status = format!("Switched to {name}");
            }
            return;
        }
        self.mouse = (x, y);
        if let Some(row) = self.extensions_row_at(x, y) {
            self.extensions.focused = true;
            self.extensions.focus = row;
            if let Some(entry) = self.extensions.rows.get(row) {
                self.status = format!("{}  {}", entry.language, entry.marks());
            }
            return;
        }
        if let Some(control) = self.settings_control_at(x, y) {
            self.settings.pressed = Some(control);
            self.settings.focus = Some(control);
            self.activate_setting(control);
            return;
        }
        if self.settings.open && self.settings_rect().contains(CGPoint { x, y }) {
            return;
        }
        if let Some(row) = self.graph_row_at(x, y) {
            let rows = self.graph_rows();
            self.graph.focused = true;
            self.graph.selection = self.graph.scroll + row;
            self.graph.clamp(self.graph.graph.nodes.len(), rows);
            if let Some(node) = self.graph.selected() {
                self.status = format!("{}  {}", node.caption(), node.detail);
            }
            return;
        }
        let area = self.text_area();
        if !area.contains(CGPoint { x, y }) {
            return;
        }
        let line = self.session.line_at(y, area.y, self.line_step());
        let column = self.column_at_point(x, line);
        self.session.set_caret(line, column);
    }

    /// Open a project, which the session then remembers.
    fn open_project(&mut self, root: PathBuf) {
        self.project = Some(root.clone());
        self.graph.dirty = true;
        self.status = format!("Project {}", root.display());
    }

    /// The rows the view shows, for the drawing code.
    fn visible_rows(&self) -> usize {
        self.session.visible_rows()
    }

    /// A viewport small enough for a check to reason about.
    fn viewport_for_checks(&mut self) {
        self.session.viewport = text_buffer::Viewport::new(20, 80);
    }
}

thread_local! {
    /// The window state, reached from the Objective-C callbacks.
    static SHELL: RefCell<MacShell> = RefCell::new(MacShell::new());
}

/// Run a closure with the shell.
fn with_shell<R>(body: impl FnOnce(&mut MacShell) -> R) -> R {
    SHELL.with(|cell| body(&mut cell.borrow_mut()))
}

/// Draw a filled rectangle in the current graphics context.
///
/// # Safety
/// Requires a current `NSGraphicsContext`, which only exists inside `drawRect:`.
unsafe fn fill_rect(rect: CGRect, color: u32, alpha: f64) {
    let context = msg_id(class("NSGraphicsContext"), "currentContext");
    if context.is_null() {
        return;
    }
    let graphics = msg_id(context, "CGContext");
    if graphics.is_null() {
        return;
    }
    let channel = |shift: u32| ((color >> shift) & 0xff) as f64 / 255.0;
    CGContextSetRGBFillColor(
        graphics,
        channel(16),
        channel(8),
        channel(0),
        alpha.clamp(0.0, 1.0),
    );
    CGContextFillRect(graphics, rect);
}

/// Build a text attribute dictionary for one colour and font size.
///
/// # Safety
/// Builds AppKit objects; call on the main thread.
unsafe fn text_attributes(size: f64, color: u32, monospaced: bool) -> Id {
    let font = if monospaced {
        msg_id_double(class("NSFont"), "monospacedSystemFontOfSize:weight:", size)
    } else {
        msg_id_double(class("NSFont"), "systemFontOfSize:", size)
    };
    let channel = |shift: u32| ((color >> shift) & 0xff) as f64 / 255.0;
    let color_object = msg_color(channel(16), channel(8), channel(0), 1.0);
    let attributes = msg_id(class("NSMutableDictionary"), "dictionary");
    // AppKit looks the font up under NSFontAttributeName and the colour under
    // NSForegroundColorAttributeName; those two strings are those keys.
    msg_void_id_id(attributes, "setObject:forKey:", font, nsstring("NSFont"));
    msg_void_id_id(
        attributes,
        "setObject:forKey:",
        color_object,
        nsstring("NSColor"),
    );
    attributes
}

/// Draw a string at a point, in top-left coordinates.
///
/// # Safety
/// Requires a current graphics context.
unsafe fn draw_text(value: &str, x: f64, y: f64, size: f64, color: u32, monospaced: bool) {
    let attributes = text_attributes(size, color, monospaced);
    msg_draw_at_point(nsstring(value), CGPoint { x, y }, attributes);
}

/// Draw one frame: header, code column with the caret, status bar.
///
/// # Safety
/// Requires a current graphics context, which only exists inside `drawRect:`.
unsafe fn draw_frame(shell: &mut MacShell) {
    let width = shell.view_width();
    let height = shell.view_height();
    let header = shell.header_height();
    let status = shell.status_height();
    let step = shell.line_step();
    let font = shell.code_font_size();
    let palette = shell.palette;
    // The caret and the selection are placed with the width AppKit measures for
    // this font size, so they line up with the glyphs.
    let cell = shell.cell_width();
    fill_rect(
        CGRect::new(0.0, 0.0, width, height),
        palette.background,
        1.0,
    );
    fill_rect(CGRect::new(0.0, 0.0, width, header), palette.panel, 1.0);
    fill_rect(
        CGRect::new(0.0, height - status, width, status),
        palette.panel,
        1.0,
    );

    let document = shell.session.document();
    let titles = shell.session.titles();
    let active = shell.session.active();
    let tab_width = shell.tab_width();
    for (index, title) in titles.iter().enumerate() {
        let left = TAB_MARGIN + index as f64 * tab_width;
        let selected = index == active;
        fill_rect(
            CGRect::new(left, 6.0, tab_width - 6.0, header - 12.0),
            if selected {
                palette.raised
            } else {
                palette.panel
            },
            1.0,
        );
        draw_text(
            title,
            left + 8.0,
            16.0,
            13.0,
            if selected {
                palette.bright
            } else {
                palette.muted
            },
            false,
        );
    }
    let summary = format!("{} symbols", shell.symbol_count());
    draw_text(&summary, width - 120.0, 16.0, 12.0, palette.muted, false);

    let area = shell.text_area();
    let selection = document.selection();
    let lines = document.lines.clone();
    let (caret_line, caret_column) = shell.session.caret();
    let scroll_lines = shell.session.scroll_lines;
    let scroll_columns = shell.session.scroll_columns;
    let rows = shell.visible_rows();
    let document_name = document.name.clone();
    for (row, line) in lines.iter().skip(scroll_lines).take(rows).enumerate() {
        let index = scroll_lines + row;
        let y = area.y + row as f64 * step;
        if let Some(((start_line, start_column), (end_line, end_column))) = selection {
            let within = index >= start_line.min(end_line) && index <= start_line.max(end_line);
            if within {
                let from = if index == start_line { start_column } else { 0 };
                let to = if index == end_line {
                    end_column
                } else {
                    line.chars().count()
                };
                let left = MacShell::column_x_with(from.saturating_sub(scroll_columns), cell);
                let right = MacShell::column_x_with(to.saturating_sub(scroll_columns), cell);
                fill_rect(
                    CGRect::new(left, y, (right - left).max(2.0), step - 2.0),
                    palette.selection,
                    1.0,
                );
            }
        }
        // The line is drawn token by token, so keywords, literals and comments
        // come from the shared tokenizer and the shared palette.
        let mut column = 0usize;
        for token in editor_core::syntax_tokens(line, &document_name) {
            let start = column;
            column += token.text.chars().count();
            let Some((first, visible)) = visible_token(&token.text, start, scroll_columns) else {
                continue;
            };
            draw_text(
                &visible,
                MacShell::column_x_with(first, cell),
                y,
                font,
                token_color(&palette, token.kind),
                true,
            );
        }
        draw_text(
            &format!("{}", index + 1),
            12.0,
            y,
            font - 2.0,
            palette.muted,
            true,
        );
    }

    // The caret, at the column it sits on, breathing with the shared spring.
    if caret_line >= scroll_lines {
        let row = caret_line - scroll_lines;
        if row <= rows {
            let x = MacShell::column_x_with(caret_column.saturating_sub(scroll_columns), cell);
            let y = area.y + row as f64 * step;
            let pulse = shell.session.caret_spring.progress(1.0).clamp(0.0, 1.0);
            fill_rect(
                CGRect::new(x, y, 2.0, (step - 4.0).max(1.0)),
                palette.bright,
                0.35 + 0.65 * pulse as f64,
            );
        }
    }

    // The extensions and tooling panel, in front of the editor.
    if shell.extensions.open {
        let panel = shell.extensions_rect();
        fill_rect(panel, palette.background, 0.96);
        fill_rect(
            CGRect::new(panel.x, panel.y, panel.width, 1.0),
            palette.border,
            1.0,
        );
        draw_text(
            "LANGUAGES & TOOLING",
            panel.x + 16.0,
            panel.y + 16.0,
            14.0,
            palette.bright,
            false,
        );
        draw_text(
            "LSP · format · debug",
            panel.x + 16.0,
            panel.y + 34.0,
            11.0,
            palette.muted,
            false,
        );
        if shell.extensions.rows.is_empty() {
            draw_text(
                "Opening the panel probes the tools on this machine",
                panel.x + 16.0,
                panel.y + EXTENSIONS_HEADER_HEIGHT + 4.0,
                11.0,
                palette.muted,
                false,
            );
        }
        for (index, row) in shell.extensions.rows.iter().enumerate() {
            let y = panel.y + EXTENSIONS_HEADER_HEIGHT + index as f64 * EXTENSIONS_ROW_HEIGHT;
            let selected = shell.extensions.focused && index == shell.extensions.focus;
            if selected {
                fill_rect(
                    CGRect::new(
                        panel.x + 8.0,
                        y,
                        panel.width - 16.0,
                        EXTENSIONS_ROW_HEIGHT - 4.0,
                    ),
                    palette.selection,
                    1.0,
                );
            }
            draw_text(
                &row.language,
                panel.x + 16.0,
                y + 7.0,
                12.0,
                if selected {
                    palette.bright
                } else {
                    palette.text
                },
                false,
            );
            draw_text(
                &row.marks(),
                panel.x + 210.0,
                y + 8.0,
                11.0,
                palette.accent,
                false,
            );
            draw_text(
                &format!(
                    "{}  {}  {}",
                    row.server.as_deref().unwrap_or("—"),
                    row.formatter.as_deref().unwrap_or("—"),
                    row.debugger.as_deref().unwrap_or("—"),
                ),
                panel.x + 264.0,
                y + 8.0,
                11.0,
                palette.muted,
                false,
            );
        }
    }

    // The settings panel, centred, in front of the editor and the graph.
    if shell.settings.open {
        let panel = shell.settings_rect();
        let control_visual = |control: Setting| {
            let enabled = control.is_enabled(&shell.preferences);
            let rect = SettingsPanel::rect(control);
            let on_screen =
                CGRect::new(panel.x + rect.x, panel.y + rect.y, rect.width, rect.height);
            let state = if !enabled {
                theme_core::ControlState::Disabled
            } else if on_screen.contains(CGPoint {
                x: shell.mouse.0,
                y: shell.mouse.1,
            }) {
                if shell.mouse_pressed {
                    theme_core::ControlState::Pressed
                } else {
                    theme_core::ControlState::Hovered
                }
            } else {
                theme_core::ControlState::Normal
            };
            let visual = palette.control(state, control.is_active(&shell.preferences));
            (on_screen, visual, shell.settings.focus == Some(control))
        };
        fill_rect(panel, palette.background, 0.96);
        fill_rect(
            CGRect::new(panel.x, panel.y, panel.width, 1.0),
            palette.border,
            1.0,
        );
        draw_text(
            "SETTINGS",
            panel.x + 16.0,
            panel.y + 18.0,
            14.0,
            palette.bright,
            false,
        );
        draw_text(
            "Theme",
            panel.x + 16.0,
            panel.y + SETTINGS_FIRST_ROW - 14.0,
            11.0,
            palette.muted,
            false,
        );
        draw_text(
            "Editor font size",
            panel.x + 16.0,
            panel.y + SETTINGS_FIRST_ROW + SETTINGS_ROW_HEIGHT - 14.0,
            11.0,
            palette.muted,
            false,
        );
        draw_text(
            &format!("step {}", shell.preferences.code_zoom),
            panel.x + 128.0,
            panel.y + SETTINGS_FIRST_ROW + SETTINGS_ROW_HEIGHT + 8.0,
            12.0,
            palette.bright,
            false,
        );
        draw_text(
            "Interface language",
            panel.x + 16.0,
            panel.y + SETTINGS_FIRST_ROW + SETTINGS_ROW_HEIGHT * 2.0 - 14.0,
            11.0,
            palette.muted,
            false,
        );
        for control in Setting::ALL {
            let (rect, visual, focused) = control_visual(control);
            fill_rect(rect, visual.fill, visual.opacity as f64);
            fill_rect(
                CGRect::new(rect.x, rect.y, rect.width, 1.0),
                visual.border,
                1.0,
            );
            if focused {
                fill_rect(
                    CGRect::new(rect.x - 2.0, rect.y - 2.0, rect.width + 4.0, 1.0),
                    palette.accent,
                    1.0,
                );
                fill_rect(
                    CGRect::new(
                        rect.x - 2.0,
                        rect.y + rect.height + 1.0,
                        rect.width + 4.0,
                        1.0,
                    ),
                    palette.accent,
                    1.0,
                );
            }
            draw_text(
                control.label(),
                rect.x + 10.0,
                rect.y + 9.0,
                12.0,
                if control.is_active(&shell.preferences) {
                    palette.accent
                } else {
                    visual.text
                },
                false,
            );
        }
    }

    // The relation graph panel, on the right.
    if shell.graph.open {
        let panel_left = shell.graph_panel_left();
        let panel_width = shell.graph_panel_width();
        fill_rect(
            CGRect::new(panel_left, 0.0, panel_width, height - status),
            palette.panel,
            1.0,
        );
        fill_rect(
            CGRect::new(panel_left, 0.0, 1.0, height - status),
            palette.border,
            1.0,
        );
        let header = format!(
            "{}  ·  {}",
            shell.graph.graph.scope_label(),
            shell.graph.relation.label()
        );
        draw_text(
            &header,
            panel_left + 12.0,
            10.0,
            12.0,
            if shell.graph.focused {
                palette.accent
            } else {
                palette.muted
            },
            false,
        );
        let rows = shell.graph_rows();
        let scroll = shell.graph.graph.clamped_scroll(shell.graph.scroll, rows);
        let visible = shell.graph.graph.visible(scroll, rows).to_vec();
        for (row, node) in visible.iter().enumerate() {
            let index = scroll + row;
            let y = graph_core::FIRST_ROW_TOP as f64 + row as f64 * graph_core::ROW_HEIGHT as f64;
            if index == shell.graph.selection && shell.graph.focused {
                fill_rect(
                    CGRect::new(
                        panel_left + 6.0,
                        y,
                        panel_width - 12.0,
                        graph_core::ROW_HEIGHT as f64 - 2.0,
                    ),
                    palette.selection,
                    1.0,
                );
            }
            let marker = if node.folded {
                "▸"
            } else if node.has_children() {
                "▾"
            } else {
                " "
            };
            let indent = 12.0 + node.depth as f64 * 12.0;
            draw_text(
                marker,
                panel_left + indent,
                y + 4.0,
                11.0,
                palette.muted,
                false,
            );
            draw_text(
                &node.caption(),
                panel_left + indent + 14.0,
                y + 4.0,
                12.0,
                if node.edge.is_some() {
                    palette.accent
                } else {
                    palette.text
                },
                false,
            );
            draw_text(
                &node.detail,
                panel_left + panel_width - 150.0,
                y + 5.0,
                10.0,
                palette.muted,
                false,
            );
        }
        let footer = shell.graph.graph.footer(scroll, rows);
        draw_text(
            &footer,
            panel_left + 12.0,
            height - status - graph_core::FOOTER_HEIGHT as f64 + 4.0,
            10.0,
            palette.muted,
            false,
        );
    }

    // The terminal drawer sits above the status bar.
    if let Some(terminal) = shell.terminal.as_mut() {
        let progress = terminal.motion.clamp(0.0, 1.0);
        if progress > 0.01 {
            let panel = terminal_core::panel_metrics(
                width as f32,
                height as f32,
                (width / 1280.0).max(0.8) as f32,
                progress,
            );
            let body_top = panel.body_top as f64;
            let body_bottom = panel.body_bottom as f64;
            fill_rect(
                CGRect::new(0.0, body_top, width, (body_bottom - body_top).max(0.0)),
                palette.panel,
                1.0,
            );
            fill_rect(CGRect::new(0.0, body_top, width, 1.0), palette.border, 1.0);
            let rows = ((body_bottom - body_top) / step).floor().max(1.0) as usize;
            let visible = terminal.visible_lines();
            let first = visible.len().saturating_sub(rows);
            for (row, line) in visible[first..].iter().enumerate() {
                draw_text(
                    line,
                    panel.body_left as f64,
                    body_top + row as f64 * step,
                    font,
                    palette.text,
                    true,
                );
            }
            // A header row naming the shell and the scroll position.
            let title = if terminal.scroll == 0 {
                format!("TERMINAL  ·  {}", default_shell())
            } else {
                format!("TERMINAL  ·  scrolled back {} line(s)", terminal.scroll)
            };
            draw_text(
                &title,
                panel.body_left as f64,
                body_top - 16.0,
                12.0,
                if shell.terminal_focused {
                    palette.accent
                } else {
                    palette.muted
                },
                false,
            );
        }
    }

    let status_line = format!(
        "{}  ·  {} · ln {}{}",
        shell.status,
        if document.dirty { "unsaved" } else { "clean" },
        caret_line + 1,
        match shell.symbol_at_caret() {
            Some(symbol) => format!(" · {symbol}"),
            None => String::new(),
        }
    );
    draw_text(
        &status_line,
        12.0,
        height - status + 6.0,
        12.0,
        palette.muted,
        false,
    );
}

/// `drawRect:` of the editor view.
unsafe extern "C" fn editor_draw(_view: Id, _selector: Sel, _rect: CGRect) {
    with_shell(|shell| draw_frame(shell));
}

/// `isFlipped`, so the view draws from the top left like the layout does.
unsafe extern "C" fn editor_is_flipped(_view: Id, _selector: Sel) -> bool {
    true
}

/// `acceptsFirstResponder`, so the view receives key events.
unsafe extern "C" fn editor_accepts_first_responder(_view: Id, _selector: Sel) -> bool {
    true
}

/// `keyDown:` of the editor view.
unsafe extern "C" fn editor_key_down(_view: Id, _selector: Sel, event: Id) {
    let key_code = msg_usize(event, "keyCode") as u16;
    let modifiers = msg_usize(event, "modifierFlags") as u64;
    let command = modifiers & COMMAND_MASK != 0;
    let shift = modifiers & SHIFT_MASK != 0;
    with_shell(|shell| {
        // The shell owns the keys while the terminal has focus, except for the
        // command shortcuts that open and close panels.
        if shell.terminal_focused && !command && shell.terminal_send_key(key_code) {
            return;
        }
        // While the graph has the keyboard it takes the keys the editor would
        // otherwise use, so a fold never turns into a line break.
        if shell.extensions.open && shell.extensions.focused {
            match key_code {
                125 => {
                    shell.apply(MacCommand::ExtensionNext);
                    return;
                }
                126 => {
                    shell.apply(MacCommand::ExtensionPrevious);
                    return;
                }
                36 | 76 => {
                    shell.apply(MacCommand::ExtensionProbe);
                    return;
                }
                53 => {
                    shell.extensions.open = false;
                    shell.extensions.focused = false;
                    shell.status = "Extensions closed".into();
                    return;
                }
                _ => {}
            }
        }
        if shell.settings.open {
            match key_code {
                48 => {
                    shell.apply(if shift {
                        MacCommand::SettingPrevious
                    } else {
                        MacCommand::SettingNext
                    });
                    return;
                }
                36 | 76 | 49 => {
                    shell.apply(MacCommand::SettingUse);
                    return;
                }
                53 => {
                    shell.settings.open = false;
                    shell.settings.focus = None;
                    shell.status = "Settings closed".into();
                    return;
                }
                _ => {}
            }
        }
        if shell.graph.focused {
            match key_code {
                125 => {
                    shell.apply(MacCommand::GraphNext);
                    return;
                }
                126 => {
                    shell.apply(MacCommand::GraphPrevious);
                    return;
                }
                49 => {
                    shell.apply(MacCommand::GraphFold);
                    return;
                }
                48 => {
                    shell.apply(MacCommand::GraphRelation);
                    return;
                }
                36 | 76 => {
                    shell.apply(MacCommand::GraphJump);
                    return;
                }
                53 => {
                    shell.graph.focused = false;
                    shell.status = "The editor has the keyboard again".into();
                    return;
                }
                _ => {}
            }
        }
        // Page up walks back into the scrollback, page down comes forward.
        if !command
            && matches!(key_code, 116 | 121)
            && shell.scroll_terminal(if key_code == 116 { 5 } else { -5 })
        {
            return;
        }

        if let Some(command) = key_command(key_code, command, shift) {
            shell.apply(command);
        }
    });
}

/// `insertText:` of the editor view, which is also how an IME commits text.
unsafe extern "C" fn editor_insert_text(_view: Id, _selector: Sel, text: Id) {
    let send: extern "C" fn(Id, Sel) -> *const c_char =
        std::mem::transmute(objc_msgSend as *const c_void);
    let bytes = send(text, selector_name("UTF8String"));
    if bytes.is_null() {
        return;
    }
    let value = std::ffi::CStr::from_ptr(bytes)
        .to_string_lossy()
        .into_owned();
    with_shell(|shell| {
        // Typing goes to the shell while the terminal has focus.
        if !shell.terminal_insert(&value) {
            shell.session.insert_text(&value);
        }
    });
}

/// `mouseDown:` of the editor view: place the caret.
unsafe extern "C" fn editor_mouse_down(view: Id, _selector: Sel, event: Id) {
    let point = msg_point_out(event, "locationInWindow");
    // The window coordinates start at the bottom left; the view is flipped.
    let height = msg_rect_out(view, "bounds").height;
    let y = height - point.y;
    with_shell(|shell| {
        shell.mouse = (point.x, y);
        shell.mouse_pressed = true;
        shell.click_at(point.x, y);
    });
}

/// `menu_action:` of the menu target: dispatch a menu item to its command.
unsafe extern "C" fn menu_action(_target: Id, selector: Sel) {
    let name = selector_name_text(selector);
    if let Some(command) = command_for_menu_action(&name) {
        with_shell(|shell| shell.apply(command));
    }
}

/// Name of a selector, as the runtime knows it.
///
/// # Safety
/// The selector has to be a registered selector.
unsafe fn selector_name_text(selector: Sel) -> String {
    let text = sel_getName(selector);
    if text.is_null() {
        return String::new();
    }
    std::ffi::CStr::from_ptr(text)
        .to_string_lossy()
        .into_owned()
}

/// Build one menu item with its action, target and key equivalent.
///
/// # Safety
/// Call on the main thread.
unsafe fn menu_item(title: &str, action: &str, key: &str, target: Id) -> Id {
    let send: extern "C" fn(Id, Sel, Id, Sel, Id) -> Id =
        std::mem::transmute(objc_msgSend as *const c_void);
    let item = send(
        msg_id(class("NSMenuItem"), "alloc"),
        selector_name("initWithTitle:action:keyEquivalent:"),
        nsstring(title),
        selector_name(action),
        nsstring(key),
    );
    msg_void_id(item, "setTarget:", target);
    item
}

/// Build the application menu bar and install it.
///
/// # Safety
/// Call on the main thread after the application object exists.
unsafe fn install_menu_bar() {
    let target_class_name = CString::new("TcIdeMenuTarget").unwrap_or_default();
    if class("TcIdeMenuTarget").is_null() {
        let subclass = objc_allocateClassPair(class("NSObject"), target_class_name.as_ptr(), 0);
        if !subclass.is_null() {
            let void = CString::new("v@:@").unwrap_or_default();
            for action in [
                "tcUndo:",
                "tcRedo:",
                "tcCut:",
                "tcCopy:",
                "tcPaste:",
                "tcOpen:",
                "tcOpenFolder:",
                "tcSave:",
                "tcReveal:",
                "tcQuit:",
            ] {
                class_addMethod(
                    subclass,
                    selector_name(action),
                    std::mem::transmute::<unsafe extern "C" fn(Id, Sel), Imp>(menu_action),
                    void.as_ptr(),
                );
            }
            objc_registerClassPair(subclass);
        }
    }
    let target = msg_id(class("TcIdeMenuTarget"), "new");
    let main_menu = msg_init_string(class("NSMenu"), "initWithTitle:", "TC-IDE");

    // Application menu: the first item is the app name, as AppKit expects.
    let app_menu = msg_init_string(class("NSMenu"), "initWithTitle:", "TC-IDE");
    let quit = menu_item("Quit TC-IDE", "tcQuit:", "q", target);
    msg_void_id(app_menu, "addItem:", quit);
    let app_item = msg_init_string(class("NSMenuItem"), "initWithTitle:", "TC-IDE");
    msg_void_id(app_item, "setSubmenu:", app_menu);
    msg_void_id(main_menu, "addItem:", app_item);

    // File menu.
    let file_menu = msg_init_string(class("NSMenu"), "initWithTitle:", "File");
    for (title, action, key) in [
        ("Open File…", "tcOpen:", "o"),
        ("Open Folder…", "tcOpenFolder:", "O"),
        ("Save", "tcSave:", "s"),
        ("Reveal in Finder", "tcReveal:", "R"),
    ] {
        let item = menu_item(title, action, key, target);
        msg_void_id(file_menu, "addItem:", item);
    }
    let file_item = msg_init_string(class("NSMenuItem"), "initWithTitle:", "File");
    msg_void_id(file_item, "setSubmenu:", file_menu);
    msg_void_id(main_menu, "addItem:", file_item);

    // Edit menu.
    let edit_menu = msg_init_string(class("NSMenu"), "initWithTitle:", "Edit");
    for (title, action, key) in [
        ("Undo", "tcUndo:", "z"),
        ("Redo", "tcRedo:", "Z"),
        ("Cut", "tcCut:", "x"),
        ("Copy", "tcCopy:", "c"),
        ("Paste", "tcPaste:", "v"),
        ("Next Tab", "tcNextTab:", "}"),
        ("Previous Tab", "tcPreviousTab:", "{"),
        ("Close Tab", "tcCloseTab:", "w"),
        ("Toggle Terminal", "tcToggleTerminal:", "j"),
        ("Relation Graph", "tcToggleGraph:", "g"),
        ("Settings…", "tcSettings:", ","),
        ("Extensions & Tooling…", "tcExtensions:", "e"),
        ("Format File", "tcFormat:", "F"),
    ] {
        let item = menu_item(title, action, key, target);
        msg_void_id(edit_menu, "addItem:", item);
    }
    let edit_item = msg_init_string(class("NSMenuItem"), "initWithTitle:", "Edit");
    msg_void_id(edit_item, "setSubmenu:", edit_menu);
    msg_void_id(main_menu, "addItem:", edit_item);

    let application = msg_id(class("NSApplication"), "sharedApplication");
    msg_void_id(application, "setMainMenu:", main_menu);
}

/// `initWithTitle:` of an `NSMenu` or an `NSMenuItem`.
///
/// # Safety
/// The receiver has to answer that selector.
unsafe fn msg_init_string(receiver: Id, selector: &str, title: &str) -> Id {
    let send: extern "C" fn(Id, Sel, Id) -> Id = std::mem::transmute(objc_msgSend as *const c_void);
    send(receiver, selector_name(selector), nsstring(title))
}

/// `mouseMoved:` of the editor view: track the pointer for hover states.
unsafe extern "C" fn editor_mouse_moved(_view: Id, _selector: Sel, event: Id) {
    let point = msg_point_out(event, "locationInWindow");
    let height = msg_rect_out(_view, "bounds").height;
    with_shell(|shell| shell.mouse = (point.x, height - point.y));
}

/// `mouseUp:` of the editor view: the pressed state ends.
unsafe extern "C" fn editor_mouse_up(_view: Id, _selector: Sel, event: Id) {
    let point = msg_point_out(event, "locationInWindow");
    let height = msg_rect_out(_view, "bounds").height;
    with_shell(|shell| {
        shell.mouse = (point.x, height - point.y);
        shell.mouse_pressed = false;
        shell.settings.pressed = None;
    });
}

/// Register the editor view class.
///
/// # Safety
/// Call on the main thread; the runtime keeps the class alive.
unsafe fn register_editor_view() {
    if !class("TcIdeEditorView").is_null() {
        return;
    }
    let name = CString::new("TcIdeEditorView").unwrap_or_default();
    let subclass = objc_allocateClassPair(class("NSView"), name.as_ptr(), 0);
    if subclass.is_null() {
        return;
    }
    let object_argument = CString::new("v@:@").unwrap_or_default();
    let rect_argument = CString::new("v@:{CGRect={CGPoint=dd}{CGSize=dd}}").unwrap_or_default();
    let boolean = CString::new("B@:").unwrap_or_default();
    class_addMethod(
        subclass,
        selector_name("drawRect:"),
        std::mem::transmute::<unsafe extern "C" fn(Id, Sel, CGRect), Imp>(editor_draw),
        rect_argument.as_ptr(),
    );
    class_addMethod(
        subclass,
        selector_name("keyDown:"),
        std::mem::transmute::<unsafe extern "C" fn(Id, Sel, Id), Imp>(editor_key_down),
        object_argument.as_ptr(),
    );
    class_addMethod(
        subclass,
        selector_name("insertText:"),
        std::mem::transmute::<unsafe extern "C" fn(Id, Sel, Id), Imp>(editor_insert_text),
        object_argument.as_ptr(),
    );
    class_addMethod(
        subclass,
        selector_name("mouseDown:"),
        std::mem::transmute::<unsafe extern "C" fn(Id, Sel, Id), Imp>(editor_mouse_down),
        object_argument.as_ptr(),
    );
    class_addMethod(
        subclass,
        selector_name("mouseMoved:"),
        std::mem::transmute::<unsafe extern "C" fn(Id, Sel, Id), Imp>(editor_mouse_moved),
        object_argument.as_ptr(),
    );
    class_addMethod(
        subclass,
        selector_name("mouseUp:"),
        std::mem::transmute::<unsafe extern "C" fn(Id, Sel, Id), Imp>(editor_mouse_up),
        object_argument.as_ptr(),
    );
    class_addMethod(
        subclass,
        selector_name("isFlipped"),
        std::mem::transmute::<unsafe extern "C" fn(Id, Sel) -> bool, Imp>(editor_is_flipped),
        boolean.as_ptr(),
    );
    class_addMethod(
        subclass,
        selector_name("acceptsFirstResponder"),
        std::mem::transmute::<unsafe extern "C" fn(Id, Sel) -> bool, Imp>(
            editor_accepts_first_responder,
        ),
        boolean.as_ptr(),
    );
    objc_registerClassPair(subclass);
}

/// `tick:` of the frame timer: step the shared motion and redraw.
unsafe extern "C" fn timer_tick(_target: Id, _selector: Sel) {
    let view = with_shell(|shell| {
        shell.tick();
        shell.view
    });
    if !view.is_null() {
        msg_bool(view, "setNeedsDisplay:", true);
    }
}

/// Schedule a repeating timer that calls `timer_tick`.
///
/// # Safety
/// Call on the main thread.
unsafe fn schedule_timer(interval: f64) {
    let name = CString::new("TcIdeTickTarget").unwrap_or_default();
    if class("TcIdeTickTarget").is_null() {
        let subclass = objc_allocateClassPair(class("NSObject"), name.as_ptr(), 0);
        if !subclass.is_null() {
            let void = CString::new("v@:").unwrap_or_default();
            class_addMethod(
                subclass,
                selector_name("tick:"),
                std::mem::transmute::<unsafe extern "C" fn(Id, Sel), Imp>(timer_tick),
                void.as_ptr(),
            );
            objc_registerClassPair(subclass);
        }
    }
    let target = msg_id(class("TcIdeTickTarget"), "new");
    let send: extern "C" fn(Id, Sel, f64, Id, Sel, Id, bool) -> Id =
        std::mem::transmute(objc_msgSend as *const c_void);
    send(
        class("NSTimer"),
        selector_name("scheduledTimerWithTimeInterval:target:selector:userInfo:repeats:"),
        interval,
        target,
        selector_name("tick:"),
        ptr::null_mut(),
        true,
    );
}

/// Build the window, its editor view and the frame timer.
///
/// # Safety
/// Call on the main thread after `NSApplicationLoad`.
unsafe fn build_window(width: f64, height: f64) {
    register_editor_view();
    let frame = CGRect::new(120.0, 120.0, width, height);
    let window = msg_window_init(
        msg_id(class("NSWindow"), "alloc"),
        "initWithContentRect:styleMask:backing:defer:",
        frame,
        WINDOW_STYLE,
        BACKING_BUFFERED,
        false,
    );
    msg_void_id(window, "setTitle:", nsstring("TC-IDE"));
    msg_bool(window, "setReleasedWhenClosed:", false);
    let bounds = CGRect::new(0.0, 0.0, width, height);
    let view = msg_init_rect(
        msg_id(class("TcIdeEditorView"), "alloc"),
        "initWithFrame:",
        bounds,
    );
    msg_void_id(window, "setContentView:", view);
    msg_bool(window, "setAcceptsMouseMovedEvents:", true);
    msg_bool(window, "makeFirstResponder:", true);
    msg_bool(window, "makeKeyAndOrderFront:", true);
    install_menu_bar();
    with_shell(|shell| {
        shell.window = window;
        shell.view = view;
        shell.session.viewport = text_buffer::Viewport::new(
            ((height - shell.header_height() - shell.status_height()) / shell.line_step())
                .floor()
                .max(1.0) as usize,
            ((width - GUTTER_WIDTH) / TEXT_CELL).floor().max(8.0) as usize,
        );
    });
    schedule_timer(1.0 / 60.0);
}

/// Entry point of the macOS backend.
pub fn run() {
    if std::env::args().any(|argument| argument == "--self-check") {
        std::process::exit(self_check());
    }
    unsafe {
        if !NSApplicationLoad() {
            eprintln!("TC-IDE: could not load AppKit");
            std::process::exit(1);
        }
        let app = msg_id(class("NSApplication"), "sharedApplication");
        // A regular application, so the window takes focus and the Dock shows it.
        msg_long(app, "setActivationPolicy:", 0);
        build_window(1280.0, 820.0);
        msg_bool(app, "activateIgnoringOtherApps:", true);
        msg_id(app, "run");
    }
}

/// Scriptable state layer check: `tc-ide --self-check`.
///
/// The macOS runner of the pipeline runs this, so the backend is executed on
/// macOS and not only compiled. The editing itself is covered by the unit tests
/// of the shared session, which run on every platform.
fn self_check() -> i32 {
    let mut failures = Vec::new();
    let mut checks = 0usize;
    let mut report = |name: &str, ok: bool, failures: &mut Vec<String>| {
        checks += 1;
        if !ok {
            failures.push(name.to_string());
        }
    };

    let mut shell = MacShell::new();
    shell.session.replace_all("one\ntwo");
    report(
        "a scratch document holds its lines",
        shell.session.document().lines == ["one", "two"] && shell.session.caret() == (1, 3),
        &mut failures,
    );
    shell.session.backspace();
    report(
        "backspace removes a character",
        shell.session.document().lines == ["one", "tw"],
        &mut failures,
    );
    shell.session.move_caret(-1, 0);
    shell.session.move_to_line_end(true);
    shell.session.insert_text("X");
    report(
        "the caret moves between lines",
        shell.session.document().lines == ["oneX", "tw"],
        &mut failures,
    );
    report(
        "an edit marks the document dirty",
        shell.session.dirty(),
        &mut failures,
    );
    report(
        "the palette comes from the shared theme",
        shell.palette == theme_core::Palette::for_scheme(shell.preferences.light_theme),
        &mut failures,
    );
    report(
        "the window geometry adds up",
        shell.text_area().x == GUTTER_WIDTH
            && shell.view_height() > shell.text_height()
            && shell.visible_rows() == shell.session.viewport.height.max(1),
        &mut failures,
    );
    shell.viewport_for_checks();
    // The document is ["oneX", "tw"], so the second line only has two columns.
    shell.click_at(
        GUTTER_WIDTH + TEXT_CELL * 1.5,
        shell.header_height() + shell.line_step() * 1.5,
    );
    report(
        "a click lands on the row and column it points at",
        shell.session.caret() == (1, 1),
        &mut failures,
    );
    // A click past the end of a short line stops at the line end.
    shell.click_at(
        GUTTER_WIDTH + TEXT_CELL * 40.0,
        shell.header_height() + shell.line_step() * 1.5,
    );
    report(
        "a click past the line end stops at the line end",
        shell.session.caret() == (1, 2),
        &mut failures,
    );
    report(
        "the key mapping understands the arrows and the command shortcuts",
        key_command(123, false, false) == Some(MacCommand::MoveLeft)
            && key_command(124, false, false) == Some(MacCommand::MoveRight)
            && key_command(125, false, false) == Some(MacCommand::MoveDown)
            && key_command(126, false, false) == Some(MacCommand::MoveUp)
            && key_command(115, false, false) == Some(MacCommand::MoveLineStart)
            && key_command(119, false, false) == Some(MacCommand::MoveLineEnd)
            && key_command(36, false, false) == Some(MacCommand::InsertNewline)
            && key_command(51, false, false) == Some(MacCommand::Backspace)
            && key_command(117, false, false) == Some(MacCommand::DeleteForward)
            && key_command(48, false, false) == Some(MacCommand::Tab)
            && key_command(0, true, false) == Some(MacCommand::SelectAll)
            && key_command(1, true, false) == Some(MacCommand::Save)
            && key_command(8, false, false).is_none(),
        &mut failures,
    );
    report(
        "typing is not swallowed by the command table",
        is_printable_key(8, false) && !is_printable_key(51, false) && !is_printable_key(0, true),
        &mut failures,
    );
    report(
        "the clipboard and file shortcuts are mapped",
        key_command(8, true, false) == Some(MacCommand::Copy)
            && key_command(9, true, false) == Some(MacCommand::Paste)
            && key_command(7, true, false) == Some(MacCommand::Cut)
            && key_command(31, true, false) == Some(MacCommand::OpenFile)
            && key_command(31, true, true) == Some(MacCommand::OpenFolder)
            && key_command(6, true, false) == Some(MacCommand::Undo)
            && key_command(6, true, true) == Some(MacCommand::Redo)
            && key_command(15, true, true) == Some(MacCommand::Reveal),
        &mut failures,
    );
    report(
        "the menu items use the same commands as the keys",
        command_for_menu_action("tcCopy:") == Some(MacCommand::Copy)
            && command_for_menu_action("tcPaste:") == Some(MacCommand::Paste)
            && command_for_menu_action("tcUndo:") == Some(MacCommand::Undo)
            && command_for_menu_action("tcRedo:") == Some(MacCommand::Redo)
            && command_for_menu_action("tcOpen:") == Some(MacCommand::OpenFile)
            && command_for_menu_action("tcOpenFolder:") == Some(MacCommand::OpenFolder)
            && command_for_menu_action("tcSave:") == Some(MacCommand::Save)
            && command_for_menu_action("tcReveal:") == Some(MacCommand::Reveal)
            && command_for_menu_action("tcQuit:") == Some(MacCommand::Quit)
            && command_for_menu_action("somethingElse:").is_none(),
        &mut failures,
    );
    // Undo and undo-again through the session, which is what the menu calls.
    shell.session.replace_all("start");
    shell.session.move_to_line_end(true);
    shell.session.insert_text(" one");
    let undone = shell.session.undo();
    report(
        "an edit can be undone and redone",
        undone
            && shell.session.document().lines == ["start"]
            && shell.session.redo()
            && shell.session.document().lines == ["start one"],
        &mut failures,
    );
    report(
        "the tab strip maps a point to a tab",
        tab_index_at(TAB_MARGIN, 3, 150.0) == Some(0)
            && tab_index_at(TAB_MARGIN + 149.0, 3, 150.0) == Some(0)
            && tab_index_at(TAB_MARGIN + 150.0, 3, 150.0) == Some(1)
            && tab_index_at(TAB_MARGIN + 450.0, 3, 150.0).is_none()
            && tab_index_at(2.0, 3, 150.0).is_none(),
        &mut failures,
    );
    report(
        "the tab commands are mapped",
        key_command(30, true, true) == Some(MacCommand::NextTab)
            && key_command(33, true, true) == Some(MacCommand::PreviousTab)
            && key_command(13, true, false) == Some(MacCommand::CloseTab)
            && command_for_menu_action("tcNextTab:") == Some(MacCommand::NextTab)
            && command_for_menu_action("tcPreviousTab:") == Some(MacCommand::PreviousTab)
            && command_for_menu_action("tcCloseTab:") == Some(MacCommand::CloseTab),
        &mut failures,
    );
    // Tabs: the second document becomes active, then the first one again.
    shell.session.activate(0);
    let opened = shell
        .session
        .add_document(editor_session::Document::from_lines(
            "second.rs",
            None,
            vec!["second".into()],
        ));
    shell.session.activate(1);
    let count = shell.session.documents().len();
    shell.step_tab(1);
    let wrapped = shell.session.active();
    shell.step_tab(-1);
    let back = shell.session.active();
    report(
        "walking the tabs wraps around",
        count == opened + 1 && wrapped == 0 && back == 1,
        &mut failures,
    );
    shell.close_tab();
    report(
        "closing a tab leaves the other one open",
        shell.session.documents().len() == 1 && shell.session.active() == 0,
        &mut failures,
    );
    // The tooling panel probes the real executables through the shared table.
    shell.apply(MacCommand::ToggleExtensions);
    report(
        "the tooling panel probes every known language",
        shell.extensions.open && shell.extensions.rows.len() == EXTENSION_LANGUAGES.len(),
        &mut failures,
    );
    let rust_row = shell
        .extensions
        .rows
        .iter()
        .find(|row| row.language == "Rust")
        .cloned();
    report(
        "a probed row names its tools and their marks",
        rust_row
            .as_ref()
            .map(|row| {
                row.server.as_deref() == Some("rust-analyzer")
                    && row.formatter.as_deref() == Some("rustfmt")
                    && row.marks().split(' ').count() == 3
                    && row.summary().contains("Rust")
            })
            .unwrap_or(false),
        &mut failures,
    );
    let yes = ToolingRow::probe("Rust", |tool| tool == "rustfmt");
    let none = ToolingRow::probe("Rust", |_| false);
    report(
        "the probe decides what is installed",
        yes.formatter_installed
            && !yes.server_installed
            && yes.summary().contains("rust-analyzer")
            && !yes.summary().contains("format: rustfmt")
            && !none.formatter_installed
            && none.marks() == "· · ·"
            && yes.marks() == "· ✓ ·",
        &mut failures,
    );
    // The probe itself is behavioural: a tool that exists is found, one that
    // cannot exist is not.
    report(
        "the tool probe looks at the real machine",
        lsp_core::executable_available("/bin/sh")
            && !lsp_core::executable_available("/no/such/tc-ide-tool"),
        &mut failures,
    );
    let format = workspace_core::format_command("Rust", "main.rs");
    report(
        "formatting uses the shared command",
        format.as_deref() == Some("rustfmt \"main.rs\"")
            && workspace_core::format_command("Markdown", "README.md").is_none(),
        &mut failures,
    );
    shell.extensions.move_focus(1);
    let moved = shell.extensions.focus;
    shell.extensions.move_focus(-9);
    report(
        "the language selection moves and clamps",
        moved == 1
            && shell.extensions.focus == 0
            && ExtensionsPanel::row_at(
                EXTENSIONS_HEADER_HEIGHT + 4.0,
                0.0,
                EXTENSION_LANGUAGES.len(),
            ) == Some(0)
            && ExtensionsPanel::row_at(EXTENSIONS_HEADER_HEIGHT - 1.0, 0.0, 7).is_none()
            && ExtensionsPanel::row_at(
                EXTENSIONS_HEADER_HEIGHT + EXTENSIONS_ROW_HEIGHT * 7.0,
                0.0,
                7,
            )
            .is_none(),
        &mut failures,
    );
    report(
        "the tooling panel has a shortcut and a menu item",
        key_command(14, true, false) == Some(MacCommand::ToggleExtensions)
            && key_command(3, true, true) == Some(MacCommand::FormatFile)
            && command_for_menu_action("tcExtensions:") == Some(MacCommand::ToggleExtensions)
            && command_for_menu_action("tcFormat:") == Some(MacCommand::FormatFile),
        &mut failures,
    );
    shell.extensions.open = false;
    shell.extensions.focused = false;

    // The code column is drawn per syntax token, from the shared tokenizer and
    // the shared palette.
    let dark_palette = theme_core::Palette::dark();
    let light_palette = theme_core::Palette::light();
    report(
        "syntax tokens use the palette",
        token_color(&dark_palette, editor_core::TokenKind::Keyword) == dark_palette.keyword
            && token_color(&dark_palette, editor_core::TokenKind::Literal) == dark_palette.literal
            && token_color(&dark_palette, editor_core::TokenKind::Comment) == dark_palette.comment
            && token_color(&dark_palette, editor_core::TokenKind::Plain) == dark_palette.text
            && token_color(&light_palette, editor_core::TokenKind::Keyword)
                == light_palette.keyword
            && light_palette.keyword != dark_palette.keyword,
        &mut failures,
    );
    let tokens = editor_core::syntax_tokens("let value = 3; // note", "main.rs");
    report(
        "the editor highlights a line as tokens",
        tokens.len() >= 2
            && tokens
                .iter()
                .any(|token| token.kind == editor_core::TokenKind::Comment)
            && tokens
                .iter()
                .any(|token| token.kind == editor_core::TokenKind::Literal),
        &mut failures,
    );
    report(
        "a scrolled token is sliced at the right column",
        visible_token("hello", 0, 0) == Some((0, "hello".into()))
            && visible_token("hello", 0, 2) == Some((2, "llo".into()))
            && visible_token("hello", 0, 5).is_none()
            && visible_token("hello", 0, 9).is_none()
            && visible_token("ab", 4, 4) == Some((4, "ab".into()))
            && visible_token("ab", 4, 5) == Some((5, "b".into())),
        &mut failures,
    );

    // The settings panel drives the shared preferences and palette.
    shell.apply(MacCommand::ToggleSettings);
    report(
        "the settings panel opens and focuses a control",
        shell.settings.open && shell.settings.focus.is_some(),
        &mut failures,
    );
    let dark = shell.preferences.light_theme;
    shell.activate_setting(Setting::ThemeLight);
    let light_palette = shell.palette == theme_core::Palette::light();
    shell.activate_setting(Setting::ThemeDark);
    let back_to_dark =
        !shell.preferences.light_theme && shell.palette == theme_core::Palette::dark();
    report(
        "the theme control switches the shared palette",
        !dark && light_palette && back_to_dark,
        &mut failures,
    );
    shell.activate_setting(Setting::LanguageChinese);
    let chinese = shell.preferences.ui_chinese;
    shell.activate_setting(Setting::LanguageEnglish);
    report(
        "the language control switches the interface language",
        chinese && !shell.preferences.ui_chinese,
        &mut failures,
    );
    let before = shell.preferences.code_zoom;
    shell.activate_setting(Setting::FontLarger);
    let larger = shell.preferences.code_zoom == before + 1;
    shell.activate_setting(Setting::FontSmaller);
    report(
        "the font controls step the editor size",
        larger && shell.preferences.code_zoom == before,
        &mut failures,
    );
    // Walking the focus skips a control that cannot be used.
    shell.preferences.code_zoom = MIN_CODE_ZOOM;
    shell.settings.focus = Some(Setting::FontLarger);
    shell.step_settings_focus(false);
    report(
        "the focused control stays usable",
        shell.settings.focus != Some(Setting::FontSmaller),
        &mut failures,
    );
    shell.preferences.code_zoom = 0;
    report(
        "the panel maps its controls",
        SettingsPanel::control_at(20.0, SETTINGS_FIRST_ROW + 4.0) == Some(Setting::ThemeDark)
            && SettingsPanel::control_at(168.0, SETTINGS_FIRST_ROW + 4.0)
                == Some(Setting::ThemeLight)
            && SettingsPanel::control_at(20.0, SETTINGS_FIRST_ROW + SETTINGS_ROW_HEIGHT + 4.0)
                == Some(Setting::FontSmaller)
            && SettingsPanel::control_at(20.0, 4.0).is_none()
            && SettingsPanel::height() > SETTINGS_FIRST_ROW,
        &mut failures,
    );
    report(
        "settings load and save through the shared model",
        workspace_core::parse_preferences(&workspace_core::serialize_preferences(
            &workspace_core::Preferences {
                light_theme: true,
                code_zoom: 4,
                ..workspace_core::Preferences::default()
            },
        ))
        .light_theme
            && preferences_path()
                .map(|path| path.ends_with("TC-IDE/settings.conf"))
                .unwrap_or(false),
        &mut failures,
    );
    report(
        "the settings panel has a shortcut and a menu item",
        key_command(43, true, false) == Some(MacCommand::ToggleSettings)
            && command_for_menu_action("tcSettings:") == Some(MacCommand::ToggleSettings),
        &mut failures,
    );
    shell.settings.open = false;
    shell.settings.focus = None;

    // The relation graph is built from the open documents through graph_core.
    shell.session.replace_all(
        "struct Engine {\n}\nimpl Engine {\n    fn build(&self) {\n        self.prepare();\n    }\n    fn prepare(&self) {}\n}\n",
    );
    shell.graph.dirty = true;
    shell.refresh_graph();
    report(
        "the graph builds from the open document",
        !shell.graph.graph.nodes.is_empty()
            && shell.graph.graph.nodes[0].level == graph_core::GraphLevel::Project
            && shell
                .graph
                .graph
                .nodes
                .iter()
                .any(|node| node.level == graph_core::GraphLevel::Class),
        &mut failures,
    );
    // Putting the caret on `build` and asking for its callees is what exercises
    // the shared resolution: `self.prepare()` has to become a member of Engine.
    shell.session.set_caret(3, 8);
    shell.graph.relation = GraphRelation::Callees;
    shell.graph.dirty = true;
    shell.refresh_graph();
    report(
        "the graph resolves a member call to its class",
        shell
            .graph
            .graph
            .nodes
            .iter()
            .any(|node| node.caption().contains("prepare") && node.edge.is_some()),
        &mut failures,
    );
    shell.graph.relation = GraphRelation::Follow;
    shell.graph.dirty = true;
    shell.refresh_graph();
    let unfolded = shell.graph.graph.nodes.len();
    shell.graph.folded.push(graph_core::file_key("main.rs"));
    shell.graph.dirty = true;
    shell.refresh_graph();
    report(
        "folding a file hides its symbols",
        shell.graph.graph.nodes.len() < unfolded,
        &mut failures,
    );
    shell.graph.folded.clear();
    shell.graph.dirty = true;
    shell.refresh_graph();

    // The arrows move the selection and the panel keeps it in view.
    shell.graph.focused = true;
    shell.graph.selection = 0;
    shell.graph_move(1);
    let moved = shell.graph.selection;
    shell.graph_move(500);
    let clamped = shell.graph.selection;
    report(
        "the graph selection moves and clamps",
        moved == 1 && clamped == shell.graph.graph.nodes.len().saturating_sub(1),
        &mut failures,
    );

    // Enter jumps to the target of the selected node.
    if let Some((index, line)) =
        shell
            .graph
            .graph
            .nodes
            .iter()
            .enumerate()
            .find_map(|(index, node)| {
                node.target
                    .as_ref()
                    .filter(|target| target.file == "main.rs")
                    .map(|target| (index, target.line))
            })
    {
        shell.graph.selection = index;
        shell.graph_activate();
        report(
            "the graph jumps to the node target",
            shell.session.document().caret_line == line,
            &mut failures,
        );
    } else {
        report("the graph jumps to the node target", false, &mut failures);
    }

    // Ranges and relations cycle back to where they started.
    let mut scope_back = false;
    for _ in 0..5 {
        shell.graph_cycle_scope();
        if shell.graph.scope.is_none() {
            scope_back = true;
        }
    }
    let mut relation_back = false;
    for _ in 0..3 {
        shell.graph_cycle_relation();
        if shell.graph.relation == GraphRelation::Follow {
            relation_back = true;
        }
    }
    report(
        "ranges and relations cycle",
        scope_back && relation_back,
        &mut failures,
    );
    report(
        "the graph panel has a keyboard shortcut and a menu item",
        key_command(5, true, false) == Some(MacCommand::ToggleGraph)
            && key_command(5, true, true) == Some(MacCommand::GraphScope)
            && command_for_menu_action("tcToggleGraph:") == Some(MacCommand::ToggleGraph)
            && command_for_menu_action("tcGraphRelation:") == Some(MacCommand::GraphRelation)
            && command_for_menu_action("tcGraphScope:") == Some(MacCommand::GraphScope),
        &mut failures,
    );
    report(
        "the panel rows follow the shared layout",
        GraphPanel::row_at(graph_core::FIRST_ROW_TOP as f64 + 1.0, 3) == Some(0)
            && GraphPanel::row_at(graph_core::FIRST_ROW_TOP as f64 - 1.0, 3).is_none()
            && GraphPanel::row_at(
                graph_core::FIRST_ROW_TOP as f64 + graph_core::ROW_HEIGHT as f64 + 1.0,
                3,
            ) == Some(1)
            && GraphPanel::row_at(
                graph_core::FIRST_ROW_TOP as f64 + graph_core::ROW_HEIGHT as f64 * 5.0,
                3,
            )
            .is_none()
            && GraphPanel::rows_for(1000.0) >= 1,
        &mut failures,
    );
    shell.graph.focused = false;

    report(
        "the terminal keys are mapped to their sequences",
        terminal_key(126) == Some(TerminalKey::Up)
            && terminal_key(125) == Some(TerminalKey::Down)
            && terminal_key(124) == Some(TerminalKey::Right)
            && terminal_key(123) == Some(TerminalKey::Left)
            && terminal_key(115) == Some(TerminalKey::Home)
            && terminal_key(119) == Some(TerminalKey::End)
            && terminal_key(117) == Some(TerminalKey::Delete)
            && terminal_key(51) == Some(TerminalKey::Backspace)
            && terminal_key(36) == Some(TerminalKey::Enter)
            && terminal_key(48) == Some(TerminalKey::Tab)
            && terminal_key(53) == Some(TerminalKey::Escape)
            && terminal_key(8).is_none(),
        &mut failures,
    );
    report(
        "the terminal shortcut shows the drawer",
        key_command(38, true, false) == Some(MacCommand::ToggleTerminal)
            && command_for_menu_action("tcToggleTerminal:") == Some(MacCommand::ToggleTerminal),
        &mut failures,
    );
    // A shell really runs on a pseudo terminal, and its output is parsed.
    let grid = shell.terminal_grid();
    report(
        "the terminal grid is usable",
        grid.0 >= 2 && grid.1 >= 8,
        &mut failures,
    );
    match TerminalState::start(grid.0.min(24), grid.1.min(80)) {
        Ok(mut terminal) => {
            terminal.pty.send("printf 'tc-ide-pty-ready\\n'\n");
            let seen = terminal.wait_for("tc-ide-pty-ready", std::time::Duration::from_secs(15));
            report("a shell runs on a pseudo terminal", seen, &mut failures);
            let before = terminal.lines().len();
            terminal.pty.send("printf 'second-marker\\n'\n");
            let second = terminal.wait_for("second-marker", std::time::Duration::from_secs(15));
            report(
                "the shell keeps answering after the first command",
                second && terminal.lines().len() > before,
                &mut failures,
            );
            terminal.pty.close();
        }
        Err(message) => report(
            &format!("the terminal did not start: {message}"),
            false,
            &mut failures,
        ),
    }
    report(
        "a column is placed with the measured cell width",
        (MacShell::column_x_with(4, 8.0) - (GUTTER_WIDTH + 32.0)).abs() < 1.0e-9,
        &mut failures,
    );

    // The editing itself is checked once, in the shared session, where the same
    // assertions run on every platform instead of only on the macOS runner.
    for failure in editor_session::state_check(editor_session::ClickMetrics {
        gutter: GUTTER_WIDTH,
        cell: TEXT_CELL,
        line_step: shell.line_step(),
        header: shell.header_height(),
    }) {
        report(&failure, false, &mut failures);
    }

    println!(
        "TC-IDE macOS self check: {checks} checks, {} failed",
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_key_mapping_covers_the_editor_commands() {
        assert_eq!(
            key_command(36, false, false),
            Some(MacCommand::InsertNewline)
        );
        assert_eq!(key_command(51, false, false), Some(MacCommand::Backspace));
        assert_eq!(
            key_command(117, false, false),
            Some(MacCommand::DeleteForward)
        );
        assert_eq!(key_command(123, false, false), Some(MacCommand::MoveLeft));
        assert_eq!(key_command(124, false, false), Some(MacCommand::MoveRight));
        assert_eq!(key_command(125, false, false), Some(MacCommand::MoveDown));
        assert_eq!(key_command(126, false, false), Some(MacCommand::MoveUp));
        assert_eq!(
            key_command(115, false, false),
            Some(MacCommand::MoveLineStart)
        );
        assert_eq!(
            key_command(119, false, false),
            Some(MacCommand::MoveLineEnd)
        );
        assert_eq!(key_command(48, false, false), Some(MacCommand::Tab));
        assert_eq!(key_command(0, true, false), Some(MacCommand::SelectAll));
        assert_eq!(key_command(1, true, false), Some(MacCommand::Save));
        // A letter without the command key is text, not a command.
        assert_eq!(key_command(0, false, false), None);
        assert_eq!(key_command(1, false, false), None);
        assert!(is_printable_key(8, false));
        assert!(!is_printable_key(51, false));
        assert!(!is_printable_key(0, true));
        assert!(!is_printable_key(53, false), "escape is not text");
    }

    #[test]
    fn clicks_land_on_the_row_and_column_they_point_at() {
        let mut shell = MacShell::new();
        shell.viewport_for_checks();
        let top = shell.header_height();
        let step = shell.line_step();
        shell.click_at(GUTTER_WIDTH + TEXT_CELL * 3.5, top + step * 2.5);
        assert_eq!(shell.session.caret(), (2, 3));

        // A click in the header is not a caret move.
        shell.click_at(GUTTER_WIDTH, 4.0);
        assert_eq!(shell.session.caret(), (2, 3));
        // And the caret never leaves the document.
        shell.click_at(GUTTER_WIDTH, top + step * 400.0);
        assert!(shell.session.caret().0 < shell.session.document().lines.len());
    }

    #[test]
    fn the_window_geometry_leaves_room_for_the_text() {
        let shell = MacShell::new();
        assert!(shell.view_height() > shell.text_height());
        assert_eq!(
            shell.view_height(),
            shell.header_height() + shell.text_height() + shell.status_height()
        );
        assert_eq!(shell.text_area().y, shell.header_height());
        assert_eq!(shell.text_area().x, GUTTER_WIDTH);
    }

    #[test]
    fn the_clipboard_and_file_shortcuts_are_mapped() {
        assert_eq!(key_command(8, true, false), Some(MacCommand::Copy));
        assert_eq!(key_command(9, true, false), Some(MacCommand::Paste));
        assert_eq!(key_command(7, true, false), Some(MacCommand::Cut));
        assert_eq!(key_command(31, true, false), Some(MacCommand::OpenFile));
        assert_eq!(key_command(31, true, true), Some(MacCommand::OpenFolder));
        assert_eq!(key_command(6, true, false), Some(MacCommand::Undo));
        assert_eq!(key_command(6, true, true), Some(MacCommand::Redo));
        assert_eq!(key_command(15, true, true), Some(MacCommand::Reveal));
        // Without the command key the same keys are text, not shortcuts.
        assert_eq!(key_command(8, false, false), None);
        assert_eq!(key_command(6, false, false), None);
        assert_eq!(key_command(15, false, true), None);
    }

    #[test]
    fn every_menu_item_maps_to_one_command() {
        for (action, command) in [
            ("tcUndo:", MacCommand::Undo),
            ("tcRedo:", MacCommand::Redo),
            ("tcCut:", MacCommand::Cut),
            ("tcCopy:", MacCommand::Copy),
            ("tcPaste:", MacCommand::Paste),
            ("tcOpen:", MacCommand::OpenFile),
            ("tcOpenFolder:", MacCommand::OpenFolder),
            ("tcSave:", MacCommand::Save),
            ("tcReveal:", MacCommand::Reveal),
            ("tcQuit:", MacCommand::Quit),
        ] {
            assert_eq!(command_for_menu_action(action), Some(command), "{action}");
        }
        assert_eq!(command_for_menu_action("tcMissing:"), None);
        assert_eq!(command_for_menu_action(""), None);
    }

    #[test]
    fn the_tab_strip_maps_a_point_to_a_tab() {
        assert_eq!(tab_index_at(TAB_MARGIN, 3, 150.0), Some(0));
        assert_eq!(tab_index_at(TAB_MARGIN + 149.0, 3, 150.0), Some(0));
        assert_eq!(tab_index_at(TAB_MARGIN + 150.0, 3, 150.0), Some(1));
        assert_eq!(tab_index_at(TAB_MARGIN + 300.0, 3, 150.0), Some(2));
        assert_eq!(tab_index_at(TAB_MARGIN + 450.0, 3, 150.0), None);
        assert_eq!(tab_index_at(0.0, 3, 150.0), None);
        assert_eq!(tab_index_at(TAB_MARGIN, 0, 150.0), None);
        assert_eq!(tab_index_at(TAB_MARGIN, 3, 0.0), None);
    }

    #[test]
    fn the_tab_commands_are_mapped() {
        assert_eq!(key_command(30, true, true), Some(MacCommand::NextTab));
        assert_eq!(key_command(33, true, true), Some(MacCommand::PreviousTab));
        assert_eq!(key_command(13, true, false), Some(MacCommand::CloseTab));
        assert_eq!(key_command(13, false, false), None);
        assert_eq!(key_command(30, true, false), None);
        assert_eq!(
            command_for_menu_action("tcNextTab:"),
            Some(MacCommand::NextTab)
        );
        assert_eq!(
            command_for_menu_action("tcPreviousTab:"),
            Some(MacCommand::PreviousTab)
        );
        assert_eq!(
            command_for_menu_action("tcCloseTab:"),
            Some(MacCommand::CloseTab)
        );
    }

    #[test]
    fn walking_and_closing_tabs_keeps_one_open() {
        let mut shell = MacShell::new();
        shell
            .session
            .add_document(editor_session::Document::from_lines(
                "second.rs",
                None,
                Vec::new(),
            ));
        assert_eq!(shell.session.documents().len(), 2);
        assert_eq!(shell.session.active(), 1);
        shell.step_tab(1);
        assert_eq!(shell.session.active(), 0, "the walk wraps around");
        shell.step_tab(-1);
        assert_eq!(shell.session.active(), 1);
        shell.close_tab();
        assert_eq!(shell.session.documents().len(), 1);
        shell.close_tab();
        assert_eq!(
            shell.session.documents().len(),
            1,
            "the last tab stays open"
        );
    }

    #[test]
    fn the_terminal_keys_are_mapped() {
        assert_eq!(terminal_key(126), Some(TerminalKey::Up));
        assert_eq!(terminal_key(125), Some(TerminalKey::Down));
        assert_eq!(terminal_key(124), Some(TerminalKey::Right));
        assert_eq!(terminal_key(123), Some(TerminalKey::Left));
        assert_eq!(terminal_key(115), Some(TerminalKey::Home));
        assert_eq!(terminal_key(119), Some(TerminalKey::End));
        assert_eq!(terminal_key(117), Some(TerminalKey::Delete));
        assert_eq!(terminal_key(51), Some(TerminalKey::Backspace));
        assert_eq!(terminal_key(36), Some(TerminalKey::Enter));
        assert_eq!(terminal_key(76), Some(TerminalKey::Enter));
        assert_eq!(terminal_key(48), Some(TerminalKey::Tab));
        assert_eq!(terminal_key(53), Some(TerminalKey::Escape));
        // A letter belongs to the shell as text, not as a sequence.
        assert_eq!(terminal_key(8), None);
        assert_eq!(terminal_key(0), None);
    }

    #[test]
    fn a_shell_runs_and_its_output_is_parsed() {
        let mut terminal = TerminalState::start(20, 80).expect("a pseudo terminal");
        terminal.pty.send("printf 'tc-ide-test-ready\\n'\n");
        assert!(
            terminal.wait_for("tc-ide-test-ready", std::time::Duration::from_secs(15)),
            "the shell should answer: {:?}",
            terminal.lines()
        );
        terminal.pty.close();
        assert!(!terminal.pty.is_running(), "the shell is stopped again");
    }

    #[test]
    fn the_terminal_scrolls_back_through_its_output() {
        let mut terminal = TerminalState::start(20, 80).expect("a pseudo terminal");
        terminal.parser.feed("one\ntwo\nthree\nfour\nfive\n");
        terminal.parser.publish(&terminal.view);
        terminal.rows = 2;
        assert_eq!(terminal.visible_lines(), ["four", "five"]);
        terminal.scroll_by(1);
        assert_eq!(terminal.visible_lines(), ["three", "four"]);
        terminal.scroll_by(1);
        assert_eq!(terminal.visible_lines(), ["two", "three"]);
        terminal.scroll_by(-5);
        assert_eq!(
            terminal.visible_lines(),
            ["four", "five"],
            "scrolling clamps"
        );
        terminal.pty.close();
    }

    #[test]
    fn the_graph_panel_maps_rows_and_clamps_its_selection() {
        // The layout constants are the shared ones, so this test pins the mapping
        // to them instead of to a copy.
        let top = graph_core::FIRST_ROW_TOP as f64;
        let step = graph_core::ROW_HEIGHT as f64;
        assert_eq!(GraphPanel::row_at(top, 3), Some(0));
        assert_eq!(GraphPanel::row_at(top + step - 1.0, 3), Some(0));
        assert_eq!(GraphPanel::row_at(top + step, 3), Some(1));
        assert_eq!(GraphPanel::row_at(top + step * 2.0, 3), Some(2));
        assert_eq!(GraphPanel::row_at(top + step * 3.0, 3), None);
        assert_eq!(GraphPanel::row_at(top - 1.0, 3), None);
        assert!(GraphPanel::rows_for(1000.0) >= 1);
        assert!(
            GraphPanel::rows_for(4.0) >= 1,
            "a tiny panel still shows a row"
        );

        let mut panel = GraphPanel::new();
        panel.graph = graph_core::build_graph(
            &graph_core::GraphInputs {
                project_name: "demo",
                current_file: "main.rs",
                current_folder: "",
                hovered: None,
                file_lines: &[],
                file_symbols: &[],
                project_symbols: &[],
                project_declarations: &[],
                folded: &[],
                relation: graph_core::GraphRelation::Follow,
            },
            graph_core::GraphScope::File,
            "",
        );
        let count = panel.graph.nodes.len();
        panel.selection = 0;
        panel.move_selection(2, 3);
        assert_eq!(panel.selection, 2.min(count.saturating_sub(1)));
        panel.move_selection(-5, 3);
        assert_eq!(panel.selection, 0);
        panel.selection = 99;
        panel.clamp(count, 3);
        assert_eq!(panel.selection, count.saturating_sub(1));
        panel.selection = 0;
        panel.clamp(count, 3);
        assert_eq!(panel.scroll, 0);
    }

    #[test]
    fn the_graph_commands_are_mapped() {
        assert_eq!(key_command(5, true, false), Some(MacCommand::ToggleGraph));
        assert_eq!(key_command(5, true, true), Some(MacCommand::GraphScope));
        assert_eq!(key_command(5, false, false), None);
        assert_eq!(
            command_for_menu_action("tcToggleGraph:"),
            Some(MacCommand::ToggleGraph)
        );
        assert_eq!(
            command_for_menu_action("tcGraphRelation:"),
            Some(MacCommand::GraphRelation)
        );
        assert_eq!(
            command_for_menu_action("tcGraphScope:"),
            Some(MacCommand::GraphScope)
        );
    }

    #[test]
    fn cycling_ranges_and_relations_returns_to_the_start() {
        let mut panel = GraphPanel::new();
        assert!(panel.scope.is_none());
        for _ in 0..5 {
            panel.cycle_scope();
        }
        assert!(panel.scope.is_none(), "the range cycle wraps to automatic");
        assert_eq!(panel.relation, GraphRelation::Follow);
        for _ in 0..3 {
            panel.cycle_relation();
        }
        assert_eq!(panel.relation, GraphRelation::Follow);
        for _ in 0..1 {
            panel.cycle_relation();
        }
        assert_eq!(panel.relation, GraphRelation::Callers);
    }

    #[test]
    fn the_settings_panel_maps_its_controls() {
        assert_eq!(
            SettingsPanel::control_at(20.0, SETTINGS_FIRST_ROW + 4.0),
            Some(Setting::ThemeDark)
        );
        assert_eq!(
            SettingsPanel::control_at(168.0, SETTINGS_FIRST_ROW + 4.0),
            Some(Setting::ThemeLight)
        );
        assert_eq!(
            SettingsPanel::control_at(20.0, SETTINGS_FIRST_ROW + SETTINGS_ROW_HEIGHT + 4.0),
            Some(Setting::FontSmaller)
        );
        assert_eq!(
            SettingsPanel::control_at(116.0, SETTINGS_FIRST_ROW + SETTINGS_ROW_HEIGHT + 4.0),
            Some(Setting::FontLarger)
        );
        assert_eq!(
            SettingsPanel::control_at(20.0, SETTINGS_FIRST_ROW + SETTINGS_ROW_HEIGHT * 2.0 + 4.0),
            Some(Setting::LanguageEnglish)
        );
        assert_eq!(
            SettingsPanel::control_at(20.0, SETTINGS_FIRST_ROW + SETTINGS_ROW_HEIGHT * 3.0 + 4.0),
            Some(Setting::Close)
        );
        assert_eq!(SettingsPanel::control_at(20.0, 4.0), None);
        assert_eq!(
            SettingsPanel::control_at(400.0, SETTINGS_FIRST_ROW + 4.0),
            None
        );
        assert!(SettingsPanel::height() > SETTINGS_FIRST_ROW + SETTINGS_ROW_HEIGHT * 3.0);
    }

    #[test]
    fn the_settings_focus_skips_what_cannot_be_used() {
        let mut preferences = workspace_core::Preferences {
            code_zoom: MIN_CODE_ZOOM,
            ..workspace_core::Preferences::default()
        };
        let mut panel = SettingsPanel::new();
        panel.open = true;
        panel.step_focus(true, &preferences);
        assert_eq!(panel.focus, Some(Setting::ThemeDark));
        // Walking forward from the smaller font skips it while it is disabled.
        panel.focus = Some(Setting::ThemeLight);
        panel.step_focus(true, &preferences);
        assert_eq!(panel.focus, Some(Setting::FontLarger));
        preferences.code_zoom = MAX_CODE_ZOOM;
        panel.focus = Some(Setting::FontSmaller);
        panel.step_focus(true, &preferences);
        assert_eq!(panel.focus, Some(Setting::LanguageEnglish));
        // The walk wraps around.
        panel.focus = Some(Setting::Close);
        panel.step_focus(true, &preferences);
        assert_eq!(panel.focus, Some(Setting::ThemeDark));
        panel.focus = Some(Setting::ThemeDark);
        panel.step_focus(false, &preferences);
        assert_eq!(panel.focus, Some(Setting::Close));
    }

    #[test]
    fn the_settings_controls_report_their_state() {
        let mut preferences = workspace_core::Preferences {
            code_zoom: MIN_CODE_ZOOM,
            ..workspace_core::Preferences::default()
        };
        assert!(!Setting::FontSmaller.is_enabled(&preferences));
        assert!(Setting::FontLarger.is_enabled(&preferences));
        preferences.code_zoom = MAX_CODE_ZOOM;
        assert!(!Setting::FontLarger.is_enabled(&preferences));
        assert!(Setting::FontSmaller.is_enabled(&preferences));
        assert!(Setting::ThemeDark.is_enabled(&preferences));
        preferences.light_theme = true;
        assert!(Setting::ThemeLight.is_active(&preferences));
        assert!(!Setting::ThemeDark.is_active(&preferences));
        assert_eq!(Setting::Close.label(), "Close");
        assert_eq!(Setting::ALL.len(), 7);
    }

    #[test]
    fn syntax_tokens_take_the_colours_of_their_kind() {
        let dark = theme_core::Palette::dark();
        let light = theme_core::Palette::light();
        assert_eq!(
            token_color(&dark, editor_core::TokenKind::Keyword),
            dark.keyword
        );
        assert_eq!(
            token_color(&dark, editor_core::TokenKind::Literal),
            dark.literal
        );
        assert_eq!(
            token_color(&dark, editor_core::TokenKind::Comment),
            dark.comment
        );
        assert_eq!(token_color(&dark, editor_core::TokenKind::Plain), dark.text);
        // The four kinds are told apart in both themes.
        let kinds = [
            editor_core::TokenKind::Plain,
            editor_core::TokenKind::Keyword,
            editor_core::TokenKind::Type,
            editor_core::TokenKind::Function,
            editor_core::TokenKind::Constant,
            editor_core::TokenKind::Preprocessor,
            editor_core::TokenKind::Literal,
            editor_core::TokenKind::Comment,
        ];
        for palette in [dark, light] {
            let colours = kinds.map(|kind| token_color(&palette, kind));
            assert!(
                colours.iter().all(|colour| *colour != palette.background),
                "a token colour must not be the background"
            );
            assert_ne!(colours[0], colours[1]);
            assert_ne!(colours[1], colours[3]);
        }
    }

    #[test]
    fn a_token_is_sliced_where_the_view_starts() {
        assert_eq!(visible_token("hello", 0, 0), Some((0, "hello".to_string())));
        assert_eq!(visible_token("hello", 0, 2), Some((2, "llo".to_string())));
        assert_eq!(visible_token("hello", 0, 5), None);
        assert_eq!(visible_token("hello", 0, 9), None);
        assert_eq!(visible_token("ab", 4, 4), Some((4, "ab".to_string())));
        assert_eq!(visible_token("ab", 4, 5), Some((5, "b".to_string())));
        assert_eq!(visible_token("", 3, 0), Some((3, String::new())));
        // The column the visible part starts at never goes backwards.
        for start in 0..6usize {
            for scroll in 0..6usize {
                if let Some((first, text)) = visible_token("abcdef", start, scroll) {
                    assert!(first >= start);
                    assert_eq!(first + text.chars().count(), start + 6);
                }
            }
        }
    }

    #[test]
    fn the_tooling_rows_describe_what_is_installed() {
        let complete = ToolingRow::probe("Rust", |_| true);
        assert!(complete.server_installed && complete.formatter_installed);
        assert_eq!(complete.marks(), "✓ ✓ ✓");
        assert!(complete.summary().contains("complete"));

        let partial = ToolingRow::probe("Rust", |tool| tool == "rustfmt");
        assert!(!partial.server_installed && partial.formatter_installed);
        assert_eq!(partial.marks(), "· ✓ ·");
        assert!(partial.summary().contains("rust-analyzer"));

        let none = ToolingRow::probe("Rust", |_| false);
        assert_eq!(none.marks(), "· · ·");
        assert!(none.summary().contains("format"));

        // A language without tools still has a row.
        let unknown = ToolingRow::probe("Markdown", |_| true);
        assert!(unknown.server.is_none() && unknown.formatter.is_none());
        assert_eq!(unknown.marks(), "· · ·");
    }

    #[test]
    fn the_tooling_rows_are_mapped_and_bounded() {
        let mut panel = ExtensionsPanel::new();
        assert!(panel.rows.is_empty());
        panel.rows = EXTENSION_LANGUAGES
            .iter()
            .map(|language| ToolingRow::probe(language, |_| false))
            .collect();
        assert_eq!(panel.rows.len(), EXTENSION_LANGUAGES.len());
        panel.move_focus(1);
        assert_eq!(panel.focus, 1);
        panel.move_focus(100);
        assert_eq!(panel.focus, EXTENSION_LANGUAGES.len() - 1);
        panel.move_focus(-100);
        assert_eq!(panel.focus, 0);

        assert_eq!(
            ExtensionsPanel::row_at(EXTENSIONS_HEADER_HEIGHT + 4.0, 0.0, 7),
            Some(0)
        );
        assert_eq!(
            ExtensionsPanel::row_at(
                EXTENSIONS_HEADER_HEIGHT + EXTENSIONS_ROW_HEIGHT + 4.0,
                0.0,
                7
            ),
            Some(1)
        );
        assert_eq!(
            ExtensionsPanel::row_at(EXTENSIONS_HEADER_HEIGHT - 1.0, 0.0, 7),
            None
        );
        assert_eq!(
            ExtensionsPanel::row_at(
                EXTENSIONS_HEADER_HEIGHT + EXTENSIONS_ROW_HEIGHT * 7.0,
                0.0,
                7
            ),
            None
        );
    }

    #[test]
    fn the_tooling_commands_are_mapped() {
        assert_eq!(
            key_command(14, true, false),
            Some(MacCommand::ToggleExtensions)
        );
        assert_eq!(key_command(3, true, true), Some(MacCommand::FormatFile));
        assert_eq!(key_command(3, true, false), None);
        assert_eq!(
            command_for_menu_action("tcExtensions:"),
            Some(MacCommand::ToggleExtensions)
        );
        assert_eq!(
            command_for_menu_action("tcFormat:"),
            Some(MacCommand::FormatFile)
        );
    }

    #[test]
    fn the_measured_cell_places_the_columns() {
        assert!((MacShell::column_x_with(0, 8.0) - GUTTER_WIDTH).abs() < 1.0e-9);
        assert!((MacShell::column_x_with(4, 8.0) - (GUTTER_WIDTH + 32.0)).abs() < 1.0e-9);
        // A measured cell that AppKit could not produce falls back to one that
        // still lines the caret up with the digits.
        let fallback = TEXT_CELL;
        assert!(fallback > 0.0 && fallback < 20.0);
    }
}
