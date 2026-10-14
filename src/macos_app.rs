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
use std::ffi::{c_char, c_double, c_long, c_void, CString};
use std::path::PathBuf;
use std::ptr;
use std::time::Instant;

use crate::editor_core;
use crate::motion_core;
use crate::session_core;
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

/// Style mask of a titled, closable, resizable window.
const WINDOW_STYLE: c_long = 1 | 2 | 4 | 8;
/// Backing store kept in a buffer, which is what a resizable window wants.
const BACKING_BUFFERED: c_long = 2;
/// `⌘` in the modifier flags of an event.
const COMMAND_MASK: u64 = 1 << 20;
/// `⇧` in the modifier flags of an event.
const SHIFT_MASK: u64 = 1 << 17;

/// One open document.
struct Document {
    name: String,
    path: Option<PathBuf>,
    buffer: text_buffer::TextBuffer,
    lines: Vec<String>,
    caret_line: usize,
    caret_column: usize,
    selection_anchor: Option<(usize, usize)>,
    dirty: bool,
}

impl Document {
    fn from_lines(name: &str, path: Option<PathBuf>, lines: Vec<String>) -> Self {
        let lines = if lines.is_empty() {
            vec![String::new()]
        } else {
            lines
        };
        Self {
            name: name.to_string(),
            path,
            buffer: text_buffer::TextBuffer::from_lines(&lines),
            lines,
            caret_line: 0,
            caret_column: 0,
            selection_anchor: None,
            dirty: false,
        }
    }

    /// Rebuild the rendered lines from the piece table.
    fn sync(&mut self) {
        self.lines = self.buffer.to_lines();
    }

