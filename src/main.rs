#![allow(clippy::too_many_arguments)]

use std::ffi::{c_void, CString};
use std::collections::BTreeSet;
use std::fs;
use std::os::raw::{c_char, c_int, c_long, c_uint, c_ulong};
use std::path::PathBuf;
use std::process::Command;
use std::ptr;
use std::thread;
use std::time::{Duration, Instant};

type Window = c_ulong;
type Drawable = c_ulong;
type Colormap = c_ulong;
type Atom = c_ulong;
type Gc = *mut c_void;
type Pixmap = c_ulong;

#[repr(C)]
struct Display {
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
    fn XCreateGC(display: *mut Display, drawable: Drawable, mask: c_ulong, values: *mut c_void) -> Gc;
    fn XCreatePixmap(display: *mut Display, drawable: Drawable, width: c_uint, height: c_uint, depth: c_uint) -> Pixmap;
    fn XFreePixmap(display: *mut Display, pixmap: Pixmap) -> c_int;
    fn XCopyArea(display: *mut Display, source: Drawable, destination: Drawable, gc: Gc, source_x: c_int, source_y: c_int, width: c_uint, height: c_uint, destination_x: c_int, destination_y: c_int) -> c_int;
    fn XFreeGC(display: *mut Display, gc: Gc) -> c_int;
    fn XSetForeground(display: *mut Display, gc: Gc, foreground: c_ulong) -> c_int;
    fn XSetLineAttributes(display: *mut Display, gc: Gc, width: c_uint, style: c_int, cap: c_int, join: c_int) -> c_int;
    fn XFillRectangle(display: *mut Display, drawable: Drawable, gc: Gc, x: c_int, y: c_int, width: c_uint, height: c_uint) -> c_int;
    fn XDrawRectangle(display: *mut Display, drawable: Drawable, gc: Gc, x: c_int, y: c_int, width: c_uint, height: c_uint) -> c_int;
    fn XDrawLine(display: *mut Display, drawable: Drawable, gc: Gc, x1: c_int, y1: c_int, x2: c_int, y2: c_int) -> c_int;
    fn XFillArc(display: *mut Display, drawable: Drawable, gc: Gc, x: c_int, y: c_int, width: c_uint, height: c_uint, angle1: c_int, angle2: c_int) -> c_int;
    fn XFillPolygon(display: *mut Display, drawable: Drawable, gc: Gc, points: *mut XPoint, count: c_int, shape: c_int, mode: c_int) -> c_int;
    fn XDrawArc(display: *mut Display, drawable: Drawable, gc: Gc, x: c_int, y: c_int, width: c_uint, height: c_uint, angle1: c_int, angle2: c_int) -> c_int;
    fn XDrawString(display: *mut Display, drawable: Drawable, gc: Gc, x: c_int, y: c_int, text: *const c_char, length: c_int) -> c_int;
    fn XLoadQueryFont(display: *mut Display, name: *const c_char) -> *mut XFontStruct;
    fn XSetFont(display: *mut Display, gc: Gc, font: c_ulong) -> c_int;
    fn XFreeFont(display: *mut Display, font: *mut XFontStruct) -> c_int;
    fn XAllocNamedColor(display: *mut Display, map: Colormap, name: *const c_char, screen: *mut XColor, exact: *mut XColor) -> c_int;
    fn XPending(display: *mut Display) -> c_int;
    fn XNextEvent(display: *mut Display, event: *mut XEvent) -> c_int;
    fn XLookupString(event: *mut XKeyEvent, buffer: *mut c_char, length: c_int, keysym: *mut c_ulong, compose: *mut c_void) -> c_int;
    fn XInternAtom(display: *mut Display, name: *const c_char, only_if_exists: c_int) -> Atom;
    fn XSetWMProtocols(display: *mut Display, window: Window, protocols: *mut Atom, count: c_int) -> c_int;
    fn XFlush(display: *mut Display) -> c_int;
    fn XDestroyWindow(display: *mut Display, window: Window) -> c_int;
    fn XCloseDisplay(display: *mut Display) -> c_int;
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
const ESCAPE_KEYSYM: c_ulong = 0xff1b;
const BACKSPACE_KEYSYM: c_ulong = 0xff08;
const RETURN_KEYSYM: c_ulong = 0xff0d;
const TAB_KEYSYM: c_ulong = 0xff09;
const LEFT_KEYSYM: c_ulong = 0xff51;
const UP_KEYSYM: c_ulong = 0xff52;
const RIGHT_KEYSYM: c_ulong = 0xff53;
const DOWN_KEYSYM: c_ulong = 0xff54;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Level {
    Project,
    File,
    Class,
    Function,
}

struct Node {
    label: &'static str,
    detail: &'static str,
    level: Level,
    nx: f32,
    ny: f32,
}

struct FileBuffer {
    name: String,
    lines: Vec<String>,
}

struct ParsedComment<'a> {
    code: &'a str,
    text: &'a str,
    content_column: usize,
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
enum CreateKind { Project, Folder, File }

const NODES: &[Node] = &[
    Node { label: "TC-IDE", detail: "project", level: Level::Project, nx: 0.50, ny: 0.48 },
    Node { label: "graph.rs", detail: "src/editor", level: Level::File, nx: 0.24, ny: 0.25 },
    Node { label: "workspace.rs", detail: "src", level: Level::File, nx: 0.72, ny: 0.22 },
    Node { label: "render.rs", detail: "src", level: Level::File, nx: 0.77, ny: 0.70 },
    Node { label: "GraphController", detail: "class", level: Level::Class, nx: 0.15, ny: 0.51 },
    Node { label: "build_graph()", detail: "fn - L42", level: Level::Function, nx: 0.30, ny: 0.76 },
    Node { label: "analyze_workspace()", detail: "fn - L18", level: Level::Function, nx: 0.90, ny: 0.39 },
    Node { label: "resolve_symbols()", detail: "fn - L67", level: Level::Function, nx: 0.58, ny: 0.08 },
    Node { label: "render_graph()", detail: "fn - L12", level: Level::Function, nx: 0.93, ny: 0.86 },
];

const EDGES: &[(usize, usize)] = &[
    (0, 1), (0, 2), (0, 3),
    (1, 4), (4, 5),
    (2, 6), (2, 7),
    (3, 8),
    (5, 6), (5, 7), (5, 8),
];

struct Palette {
    bg: c_ulong,
    panel: c_ulong,
    panel_2: c_ulong,
    line: c_ulong,
    muted: c_ulong,
    text: c_ulong,
    bright: c_ulong,
    accent: c_ulong,
    project: c_ulong,
}

struct App {
    display: *mut Display,
    window: Window,
    gc: Gc,
    canvas: Pixmap,
    depth: c_int,
    font: *mut XFontStruct,
    colors: Palette,
    width: i32,
    height: i32,
    mouse_x: i32,
    mouse_y: i32,
    hovered: Option<usize>,
    hovered_from_code: bool,
    selected_file: usize,
    files: Vec<FileBuffer>,
    folders: Vec<String>,
    project_name: String,
    project_path: Option<PathBuf>,
    editor_focused: bool,
    caret_line: usize,
    caret_column: usize,
    graph_zoom: f32,
    package_open: bool,
    package_targets: [bool; 4],
    build_started: Option<Instant>,
    new_menu_open: bool,
    extensions_open: bool,
    languages: Vec<LanguagePack>,
    downloading: Option<(usize, Instant)>,
    extension_scroll: usize,
    extension_search: String,
    extension_search_focused: bool,
    selected_language: Option<usize>,
    version_menu_open: bool,
    create_kind: Option<CreateKind>,
    new_file_name: String,
    project_language: usize,
    project_environment: usize,
    project_language_menu: bool,
    project_environment_menu: bool,
    expanded_comments: BTreeSet<(usize,usize)>,
    status_message: String,
    started: Instant,
}

impl App {
    fn scale(&self) -> f32 {
        (self.width as f32 / 1440.0).min(self.height as f32 / 900.0).clamp(0.85, 2.0)
    }

    fn s(&self, value: i32) -> i32 { (value as f32 * self.scale()).round() as i32 }

    fn language_for_file(&self,name:&str)->String {
        let extension=name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
        if let Some(pack)=self.languages.iter().find(|pack|pack.extensions.iter().any(|item|*item==extension)){if pack.installed{pack.name.to_string()}else{format!("Plain Text - {} available",pack.name)}}else{"Plain Text".into()}
    }

    fn language_pack_index_for_file(&self,name:&str)->Option<usize>{let extension=name.rsplit('.').next()?.to_ascii_lowercase();self.languages.iter().position(|pack|pack.extensions.iter().any(|item|*item==extension))}

    fn completion_words(&self)->&'static [&'static str]{let language=self.language_for_file(&self.files[self.selected_file].name);if language.starts_with("Rust"){&["async","await","const","enum","fn","impl","let","match","mod","pub","Result","self","struct","trait","use"]}else if language.starts_with("Python"){&["async","await","class","def","elif","else","from","import","lambda","None","return","self","try","with"]}else if language.starts_with("TypeScript")||language.starts_with("Web"){&["async","await","class","const","export","extends","function","import","interface","return","type"]}else if language=="C"||language.starts_with("C++"){&["auto","class","const","constexpr","enum","include","namespace","return","struct","template","void"]}else if language.starts_with("Go"){&["chan","defer","func","go","import","interface","map","package","range","return","struct"]}else if language.starts_with("Java")||language.starts_with("Kotlin"){&["class","extends","fun","import","interface","new","package","private","public","return","static"]}else{&["class","const","function","import","return","struct","type","value"]}}

