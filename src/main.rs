#![allow(clippy::too_many_arguments)]

use std::ffi::{c_void, CString};
use std::os::raw::{c_char, c_int, c_long, c_uint, c_ulong};
use std::ptr;
use std::thread;
use std::time::{Duration, Instant};

type Window = c_ulong;
type Drawable = c_ulong;
type Colormap = c_ulong;
type Atom = c_ulong;
type Gc = *mut c_void;

#[repr(C)]
struct Display {
    _private: [u8; 0],
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
    configure: XConfigureEvent,
    pad: [c_long; 24],
}

#[link(name = "X11")]
extern "C" {
    fn XOpenDisplay(name: *const c_char) -> *mut Display;
    fn XDefaultScreen(display: *mut Display) -> c_int;
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
    fn XFreeGC(display: *mut Display, gc: Gc) -> c_int;
    fn XSetForeground(display: *mut Display, gc: Gc, foreground: c_ulong) -> c_int;
    fn XSetLineAttributes(display: *mut Display, gc: Gc, width: c_uint, style: c_int, cap: c_int, join: c_int) -> c_int;
    fn XFillRectangle(display: *mut Display, drawable: Drawable, gc: Gc, x: c_int, y: c_int, width: c_uint, height: c_uint) -> c_int;
    fn XDrawRectangle(display: *mut Display, drawable: Drawable, gc: Gc, x: c_int, y: c_int, width: c_uint, height: c_uint) -> c_int;
    fn XDrawLine(display: *mut Display, drawable: Drawable, gc: Gc, x1: c_int, y1: c_int, x2: c_int, y2: c_int) -> c_int;
    fn XFillArc(display: *mut Display, drawable: Drawable, gc: Gc, x: c_int, y: c_int, width: c_uint, height: c_uint, angle1: c_int, angle2: c_int) -> c_int;
    fn XDrawArc(display: *mut Display, drawable: Drawable, gc: Gc, x: c_int, y: c_int, width: c_uint, height: c_uint, angle1: c_int, angle2: c_int) -> c_int;
    fn XDrawString(display: *mut Display, drawable: Drawable, gc: Gc, x: c_int, y: c_int, text: *const c_char, length: c_int) -> c_int;
    fn XAllocNamedColor(display: *mut Display, map: Colormap, name: *const c_char, screen: *mut XColor, exact: *mut XColor) -> c_int;
    fn XPending(display: *mut Display) -> c_int;
    fn XNextEvent(display: *mut Display, event: *mut XEvent) -> c_int;
    fn XLookupKeysym(event: *mut XKeyEvent, index: c_int) -> c_ulong;
    fn XInternAtom(display: *mut Display, name: *const c_char, only_if_exists: c_int) -> Atom;
    fn XSetWMProtocols(display: *mut Display, window: Window, protocols: *mut Atom, count: c_int) -> c_int;
    fn XFlush(display: *mut Display) -> c_int;
    fn XDestroyWindow(display: *mut Display, window: Window) -> c_int;
    fn XCloseDisplay(display: *mut Display) -> c_int;
}

const KEY_PRESS: c_int = 2;
const MOTION_NOTIFY: c_int = 6;
const EXPOSE: c_int = 12;
const CONFIGURE_NOTIFY: c_int = 22;
const CLIENT_MESSAGE: c_int = 33;
const KEY_PRESS_MASK: c_long = 1;
const POINTER_MOTION_MASK: c_long = 1 << 6;
const EXPOSURE_MASK: c_long = 1 << 15;
const STRUCTURE_NOTIFY_MASK: c_long = 1 << 17;
const ESCAPE_KEYSYM: c_ulong = 0xff1b;

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

const NODES: &[Node] = &[
    Node { label: "TC-IDE", detail: "project", level: Level::Project, nx: 0.50, ny: 0.48 },
    Node { label: "graph.rs", detail: "src/editor", level: Level::File, nx: 0.24, ny: 0.25 },
    Node { label: "workspace.rs", detail: "src", level: Level::File, nx: 0.72, ny: 0.22 },
    Node { label: "render.rs", detail: "src", level: Level::File, nx: 0.77, ny: 0.70 },
    Node { label: "GraphController", detail: "class", level: Level::Class, nx: 0.15, ny: 0.51 },
    Node { label: "build_graph()", detail: "fn · L42", level: Level::Function, nx: 0.30, ny: 0.76 },
    Node { label: "analyze_workspace()", detail: "fn · L18", level: Level::Function, nx: 0.90, ny: 0.39 },
    Node { label: "resolve_symbols()", detail: "fn · L67", level: Level::Function, nx: 0.58, ny: 0.08 },
    Node { label: "render_graph()", detail: "fn · L12", level: Level::Function, nx: 0.93, ny: 0.86 },
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
    colors: Palette,
    width: i32,
    height: i32,
    mouse_x: i32,
    mouse_y: i32,
    hovered: Option<usize>,
    started: Instant,
}