    fn text(&self) -> String {
        self.lines.join("\n")
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
}

/// Map a key code and modifiers to an editor command.
///
/// Virtual key codes are layout independent on macOS, which is why the mapping
/// is a table instead of a character comparison. `command` is the `⌘` key.
fn key_command(key_code: u16, command: bool, shift: bool) -> Option<MacCommand> {
    let _ = shift;
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
        // ⌘A selects everything and ⌘S saves.
        0 if command => Some(MacCommand::SelectAll),
        1 if command => Some(MacCommand::Save),
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

/// The whole application state of the macOS backend.
struct MacApp {
    documents: Vec<Document>,
    active: usize,
    session: session_core::Session,
    viewport: text_buffer::Viewport,
    preferences: workspace_core::Preferences,
    palette: theme_core::Palette,
    status: String,
    /// Horizontal scroll offset of the code column.
    scroll_columns: usize,
    scroll_lines: usize,
    /// Spring the caret uses, so the shared motion model drives the animation.
    caret_spring: motion_core::Spring,
    started: Instant,
    last_frame: Instant,
    window: Id,
    view: Id,
}

impl MacApp {
    fn new() -> Self {
        let preferences = workspace_core::Preferences::default();
        let session = session_core::Session::default();
        let document = Document::from_lines(
            "main.rs",
            None,
            vec![
                "// TC-IDE on macOS".to_string(),
                "fn main() {".to_string(),
                "    println!(\"hello from Cocoa\");".to_string(),
                "}".to_string(),
            ],
        );
        let started = Instant::now();
        Self {
            documents: vec![document],
            active: 0,
            session,
            viewport: text_buffer::Viewport::new(28, 96),
            palette: theme_core::Palette::for_scheme(preferences.light_theme),
            preferences,
            status: "Ready".into(),
            scroll_columns: 0,
            scroll_lines: 0,
            caret_spring: motion_core::Spring::new(0.0),
            started,
            last_frame: started,
            window: ptr::null_mut(),
            view: ptr::null_mut(),
        }
    }

    fn document(&self) -> &Document {
        let index = self.active.min(self.documents.len().saturating_sub(1));
        &self.documents[index]
    }

    fn document_mut(&mut self) -> &mut Document {
        let index = self.active.min(self.documents.len().saturating_sub(1));
        &mut self.documents[index]
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

    fn view_width(&self) -> f64 {
        self.viewport.width as f64 * 8.4
    }

    fn view_height(&self) -> f64 {
        self.viewport.height as f64 * self.line_step()
    }

    fn mark_dirty(&mut self) {
        self.document_mut().dirty = true;
    }

    /// Lines that fit between the header and the status bar.
    fn visible_rows(&self) -> usize {
        let height = (self.view_height() - self.header_height() - self.status_height()).max(1.0);
        ((height / self.line_step()).floor() as usize).max(1)
    }

    /// Reveal the caret, scrolling the viewport when it leaves it.
    fn reveal_caret(&mut self) {
        let rows = self.visible_rows();
        let line = self.document().caret_line;
        if line < self.scroll_lines {
            self.scroll_lines = line;
        } else if line >= self.scroll_lines + rows {
            self.scroll_lines = line + 1 - rows;
        }
        let column = self.document().caret_column;
        let width = self.viewport.width;
        if column < self.scroll_columns {
            self.scroll_columns = column;
        } else if column >= self.scroll_columns + width {
            self.scroll_columns = column + 1 - width;
        }
    }

    fn insert_text(&mut self, value: &str) {
        if value.is_empty() {
            return;
        }
        let (mut line, mut column) = {
            let document = self.document();
            (document.caret_line, document.caret_column)
        };
        for character in value.chars() {
            if character == '\n' {
                self.document_mut().buffer.insert(line, column, "\n");
                line += 1;
                column = 0;
            } else if !character.is_control() {
                self.document_mut()
                    .buffer
                    .insert(line, column, &character.to_string());
                column += 1;
            }
        }
        let document = self.document_mut();
        document.caret_line = line;
        document.caret_column = column;
        document.selection_anchor = None;
        document.sync();
        self.mark_dirty();
        self.reveal_caret();
    }

    fn backspace(&mut self) {
        let (line, column) = {
            let document = self.document();
            (document.caret_line, document.caret_column)
        };
        if column > 0 {
            self.document_mut()
                .buffer
                .delete((line, column - 1), (line, column));
            self.document_mut().caret_column = column - 1;
        } else if line > 0 {
            let previous = self.document().buffer.line_chars(line - 1);
            self.document_mut()
                .buffer
                .delete((line - 1, previous), (line, 0));
            let document = self.document_mut();
            document.caret_line = line - 1;
            document.caret_column = previous;
        } else {
            return;
        }
        self.document_mut().sync();
        self.mark_dirty();
        self.reveal_caret();
    }

    fn delete_forward(&mut self) {
        let (line, column) = {
            let document = self.document();
            (document.caret_line, document.caret_column)
        };
        let length = self.document().buffer.line_chars(line);
        if column < length {
            self.document_mut()
                .buffer
                .delete((line, column), (line, column + 1));
        } else if line + 1 < self.document().buffer.line_count() {
            self.document_mut()
                .buffer
                .delete((line, length), (line + 1, 0));
        } else {
            return;
        }
        self.document_mut().sync();
        self.mark_dirty();
    }

    fn move_caret(&mut self, delta_line: i64, delta_column: i64) {
        let (line, column) = {
            let document = self.document();
            (document.caret_line as i64, document.caret_column as i64)
        };
        let lines = self.document().lines.len() as i64;
        let line = (line + delta_line).clamp(0, (lines - 1).max(0)) as usize;
        let length = self.document().lines[line].chars().count() as i64;
        let column = if delta_line != 0 {
            // A vertical move keeps the column inside the target line.
            column.min(length)
        } else {
            (column + delta_column).clamp(0, length)
        };
        let document = self.document_mut();
        document.caret_line = line;
        document.caret_column = column as usize;
        document.selection_anchor = None;
        self.reveal_caret();
    }

    fn move_to_line_end(&mut self, end: bool) {
        let line = self.document().caret_line;
        let length = self.document().lines[line].chars().count();
        let document = self.document_mut();
        document.caret_column = if end { length } else { 0 };
        document.selection_anchor = None;
        self.reveal_caret();
    }

    fn select_all(&mut self) {
        let (last_line, last_column) = {
            let document = self.document();
            let last = document.lines.len().saturating_sub(1);
            (last, document.lines[last].chars().count())
        };
        let document = self.document_mut();
        document.selection_anchor = Some((0, 0));
        document.caret_line = last_line;
        document.caret_column = last_column;
    }

    fn save(&mut self) {
        let path = self.document().path.clone();
        let text = self.document().text();
        match path {
            Some(path) => match std::fs::write(&path, format!("{text}\n")) {
                Ok(()) => {
                    self.document_mut().dirty = false;
                    self.status = format!("Saved {}", path.display());
                }
                Err(error) => {
                    self.status = format!("Could not save {}: {error}", path.display());
                }
            },
            None => self.status = "Save the file with Save As first".into(),
        }
    }

    fn open_project(&mut self, root: PathBuf) {
        self.session.set_project(Some(root.clone()));
        self.session.remember_project(&root);
        self.status = format!("Project {}", root.display());
    }

    /// Place the caret at a point of the code area.
    fn click_at(&mut self, x: f64, y: f64) {
        let body = self.body_area();
        if !body.contains(CGPoint { x, y }) {
            return;
        }
        let line_step = self.line_step().max(1.0);
        let line = ((y - body.y) / line_step).floor().max(0.0) as usize;
        let line = (self.scroll_lines + line).min(self.document().lines.len().saturating_sub(1));
        let text_left = 54.0;
        let column = (((x - text_left) / 7.4).floor().max(0.0) as usize + self.scroll_columns)
            .min(self.document().lines[line].chars().count());
        let document = self.document_mut();
        document.caret_line = line;
        document.caret_column = column;
        document.selection_anchor = None;
        self.reveal_caret();
    }

    /// Step the shared motion models once per frame.
    fn tick(&mut self) {
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame).as_secs_f32().min(0.05);
        self.last_frame = now;
        let target = if self.document().dirty { 1.0 } else { 0.0 };
        self.caret_spring.step(target, dt);
    }

    /// Symbols the shared occurrence scan finds in the document.
    fn symbol_count(&self) -> usize {
        let document = self.document();
        editor_core::symbol_occurrences(&document.lines, &document.name).len()
    }

    /// Symbol the caret sits on, when it sits on one.
    fn symbol_at_caret(&self) -> Option<String> {
        let document = self.document();
        editor_core::symbol_at(
            &document.lines,
            &document.name,
            document.caret_line,
            document.caret_column,
        )
        .map(|symbol| symbol.name)
    }

    /// The document area, in top-left coordinates, which is what a flipped view
    /// draws in.
    fn body_area(&self) -> CGRect {
        CGRect::new(
            0.0,
            self.header_height(),
            self.view_width(),
            (self.view_height() - self.header_height() - self.status_height()).max(0.0),
        )
    }
}

/// Apply a command to the application state.
fn apply_command(app: &mut MacApp, command: MacCommand) {
    match command {
        MacCommand::InsertNewline => app.insert_text("\n"),
        MacCommand::Backspace => app.backspace(),
        MacCommand::DeleteForward => app.delete_forward(),
        MacCommand::MoveLeft => app.move_caret(0, -1),
        MacCommand::MoveRight => app.move_caret(0, 1),
        MacCommand::MoveUp => app.move_caret(-1, 0),
        MacCommand::MoveDown => app.move_caret(1, 0),
        MacCommand::MoveLineStart => app.move_to_line_end(false),
        MacCommand::MoveLineEnd => app.move_to_line_end(true),
        MacCommand::SelectAll => app.select_all(),
        MacCommand::Save => app.save(),
        MacCommand::Tab => app.insert_text("    "),
    }
}

thread_local! {
    /// The application state, reached from the Objective-C callbacks.
    static APPLICATION: RefCell<MacApp> = RefCell::new(MacApp::new());
}

/// Run a closure with the application state.
fn with_app<R>(body: impl FnOnce(&mut MacApp) -> R) -> R {
    APPLICATION.with(|cell| body(&mut cell.borrow_mut()))
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

/// A text attribute dictionary for one colour and font size.
///
/// # Safety
/// Builds AppKit objects; call on the main thread.
unsafe fn text_attributes(size: f64, color: u32, monospaced: bool) -> Id {
    let font = if monospaced {
        msg_id_double(
            msg_id_double(class("NSFont"), "monospacedSystemFontOfSize:weight:", size),
            "monospacedSystemFontOfSize:weight:",
            size,
        )
    } else {
        msg_id_double(class("NSFont"), "systemFontOfSize:", size)
    };
    let channel = |shift: u32| ((color >> shift) & 0xff) as f64 / 255.0;
    let color_object = msg_color(channel(16), channel(8), channel(0), 1.0);
    let attributes = msg_id(class("NSMutableDictionary"), "dictionary");
    msg_void_id_id(
        attributes,
        "setObject:forKey:",
        font,
        msg_id(class("NSString"), "stringWithUTF8String:"),
    );
    // The keys are the AppKit ones; the font goes under NSFontAttributeName
    // and the colour under NSForegroundColorAttributeName.
    let font_key = nsstring("NSFont");
    let color_key = nsstring("NSColor");
    msg_void_id_id(attributes, "setObject:forKey:", font, font_key);
    msg_void_id_id(attributes, "setObject:forKey:", color_object, color_key);
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
unsafe fn draw_frame(app: &mut MacApp) {
    let width = app.view_width();
    let height = app.view_height();
    let header = app.header_height();
    let status = app.status_height();
    let step = app.line_step();
    let font = app.code_font_size();
    let palette = app.palette;
    unsafe {
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
    }
    let title = format!(
        "{}  ·  {}  ·  {} symbols",
        app.document().name,
        if app.document().dirty { "●" } else { "saved" },
        app.symbol_count()
    );
    unsafe {
        draw_text(&title, 16.0, 14.0, 15.0, palette.bright, false);
    }
    // The code column, clipped to the rows the viewport shows.
    let body = app.body_area();
    let selection = app.document().selection_anchor.map(|anchor| {
        let document = app.document();
        (anchor, (document.caret_line, document.caret_column))
    });
    let lines = app.document().lines.clone();
    let caret = (app.document().caret_line, app.document().caret_column);
    let scroll_lines = app.scroll_lines;
    let scroll_columns = app.scroll_columns;
    for (row, line) in lines
        .iter()
        .skip(scroll_lines)
        .take(app.visible_rows())
        .enumerate()
    {
        let index = scroll_lines + row;
        let y = body.y + row as f64 * step;
        if let Some(((start_line, start_column), (end_line, end_column))) = selection {
            let selected = if start_line == end_line {
                index == start_line
            } else {
                index >= start_line.min(end_line) && index <= start_line.max(end_line)
            };
            if selected {
                let from = if index == start_line { start_column } else { 0 };
                let to = if index == end_line {
                    end_column
                } else {
                    line.chars().count()
                };
                let left = 54.0 + (from.saturating_sub(scroll_columns)) as f64 * 7.4;
                let right = 54.0 + (to.saturating_sub(scroll_columns)) as f64 * 7.4;
                unsafe {
                    fill_rect(
                        CGRect::new(left, y, (right - left).max(2.0), step - 2.0),
                        palette.selection,
                        1.0,
                    );
                }
            }
        }
        let visible: String = line.chars().skip(scroll_columns).collect();
        unsafe {
            draw_text(&visible, 54.0, y, font, palette.text, true);
            draw_text(
                &format!("{}", index + 1),
                12.0,
                y,
                font - 2.0,
                palette.muted,
                true,
            );
        }
    }
    // The caret, at the column it sits on.
    if caret.0 >= scroll_lines {
        let row = caret.0 - scroll_lines;
        if row < app.visible_rows() {
            let column = caret.1.saturating_sub(scroll_columns) as f64;
            let x = 54.0 + column * 7.4;
            let y = body.y + row as f64 * step;
            let pulse = app.caret_spring.progress(1.0).clamp(0.0, 1.0);
            unsafe {
                fill_rect(
                    CGRect::new(x, y, 2.0, step - 4.0),
                    palette.bright,
                    0.35 + 0.65 * pulse as f64,
                );
            }
        }
    }
    // Status bar, with the shared status text.
    let status_line = format!(
        "{}  ·  {} · ln {}{}",
        app.status,
        if app.document().dirty {
            "unsaved"
        } else {
            "clean"
        },
        app.document().caret_line + 1,
        match app.symbol_at_caret() {
            Some(symbol) => format!(" · {symbol}"),
            None => String::new(),
        }
    );
    unsafe {
        draw_text(
            &status_line,
            12.0,
            height - status + 6.0,
            12.0,
            palette.muted,
            false,
        );
    }
}

/// `drawRect:` of the editor view.
unsafe extern "C" fn editor_draw(_view: Id, _selector: Sel, _rect: CGRect) {
    with_app(|app| unsafe { draw_frame(app) });
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
    with_app(|app| {
        if let Some(command) = key_command(key_code, command, shift) {
            apply_command(app, command);
        }
    });
}

/// `insertText:` of the editor view, which is also how an IME commits text.
unsafe extern "C" fn editor_insert_text(_view: Id, _selector: Sel, text: Id) {
    let bytes: *const c_char = {
        let send: extern "C" fn(Id, Sel) -> *const c_char =
            std::mem::transmute(objc_msgSend as *const c_void);
        send(text, selector_name("UTF8String"))
    };
    if bytes.is_null() {
        return;
    }
    let value = std::ffi::CStr::from_ptr(bytes)
        .to_string_lossy()
        .into_owned();
    with_app(|app| app.insert_text(&value));
}

/// `mouseDown:` of the editor view: place the caret.
unsafe extern "C" fn editor_mouse_down(view: Id, _selector: Sel, event: Id) {
    let point = msg_point_out(event, "locationInWindow");
    // The window coordinates start at the bottom left; the view is flipped.
    let height = msg_rect_out(view, "bounds").height;
    let y = height - point.y;
    with_app(|app| app.click_at(point.x, y));
}

/// Register the editor view class.
fn register_editor_view() {
    unsafe {
        if !class("TcIdeEditorView").is_null() {
            return;
        }
        let name = CString::new("TcIdeEditorView").unwrap_or_default();
        let subclass = objc_allocateClassPair(class("NSView"), name.as_ptr(), 0);
        if subclass.is_null() {
            return;
        }
        let object_argument = CString::new("v@:@").unwrap_or_default();
        let void = CString::new("v@:").unwrap_or_default();
        let rect_argument = CString::new("v@:{CGRect={CGPoint=dd}{CGSize=dd}}").unwrap_or_default();
        let boolean = CString::new("B@:").unwrap_or_default();
        {
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
                selector_name("isFlipped"),
                std::mem::transmute::<unsafe extern "C" fn(Id, Sel) -> bool, Imp>(
                    editor_is_flipped,
                ),
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
            let _ = void;
        }
        objc_registerClassPair(subclass);
    }
}

/// `tick:` of the frame timer: step the motion and redraw.
unsafe extern "C" fn timer_tick(_timer: Id, _selector: Sel) {
    let view = with_app(|app| {
        app.tick();
        app.view
    });
    if !view.is_null() {
        unsafe {
            msg_bool(view, "setNeedsDisplay:", true);
        }
    }
}

/// Build the window, its editor view and the timer that drives the frames.
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
    msg_bool(window, "makeFirstResponder:", true);
    msg_bool(window, "makeKeyAndOrderFront:", true);
    with_app(|app| {
        app.window = window;
        app.view = view;
        app.viewport = text_buffer::Viewport::new(
            ((height - app.header_height() - app.status_height()) / app.line_step())
                .floor()
                .max(1.0) as usize,
            ((width - 64.0) / 7.4).floor().max(8.0) as usize,
        );
    });
    // A 60 Hz timer steps the shared motion models and asks for a redraw.
    let timer = msg_timer(1.0 / 60.0, view);
    let _ = timer;
    let _ = boxed_tick;
}

/// The tick IMP as an object, kept alive for the timer.
static mut TICK_TARGET: Id = ptr::null_mut();

/// `tick` is registered on a small helper object so `NSTimer` can call it.
unsafe extern "C" fn boxed_tick(_target: Id, _selector: Sel) {
    unsafe { timer_tick(_target, _selector) };
}

/// Schedule a repeating timer that calls `boxed_tick`.
///
/// # Safety
/// Call on the main thread.
unsafe fn msg_timer(interval: f64, view: Id) -> Id {
    let name = CString::new("TcIdeTickTarget").unwrap_or_default();
    if class("TcIdeTickTarget").is_null() {
        let subclass = objc_allocateClassPair(class("NSObject"), name.as_ptr(), 0);
        if !subclass.is_null() {
            let void = CString::new("v@:").unwrap_or_default();
            class_addMethod(
                subclass,
                selector_name("tick:"),
                std::mem::transmute::<unsafe extern "C" fn(Id, Sel), Imp>(boxed_tick),
                void.as_ptr(),
            );
            objc_registerClassPair(subclass);
        }
    }
    let target = msg_id(class("TcIdeTickTarget"), "new");
    let send: extern "C" fn(Id, Sel, f64, Id, Sel, Id, bool) -> Id =
        std::mem::transmute(objc_msgSend as *const c_void);
    let timer = send(
        class("NSTimer"),
        selector_name("scheduledTimerWithTimeInterval:target:selector:userInfo:repeats:"),
        interval,
        target,
        selector_name("tick:"),
        ptr::null_mut(),
        true,
    );
    let _ = view;
    TICK_TARGET = target;
    timer
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
/// macOS and not only compiled.
fn self_check() -> i32 {
    let mut failures = Vec::new();
    let mut checks = 0usize;
    let mut report = |name: &str, ok: bool, failures: &mut Vec<String>| {
        checks += 1;
        if !ok {
            failures.push(name.to_string());
        }
    };

    let mut app = MacApp::new();
    app.insert_text("one");
    app.insert_text("\n");
    app.insert_text("two");
    report(
        "the piece table takes typed text",
        app.document().lines == ["one", "two"],
        &mut failures,
    );
    app.backspace();
    report(
        "backspace removes a character",
        app.document().lines == ["one", "tw"],
        &mut failures,
    );
    app.move_caret(-1, 0);
    app.move_to_line_end(true);
    app.insert_text("X");
    report(
        "the caret moves between lines",
        app.document().lines == ["oneX", "tw"],
        &mut failures,
    );
    report(
        "an edit marks the document dirty",
        app.document().dirty,
        &mut failures,
    );
    app.select_all();
    report(
        "select all covers the document",
        app.document().selection_anchor == Some((0, 0)),
        &mut failures,
    );
    report(
        "the palette comes from the shared theme",
        app.palette == theme_core::Palette::for_scheme(app.preferences.light_theme),
        &mut failures,
    );
    app.open_project(PathBuf::from("/tmp/tc-ide-demo"));
    report(
        "the session remembers the project",
        app.session.project().is_some(),
        &mut failures,
    );
    app.click_at(60.0, app.header_height() + app.line_step() * 0.5);
    report(
        "clicking places the caret in the first row",
        app.document().caret_line == 0,
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
    fn the_editor_keeps_its_document_consistent() {
        let mut app = MacApp::new();
        app.insert_text("hello");
        assert_eq!(
            app.document().lines,
            [
                "hello",
                "fn main() {",
                "    println!(\"hello from Cocoa\");",
                "}"
            ]
        );
        app.insert_text("\nworld");
        assert_eq!(app.document().lines[..2], ["hello", "world"]);
        app.move_caret(-1, 0);
        app.move_to_line_end(true);
        app.insert_text("!");
        assert_eq!(app.document().lines[0], "hello!");
        app.delete_forward();
        assert_eq!(app.document().lines[1], "orld");
        app.move_to_line_end(false);
        app.backspace();
        assert_eq!(app.document().lines[0], "ello!");
        app.select_all();
        app.insert_text("reset");
        assert_eq!(app.document().lines, ["reset"]);
        app.move_caret(0, 0);
        app.move_to_line_end(true);
        for _ in 0..4 {
            app.backspace();
        }
        assert_eq!(app.document().lines[0], "r");
        app.backspace();
        assert_eq!(app.document().lines[0], "");
        app.backspace();
        assert_eq!(
            app.document().lines[0],
            "",
            "backspace at the start of the document does nothing"
        );
    }

    #[test]
    fn clicks_land_on_the_row_and_column_they_point_at() {
        let mut app = MacApp::new();
        app.viewport = text_buffer::Viewport::new(20, 80);
        let header = app.header_height();
        app.click_at(54.0 + 7.4 * 3.0, header + app.line_step() * 2.0 + 1.0);
        assert_eq!(app.document().caret_line, 2);
        assert_eq!(app.document().caret_column, 3);
        // A click in the header or the status bar is not a caret move.
        app.click_at(54.0, 4.0);
        assert_eq!(app.document().caret_line, 2);
    }
}