    fn current_prefix(&self)->String{let line=&self.files[self.selected_file].lines[self.caret_line];let upto=&line[..self.caret_column.min(line.len())];upto.chars().rev().take_while(|ch|ch.is_ascii_alphanumeric()||*ch=='_').collect::<String>().chars().rev().collect()}

    fn completions(&self)->Vec<String>{let prefix=self.current_prefix();if prefix.is_empty(){return Vec::new();}self.completion_words().iter().filter(|word|word.starts_with(&prefix)&&**word!=prefix).take(4).map(|word|word.to_string()).collect()}

    fn accept_completion(&mut self){if let Some(candidate)=self.completions().first().cloned(){let prefix=self.current_prefix();let suffix=&candidate[prefix.len()..];self.files[self.selected_file].lines[self.caret_line].insert_str(self.caret_column,suffix);self.caret_column+=suffix.len();self.status_message=format!("Completed {candidate}");}}

    fn comment_parts<'a>(&self,line:&'a str)->Option<ParsedComment<'a>>{let file=&self.files[self.selected_file].name;let marker=comment_markers_for_file(file).iter().filter_map(|marker|line.find(marker).map(|index|(index,*marker))).min_by_key(|(index,_)|*index)?;if marker.1=="#"&&marker.0==0&&line.starts_with("#!"){return None;}let after=&line[marker.0+marker.1.len()..];let leading=after.len()-after.trim_start().len();let mut text=after.trim();if marker.1=="/*"{text=text.strip_suffix("*/").unwrap_or(text).trim_end();}if marker.1=="<!--"{text=text.strip_suffix("-->").unwrap_or(text).trim_end();}Some(ParsedComment{code:&line[..marker.0],text,content_column:marker.0+marker.1.len()+leading})}

    fn filtered_language_indices(&self)->Vec<usize>{let needle=self.extension_search.to_ascii_lowercase();self.languages.iter().enumerate().filter(|(_,pack)|needle.is_empty()||pack.name.to_ascii_lowercase().contains(&needle)||pack.extensions.iter().any(|ext|ext.contains(&needle))).map(|(index,_)|index).collect()}

    unsafe fn set_color(&self, color: c_ulong) {
        XSetForeground(self.display, self.gc, color);
    }

    unsafe fn rect(&self, color: c_ulong, x: i32, y: i32, width: i32, height: i32) {
        if width <= 0 || height <= 0 { return; }
        self.set_color(color);
        XFillRectangle(self.display, self.canvas, self.gc, x, y, width as u32, height as u32);
    }

    unsafe fn outline(&self, color: c_ulong, x: i32, y: i32, width: i32, height: i32) {
        self.set_color(color);
        XDrawRectangle(self.display, self.canvas, self.gc, x, y, width.max(1) as u32, height.max(1) as u32);
    }

    unsafe fn line(&self, color: c_ulong, width: u32, a: (i32, i32), b: (i32, i32)) {
        self.set_color(color);
        XSetLineAttributes(self.display, self.gc, width, 0, 1, 1);
        XDrawLine(self.display, self.canvas, self.gc, a.0, a.1, b.0, b.1);
    }

    unsafe fn circle(&self, fill: c_ulong, stroke: c_ulong, x: i32, y: i32, radius: i32) {
        self.set_color(fill);
        XFillArc(self.display, self.canvas, self.gc, x-radius, y-radius, (radius*2) as u32, (radius*2) as u32, 0, 360*64);
        self.set_color(stroke);
        XSetLineAttributes(self.display, self.gc, 1, 0, 1, 1);
        XDrawArc(self.display, self.canvas, self.gc, x-radius, y-radius, (radius*2) as u32, (radius*2) as u32, 0, 360*64);
    }

    fn comment_bubble_layout(&self,anchor_x:i32,y:i32,value:&str,expanded:bool,min_left:i32)->BubbleLayout{let s=|v|self.s(v);let right=anchor_x-s(9);let available=(right-min_left).max(s(72));let chars_per_line=((available-s(24))/s(11).max(1)).clamp(6,34) as usize;let lines=wrap_bubble_text(value,chars_per_line,if expanded{8}else{1});let longest=lines.iter().map(|line|line.chars().count()).max().unwrap_or(7) as i32;let width=(longest*s(11)+s(24)).max(s(112)).min(available);let height=s(31)+lines.len() as i32*s(22);BubbleLayout{left:right-width,top:y-s(21),width,height,lines,chars_per_line}}

    unsafe fn comment_bubble(&self,anchor_x:i32,y:i32,value:&str,expanded:bool,editing:bool,min_left:i32){let s=|v|self.s(v);let layout=self.comment_bubble_layout(anchor_x,y,value,expanded,min_left);self.rect(self.colors.bright,layout.left,layout.top,layout.width,layout.height);self.line(self.colors.line,1,(layout.left,layout.top+s(27)),(layout.left+layout.width,layout.top+s(27)));let tail_center=layout.top+s(14);let right=layout.left+layout.width;let mut tail=[XPoint{x:anchor_x as i16,y:tail_center as i16},XPoint{x:right as i16,y:(tail_center-s(7)) as i16},XPoint{x:right as i16,y:(tail_center+s(7)) as i16}];self.set_color(self.colors.bright);XFillPolygon(self.display,self.canvas,self.gc,tail.as_mut_ptr(),3,0,0);self.text(self.colors.bg,layout.left+s(9),layout.top+s(20),if expanded{"v"}else{">"});self.text(self.colors.bg,layout.left+s(29),layout.top+s(20),if editing{"EDITING"}else{"COMMENT"});self.text(self.colors.bg,layout.left+layout.width-s(48),layout.top+s(20),"EDIT");for (index,line) in layout.lines.iter().enumerate(){self.text(self.colors.bg,layout.left+s(12),layout.top+s(48)+index as i32*s(22),line);}}

    unsafe fn text(&self, color: c_ulong, x: i32, y: i32, value: &str) {
        let clean = value.replace('\0', "");
        let value = CString::new(clean).expect("text contains no NUL");
        self.set_color(color);
        XDrawString(self.display, self.canvas, self.gc, x, y, value.as_ptr(), value.as_bytes().len() as i32);
    }

    fn graph_bounds(&self) -> (i32, i32, i32, i32) {
        let left = (self.width as f32 * 0.61) as i32;
        (left, self.s(82), self.width-left, self.height-self.s(112))
    }

    fn node_position(&self, node: &Node) -> (i32, i32) {
        let (x, y, w, h) = self.graph_bounds();
        let cx=x+w/2;let cy=y+h/2;
        (cx+((node.nx-0.5)*w as f32*self.graph_zoom) as i32,cy+((node.ny-0.5)*h as f32*self.graph_zoom) as i32)
    }

    fn is_related(&self, index: usize) -> bool {
        match self.hovered {
            None => true,
            Some(active) => active == index || EDGES.iter().any(|&(a,b)| (a == active && b == index) || (b == active && a == index)),
        }
    }

    unsafe fn draw(&self) {
        self.rect(self.colors.bg, 0, 0, self.width, self.height);
        self.draw_header();
        self.draw_workspace();
        self.draw_editor();
        self.draw_graph();
        if self.hovered_from_code { self.draw_code_relations(); }
        if self.new_menu_open { self.draw_new_menu(); }
        if self.package_open { self.draw_package_menu(); }
        if self.extensions_open { self.draw_extensions(); }
        if self.create_kind.is_some() { self.draw_create_dialog(); }
        if self.build_started.is_some() { self.draw_build_animation(); }
        self.draw_status();
        XCopyArea(self.display,self.canvas,self.window,self.gc,0,0,self.width as u32,self.height as u32,0,0);
        XFlush(self.display);
    }

    unsafe fn draw_header(&self) {
        let s=|v|self.s(v);let header=s(46);
        self.rect(self.colors.panel_2,0,0,self.width,header);
        self.line(self.colors.line,1,(0,header-1),(self.width,header-1));
        self.rect(self.colors.bright,s(14),s(12),s(22),s(22));
        self.text(self.colors.bg,s(20),s(29),"T");
        self.text(self.colors.text,s(46),s(29),"TC-IDE");
        self.outline(if self.new_menu_open{self.colors.accent}else{self.colors.line},s(112),s(10),s(88),s(27));
        self.text(self.colors.text,s(130),s(29),"New  v");
        let path=format!("{}  /  {}",self.project_name,self.files[self.selected_file].name);let path_x=self.width/2-path.chars().count() as i32*s(5);self.text(self.colors.muted,path_x.max(s(220)),s(29),&path);
        self.outline(if self.extensions_open{self.colors.accent}else{self.colors.line},self.width-s(275),s(10),s(126),s(27));
        self.text(self.colors.text,self.width-s(260),s(29),"Extensions");
        self.outline(if self.package_open{self.colors.accent}else{self.colors.line},self.width-s(136),s(10),s(118),s(27));
        self.text(self.colors.text,self.width-s(119),s(29),"Package  v");
    }

    unsafe fn draw_workspace(&self) {
        let s=|v|self.s(v);
        let width = (self.width as f32 * 0.17) as i32;
        self.rect(self.colors.panel,0,s(46),width,self.height-s(70));
        self.line(self.colors.line,1,(width,s(46)),(width,self.height-s(24)));
        self.text(self.colors.muted,s(14),s(71),"PROJECT");
        self.text(self.colors.bright,s(15),s(101),&format!("v  {}",self.project_name));
        let mut row_y=134;for folder in &self.folders{self.text(self.colors.text,s(16),s(row_y),&format!("  v  {folder}"));row_y+=28;}
        for (index,file) in self.files.iter().enumerate(){let y=row_y+index as i32*29;if s(y)>self.height-s(65){break;}if index==self.selected_file{self.rect(self.colors.panel_2,s(7),s(y-21),width-s(14),s(27));}let color=if index==self.selected_file{self.colors.bright}else{self.colors.muted};self.text(color,s(16),s(y),&format!("       {}",file.name));
        }
    }

    unsafe fn draw_new_menu(&self){let s=|v|self.s(v);let x=s(112);let y=s(43);let w=s(220);let h=s(126);self.rect(self.colors.panel_2,x,y,w,h);self.outline(self.colors.line,x,y,w,h);self.text(self.colors.muted,x+s(15),y+s(23),"CREATE");for (index,label) in ["New Project","New File","New Folder"].iter().enumerate(){let row=y+s(50+index as i32*30);self.text(self.colors.text,x+s(18),row,label);self.text(self.colors.muted,x+w-s(28),row,"+");}}

    unsafe fn draw_editor(&self) {
        let s=|v|self.s(v);
        let left = (self.width as f32 * 0.17) as i32;
        let right = (self.width as f32 * 0.61) as i32;
        self.rect(self.colors.bg,left+1,s(46),right-left-1,self.height-s(70));
        self.rect(self.colors.panel_2,left+1,s(46),right-left-1,s(38));
        self.line(self.colors.line,1,(left,s(84)),(right,s(84)));
        self.text(self.colors.bright,left+s(18),s(71),&self.files[self.selected_file].name);
        let language=self.language_for_file(&self.files[self.selected_file].name);
        self.text(self.colors.muted,right-s(150),s(71),&language);
        let base_y=s(119);let step=s(28);
        let show_bubbles=!(self.mouse_x>=0&&self.mouse_x<left);
        for (index,line) in self.files[self.selected_file].lines.iter().enumerate() {
            let y=base_y+index as i32*step;if y>self.height-s(65){break;}
            self.text(self.colors.line,left+s(15),y,&(index+1).to_string());
            let color=if line.contains("fn "){self.colors.accent}else if line.contains("struct ")||line.starts_with("impl"){self.colors.bright}else{self.colors.text};
            if let Some(comment)=self.comment_parts(line){self.text(color,left+s(52),y,comment.code);let raw_anchor=left+s(62)+(comment.code.chars().count() as i32*s(10));let bubble_x=if comment.code.trim().is_empty(){left+s(350)}else{raw_anchor};if show_bubbles{let expanded=self.expanded_comments.contains(&(self.selected_file,index));let editing=self.editor_focused&&self.caret_line==index&&self.caret_column>=comment.content_column;self.comment_bubble(bubble_x,y,comment.text,expanded,editing,left+s(18));}else{self.text(self.colors.accent,raw_anchor,y,"[comment]");}}else{self.text(color,left+s(52),y,line);}
            if self.editor_focused&&index==self.caret_line&&(self.started.elapsed().as_millis()/500)%2==0 {
                let char_width=s(10);let visual_column=self.files[self.selected_file].lines[index][..self.caret_column].chars().count() as i32;let caret_x=left+s(52)+visual_column*char_width;
                self.line(self.colors.bright,1,(caret_x,y-s(18)),(caret_x,y+s(4)));
            }
        }
        let hint=if self.editor_focused{"EDITOR ACTIVE - type, Backspace, Enter, arrows"}else{"Click editor to type - hover symbols to trace"};
        self.text(self.colors.muted,left+s(20),self.height-s(50),hint);
        if self.editor_focused {let completions=self.completions();if !completions.is_empty(){let popup_x=left+s(52)+self.caret_column as i32*s(10);let popup_y=(base_y+(self.caret_line as i32+1)*step).min(self.height-s(180));let popup_w=s(210);let popup_h=s(30)*completions.len() as i32+self.s(10);self.rect(self.colors.panel_2,popup_x,popup_y,popup_w,popup_h);self.outline(self.colors.line,popup_x,popup_y,popup_w,popup_h);for (index,item) in completions.iter().enumerate(){if index==0{self.rect(self.colors.project,popup_x+s(3),popup_y+s(3),popup_w-s(6),s(27));}self.text(if index==0{self.colors.bright}else{self.colors.text},popup_x+s(12),popup_y+s(23)+index as i32*s(30),item);}self.text(self.colors.muted,popup_x+popup_w-s(44),popup_y+s(23),"TAB");}}
    }

    unsafe fn draw_graph(&self) {
        let s=|v|self.s(v);
        let (x,y,w,h)=self.graph_bounds();
        self.rect(self.colors.panel,x,s(46),w,self.height-s(70));
        self.line(self.colors.line,1,(x,s(46)),(x,self.height-s(24)));
        self.line(self.colors.line,1,(x,s(84)),(self.width,s(84)));
        self.text(self.colors.text,x+s(14),s(71),"Semantic Project Graph");
        let mode = self.hovered.map(|i| match NODES[i].level { Level::Project=>"PROJECT",Level::File=>"FILE",Level::Class=>"CLASS",Level::Function=>"FUNCTION" }).unwrap_or("AUTO");
        self.text(self.colors.muted,self.width-s(112),s(71),mode);

        for &(a,b) in EDGES {
            let related = self.hovered.map(|active| active==a || active==b).unwrap_or(true);
            let color = if related { self.colors.accent } else { self.colors.line };
            self.line(color, if related { 2 } else { 1 }, self.node_position(&NODES[a]), self.node_position(&NODES[b]));
        }

        let elapsed=self.started.elapsed().as_secs_f32();
        for (index,node) in NODES.iter().enumerate() {
            let (nx,ny)=self.node_position(node);
            let active=self.hovered==Some(index);
            let related=self.is_related(index);
            let base=s(match node.level { Level::Project=>14,Level::File=>9,Level::Class=>7,Level::Function=>5 });
            let pulse=if active { ((elapsed*5.0).sin().abs()*s(4) as f32) as i32 } else { 0 };
            if active { self.circle(self.colors.panel, self.colors.accent, nx, ny, base+pulse+5); }
            let stroke=if active { self.colors.bright } else if related { self.colors.accent } else { self.colors.line };
            let fill=if node.level==Level::Project { self.colors.project } else { self.colors.panel_2 };
            self.circle(fill, stroke, nx, ny, base);
            let label_color=if related { self.colors.text } else { self.colors.muted };
            self.text(label_color,nx+base+s(6),ny-s(2),node.label);
            self.text(self.colors.muted,nx+base+s(6),ny+s(14),node.detail);
        }
        self.text(self.colors.muted,x+s(14),y+h-s(10),"wheel: zoom - project -> file -> class -> function / call");
    }

    unsafe fn draw_code_relations(&self){let Some(active)=self.hovered else{return;};let s=|v|self.s(v);let editor_right=(self.width as f32*0.61) as i32;let start=(self.mouse_x.max((self.width as f32*0.17) as i32+s(50)),self.mouse_y);let junction=(editor_right-s(18),start.1);self.circle(self.colors.panel_2,self.colors.bright,start.0,start.1,s(4));self.line(self.colors.accent,2,start,junction);let mut targets=vec![active];for &(from,to) in EDGES{if from==active&&!targets.contains(&to){targets.push(to);}}for target in targets{let end=self.node_position(&NODES[target]);let bend=(editor_right+s(18),end.1);self.line(self.colors.accent,2,junction,bend);self.line(self.colors.accent,2,bend,(end.0-s(8),end.1));let mut arrow=[XPoint{x:end.0 as i16,y:end.1 as i16},XPoint{x:(end.0-s(10)) as i16,y:(end.1-s(5)) as i16},XPoint{x:(end.0-s(10)) as i16,y:(end.1+s(5)) as i16}];self.set_color(self.colors.accent);XFillPolygon(self.display,self.canvas,self.gc,arrow.as_mut_ptr(),3,0,0);}let travel=((self.started.elapsed().as_secs_f32()*1.8)%1.0).clamp(0.0,1.0);let pulse_x=start.0+((junction.0-start.0) as f32*travel) as i32;self.circle(self.colors.bright,self.colors.accent,pulse_x,start.1,s(3));}

    unsafe fn draw_status(&self) {
        let s=|v|self.s(v);self.rect(self.colors.panel_2,0,self.height-s(24),self.width,s(24));
        self.line(self.colors.line,1,(0,self.height-s(24)),(self.width,self.height-s(24)));
        self.text(self.colors.muted,s(12),self.height-s(7),"TC-IDE native prototype");
        let status=self.hovered.map(|i| format!("Focused: {} - {}",NODES[i].label,NODES[i].detail)).unwrap_or_else(||self.status_message.clone());
        self.text(self.colors.text,self.width/2-s(100),self.height-s(7),&status);
    }

    unsafe fn draw_package_menu(&self) {
        let s=|v|self.s(v);let x=self.width-s(270);let y=s(48);let w=s(250);let h=s(207);
        self.rect(self.colors.panel_2,x,y,w,h);self.outline(self.colors.line,x,y,w,h);
        self.text(self.colors.muted,x+s(15),y+s(25),"BUILD TARGETS");
        let labels=["Windows  .exe","Linux    .deb","Linux    .AppImage","macOS    .dmg"];
        for (index,label) in labels.iter().enumerate(){let row_y=y+s(51+index as i32*31);self.outline(if self.package_targets[index]{self.colors.accent}else{self.colors.line},x+s(15),row_y-s(15),s(16),s(16));if self.package_targets[index]{self.rect(self.colors.accent,x+s(19),row_y-s(11),s(8),s(8));}self.text(self.colors.text,x+s(42),row_y,label);}
        self.rect(self.colors.bright,x+s(15),y+h-s(37),w-s(30),s(25));self.text(self.colors.bg,x+s(72),y+h-s(19),"BUILD SELECTED");
    }

    unsafe fn draw_extensions(&self) {
        let s=|v|self.s(v);let w=s(780);let h=s(680);let x=self.width-w-s(20);let y=s(52);
        self.rect(self.colors.panel_2,x,y,w,h);self.outline(self.colors.line,x,y,w,h);
        self.text(self.colors.bright,x+s(22),y+s(34),"EXTENSIONS");
        self.text(self.colors.accent,x+s(22),y+s(67),"Programming Languages");
        self.rect(self.colors.bg,x+s(20),y+s(82),s(330),s(38));self.outline(if self.extension_search_focused{self.colors.accent}else{self.colors.line},x+s(20),y+s(82),s(330),s(38));self.text(self.colors.text,x+s(35),y+s(108),if self.extension_search.is_empty(){"Search languages or extensions"}else{&self.extension_search});
        let filtered=self.filtered_language_indices();let visible_count=10;let start=self.extension_scroll.min(filtered.len().saturating_sub(1));
        for (visible,index) in filtered.iter().skip(start).take(visible_count).enumerate(){let row=y+s(153+visible as i32*47);let selected=self.selected_language==Some(*index);if selected{self.rect(self.colors.project,x+s(20),row-s(27),s(318),s(39));}self.text(if selected{self.colors.bright}else{self.colors.text},x+s(32),row,self.languages[*index].name);if self.languages[*index].installed{self.text(self.colors.accent,x+s(250),row,"INSTALLED");}}
        let track_x=x+s(344);let track_y=y+s(135);let track_h=s(472);self.rect(self.colors.line,track_x,track_y,s(3),track_h);if !filtered.is_empty(){let thumb_h=(track_h*visible_count as i32/filtered.len().max(visible_count) as i32).max(s(34));let max_scroll=filtered.len().saturating_sub(visible_count).max(1) as i32;let thumb_y=track_y+(track_h-thumb_h)*self.extension_scroll.min(max_scroll as usize) as i32/max_scroll;self.rect(self.colors.accent,track_x-s(2),thumb_y,s(7),thumb_h);}
        self.line(self.colors.line,1,(x+s(365),y+s(74)),(x+s(365),y+h-s(24)));
        if let Some(index)=self.selected_language {let pack=&self.languages[index];let dx=x+s(392);self.text(self.colors.bright,dx,y+s(112),pack.name);self.text(self.colors.muted,dx,y+s(145),&format!("Files: {}",pack.extensions.iter().map(|ext|format!(".{ext}")).collect::<Vec<_>>().join("  ")));self.text(self.colors.muted,dx,y+s(190),"VERSION");self.rect(self.colors.bg,dx,y+s(207),s(330),s(40));self.outline(if self.version_menu_open{self.colors.accent}else{self.colors.line},dx,y+s(207),s(330),s(40));self.text(self.colors.text,dx+s(14),y+s(234),pack.versions[pack.selected_version]);self.text(self.colors.muted,dx+s(300),y+s(234),"v");let downloading=self.downloading.map(|(active,_)|active==index).unwrap_or(false);let action=if downloading{"DOWNLOADING..."}else if pack.installed{"REMOVE"}else{"DOWNLOAD"};self.rect(self.colors.bright,dx,y+s(282),s(330),s(38));self.text(self.colors.bg,dx+s(105),y+s(308),action);let state=if pack.installed{format!("Installed: {} {}",pack.name,pack.versions[pack.selected_version])}else{"Select a version, then download this language pack".into()};self.text(self.colors.muted,dx,y+s(353),&state);self.text(self.colors.muted,dx,y+s(395),"Language pack layers:");self.text(self.colors.text,dx+s(16),y+s(425),"- syntax grammar and file detection");self.text(self.colors.text,dx+s(16),y+s(455),"- completion and formatter adapter");self.text(self.colors.text,dx+s(16),y+s(485),"- LSP / debugger integration point");if self.version_menu_open{self.rect(self.colors.panel,dx,y+s(250),s(330),s(34)*pack.versions.len() as i32);for (version,value) in pack.versions.iter().enumerate(){let vy=y+s(274+version as i32*34);self.text(if version==pack.selected_version{self.colors.accent}else{self.colors.text},dx+s(14),vy,value);}}}else{self.text(self.colors.muted,x+s(420),y+s(180),"Choose a language from the first layer.");}
        self.text(self.colors.muted,x+s(22),y+h-s(18),"Search + wheel scrolling - unknown extensions use Plain Text");
    }

    unsafe fn draw_create_dialog(&self) {
        let s=|v|self.s(v);let project=self.create_kind==Some(CreateKind::Project);let w=s(if project{680}else{520});let h=s(if project{390}else{176});let x=(self.width-w)/2;let y=(self.height-h)/2;
        self.rect(self.colors.panel_2,x,y,w,h);self.outline(self.colors.accent,x,y,w,h);
        let title=match self.create_kind.unwrap(){CreateKind::Project=>"CREATE NEW PROJECT",CreateKind::Folder=>"CREATE NEW FOLDER",CreateKind::File=>"CREATE NEW FILE"};self.text(self.colors.bright,x+s(22),y+s(36),title);
        self.text(self.colors.muted,x+s(22),y+s(66),if project{"Choose a language and isolated environment"}else if self.create_kind==Some(CreateKind::File){"Extension automatically selects its language pack"}else{"Folder will be created inside the active project"});
        self.rect(self.colors.bg,x+s(22),y+s(82),w-s(44),s(40));self.outline(self.colors.line,x+s(22),y+s(82),w-s(44),s(40));
        let placeholder=if project{"project-name"}else if self.create_kind==Some(CreateKind::File){"filename.ext"}else{"folder-name"};self.text(self.colors.text,x+s(35),y+s(109),if self.new_file_name.is_empty(){placeholder}else{&self.new_file_name});
        if project {let pack=&self.languages[self.project_language];let envs=project_environments(pack.name);self.text(self.colors.muted,x+s(22),y+s(157),"LANGUAGE");self.rect(self.colors.bg,x+s(22),y+s(171),s(300),s(40));self.outline(if self.project_language_menu{self.colors.accent}else{self.colors.line},x+s(22),y+s(171),s(300),s(40));self.text(self.colors.text,x+s(36),y+s(198),pack.name);self.text(self.colors.muted,x+s(294),y+s(198),"v");self.text(self.colors.muted,x+s(350),y+s(157),"ENVIRONMENT / TOOLCHAIN");self.rect(self.colors.bg,x+s(350),y+s(171),s(308),s(40));self.outline(if self.project_environment_menu{self.colors.accent}else{self.colors.line},x+s(350),y+s(171),s(308),s(40));self.text(self.colors.text,x+s(364),y+s(198),envs[self.project_environment.min(envs.len()-1)]);self.text(self.colors.muted,x+s(630),y+s(198),"v");self.text(self.colors.muted,x+s(22),y+s(250),&format!("Location: ~/TC-IDE-Projects/{}",if self.new_file_name.is_empty(){"project-name"}else{&self.new_file_name}));self.text(self.colors.text,x+s(22),y+s(286),"Python venv creates a real .venv; other choices store their toolchain preset.");if self.project_language_menu{self.rect(self.colors.panel,x+s(22),y+s(214),s(300),s(34)*8);for (visible,index) in self.languages.iter().enumerate().take(8){self.text(if visible==self.project_language{self.colors.accent}else{self.colors.text},x+s(36),y+s(238+visible as i32*34),index.name);}}if self.project_environment_menu{self.rect(self.colors.panel,x+s(350),y+s(214),s(308),s(34)*envs.len() as i32);for (index,env) in envs.iter().enumerate(){self.text(if index==self.project_environment{self.colors.accent}else{self.colors.text},x+s(364),y+s(238+index as i32*34),env);}}}
        self.text(self.colors.muted,x+s(22),y+h-s(18),"Enter: create   Esc: cancel");
    }

    unsafe fn draw_build_animation(&self) {
        let s=|v|self.s(v);let elapsed=self.build_started.unwrap().elapsed().as_secs_f32();let cx=self.width/2;let cy=self.height/2;
        if elapsed<2.4{let t=(elapsed/2.4).clamp(0.0,1.0);if t>0.72{let left=(self.width as f32*0.17) as i32;let right=(self.width as f32*0.61) as i32;self.rect(self.colors.bg,left+s(1),s(85),right-left-s(1),self.height-s(109));}let sources=[(s(250),s(160),"fn build_graph()"),(s(420),s(310),"resolve_symbols(files)"),(self.width-s(460),s(220),"GraphController"),(self.width-s(330),self.height-s(320),"render_graph(nodes)"),(s(310),self.height-s(260),"workspace.index()")];for (index,(sx,sy,label)) in sources.iter().enumerate(){let local=(t+index as f32*0.035).min(1.0);let px=*sx+((cx-*sx) as f32*local) as i32;let py=*sy+((cy-*sy) as f32*local) as i32;let color=if local>0.82{self.colors.line}else{self.colors.text};self.text(color,px,py,label);self.line(self.colors.accent,1,(px,py+s(5)),(cx,cy));}let pulse=((elapsed*7.0).sin().abs()*s(8) as f32) as i32;self.circle(self.colors.bg,self.colors.accent,cx,cy,s(34)+pulse);self.circle(self.colors.bg,self.colors.line,cx,cy,s(53)-pulse/2);self.text(self.colors.muted,cx-s(104),cy+s(105),"CODE IS COLLAPSING INTO THE BUILD CORE");}
        else{let t=((elapsed-2.4)/2.2).clamp(0.0,1.0);let radius=s(28)+(t*s(24) as f32) as i32;self.circle(self.colors.bright,self.colors.accent,cx,cy,radius);self.circle(self.colors.panel,self.colors.bright,cx,cy,radius+s(13));let labels=[(".exe",-190,-85),(".deb",180,-80),(".AppImage",-210,90),(".dmg",185,95)];for (index,(label,dx,dy)) in labels.iter().enumerate(){if !self.package_targets[index]{continue;}let spread=((t-0.08*index as f32).max(0.0)).min(1.0);let px=cx+(*dx as f32*spread) as i32;let py=cy+(*dy as f32*spread) as i32;self.line(self.colors.accent,2,(cx,cy),(px,py));self.rect(self.colors.bright,px-s(42),py-s(17),s(84),s(31));self.text(self.colors.bg,px-s(28),py+s(5),label);}self.text(self.colors.bright,cx-s(106),cy+s(150),if t<0.85{"WHITE HOLE IS EMITTING PACKAGES"}else{"BUILD COMPLETE"});}
    }

    unsafe fn resize(&mut self,width:i32,height:i32){
        self.width=width.max(900);self.height=height.max(600);XFreePixmap(self.display,self.canvas);
        self.canvas=XCreatePixmap(self.display,self.window,self.width as u32,self.height as u32,self.depth as u32);
        self.update_hover();
    }

    fn update_background(&mut self){if let Some((index,started))=self.downloading{if started.elapsed()>Duration::from_millis(900){self.languages[index].installed=true;self.status_message=format!("{} {} language pack installed",self.languages[index].name,self.languages[index].versions[self.languages[index].selected_version]);save_language_settings(&self.languages);self.downloading=None;}}if self.build_started.map(|started|started.elapsed()>Duration::from_millis(5000)).unwrap_or(false){self.build_started=None;self.status_message="Build animation complete - selected packages emitted".into();}}

    fn active_root(&mut self)->PathBuf {if let Some(path)=&self.project_path{return path.clone();}let base=projects_root().join("TC-IDE-Scratch");let _=fs::create_dir_all(&base);self.project_path=Some(base.clone());base}

    fn create_requested_item(&mut self){let name=self.new_file_name.trim().to_string();if name.is_empty()||name.contains('/')||name.contains('\\')||name=="."||name==".."{self.status_message="Use a safe non-empty name".into();return;}match self.create_kind.unwrap(){CreateKind::File=>{if self.files.iter().any(|file|file.name==name){self.status_message="A file with that name already exists".into();return;}let root=self.active_root();let _=fs::create_dir_all(&root);let _=fs::write(root.join(&name),"");let pack_index=self.language_pack_index_for_file(&name);self.files.push(FileBuffer{name:name.clone(),lines:vec![String::new()]});self.selected_file=self.files.len()-1;self.caret_line=0;self.caret_column=0;self.editor_focused=true;if let Some(index)=pack_index{let language=self.languages[index].name;if self.languages[index].installed{self.status_message=format!("Created {name} - selected {language} {}",self.languages[index].versions[self.languages[index].selected_version]);}else{self.downloading=Some((index,Instant::now()));self.status_message=format!("Detected {language} - downloading selected language pack...");}}else{self.status_message=format!("Created {name} - Plain Text fallback");}},CreateKind::Folder=>{if !self.folders.contains(&name){let root=self.active_root();match fs::create_dir_all(root.join(&name)){Ok(_)=>{self.folders.push(name.clone());self.status_message=format!("Created folder {name}");},Err(error)=>self.status_message=format!("Could not create folder: {error}")}}},CreateKind::Project=>self.create_project(&name)}self.create_kind=None;self.project_language_menu=false;self.project_environment_menu=false;}

    fn create_project(&mut self,name:&str){let root=projects_root().join(name);if root.exists(){self.status_message=format!("Project {name} already exists; choose another name");return;}if let Err(error)=fs::create_dir_all(&root){self.status_message=format!("Could not create project: {error}");return;}let language=self.languages[self.project_language].name;let envs=project_environments(language);let environment=envs[self.project_environment.min(envs.len()-1)];let version=self.languages[self.project_language].versions[self.languages[self.project_language].selected_version];let manifest=format!("name={name}\nlanguage={language}\nversion={version}\nenvironment={environment}\n");let _=fs::write(root.join(".tc-ide-project"),manifest);let (file_name,content,folders):(String,String,Vec<String>)=if language=="Python"{if environment=="venv"{let _=Command::new("python3").args(["-m","venv",".venv"]).current_dir(&root).spawn();}("main.py".into(),format!("# {name} - {environment}\n\nprint(\"Hello from TC-IDE\")\n"),vec![".venv".to_string()])}else if language=="Rust"{let src=root.join("src");let _=fs::create_dir_all(&src);let _=fs::write(root.join("Cargo.toml"),format!("[package]\nname = \"{}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",name.to_ascii_lowercase().replace(' ',"-")));let content="fn main() {\n    println!(\"Hello from TC-IDE\");\n}\n".to_string();let _=fs::write(src.join("main.rs"),&content);("main.rs".into(),content,vec!["src".to_string()])}else{let extension=self.languages[self.project_language].extensions.first().copied().unwrap_or("txt");let file=if language=="Java"{"Main.java".to_string()}else{format!("main.{extension}")};let content=format!("// {name} - {language} {version} - {environment}\n");let _=fs::write(root.join(&file),&content);(file,content,Vec::new())};if language=="Python"{let _=fs::write(root.join(&file_name),&content);}self.project_name=name.to_string();self.project_path=Some(root.clone());self.folders=folders;self.files=vec![FileBuffer{name:file_name,lines:content.lines().map(str::to_string).chain(std::iter::once(String::new())).collect()}];self.selected_file=0;self.caret_line=0;self.caret_column=0;self.status_message=format!("Project {name} created with {language} {version} / {environment}");}

    fn handle_click(&mut self,x:i32,y:i32,button:u32){
        let scale=self.scale();let s=move|v:i32|(v as f32*scale).round() as i32;
        let graph_left=(self.width as f32*0.61) as i32;
        if let Some(kind)=self.create_kind {if kind==CreateKind::Project {let w=s(680);let h=s(390);let dx=(self.width-w)/2;let dy=(self.height-h)/2;if self.project_language_menu {for visible in 0..8.min(self.languages.len()){let row=dy+s(238+visible as i32*34);if (y-row).abs()<s(16){self.project_language=visible;self.project_environment=0;self.project_language_menu=false;return;}}}if self.project_environment_menu {let envs=project_environments(self.languages[self.project_language].name);for index in 0..envs.len(){let row=dy+s(238+index as i32*34);if (y-row).abs()<s(16){self.project_environment=index;self.project_environment_menu=false;return;}}}if x>=dx+s(22)&&x<=dx+s(322)&&y>=dy+s(171)&&y<=dy+s(211){self.project_language_menu=!self.project_language_menu;self.project_environment_menu=false;return;}if x>=dx+s(350)&&x<=dx+s(658)&&y>=dy+s(171)&&y<=dy+s(211){self.project_environment_menu=!self.project_environment_menu;self.project_language_menu=false;return;}}return;}
        if self.extensions_open&&(button==4||button==5){let filtered=self.filtered_language_indices();let visible=10;let maximum=filtered.len().saturating_sub(visible);if button==4{self.extension_scroll=self.extension_scroll.saturating_sub(1);}else{self.extension_scroll=(self.extension_scroll+1).min(maximum);}return;}
        if x>=graph_left&&button==4 {self.graph_zoom=(self.graph_zoom*1.1).min(1.55);return;}
        if x>=graph_left&&button==5 {self.graph_zoom=(self.graph_zoom/1.1).max(0.65);return;}
        if x>=s(112)&&x<=s(200)&&y<s(46){self.new_menu_open=!self.new_menu_open;self.extensions_open=false;self.package_open=false;self.editor_focused=false;return;}
        if self.new_menu_open{let menu_x=s(112);let menu_y=s(43);if x>=menu_x&&x<=menu_x+s(220)&&y>=menu_y&&y<=menu_y+s(126){for (index,kind) in [CreateKind::Project,CreateKind::File,CreateKind::Folder].iter().enumerate(){let row=menu_y+s(50+index as i32*30);if (y-row).abs()<=s(14){self.create_kind=Some(*kind);self.new_file_name.clear();self.new_menu_open=false;return;}}return;}self.new_menu_open=false;}
        if x>self.width-s(290)&&x<self.width-s(145)&&y<s(46){self.extensions_open=!self.extensions_open;self.new_menu_open=false;self.package_open=false;self.editor_focused=false;return;}
        if x>self.width-s(150)&&y<s(46){self.package_open=!self.package_open;self.new_menu_open=false;self.extensions_open=false;self.editor_focused=false;return;}
        if self.extensions_open {
            let w=s(780);let h=s(680);let menu_x=self.width-w-s(20);let menu_y=s(52);
            if x>=menu_x&&x<=menu_x+w&&y>=menu_y&&y<=menu_y+h {if x>=menu_x+s(20)&&x<=menu_x+s(350)&&y>=menu_y+s(82)&&y<=menu_y+s(120){self.extension_search_focused=true;return;}let filtered=self.filtered_language_indices();for visible in 0..10{let position=self.extension_scroll+visible;if position>=filtered.len(){break;}let index=filtered[position];let row=menu_y+s(153+visible as i32*47);if x<menu_x+s(350)&&(y-row).abs()<s(20){self.selected_language=Some(index);self.version_menu_open=false;self.extension_search_focused=false;return;}}if let Some(index)=self.selected_language{let dx=menu_x+s(392);if self.version_menu_open{let versions=self.languages[index].versions;for version in 0..versions.len(){let row=menu_y+s(274+version as i32*34);if x>=dx&&x<=dx+s(330)&&(y-row).abs()<s(16){if self.languages[index].selected_version!=version&&self.languages[index].installed{self.languages[index].installed=false;save_language_settings(&self.languages);self.status_message="Version changed; download the selected language pack".into();}self.languages[index].selected_version=version;self.version_menu_open=false;return;}}}if x>=dx&&x<=dx+s(330)&&y>=menu_y+s(207)&&y<=menu_y+s(247){self.version_menu_open=!self.version_menu_open;return;}if x>=dx&&x<=dx+s(330)&&y>=menu_y+s(282)&&y<=menu_y+s(320){if self.languages[index].installed{self.languages[index].installed=false;self.status_message=format!("{} language pack removed",self.languages[index].name);save_language_settings(&self.languages);}else{self.downloading=Some((index,Instant::now()));self.status_message=format!("Downloading {} {}...",self.languages[index].name,self.languages[index].versions[self.languages[index].selected_version]);}return;}}return;}
            self.extensions_open=false;
            self.extension_search_focused=false;
        }
        if self.package_open {
            let menu_x=self.width-s(270);let menu_y=s(48);
            if x>=menu_x&&x<=self.width-s(20)&&y>=menu_y {
                for index in 0..4 {let center=menu_y+s(51+index as i32*31);if (y-center).abs()<s(15){self.package_targets[index]=!self.package_targets[index];return;}}
                if y>menu_y+s(170)&&self.package_targets.iter().any(|v|*v){self.build_started=Some(Instant::now());self.status_message="Building selected targets...".into();self.package_open=false;return;}
            }
            self.package_open=false;
        }
        let workspace_right=(self.width as f32*0.17) as i32;
        if x>=workspace_right&&x<graph_left&&button==1{let base=s(119);let step=s(28);for index in 0..self.files[self.selected_file].lines.len(){let y_line=base+index as i32*step;let Some(comment)=self.comment_parts(&self.files[self.selected_file].lines[index]) else{continue;};let text=comment.text.to_string();let content_column=comment.content_column;let raw_anchor=workspace_right+s(62)+comment.code.chars().count() as i32*s(10);let anchor=if comment.code.trim().is_empty(){workspace_right+s(350)}else{raw_anchor};let expanded=self.expanded_comments.contains(&(self.selected_file,index));let layout=self.comment_bubble_layout(anchor,y_line,&text,expanded,workspace_right+s(18));if x>=layout.left&&x<=layout.left+layout.width&&y>=layout.top&&y<=layout.top+layout.height{if y<=layout.top+s(27)&&x<=layout.left+s(28){let key=(self.selected_file,index);if !self.expanded_comments.remove(&key){self.expanded_comments.insert(key);}return;}self.expanded_comments.insert((self.selected_file,index));self.editor_focused=true;self.caret_line=index;if y>layout.top+s(27){let visual_line=((y-layout.top-s(38))/s(22).max(1)).max(0) as usize;let visual_column=((x-layout.left-s(12))/s(11).max(1)).max(0) as usize;let char_offset=(visual_line*layout.chars_per_line+visual_column).min(text.chars().count());self.caret_column=content_column+char_to_byte_index(&text,char_offset);}else{self.caret_column=content_column+text.len();}self.status_message="Comment editing active - click text to place the caret".into();return;}}}
        if x<workspace_right {
            let file_start=134+self.folders.len() as i32*28;for index in 0..self.files.len(){let center=file_start+index as i32*29;if (y-s(center)).abs()<s(15){self.selected_file=index;self.caret_line=0;self.caret_column=0;self.editor_focused=false;self.hovered=if index<3{Some(index+1)}else{None};self.hovered_from_code=false;return;}}
        }
        if x>=workspace_right&&x<graph_left&&y>s(85)&&y<self.height-s(60){
            self.editor_focused=true;let base=s(119);let step=s(28);let line=((y-base+step/2)/step).max(0) as usize;self.caret_line=line.min(self.files[self.selected_file].lines.len().saturating_sub(1));let text_x=workspace_right+s(52);let visual_column=(((x-text_x).max(0))/s(10).max(1)) as usize;self.caret_column=char_to_byte_index(&self.files[self.selected_file].lines[self.caret_line],visual_column);return;
        }
        self.editor_focused=false;
    }

    unsafe fn handle_key(&mut self,event:&mut XKeyEvent)->bool {
        let mut keysym=0;let mut buffer=[0_i8;32];let count=XLookupString(event,buffer.as_mut_ptr(),buffer.len() as i32,&mut keysym,ptr::null_mut());
        if self.extension_search_focused {if keysym==ESCAPE_KEYSYM{self.extension_search_focused=false;return true;}if keysym==BACKSPACE_KEYSYM{self.extension_search.pop();self.extension_scroll=0;return true;}if keysym==RETURN_KEYSYM{self.extension_search_focused=false;return true;}if count>0{let bytes=buffer[..count as usize].iter().map(|v|*v as u8).collect::<Vec<_>>();if let Ok(input)=std::str::from_utf8(&bytes){if !input.chars().any(char::is_control){self.extension_search.push_str(input);self.extension_scroll=0;}}}return true;}
        if self.create_kind.is_some() {if keysym==ESCAPE_KEYSYM{if self.project_language_menu||self.project_environment_menu{self.project_language_menu=false;self.project_environment_menu=false;}else{self.create_kind=None;}return true;}if keysym==BACKSPACE_KEYSYM{self.new_file_name.pop();return true;}if keysym==RETURN_KEYSYM{self.create_requested_item();return true;}if count>0{let bytes=buffer[..count as usize].iter().map(|v|*v as u8).collect::<Vec<_>>();if let Ok(input)=std::str::from_utf8(&bytes){if !input.chars().any(char::is_control){self.new_file_name.push_str(input);}}}return true;}
        if keysym==ESCAPE_KEYSYM {if self.extensions_open||self.new_menu_open||self.package_open||self.build_started.is_some(){self.extensions_open=false;self.new_menu_open=false;self.version_menu_open=false;self.package_open=false;self.build_started=None;return true;}return false;}
        if !self.editor_focused{return true;}
        match keysym {
            TAB_KEYSYM=>self.accept_completion(),
            BACKSPACE_KEYSYM=>{if self.caret_column>0 {let line=&mut self.files[self.selected_file].lines[self.caret_line];let previous=previous_char_boundary(line,self.caret_column);line.replace_range(previous..self.caret_column,"");self.caret_column=previous;}else if self.caret_line>0{let current=self.files[self.selected_file].lines.remove(self.caret_line);self.caret_line-=1;self.caret_column=self.files[self.selected_file].lines[self.caret_line].len();self.files[self.selected_file].lines[self.caret_line].push_str(&current);}},
            RETURN_KEYSYM=>{let tail=self.files[self.selected_file].lines[self.caret_line].split_off(self.caret_column);self.caret_line+=1;self.caret_column=0;self.files[self.selected_file].lines.insert(self.caret_line,tail);},
            LEFT_KEYSYM=>self.caret_column=previous_char_boundary(&self.files[self.selected_file].lines[self.caret_line],self.caret_column),
            RIGHT_KEYSYM=>self.caret_column=next_char_boundary(&self.files[self.selected_file].lines[self.caret_line],self.caret_column),
            UP_KEYSYM=>{let visual=self.files[self.selected_file].lines[self.caret_line][..self.caret_column].chars().count();self.caret_line=self.caret_line.saturating_sub(1);self.caret_column=char_to_byte_index(&self.files[self.selected_file].lines[self.caret_line],visual);},
            DOWN_KEYSYM=>{let visual=self.files[self.selected_file].lines[self.caret_line][..self.caret_column].chars().count();self.caret_line=(self.caret_line+1).min(self.files[self.selected_file].lines.len()-1);self.caret_column=char_to_byte_index(&self.files[self.selected_file].lines[self.caret_line],visual);},
            _=>if count>0 {let bytes=buffer[..count as usize].iter().map(|v|*v as u8).collect::<Vec<_>>();if let Ok(input)=std::str::from_utf8(&bytes){if !input.chars().any(char::is_control){self.files[self.selected_file].lines[self.caret_line].insert_str(self.caret_column,input);self.caret_column+=input.len();}}}
        }
        true
    }

    fn update_hover(&mut self) {
        let scale=self.scale();let s=move|v:i32|(v as f32*scale).round() as i32;let left=(self.width as f32*0.17) as i32;let graph=(self.width as f32*0.61) as i32;
        if self.mouse_x>=left&&self.mouse_x<graph&&self.mouse_y>s(90){let row=((self.mouse_y-s(119)+s(14))/s(28).max(1)).max(0) as usize;if let Some(line)=self.files[self.selected_file].lines.get(row){self.hovered=if line.contains("build_graph"){Some(5)}else if line.contains("analyze_workspace"){Some(6)}else if line.contains("resolve_symbols"){Some(7)}else if line.contains("render_graph"){Some(8)}else{None};self.hovered_from_code=self.hovered.is_some();return;}}
        self.hovered_from_code=false;
        let mut best=None;
        let mut distance=self.s(40) as i64*self.s(40) as i64;
        for (index,node) in NODES.iter().enumerate() {
            let (x,y)=self.node_position(node);
            let dx=(self.mouse_x-x) as i64;
            let dy=(self.mouse_y-y) as i64;
            let candidate=dx*dx+dy*dy;
            if candidate<distance { distance=candidate;best=Some(index); }
        }
        self.hovered=best;
    }
}

