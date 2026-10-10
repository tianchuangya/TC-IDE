#![allow(clippy::too_many_arguments)]

use std::ffi::{c_void, CString};
use std::fs;
use std::os::raw::{c_char, c_int, c_long, c_uint, c_ulong};
use std::path::PathBuf;
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

struct LanguagePack {
    name: &'static str,
    extensions: &'static [&'static str],
    installed: bool,
}

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
    selected_file: usize,
    files: Vec<FileBuffer>,
    editor_focused: bool,
    caret_line: usize,
    caret_column: usize,
    graph_zoom: f32,
    package_open: bool,
    package_targets: [bool; 3],
    build_started: Option<Instant>,
    extensions_open: bool,
    languages: Vec<LanguagePack>,
    downloading: Option<(usize, Instant)>,
    extension_scroll: usize,
    new_file_open: bool,
    new_file_name: String,
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
        if self.package_open { self.draw_package_menu(); }
        if self.extensions_open { self.draw_extensions(); }
        if self.new_file_open { self.draw_new_file_dialog(); }
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
        self.text(self.colors.muted,s(122),s(29),"motion-core / src / editor / graph.rs");
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
        self.text(self.colors.bright,s(15),s(101),"v  TC-IDE");
        self.outline(self.colors.line,s(12),s(113),width-s(24),s(26));
        self.text(self.colors.text,s(24),s(132),"+ New file");
        self.text(self.colors.text,s(16),s(164),"  v  src");
        self.text(self.colors.text,s(16),s(191),"    v  editor");
        for (index,file) in self.files.iter().enumerate(){let y=218+index as i32*29;if s(y)>self.height-s(65){break;}if index==self.selected_file{self.rect(self.colors.panel_2,s(7),s(y-21),width-s(14),s(27));}let color=if index==self.selected_file{self.colors.bright}else{self.colors.muted};self.text(color,s(16),s(y),&format!("       {}",file.name));
        }
    }

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
        for (index,line) in self.files[self.selected_file].lines.iter().enumerate() {
            let y=base_y+index as i32*step;if y>self.height-s(65){break;}
            self.text(self.colors.line,left+s(15),y,&(index+1).to_string());
            let color=if line.contains("fn "){self.colors.accent}else if line.contains("struct ")||line.starts_with("impl"){self.colors.bright}else{self.colors.text};
            self.text(color,left+s(52),y,line);
            if self.editor_focused&&index==self.caret_line&&(self.started.elapsed().as_millis()/500)%2==0 {
                let char_width=s(10);let caret_x=left+s(52)+self.caret_column as i32*char_width;
                self.line(self.colors.bright,1,(caret_x,y-s(18)),(caret_x,y+s(4)));
            }
        }
        let hint=if self.editor_focused{"EDITOR ACTIVE · type, Backspace, Enter, arrows"}else{"Click editor to type · hover symbols to trace"};
        self.text(self.colors.muted,left+s(20),self.height-s(50),hint);
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
        self.text(self.colors.muted,x+s(14),y+h-s(10),"wheel: zoom · project -> file -> class -> function / call");
    }

    unsafe fn draw_status(&self) {
        let s=|v|self.s(v);self.rect(self.colors.panel_2,0,self.height-s(24),self.width,s(24));
        self.line(self.colors.line,1,(0,self.height-s(24)),(self.width,self.height-s(24)));
        self.text(self.colors.muted,s(12),self.height-s(7),"TC-IDE native prototype");
        let status=self.hovered.map(|i| format!("Focused: {} - {}",NODES[i].label,NODES[i].detail)).unwrap_or_else(||self.status_message.clone());
        self.text(self.colors.text,self.width/2-s(100),self.height-s(7),&status);
    }

    unsafe fn draw_package_menu(&self) {
        let s=|v|self.s(v);let x=self.width-s(270);let y=s(48);let w=s(250);let h=s(176);
        self.rect(self.colors.panel_2,x,y,w,h);self.outline(self.colors.line,x,y,w,h);
        self.text(self.colors.muted,x+s(15),y+s(25),"BUILD TARGETS");
        let labels=["Windows  .exe","Linux    .deb","Linux    .AppImage"];
        for (index,label) in labels.iter().enumerate(){let row_y=y+s(51+index as i32*31);self.outline(if self.package_targets[index]{self.colors.accent}else{self.colors.line},x+s(15),row_y-s(15),s(16),s(16));if self.package_targets[index]{self.rect(self.colors.accent,x+s(19),row_y-s(11),s(8),s(8));}self.text(self.colors.text,x+s(42),row_y,label);}
        self.rect(self.colors.bright,x+s(15),y+h-s(37),w-s(30),s(25));self.text(self.colors.bg,x+s(72),y+h-s(19),"BUILD SELECTED");
    }

    unsafe fn draw_extensions(&self) {
        let s=|v|self.s(v);let w=s(610);let h=s(650);let x=self.width-w-s(20);let y=s(52);
        self.rect(self.colors.panel_2,x,y,w,h);self.outline(self.colors.line,x,y,w,h);
        self.text(self.colors.bright,x+s(22),y+s(34),"EXTENSIONS");
        self.text(self.colors.accent,x+s(22),y+s(67),"Programming Languages");
        self.text(self.colors.muted,x+s(230),y+s(67),"Language packs are installed per user");
        for (visible,(index,pack)) in self.languages.iter().enumerate().skip(self.extension_scroll).take(12).enumerate(){let row=y+s(103+visible as i32*43);self.line(self.colors.line,1,(x+s(18),row+s(11)),(x+w-s(18),row+s(11)));self.text(self.colors.text,x+s(24),row,pack.name);self.text(self.colors.muted,x+s(165),row,&pack.extensions.iter().take(5).map(|ext|format!(".{ext}")).collect::<Vec<_>>().join("  "));let downloading=self.downloading.map(|(active,_)|active==index).unwrap_or(false);let label=if downloading{"DOWNLOADING..."}else if pack.installed{"INSTALLED"}else{"DOWNLOAD"};let color=if pack.installed{self.colors.accent}else{self.colors.bright};self.outline(color,x+w-s(150),row-s(20),s(126),s(27));self.text(color,x+w-s(137),row,label);}
        self.text(self.colors.muted,x+s(22),y+h-s(18),"Wheel: more packs · unknown extensions use Plain Text");
    }

    unsafe fn draw_new_file_dialog(&self) {
        let s=|v|self.s(v);let w=s(520);let h=s(176);let x=(self.width-w)/2;let y=(self.height-h)/2;
        self.rect(self.colors.panel_2,x,y,w,h);self.outline(self.colors.accent,x,y,w,h);
        self.text(self.colors.bright,x+s(22),y+s(36),"CREATE NEW FILE");
        self.text(self.colors.muted,x+s(22),y+s(66),"Extension selects an installed language pack");
        self.rect(self.colors.bg,x+s(22),y+s(82),w-s(44),s(40));self.outline(self.colors.line,x+s(22),y+s(82),w-s(44),s(40));
        self.text(self.colors.text,x+s(35),y+s(109),if self.new_file_name.is_empty(){"filename.ext"}else{&self.new_file_name});
        self.text(self.colors.muted,x+s(22),y+h-s(18),"Enter: create   Esc: cancel");
    }

    unsafe fn draw_build_animation(&self) {
        let s=|v|self.s(v);let elapsed=self.build_started.unwrap().elapsed().as_secs_f32();
        let cx=self.width/2;let cy=self.height/2;let radius=s((32.0+(elapsed.min(1.0)*30.0)) as i32);
        self.rect(self.colors.bg,cx-s(230),cy-s(140),s(460),s(280));self.outline(self.colors.line,cx-s(230),cy-s(140),s(460),s(280));
        if elapsed<2.2 {self.circle(self.colors.bg,self.colors.accent,cx,cy,radius);self.circle(self.colors.bg,self.colors.line,cx,cy,radius+s(18));self.text(self.colors.text,cx-s(70),cy+s(105),"CODE COLLAPSING INTO BUILD CORE");}
        else {self.circle(self.colors.bright,self.colors.accent,cx,cy,radius);self.circle(self.colors.panel,self.colors.bright,cx,cy,radius+s(18));self.text(self.colors.bright,cx-s(70),cy+s(105),"BUILD COMPLETE - PACKAGES EMITTED");}
    }

    unsafe fn resize(&mut self,width:i32,height:i32){
        self.width=width.max(900);self.height=height.max(600);XFreePixmap(self.display,self.canvas);
        self.canvas=XCreatePixmap(self.display,self.window,self.width as u32,self.height as u32,self.depth as u32);
        self.update_hover();
    }

    fn update_background(&mut self){if let Some((index,started))=self.downloading{if started.elapsed()>Duration::from_millis(900){self.languages[index].installed=true;self.status_message=format!("{} language pack installed",self.languages[index].name);save_language_settings(&self.languages);self.downloading=None;}}}

    fn handle_click(&mut self,x:i32,y:i32,button:u32){
        let scale=self.scale();let s=move|v:i32|(v as f32*scale).round() as i32;
        let graph_left=(self.width as f32*0.61) as i32;
        if self.new_file_open{return;}
        if self.extensions_open&&(button==4||button==5){let visible=12;let maximum=self.languages.len().saturating_sub(visible);if button==4{self.extension_scroll=self.extension_scroll.saturating_sub(1);}else{self.extension_scroll=(self.extension_scroll+1).min(maximum);}return;}
        if x>=graph_left&&button==4 {self.graph_zoom=(self.graph_zoom*1.1).min(1.55);return;}
        if x>=graph_left&&button==5 {self.graph_zoom=(self.graph_zoom/1.1).max(0.65);return;}
        if x>self.width-s(290)&&x<self.width-s(145)&&y<s(46){self.extensions_open=!self.extensions_open;self.package_open=false;self.editor_focused=false;return;}
        if x>self.width-s(150)&&y<s(46){self.package_open=!self.package_open;self.extensions_open=false;self.editor_focused=false;return;}
        if self.extensions_open {
            let w=s(610);let h=s(650);let menu_x=self.width-w-s(20);let menu_y=s(52);
            if x>=menu_x&&x<=menu_x+w&&y>=menu_y&&y<=menu_y+h {for visible in 0..12{let index=self.extension_scroll+visible;if index>=self.languages.len(){break;}let row=menu_y+s(103+visible as i32*43);if (y-row).abs()<s(19)&&x>menu_x+w-s(170){if self.languages[index].installed{self.languages[index].installed=false;self.status_message=format!("{} language pack removed",self.languages[index].name);save_language_settings(&self.languages);}else{self.downloading=Some((index,Instant::now()));self.status_message=format!("Downloading {} language pack...",self.languages[index].name);}return;}}return;}
            self.extensions_open=false;
        }
        if self.package_open {
            let menu_x=self.width-s(270);let menu_y=s(48);
            if x>=menu_x&&x<=self.width-s(20)&&y>=menu_y {
                for index in 0..3 {let center=menu_y+s(51+index as i32*31);if (y-center).abs()<s(15){self.package_targets[index]=!self.package_targets[index];return;}}
                if y>menu_y+s(139)&&self.package_targets.iter().any(|v|*v){self.build_started=Some(Instant::now());self.package_open=false;return;}
            }
            self.package_open=false;
        }
        let workspace_right=(self.width as f32*0.17) as i32;
        if x<workspace_right {
            if y>=s(113)&&y<=s(142){self.new_file_open=true;self.new_file_name.clear();self.editor_focused=false;return;}
            for index in 0..self.files.len(){let center=218+index as i32*29;if (y-s(center)).abs()<s(15){self.selected_file=index;self.caret_line=0;self.caret_column=0;self.editor_focused=false;self.hovered=if index<3{Some(index+1)}else{None};return;}}
        }
        if x>=workspace_right&&x<graph_left&&y>s(85)&&y<self.height-s(60){
            self.editor_focused=true;let base=s(119);let step=s(28);let line=((y-base+step/2)/step).max(0) as usize;self.caret_line=line.min(self.files[self.selected_file].lines.len().saturating_sub(1));let text_x=workspace_right+s(52);self.caret_column=(((x-text_x).max(0))/s(10).max(1)) as usize;self.caret_column=self.caret_column.min(self.files[self.selected_file].lines[self.caret_line].len());return;
        }
        self.editor_focused=false;
    }

    unsafe fn handle_key(&mut self,event:&mut XKeyEvent)->bool {
        let mut keysym=0;let mut buffer=[0_i8;32];let count=XLookupString(event,buffer.as_mut_ptr(),buffer.len() as i32,&mut keysym,ptr::null_mut());
        if self.new_file_open {if keysym==ESCAPE_KEYSYM{self.new_file_open=false;return true;}if keysym==BACKSPACE_KEYSYM{self.new_file_name.pop();return true;}if keysym==RETURN_KEYSYM{let name=self.new_file_name.trim().to_string();if !name.is_empty()&&!name.contains('/')&&!self.files.iter().any(|file|file.name==name){self.files.push(FileBuffer{name:name.clone(),lines:vec![String::new()]});self.selected_file=self.files.len()-1;self.caret_line=0;self.caret_column=0;self.editor_focused=true;self.new_file_open=false;self.status_message=format!("Created {name} - {}",self.language_for_file(&name));}else{self.status_message="Choose a unique filename without '/'".into();}return true;}if count>0{let bytes=buffer[..count as usize].iter().map(|v|*v as u8).collect::<Vec<_>>();if let Ok(input)=std::str::from_utf8(&bytes){if !input.chars().any(char::is_control){self.new_file_name.push_str(input);}}}return true;}
        if keysym==ESCAPE_KEYSYM {if self.extensions_open||self.package_open||self.build_started.is_some(){self.extensions_open=false;self.package_open=false;self.build_started=None;return true;}return false;}
        if !self.editor_focused{return true;}
        match keysym {
            BACKSPACE_KEYSYM=>{if self.caret_column>0 {let line=&mut self.files[self.selected_file].lines[self.caret_line];self.caret_column-=1;line.remove(self.caret_column);}else if self.caret_line>0{let current=self.files[self.selected_file].lines.remove(self.caret_line);self.caret_line-=1;self.caret_column=self.files[self.selected_file].lines[self.caret_line].len();self.files[self.selected_file].lines[self.caret_line].push_str(&current);}},
            RETURN_KEYSYM=>{let tail=self.files[self.selected_file].lines[self.caret_line].split_off(self.caret_column);self.caret_line+=1;self.caret_column=0;self.files[self.selected_file].lines.insert(self.caret_line,tail);},
            LEFT_KEYSYM=>self.caret_column=self.caret_column.saturating_sub(1),
            RIGHT_KEYSYM=>self.caret_column=(self.caret_column+1).min(self.files[self.selected_file].lines[self.caret_line].len()),
            UP_KEYSYM=>{self.caret_line=self.caret_line.saturating_sub(1);self.caret_column=self.caret_column.min(self.files[self.selected_file].lines[self.caret_line].len());},
            DOWN_KEYSYM=>{self.caret_line=(self.caret_line+1).min(self.files[self.selected_file].lines.len()-1);self.caret_column=self.caret_column.min(self.files[self.selected_file].lines[self.caret_line].len());},
            _=>if count>0 {let bytes=buffer[..count as usize].iter().map(|v|*v as u8).collect::<Vec<_>>();if let Ok(input)=std::str::from_utf8(&bytes){if !input.chars().any(char::is_control){self.files[self.selected_file].lines[self.caret_line].insert_str(self.caret_column,input);self.caret_column+=input.len();}}}
        }
        true
    }

    fn update_hover(&mut self) {
        let scale=self.scale();let s=move|v:i32|(v as f32*scale).round() as i32;let left=(self.width as f32*0.17) as i32;let graph=(self.width as f32*0.61) as i32;
        if self.mouse_x>=left&&self.mouse_x<graph&&self.mouse_y>s(90){let row=((self.mouse_y-s(119)+s(14))/s(28).max(1)).max(0) as usize;if let Some(line)=self.files[self.selected_file].lines.get(row){self.hovered=if line.contains("build_graph"){Some(5)}else if line.contains("analyze_workspace"){Some(6)}else if line.contains("resolve_symbols"){Some(7)}else if line.contains("render_graph"){Some(8)}else{None};return;}}
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
        ("graph.rs",["use tc_graph::{Project, Relation};","","pub struct GraphController {","    project: Project,","}","","impl GraphController {","    pub fn build_graph(&mut self) {","        let files = analyze_workspace(&self.project);","        let symbols = resolve_symbols(files);","        render_graph(self.project.root(), symbols);","    }","}"].as_slice()),
        ("workspace.rs",["use crate::workspace::Project;","","pub fn analyze_workspace(project: &Project) {","    project.index_files();","}","","pub fn resolve_symbols(files: Vec<File>) {","    SemanticIndex::from(files);","}"].as_slice()),
        ("render.rs",["use crate::graph::SymbolNode;","","pub struct GraphRenderer;","","impl GraphRenderer {","    pub fn render_graph(nodes: &[SymbolNode]) {","        scene.draw(nodes);","    }","}"].as_slice()),
    ];
    sources.iter().map(|(name,lines)|FileBuffer{name:(*name).into(),lines:lines.iter().map(|line|line.to_string()).collect()}).collect()
}