impl App {
    unsafe fn set_color(&self, color: c_ulong) {
        XSetForeground(self.display, self.gc, color);
    }

    unsafe fn rect(&self, color: c_ulong, x: i32, y: i32, width: i32, height: i32) {
        if width <= 0 || height <= 0 { return; }
        self.set_color(color);
        XFillRectangle(self.display, self.window, self.gc, x, y, width as u32, height as u32);
    }

    unsafe fn outline(&self, color: c_ulong, x: i32, y: i32, width: i32, height: i32) {
        self.set_color(color);
        XDrawRectangle(self.display, self.window, self.gc, x, y, width.max(1) as u32, height.max(1) as u32);
    }

    unsafe fn line(&self, color: c_ulong, width: u32, a: (i32, i32), b: (i32, i32)) {
        self.set_color(color);
        XSetLineAttributes(self.display, self.gc, width, 0, 1, 1);
        XDrawLine(self.display, self.window, self.gc, a.0, a.1, b.0, b.1);
    }

    unsafe fn circle(&self, fill: c_ulong, stroke: c_ulong, x: i32, y: i32, radius: i32) {
        self.set_color(fill);
        XFillArc(self.display, self.window, self.gc, x-radius, y-radius, (radius*2) as u32, (radius*2) as u32, 0, 360*64);
        self.set_color(stroke);
        XSetLineAttributes(self.display, self.gc, 1, 0, 1, 1);
        XDrawArc(self.display, self.window, self.gc, x-radius, y-radius, (radius*2) as u32, (radius*2) as u32, 0, 360*64);
    }

    unsafe fn text(&self, color: c_ulong, x: i32, y: i32, value: &str) {
        let clean = value.replace('\0', "");
        let value = CString::new(clean).expect("text contains no NUL");
        self.set_color(color);
        XDrawString(self.display, self.window, self.gc, x, y, value.as_ptr(), value.as_bytes().len() as i32);
    }

    fn graph_bounds(&self) -> (i32, i32, i32, i32) {
        let left = (self.width as f32 * 0.61) as i32;
        (left, 82, self.width-left, self.height-112)
    }

    fn node_position(&self, node: &Node) -> (i32, i32) {
        let (x, y, w, h) = self.graph_bounds();
        (x + (node.nx*w as f32) as i32, y + (node.ny*h as f32) as i32)
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
        self.draw_status();
        XFlush(self.display);
    }

    unsafe fn draw_header(&self) {
        self.rect(self.colors.panel_2, 0, 0, self.width, 46);
        self.line(self.colors.line, 1, (0,45), (self.width,45));
        self.rect(self.colors.bright, 14, 12, 22, 22);
        self.text(self.colors.bg, 20, 28, "T");
        self.text(self.colors.text, 46, 28, "TC-IDE");
        self.text(self.colors.muted, 122, 28, "motion-core / src / editor / graph.rs");
        self.outline(self.colors.line, self.width-116, 11, 98, 24);
        self.text(self.colors.text, self.width-101, 28, "Package  v");
    }

    unsafe fn draw_workspace(&self) {
        let width = (self.width as f32 * 0.17) as i32;
        self.rect(self.colors.panel, 0, 46, width, self.height-70);
        self.line(self.colors.line, 1, (width,46), (width,self.height-24));
        self.text(self.colors.muted, 14, 70, "PROJECT");
        self.text(self.colors.bright, 15, 99, "v  TC-IDE");
        let rows = [
            ("  v  src", 126, self.colors.text),
            ("    v  editor", 151, self.colors.text),
            ("       graph.rs", 176, self.colors.bright),
            ("       workspace.rs", 201, self.colors.muted),
            ("       render.rs", 226, self.colors.muted),
            ("     Cargo.toml", 258, self.colors.muted),
            ("     README.md", 283, self.colors.muted),
        ];
        for (label,y,color) in rows { self.text(color, 16, y, label); }
    }

    unsafe fn draw_editor(&self) {
        let left = (self.width as f32 * 0.17) as i32;
        let right = (self.width as f32 * 0.61) as i32;
        self.rect(self.colors.bg, left+1, 46, right-left-1, self.height-70);
        self.rect(self.colors.panel_2, left+1, 46, right-left-1, 38);
        self.line(self.colors.line, 1, (left,84), (right,84));
        self.text(self.colors.bright, left+18, 70, "graph.rs");
        let code = [
            ("use tc_graph::{Project, Relation};", self.colors.muted),
            ("", self.colors.text),
            ("pub struct GraphController {", self.colors.bright),
            ("    project: Project,", self.colors.text),
            ("}", self.colors.bright),
            ("", self.colors.text),
            ("impl GraphController {", self.colors.bright),
            ("    pub fn build_graph(&mut self) {", self.colors.accent),
            ("        let files = analyze_workspace(&self.project);", self.colors.text),
            ("        let symbols = resolve_symbols(files);", self.colors.text),
            ("        render_graph(self.project.root(), symbols);", self.colors.text),
            ("    }", self.colors.bright),
            ("}", self.colors.bright),
        ];
        let mut y=116;
        for (index,(line,color)) in code.iter().enumerate() {
            self.text(self.colors.line, left+15, y, &(index+1).to_string());
            self.text(*color, left+48, y, line);
            y+=25;
        }
        self.text(self.colors.muted, left+20, self.height-50, "Hover graph nodes: scope changes automatically");
    }