unsafe fn color(display: *mut Display, map: Colormap, value: &str) -> c_ulong {
    let name=CString::new(value).unwrap();
    let mut screen=XColor { pixel:0,red:0,green:0,blue:0,flags:0,pad:0 };
    let mut exact=screen;
    if XAllocNamedColor(display,map,name.as_ptr(),&mut screen,&mut exact)==0 { 0 } else { screen.pixel }
}

fn initial_files() -> Vec<FileBuffer> {
    let sources=[
        ("graph.rs",["use tc_graph::{Project, Relation};","","// AI maps project relations in real time","pub struct GraphController {","    project: Project,","}","","impl GraphController {","    pub fn build_graph(&mut self) {","        let files = analyze_workspace(&self.project);","        let symbols = resolve_symbols(files);","        render_graph(self.project.root(), symbols);","    }","}"].as_slice()),
        ("workspace.rs",["use crate::workspace::Project;","","pub fn analyze_workspace(project: &Project) {","    project.index_files();","}","","pub fn resolve_symbols(files: Vec<File>) {","    SemanticIndex::from(files);","}"].as_slice()),
        ("render.rs",["use crate::graph::SymbolNode;","","pub struct GraphRenderer;","","impl GraphRenderer {","    pub fn render_graph(nodes: &[SymbolNode]) {","        scene.draw(nodes);","    }","}"].as_slice()),
    ];
    sources.iter().map(|(name,lines)|FileBuffer{name:(*name).into(),lines:lines.iter().map(|line|line.to_string()).collect()}).collect()
}

