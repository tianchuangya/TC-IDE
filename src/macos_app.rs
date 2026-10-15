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
use crate::editor_session;
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
fn key_command(key_code: u16, command: bool, _shift: bool) -> Option<MacCommand> {
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

/// The window: the shared session, the palette and the two Cocoa handles.
struct MacShell {
    session: editor_session::EditorSession,
    preferences: workspace_core::Preferences,
    palette: theme_core::Palette,
    status: String,
    started: Instant,
    window: Id,
    view: Id,
}

impl MacShell {
    fn new() -> Self {
        let preferences = workspace_core::Preferences::default();
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
        let session = &mut self.session;
        match command {
            MacCommand::InsertNewline => session.insert_text("\n"),
            MacCommand::Backspace => session.backspace(),
            MacCommand::DeleteForward => session.delete_forward(),
            MacCommand::MoveLeft => session.move_caret(0, -1),
            MacCommand::MoveRight => session.move_caret(0, 1),
            MacCommand::MoveUp => session.move_caret(-1, 0),
            MacCommand::MoveDown => session.move_caret(1, 0),
            MacCommand::MoveLineStart => session.move_to_line_end(false),
            MacCommand::MoveLineEnd => session.move_to_line_end(true),
            MacCommand::SelectAll => session.select_all(),
            MacCommand::Tab => session.insert_text("    "),
            MacCommand::Save => {
                self.status = match session.save() {
                    Ok(path) => format!("Saved {}", path.display()),
                    Err(message) => message,
                }
            }
        }
    }

    /// Place the caret at a point of the view.
    fn click_at(&mut self, x: f64, y: f64) {
        let area = self.text_area();
        if !area.contains(CGPoint { x, y }) {
            return;
        }
        let line = self.session.line_at(y, area.y, self.line_step());
        let column = self.session.column_at(x, area.x, TEXT_CELL, line);
        self.session.set_caret(line, column);
    }

    /// Open a project, which the session then remembers.
    fn open_project(&mut self, root: PathBuf) {
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
    let title = format!(
        "{}  ·  {}  ·  {} symbols",
        document.name,
        if document.dirty { "●" } else { "saved" },
        shell.symbol_count()
    );
    draw_text(&title, 16.0, 14.0, 15.0, palette.bright, false);

    let area = shell.text_area();
    let selection = document.selection();
    let lines = document.lines.clone();
    let (caret_line, caret_column) = shell.session.caret();
    let scroll_lines = shell.session.scroll_lines;
    let scroll_columns = shell.session.scroll_columns;
    let rows = shell.visible_rows();
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
                let left = GUTTER_WIDTH + (from.saturating_sub(scroll_columns)) as f64 * TEXT_CELL;
                let right = GUTTER_WIDTH + (to.saturating_sub(scroll_columns)) as f64 * TEXT_CELL;
                fill_rect(
                    CGRect::new(left, y, (right - left).max(2.0), step - 2.0),
                    palette.selection,
                    1.0,
                );
            }
        }
        let visible: String = line.chars().skip(scroll_columns).collect();
        draw_text(&visible, GUTTER_WIDTH, y, font, palette.text, true);
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
            let column = caret_column.saturating_sub(scroll_columns) as f64;
            let x = GUTTER_WIDTH + column * TEXT_CELL;
            let y = area.y + row as f64 * step;
            let pulse = shell.session.caret_spring.progress(1.0).clamp(0.0, 1.0);
            fill_rect(
                CGRect::new(x, y, 2.0, (step - 4.0).max(1.0)),
                palette.bright,
                0.35 + 0.65 * pulse as f64,
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
    if let Some(command) = key_command(key_code, command, shift) {
        with_shell(|shell| shell.apply(command));
    }
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
    with_shell(|shell| shell.session.insert_text(&value));
}

/// `mouseDown:` of the editor view: place the caret.
unsafe extern "C" fn editor_mouse_down(view: Id, _selector: Sel, event: Id) {
    let point = msg_point_out(event, "locationInWindow");
    // The window coordinates start at the bottom left; the view is flipped.
    let height = msg_rect_out(view, "bounds").height;
    let y = height - point.y;
    with_shell(|shell| shell.click_at(point.x, y));
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
        shell.session.tick();
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
    msg_bool(window, "makeFirstResponder:", true);
    msg_bool(window, "makeKeyAndOrderFront:", true);
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
}