    unsafe fn draw_graph(&self) {
        let (x,y,w,h)=self.graph_bounds();
        self.rect(self.colors.panel, x, 46, w, self.height-70);
        self.line(self.colors.line, 1, (x,46), (x,self.height-24));
        self.line(self.colors.line, 1, (x,84), (self.width,84));
        self.text(self.colors.text, x+14, 70, "Semantic Project Graph");
        let mode = self.hovered.map(|i| match NODES[i].level { Level::Project=>"PROJECT",Level::File=>"FILE",Level::Class=>"CLASS",Level::Function=>"FUNCTION" }).unwrap_or("AUTO");
        self.text(self.colors.muted, self.width-100, 70, mode);

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
            let base=match node.level { Level::Project=>14,Level::File=>9,Level::Class=>7,Level::Function=>5 };
            let pulse=if active { ((elapsed*5.0).sin().abs()*4.0) as i32 } else { 0 };
            if active { self.circle(self.colors.panel, self.colors.accent, nx, ny, base+pulse+5); }
            let stroke=if active { self.colors.bright } else if related { self.colors.accent } else { self.colors.line };
            let fill=if node.level==Level::Project { self.colors.project } else { self.colors.panel_2 };
            self.circle(fill, stroke, nx, ny, base);
            let label_color=if related { self.colors.text } else { self.colors.muted };
            self.text(label_color, nx+base+6, ny-2, node.label);
            self.text(self.colors.muted, nx+base+6, ny+11, node.detail);
        }
        self.text(self.colors.muted, x+14, y+h-10, "project  ->  file  ->  class  ->  function / call");
    }

    unsafe fn draw_status(&self) {
        self.rect(self.colors.panel_2, 0, self.height-24, self.width, 24);
        self.line(self.colors.line, 1, (0,self.height-24), (self.width,self.height-24));
        self.text(self.colors.muted, 12, self.height-8, "TC-IDE native prototype");
        let status=self.hovered.map(|i| format!("Focused: {} · {}",NODES[i].label,NODES[i].detail)).unwrap_or_else(||"Rust / X11 · 60 FPS scene clock".into());
        self.text(self.colors.text, self.width/2-90, self.height-8, &status);
    }

    fn update_hover(&mut self) {
        let mut best=None;
        let mut distance=34_i64*34;
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

fn main() {
    unsafe {
        let display=XOpenDisplay(ptr::null());
        if display.is_null() {
            eprintln!("TC-IDE: cannot open X11 display. Run inside a desktop session.");
            std::process::exit(1);
        }
        let screen=XDefaultScreen(display);
        let root=XRootWindow(display,screen);
        let map=XDefaultColormap(display,screen);
        let bg=color(display,map,"#0d0f11");
        let window=XCreateSimpleWindow(display,root,70,60,1320,780,0,bg,bg);
        let title=CString::new("TC-IDE · Native Semantic Workspace").unwrap();
        XStoreName(display,window,title.as_ptr());
        XSelectInput(display,window,KEY_PRESS_MASK|POINTER_MOTION_MASK|EXPOSURE_MASK|STRUCTURE_NOTIFY_MASK);
        let delete_name=CString::new("WM_DELETE_WINDOW").unwrap();
        let mut delete_atom=XInternAtom(display,delete_name.as_ptr(),0);
        XSetWMProtocols(display,window,&mut delete_atom,1);
        XMapWindow(display,window);
        let gc=XCreateGC(display,window,0,ptr::null_mut());
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
        let mut app=App { display,window,gc,colors,width:1320,height:780,mouse_x:-100,mouse_y:-100,hovered:None,started:Instant::now() };
        let frame=Duration::from_micros(16_667);
        let mut running=true;
        while running {
            let began=Instant::now();
            while XPending(display)>0 {
                let mut event=XEvent { pad:[0;24] };
                XNextEvent(display,&mut event);
                match event.type_ {
                    KEY_PRESS => { let mut key=event.key;if XLookupKeysym(&mut key,0)==ESCAPE_KEYSYM { running=false; } }
                    MOTION_NOTIFY => { let motion=event.motion;app.mouse_x=motion.x;app.mouse_y=motion.y;app.update_hover(); }
                    CONFIGURE_NOTIFY => { let config=event.configure;app.width=config.width.max(760);app.height=config.height.max(520);app.update_hover(); }
                    CLIENT_MESSAGE => running=false,
                    EXPOSE => {},
                    _ => {},
                }
            }
            app.draw();
            let used=began.elapsed();
            if used<frame { thread::sleep(frame-used); }
        }
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
}