fn language_settings_path()->PathBuf {std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).or_else(||std::env::var_os("HOME").map(|home|PathBuf::from(home).join(".config"))).unwrap_or_else(||PathBuf::from(".")).join("tc-ide/languages.conf")}

fn projects_root()->PathBuf {std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(||PathBuf::from(".")).join("TC-IDE-Projects")}

fn comment_markers_for_file(name:&str)->&'static [&'static str]{let extension=name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();match extension.as_str(){"py"|"pyw"|"rb"|"sh"|"bash"|"zsh"|"fish"|"r"=>&["#"],"sql"|"lua"|"hs"|"lhs"=>&["--"],"html"|"htm"|"md"|"mdx"=>&["<!--"],"css"|"scss"|"sass"=>&["/*"],"c"|"h"|"cc"|"cpp"|"cxx"|"hpp"|"rs"|"java"|"kt"|"kts"|"swift"|"go"|"cs"|"ts"|"tsx"|"js"|"jsx"|"mjs"|"dart"|"php"|"scala"|"vue"|"svelte"=>&["//","/*"],_=>&[]}}

fn char_to_byte_index(value:&str,character:usize)->usize{value.char_indices().nth(character).map(|(index,_)|index).unwrap_or(value.len())}

fn previous_char_boundary(value:&str,index:usize)->usize{value[..index.min(value.len())].char_indices().last().map(|(position,_)|position).unwrap_or(0)}