fn language_settings_path()->PathBuf {std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).or_else(||std::env::var_os("HOME").map(|home|PathBuf::from(home).join(".config"))).unwrap_or_else(||PathBuf::from(".")).join("tc-ide/languages.conf")}

fn initial_languages()->Vec<LanguagePack>{
    let installed=fs::read_to_string(language_settings_path()).unwrap_or_else(|_|"Rust\n".into());
    let enabled=installed.lines().collect::<Vec<_>>();
    [("Rust",&["rs"][..]),("Python",&["py","pyw"][..]),("TypeScript / JavaScript",&["ts","tsx","js","jsx","mjs"][..]),("C / C++",&["c","h","cc","cpp","hpp"][..]),("Go",&["go"][..]),("Java",&["java"][..]),("C#",&["cs"][..]),("Kotlin",&["kt","kts"][..]),("Swift",&["swift"][..]),("Zig",&["zig"][..]),("Lua",&["lua"][..]),("Ruby",&["rb"][..]),("Web / HTML / CSS",&["html","htm","css","scss","sass"][..]),("PHP",&["php"][..]),("Dart",&["dart"][..]),("Shell",&["sh","bash","zsh","fish"][..]),("SQL",&["sql"][..]),("R",&["r"][..]),("Elixir",&["ex","exs"][..]),("Haskell",&["hs","lhs"][..]),("OCaml",&["ml","mli"][..]),("Scala",&["scala","sc"][..]),("Clojure",&["clj","cljs","cljc"][..]),("Julia",&["jl"][..]),("Perl",&["pl","pm"][..]),("Objective-C",&["m","mm"][..]),("Markdown",&["md","mdx"][..]),("Data / Config",&["json","yaml","yml","toml","xml"][..]),("Vue / Svelte",&["vue","svelte"][..])].iter().map(|(name,extensions)|LanguagePack{name,extensions,installed:enabled.contains(name)}).collect()
}

fn save_language_settings(languages:&[LanguagePack]){let path=language_settings_path();if let Some(parent)=path.parent(){let _=fs::create_dir_all(parent);}let content=languages.iter().filter(|pack|pack.installed).map(|pack|pack.name).collect::<Vec<_>>().join("\n");let _=fs::write(path,format!("{content}\n"));}

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
        let title=CString::new("TC-IDE · Native Semantic Workspace").unwrap();
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
        let mut app=App { display,window,gc,canvas,depth,font,colors,width,height,mouse_x:-100,mouse_y:-100,hovered:None,selected_file:0,files:initial_files(),editor_focused:false,caret_line:0,caret_column:0,graph_zoom:1.0,package_open:false,package_targets:[true,true,true],build_started:None,extensions_open:false,languages:initial_languages(),downloading:None,extension_scroll:0,new_file_open:false,new_file_name:String::new(),status_message:"Rust / X11 - 60 FPS double-buffered scene".into(),started:Instant::now() };
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
}