fn next_char_boundary(value:&str,index:usize)->usize{if index>=value.len(){return value.len();}value[index..].char_indices().nth(1).map(|(offset,_)|index+offset).unwrap_or(value.len())}

fn wrap_bubble_text(value:&str,max_chars:usize,max_lines:usize)->Vec<String>{let source=if value.trim().is_empty(){"comment"}else{value.trim()};let mut lines=Vec::new();for paragraph in source.lines(){let chars=paragraph.chars().collect::<Vec<_>>();if chars.is_empty(){lines.push(String::new());continue;}for chunk in chars.chunks(max_chars.max(1)){lines.push(chunk.iter().collect());}}if lines.len()>max_lines{lines.truncate(max_lines);if let Some(last)=lines.last_mut(){if last.chars().count()>=3{let keep=last.chars().take(max_chars.saturating_sub(3)).collect::<String>();*last=format!("{keep}...");}}}lines}

fn project_environments(language:&str)->&'static [&'static str]{match language{"Python"=>&["venv","uv","conda","poetry"],"C"=>&["GCC","Clang","CMake"],"C++"=>&["CMake + GCC","CMake + Clang","Meson","Conan"],"Java"=>&["JDK 25","JDK 21 LTS","Gradle","Maven"],"Rust"=>&["cargo stable","cargo nightly","Nix shell"],"TypeScript / JavaScript"=>&["pnpm","npm","yarn","bun"],"Go"=>&["Go modules","GVM","container"],_=>&["system toolchain","container","custom"]}}

fn initial_languages()->Vec<LanguagePack>{
    let installed=fs::read_to_string(language_settings_path()).unwrap_or_else(|_|"Rust\n".into());
    let specs=[("C",&["c","h"][..],&["C23","C17","C11"][..]),("Python",&["py","pyw"][..],&["3.13","3.12","3.11","3.10","3.9"][..]),("C++",&["cc","cpp","cxx","hpp"][..],&["C++23","C++20","C++17","C++14"][..]),("Java",&["java"][..],&["25","21 LTS","17 LTS","11 LTS"][..]),("Rust",&["rs"][..],&["stable","beta","nightly"][..]),("TypeScript / JavaScript",&["ts","tsx","js","jsx","mjs"][..],&["ES2025","ES2024","ES2023"][..]),("Go",&["go"][..],&["1.25","1.24","1.23"][..]),("C#",&["cs"][..],&["14 / .NET 10","13 / .NET 9","12 / .NET 8"][..]),("Kotlin",&["kt","kts"][..],&["2.2","2.1","2.0"][..]),("Swift",&["swift"][..],&["6.2","6.1","6.0"][..]),("Zig",&["zig"][..],&["0.15","0.14","master"][..]),("Lua",&["lua"][..],&["5.4","5.3","LuaJIT"][..]),("Ruby",&["rb"][..],&["3.4","3.3","3.2"][..]),("Web / HTML / CSS",&["html","htm","css","scss","sass"][..],&["Living Standard","HTML5"][..]),("PHP",&["php"][..],&["8.4","8.3","8.2"][..]),("Dart",&["dart"][..],&["3.9","3.8","3.7"][..]),("Shell",&["sh","bash","zsh","fish"][..],&["Bash 5","POSIX sh","Zsh"][..]),("SQL",&["sql"][..],&["PostgreSQL","SQLite","MySQL"][..]),("R",&["r"][..],&["4.5","4.4","4.3"][..]),("Elixir",&["ex","exs"][..],&["1.19","1.18","1.17"][..]),("Haskell",&["hs","lhs"][..],&["GHC 9.12","GHC 9.10","GHC 9.8"][..]),("OCaml",&["ml","mli"][..],&["5.3","5.2","5.1"][..]),("Scala",&["scala","sc"][..],&["3.7","3.6","2.13"][..]),("Clojure",&["clj","cljs","cljc"][..],&["1.12","1.11"][..]),("Julia",&["jl"][..],&["1.11","1.10 LTS"][..]),("Perl",&["pl","pm"][..],&["5.40","5.38"][..]),("Objective-C",&["m","mm"][..],&["Clang 20","Clang 19"][..]),("Markdown",&["md","mdx"][..],&["CommonMark","GFM","MDX"][..]),("Data / Config",&["json","yaml","yml","toml","xml"][..],&["Current","Legacy"][..]),("Vue / Svelte",&["vue","svelte"][..],&["Vue 3 / Svelte 5","Vue 2 / Svelte 4"][..])];
    specs.iter().map(|(name,extensions,versions)|{let saved=installed.lines().find_map(|line|{let (saved_name,version)=line.split_once('|').unwrap_or((line,""));(saved_name==*name).then_some(version)});let selected_version=saved.and_then(|value|versions.iter().position(|candidate|*candidate==value)).unwrap_or(0);LanguagePack{name,extensions,versions,selected_version,installed:saved.is_some()}}).collect()
}

fn save_language_settings(languages:&[LanguagePack]){let path=language_settings_path();if let Some(parent)=path.parent(){let _=fs::create_dir_all(parent);}let content=languages.iter().filter(|pack|pack.installed).map(|pack|format!("{}|{}",pack.name,pack.versions[pack.selected_version])).collect::<Vec<_>>().join("\n");let _=fs::write(path,format!("{content}\n"));}

fn main() {
    unsafe {
        let display=XOpenDisplay(ptr::null());
        if display.is_null() {
            eprintln!("TC-IDE: cannot open X11 display. Run inside a desktop session.");
            std::process::exit(1);
        }
        let screen=XDefaultScreen(display);
        let screen_width=XDisplayWidth(display,screen);
        let screen_height=XDisplayHeight(display,screen);
        let root=XRootWindow(display,screen);
        let map=XDefaultColormap(display,screen);
        let bg=color(display,map,"#0d0f11");
        let width=(screen_width as f32*0.94) as i32;let height=(screen_height as f32*0.90) as i32;
        let window=XCreateSimpleWindow(display,root,(screen_width-width)/2,(screen_height-height)/2,width as u32,height as u32,0,bg,bg);
        let title=CString::new("TC-IDE - Native Semantic Workspace").unwrap();
        XStoreName(display,window,title.as_ptr());
        XSelectInput(display,window,KEY_PRESS_MASK|BUTTON_PRESS_MASK|POINTER_MOTION_MASK|EXPOSURE_MASK|STRUCTURE_NOTIFY_MASK);
        let delete_name=CString::new("WM_DELETE_WINDOW").unwrap();
        let mut delete_atom=XInternAtom(display,delete_name.as_ptr(),0);
        XSetWMProtocols(display,window,&mut delete_atom,1);
        XMapWindow(display,window);
        let gc=XCreateGC(display,window,0,ptr::null_mut());
        let font_pattern=if width>2200{"-misc-fixed-medium-r-normal--24-*-*-*-*-*-iso10646-1"}else{"-misc-fixed-medium-r-normal--18-*-*-*-*-*-iso10646-1"};
        let font_name=CString::new(font_pattern).unwrap();
        let mut font=XLoadQueryFont(display,font_name.as_ptr());
        let fallback_font=CString::new("fixed").unwrap();
        if font.is_null(){font=XLoadQueryFont(display,fallback_font.as_ptr());}
        if !font.is_null(){XSetFont(display,gc,(*font).fid);}
        let depth=XDefaultDepth(display,screen);
        let canvas=XCreatePixmap(display,window,width as u32,height as u32,depth as u32);
        let colors=Palette {
            bg,
            panel:color(display,map,"#121518"),
            panel_2:color(display,map,"#171b1e"),
            line:color(display,map,"#30363a"),
            muted:color(display,map,"#687278"),
            text:color(display,map,"#b7c0c4"),
            bright:color(display,map,"#e1ecef"),
            accent:color(display,map,"#9bbbc2"),
            project:color(display,map,"#29353a"),
        };
        let mut app=App { display,window,gc,canvas,depth,font,colors,width,height,mouse_x:-100,mouse_y:-100,hovered:None,hovered_from_code:false,selected_file:0,files:initial_files(),folders:vec!["src".into(),"src/editor".into()],project_name:"TC-IDE".into(),project_path:None,editor_focused:false,caret_line:0,caret_column:0,graph_zoom:1.0,package_open:false,package_targets:[true,true,true,true],build_started:None,new_menu_open:false,extensions_open:false,languages:initial_languages(),downloading:None,extension_scroll:0,extension_search:String::new(),extension_search_focused:false,selected_language:Some(1),version_menu_open:false,create_kind:None,new_file_name:String::new(),project_language:1,project_environment:0,project_language_menu:false,project_environment_menu:false,expanded_comments:BTreeSet::new(),status_message:"Rust / X11 - 60 FPS double-buffered scene".into(),started:Instant::now() };
        let frame=Duration::from_micros(16_667);
        let mut running=true;
        while running {
            let began=Instant::now();
            while XPending(display)>0 {
                let mut event=XEvent { pad:[0;24] };
                XNextEvent(display,&mut event);
                match event.type_ {
                    KEY_PRESS => { let mut key=event.key;running=app.handle_key(&mut key); }
                    BUTTON_PRESS => {let button=event.button;app.handle_click(button.x,button.y,button.button);}
                    MOTION_NOTIFY => { let motion=event.motion;app.mouse_x=motion.x;app.mouse_y=motion.y;app.update_hover(); }
                    CONFIGURE_NOTIFY => { let config=event.configure;if config.width!=app.width||config.height!=app.height{app.resize(config.width,config.height);} }
                    CLIENT_MESSAGE => running=false,
                    EXPOSE => {},
                    _ => {},
                }
            }
            app.update_background();
            app.draw();
            let used=began.elapsed();
            if used<frame { thread::sleep(frame-used); }
        }
        XFreePixmap(display,app.canvas);
        if !app.font.is_null(){XFreeFont(display,app.font);}
        XFreeGC(display,gc);
        XDestroyWindow(display,window);
        XCloseDisplay(display);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graph_starts_at_project_scope() {
        assert_eq!(NODES.iter().filter(|n| n.level==Level::Project).count(),1);
        assert!(EDGES.iter().filter(|(a,_)| *a==0).count()>=3);
    }

    #[test]
    fn every_non_project_node_is_connected() {
        for index in 1..NODES.len() {
            assert!(EDGES.iter().any(|(a,b)| *a==index || *b==index),"node {index} is isolated");
        }
    }

    #[test]
    fn language_registry_has_common_and_fallback_ready_formats() {
        let languages=initial_languages();
        assert!(languages.len()>=25);
        assert!(languages.iter().any(|pack|pack.name=="Python"&&pack.extensions.contains(&"py")));
        assert!(languages.iter().any(|pack|pack.name=="Data / Config"&&pack.extensions.contains(&"json")));
    }

    #[test]
    fn primary_language_layer_and_versions_are_available() {
        let languages=initial_languages();
        assert_eq!(languages.iter().take(4).map(|pack|pack.name).collect::<Vec<_>>(),vec!["C","Python","C++","Java"]);
        let python=languages.iter().find(|pack|pack.name=="Python").unwrap();
        assert!(python.versions.contains(&"3.13"));
        assert!(python.versions.contains(&"3.12"));
    }

    #[test]
    fn python_projects_offer_real_venv_option() {
        assert_eq!(project_environments("Python").first(),Some(&"venv"));
    }

    #[test]
    fn comment_bubbles_wrap_and_keep_explicit_lines() {
        assert_eq!(wrap_bubble_text("first line\nsecond line",30,4),vec!["first line","second line"]);
        assert_eq!(wrap_bubble_text("123456789",4,4),vec!["1234","5678","9"]);
        assert!(wrap_bubble_text(&"x".repeat(100),10,3).last().unwrap().ends_with("..."));
    }

    #[test]
    fn comment_syntax_depends_on_file_language() {
        assert_eq!(comment_markers_for_file("main.c"),&["//","/*"]);
        assert!(!comment_markers_for_file("main.c").contains(&"#"));
        assert_eq!(comment_markers_for_file("main.py"),&["#"]);
        assert_eq!(comment_markers_for_file("query.sql"),&["--"]);
        assert!(comment_markers_for_file("data.json").is_empty());
    }

    #[test]
    fn unicode_caret_helpers_stay_on_character_boundaries() {
        let value="a注b";
        assert_eq!(char_to_byte_index(value,2),4);
        assert_eq!(previous_char_boundary(value,4),1);
        assert_eq!(next_char_boundary(value,1),4);
    }
}
