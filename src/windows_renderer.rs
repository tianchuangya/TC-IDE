use std::ffi::c_void;

use windows::core::{w, Interface, Result, PCWSTR};
use windows::Win32::Foundation::{GENERIC_READ, HWND};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D1_ALPHA_MODE_UNKNOWN, D2D1_COLOR_F, D2D1_PIXEL_FORMAT, D2D_RECT_F, D2D_SIZE_U,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1CreateFactory, ID2D1Bitmap, ID2D1Factory, ID2D1HwndRenderTarget, ID2D1RenderTarget,
    ID2D1SolidColorBrush, D2D1_BITMAP_INTERPOLATION_MODE_LINEAR, D2D1_DRAW_TEXT_OPTIONS_NONE,
    D2D1_ELLIPSE, D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_FEATURE_LEVEL_DEFAULT,
    D2D1_HWND_RENDER_TARGET_PROPERTIES, D2D1_PRESENT_OPTIONS_NONE, D2D1_RENDER_TARGET_PROPERTIES,
    D2D1_RENDER_TARGET_TYPE_DEFAULT, D2D1_RENDER_TARGET_USAGE_NONE, D2D1_ROUNDED_RECT,
};
use windows::Win32::Graphics::DirectWrite::{
    DWriteCreateFactory, IDWriteFactory, IDWriteTextFormat, DWRITE_FACTORY_TYPE_SHARED,
    DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT_NORMAL,
    DWRITE_MEASURING_MODE_NATURAL,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_UNKNOWN;
use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory, GUID_WICPixelFormat32bppPBGRA, IWICImagingFactory,
    WICBitmapDitherTypeNone, WICBitmapPaletteTypeMedianCut, WICDecodeMetadataCacheOnLoad,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
};

use crate::editor_core::{self, SymbolOccurrence, TokenKind};
use crate::graph_core;
use crate::terminal_core;

pub struct WindowsScene<'a> {
    pub file_name: &'a str,
    pub project_name: &'a str,
    pub project_entries: &'a [String],
    pub workspace_search: &'a str,
    pub workspace_search_focused: bool,
    pub create_kind: Option<&'a str>,
    pub create_name: &'a str,
    pub create_error: &'a str,
    pub create_location: &'a str,
    pub create_language: &'a str,
    pub create_version: &'a str,
    pub create_environment: &'a str,
    pub lines: &'a [String],
    pub caret_line: usize,
    pub caret_column: usize,
    pub selection: Option<((usize, usize), (usize, usize))>,
    pub ime_composition: &'a str,
    pub completions: &'a [String],
    pub light_theme: bool,
    pub ui_chinese: bool,
    pub code_zoom: i32,
    pub code_font_family: usize,
    pub code_font_name: &'a str,
    pub font_search: &'a str,
    pub font_search_focused: bool,
    pub font_options: &'a [&'a str],
    pub settings_open: bool,
    pub settings_motion: f32,
    pub extensions_open: bool,
    pub extensions_motion: f32,
    pub extension_category: usize,
    pub selected_language: usize,
    pub extension_search: &'a str,
    pub extension_search_focused: bool,
    pub extension_scroll: usize,
    pub language_options: &'a [&'a str],
    pub language_installed: &'a [bool],
    pub language_versions: &'a [usize],
    pub ai_endpoint: &'a str,
    pub ai_model: &'a str,
    pub ai_key_present: bool,
    pub ai_field_focused: Option<usize>,
    pub symbols: &'a [SymbolOccurrence],
    pub hovered_symbol: Option<&'a SymbolOccurrence>,
    pub graph_nodes: &'a [graph_core::GraphNode],
    /// Absolute index of the first drawn node.
    pub graph_first_index: usize,
    /// Absolute index of the keyboard selected node.
    pub graph_selected: Option<usize>,
    pub graph_search: &'a str,
    pub graph_search_focused: bool,
    pub graph_scope_label: &'a str,
    pub graph_summary: &'a str,
    pub expanded_comments: &'a [usize],
    pub editing_comment: Option<usize>,
    pub pointer_x: i32,
    pub package_open: bool,
    pub package_motion: f32,
    pub package_targets: [bool; 3],
    pub package_running: bool,
    pub package_progress: f32,
    pub background_path: Option<&'a str>,
    pub background_opacity: f32,
    pub background_offset_x: f32,
    pub background_offset_y: f32,
    pub background_zoom: f32,
    pub status: &'a str,
    pub terminal_lines: &'a [String],
    /// Buffer line index of the first drawn terminal row.
    pub terminal_first_line: usize,
    /// Selected terminal range as `((line, column), (line, column))`.
    pub terminal_selection: Option<((usize, usize), (usize, usize))>,
    pub terminal_names: &'a [String],
    pub active_terminal: usize,
    pub terminal_input: &'a str,
    pub terminal_open: bool,
    pub terminal_progress: f32,
    /// Rows the panel is scrolled back from the newest output.
    pub terminal_scrolled: usize,
    pub animation_seconds: f32,
    pub edit_age: Option<f32>,
    pub create_motion: f32,
}

pub struct DirectEditorRenderer {
    d2d_factory: ID2D1Factory,
    _dwrite_factory: IDWriteFactory,
    target: Option<ID2D1HwndRenderTarget>,
    code_format: IDWriteTextFormat,
    ui_format: IDWriteTextFormat,
    small_format: IDWriteTextFormat,
    code_cell: f32,
    wic_factory: IWICImagingFactory,
    background_bitmap: Option<ID2D1Bitmap>,
    background_path: Option<String>,
    dpi: f32,
}

impl DirectEditorRenderer {
    fn language_version(language: usize, version: usize) -> &'static str {
        let values = match language {
            0 => ["stable", "beta", "nightly"],
            1 => ["3.13", "3.12", "3.11"],
            2 => ["C23", "C17", "C11"],
            3 => ["C++23", "C++20", "C++17"],
            4 => ["25", "21 LTS", "17 LTS"],
            5 => ["1.25", "1.24", "1.23"],
            6 => ["ES2025", "ES2024", "ES2023"],
            7 => [".NET 10", ".NET 9", ".NET 8"],
            8 => ["2.2", "2.1", "2.0"],
            9 => ["6.2", "6.1", "6.0"],
            10 => ["0.15", "0.14", "master"],
            11 => ["5.4", "5.3", "LuaJIT"],
            12 => ["3.4", "3.3", "3.2"],
            13 => ["8.4", "8.3", "8.2"],
            14 => ["2025", "2023", "2019"],
            _ => ["system", "bash", "PowerShell"],
        };
        values[version.min(2)]
    }

    fn character_cells(value: char) -> usize {
        match value as u32 {
            0x1100..=0x115f
            | 0x2329..=0x232a
            | 0x2e80..=0xa4cf
            | 0xac00..=0xd7a3
            | 0xf900..=0xfaff
            | 0xfe10..=0xfe6f
            | 0xff00..=0xff60
            | 0xffe0..=0xffe6
            | 0x1f300..=0x1faff
            | 0x20000..=0x3fffd => 2,
            _ => 1,
        }
    }

    fn visual_cells(value: &str, columns: usize) -> usize {
        value.chars().take(columns).map(Self::character_cells).sum()
    }

    pub unsafe fn new(dpi: u32, code_zoom: i32, code_font_name: &str) -> Result<Self> {
        let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        let d2d_factory: ID2D1Factory =
            unsafe { D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None) }?;
        let dwrite_factory: IDWriteFactory =
            unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED) }?;
        let wic_factory: IWICImagingFactory =
            unsafe { CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER) }?;
        let scale = dpi.max(96) as f32 / 96.0;
        let code_font_wide = code_font_name
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>();
        let code_format = unsafe {
            dwrite_factory.CreateTextFormat(
                PCWSTR(code_font_wide.as_ptr()),
                None,
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                (15.0 + code_zoom.clamp(-2, 8) as f32) * scale,
                w!("zh-CN"),
            )
        }?;
        let ui_format = unsafe {
            dwrite_factory.CreateTextFormat(
                w!("Segoe UI Variable"),
                None,
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                14.0 * scale,
                w!("zh-CN"),
            )
        }?;
        let small_format = unsafe {
            dwrite_factory.CreateTextFormat(
                w!("Segoe UI Variable"),
                None,
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                12.0 * scale,
                w!("zh-CN"),
            )
        }?;
        Ok(Self {
            d2d_factory,
            _dwrite_factory: dwrite_factory,
            target: None,
            code_format,
            ui_format,
            small_format,
            code_cell: terminal_core::code_cell_width(code_zoom),
            wic_factory,
            background_bitmap: None,
            background_path: None,
            dpi: dpi.max(96) as f32,
        })
    }

    unsafe fn sync_background(&mut self, target: &ID2D1RenderTarget, requested: Option<&str>) {
        if self.background_path.as_deref() == requested {
            return;
        }
        self.background_bitmap = None;
        self.background_path = requested.map(str::to_string);
        let Some(path) = requested else { return };
        let wide = path
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>();
        let loaded = (|| -> Result<ID2D1Bitmap> {
            let decoder = unsafe {
                self.wic_factory.CreateDecoderFromFilename(
                    PCWSTR(wide.as_ptr()),
                    None,
                    GENERIC_READ,
                    WICDecodeMetadataCacheOnLoad,
                )
            }?;
            let frame = unsafe { decoder.GetFrame(0) }?;
            let converter = unsafe { self.wic_factory.CreateFormatConverter() }?;
            unsafe {
                converter.Initialize(
                    &frame,
                    &GUID_WICPixelFormat32bppPBGRA,
                    WICBitmapDitherTypeNone,
                    None,
                    0.0,
                    WICBitmapPaletteTypeMedianCut,
                )
            }?;
            unsafe { target.CreateBitmapFromWicBitmap(&converter, None) }
        })();
        self.background_bitmap = loaded.ok();
    }

    pub unsafe fn resize(&mut self, width: u32, height: u32, dpi: u32) {
        self.dpi = dpi.max(96) as f32;
        if let Some(target) = &self.target {
            let _ = unsafe { target.Resize(&D2D_SIZE_U { width, height }) };
            // Layout coordinates are already scaled explicitly. Keep the render
            // target in 96-DPI DIPs so Windows does not apply DPI twice.
            unsafe { target.SetDpi(96.0, 96.0) };
        }
    }

    unsafe fn ensure_target(
        &mut self,
        window: *mut c_void,
        width: u32,
        height: u32,
    ) -> Result<ID2D1HwndRenderTarget> {
        if let Some(target) = &self.target {
            return Ok(target.clone());
        }
        let properties = D2D1_RENDER_TARGET_PROPERTIES {
            r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_UNKNOWN,
                alphaMode: D2D1_ALPHA_MODE_UNKNOWN,
            },
            dpiX: 96.0,
            dpiY: 96.0,
            usage: D2D1_RENDER_TARGET_USAGE_NONE,
            minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
        };
        let window_properties = D2D1_HWND_RENDER_TARGET_PROPERTIES {
            hwnd: HWND(window),
            pixelSize: D2D_SIZE_U { width, height },
            presentOptions: D2D1_PRESENT_OPTIONS_NONE,
        };
        let target = unsafe {
            self.d2d_factory
                .CreateHwndRenderTarget(&properties, &window_properties)
        }?;
        self.background_bitmap = None;
        self.background_path = None;
        self.target = Some(target.clone());
        Ok(target)
    }

    unsafe fn brush(
        target: &ID2D1RenderTarget,
        rgb: u32,
        alpha: f32,
    ) -> Result<ID2D1SolidColorBrush> {
        let color = D2D1_COLOR_F {
            r: ((rgb >> 16) & 255) as f32 / 255.0,
            g: ((rgb >> 8) & 255) as f32 / 255.0,
            b: (rgb & 255) as f32 / 255.0,
            a: alpha,
        };
        unsafe { target.CreateSolidColorBrush(&color, None) }
    }

    unsafe fn fill(
        target: &ID2D1RenderTarget,
        brush: &ID2D1SolidColorBrush,
        left: f32,
        top: f32,
        right: f32,
        bottom: f32,
    ) {
        unsafe {
            target.FillRectangle(
                &D2D_RECT_F {
                    left,
                    top,
                    right,
                    bottom,
                },
                brush,
            )
        };
    }

    unsafe fn rounded(
        target: &ID2D1RenderTarget,
        brush: &ID2D1SolidColorBrush,
        rect: D2D_RECT_F,
        radius: f32,
    ) {
        unsafe {
            target.FillRoundedRectangle(
                &D2D1_ROUNDED_RECT {
                    rect,
                    radiusX: radius,
                    radiusY: radius,
                },
                brush,
            )
        };
    }

    unsafe fn outline(
        target: &ID2D1RenderTarget,
        brush: &ID2D1SolidColorBrush,
        rect: D2D_RECT_F,
        radius: f32,
        width: f32,
    ) {
        unsafe {
            target.DrawRoundedRectangle(
                &D2D1_ROUNDED_RECT {
                    rect,
                    radiusX: radius,
                    radiusY: radius,
                },
                brush,
                width,
                None,
            )
        };
    }

    unsafe fn text(
        target: &ID2D1RenderTarget,
        format: &IDWriteTextFormat,
        brush: &ID2D1SolidColorBrush,
        value: &str,
        rect: D2D_RECT_F,
    ) {
        let value = value.encode_utf16().collect::<Vec<_>>();
        unsafe {
            target.DrawText(
                &value,
                format,
                &rect,
                brush,
                D2D1_DRAW_TEXT_OPTIONS_NONE,
                DWRITE_MEASURING_MODE_NATURAL,
            )
        };
    }

    pub unsafe fn render(
        &mut self,
        window: *mut c_void,
        width: u32,
        height: u32,
        scene: &WindowsScene<'_>,
    ) -> Result<()> {
        let target = unsafe { self.ensure_target(window, width, height) }?;
        let target_base: ID2D1RenderTarget = target.cast()?;
        unsafe { self.sync_background(&target_base, scene.background_path) };
        let colors = if scene.light_theme {
            [
                0xf5f5f7, 0xffffff, 0xf2f2f7, 0xd1d1d6, 0x3a3a3c, 0x6e6e73, 0x1d1d1f,
            ]
        } else {
            [
                0x000000, 0x1c1c1e, 0x2c2c2e, 0x38383a, 0xd1d1d6, 0x8e8e93, 0xf5f5f7,
            ]
        };
        let bg = unsafe { Self::brush(&target_base, colors[0], 1.0) }?;
        let panel = unsafe { Self::brush(&target_base, colors[1], 1.0) }?;
        let raised = unsafe { Self::brush(&target_base, colors[2], 1.0) }?;
        let line = unsafe { Self::brush(&target_base, colors[3], 1.0) }?;
        let text = unsafe { Self::brush(&target_base, colors[4], 1.0) }?;
        let muted = unsafe { Self::brush(&target_base, colors[5], 1.0) }?;
        let bright = unsafe { Self::brush(&target_base, colors[6], 1.0) }?;
        let accent = unsafe { Self::brush(&target_base, 0x0a84ff, 1.0) }?;
        let selection = unsafe { Self::brush(&target_base, 0x193451, 1.0) }?;
        let selected_text = unsafe { Self::brush(&target_base, 0x0a84ff, 0.46) }?;
        let keyword = unsafe { Self::brush(&target_base, 0x64d2ff, 1.0) }?;
        let literal = unsafe { Self::brush(&target_base, 0xff9f0a, 1.0) }?;
        let comment = unsafe { Self::brush(&target_base, 0x8e8e93, 1.0) }?;
        let width = width as f32;
        let height = height as f32;
        let scale = self.dpi / 96.0;
        let header_h = 48.0 * scale;
        let status_h = 28.0 * scale;
        let terminal_progress = scene.terminal_progress.clamp(0.0, 1.0);
        let terminal_panel = terminal_core::panel_metrics(width, height, scale, terminal_progress);
        let terminal_h = terminal_panel.height;
        let body_bottom = height - terminal_h;
        let tree_w = (width * 0.19).max(220.0 * scale);
        let graph_w = (width * 0.27).max(300.0 * scale);
        let editor_right = width - graph_w;
        let code_top = header_h + 58.0 * scale;
        let line_step = 25.0 * scale;
        let max_lines = ((body_bottom - code_top) / line_step).max(0.0) as usize;

        unsafe { target_base.BeginDraw() };
        unsafe { target_base.Clear(Some(&D2D1_COLOR_F::default())) };
        unsafe { Self::fill(&target_base, &bg, 0.0, 0.0, width, height) };
        unsafe { Self::fill(&target_base, &raised, 0.0, 0.0, width, header_h) };
        unsafe { Self::fill(&target_base, &panel, 0.0, header_h, tree_w, body_bottom) };
        unsafe {
            Self::fill(
                &target_base,
                &panel,
                editor_right,
                header_h,
                width,
                body_bottom,
            )
        };
        if let Some(bitmap) = &self.background_bitmap {
            let image = unsafe { bitmap.GetSize() };
            let region = D2D_RECT_F {
                left: tree_w,
                top: header_h + 38.0 * scale,
                right: editor_right,
                bottom: body_bottom,
            };
            let region_w = (region.right - region.left).max(1.0);
            let region_h = (region.bottom - region.top).max(1.0);
            let cover = (region_w / image.width.max(1.0)).max(region_h / image.height.max(1.0));
            let image_scale = cover * scene.background_zoom.clamp(1.0, 3.0);
            let source_w = (region_w / image_scale).min(image.width);
            let source_h = (region_h / image_scale).min(image.height);
            let source_left =
                (image.width - source_w) * (scene.background_offset_x.clamp(-1.0, 1.0) + 1.0) * 0.5;
            let source_top = (image.height - source_h)
                * (scene.background_offset_y.clamp(-1.0, 1.0) + 1.0)
                * 0.5;
            let source = D2D_RECT_F {
                left: source_left,
                top: source_top,
                right: source_left + source_w,
                bottom: source_top + source_h,
            };
            unsafe {
                target_base.DrawBitmap(
                    bitmap,
                    Some(&region),
                    scene.background_opacity.clamp(0.0, 1.0),
                    D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
                    Some(&source),
                )
            };
        }
        unsafe {
            Self::fill(
                &target_base,
                &line,
                tree_w - 1.0,
                header_h,
                tree_w,
                body_bottom,
            )
        };
        unsafe {
            Self::fill(
                &target_base,
                &line,
                editor_right,
                header_h,
                editor_right + 1.0,
                body_bottom,
            )
        };

        if let Some(focused) = scene.hovered_symbol {
            let focused_line = scene
                .lines
                .get(focused.line)
                .map(String::as_str)
                .unwrap_or("");
            let focused_left = tree_w
                + (49.0 + Self::visual_cells(focused_line, focused.start) as f32 * self.code_cell)
                    * scale;
            let focused_right = tree_w
                + (55.0 + Self::visual_cells(focused_line, focused.end) as f32 * self.code_cell)
                    * scale;
            let focused_top = code_top + focused.line as f32 * line_step - 3.0 * scale;
            unsafe {
                Self::outline(
                    &target_base,
                    &accent,
                    D2D_RECT_F {
                        left: focused_left,
                        top: focused_top,
                        right: focused_right,
                        bottom: focused_top + 25.0 * scale,
                    },
                    5.0 * scale,
                    1.5 * scale,
                )
            };
            let route_x = editor_right - 13.0 * scale;
            let source_y = focused_top + 12.0 * scale;
            unsafe {
                Self::fill(
                    &target_base,
                    &accent,
                    focused_right,
                    source_y,
                    route_x,
                    source_y + 1.5 * scale,
                )
            };
            for related in scene
                .symbols
                .iter()
                .filter(|symbol| symbol.name == focused.name && *symbol != focused)
                .take(8)
            {
                if related.line >= max_lines {
                    continue;
                }
                let line_value = scene
                    .lines
                    .get(related.line)
                    .map(String::as_str)
                    .unwrap_or("");
                let related_left = tree_w
                    + (49.0
                        + Self::visual_cells(line_value, related.start) as f32 * self.code_cell)
                        * scale;
                let related_right = tree_w
                    + (55.0 + Self::visual_cells(line_value, related.end) as f32 * self.code_cell)
                        * scale;
                let related_top = code_top + related.line as f32 * line_step - 3.0 * scale;
                let related_y = related_top + 12.0 * scale;
                unsafe {
                    Self::outline(
                        &target_base,
                        &muted,
                        D2D_RECT_F {
                            left: related_left,
                            top: related_top,
                            right: related_right,
                            bottom: related_top + 25.0 * scale,
                        },
                        5.0 * scale,
                        1.0 * scale,
                    );
                    Self::fill(
                        &target_base,
                        &accent,
                        route_x,
                        source_y.min(related_y),
                        route_x + 1.5 * scale,
                        source_y.max(related_y) + 1.5 * scale,
                    );
                    Self::fill(
                        &target_base,
                        &accent,
                        related_right,
                        related_y,
                        route_x,
                        related_y + 1.5 * scale,
                    );
                }
            }
        }

        unsafe {
            Self::text(
                &target_base,
                &self.ui_format,
                &bright,
                "TCIDE",
                D2D_RECT_F {
                    left: 18.0 * scale,
                    top: 13.0 * scale,
                    right: 110.0 * scale,
                    bottom: header_h,
                },
            )
        };
        unsafe {
            Self::text(
                &target_base,
                &self.small_format,
                &accent,
                if scene.package_running {
                    "Packaging..."
                } else {
                    "Package  v"
                },
                D2D_RECT_F {
                    left: width - 142.0 * scale,
                    top: 15.0 * scale,
                    right: width - 16.0 * scale,
                    bottom: header_h,
                },
            )
        };
        unsafe {
            Self::text(
                &target_base,
                &self.small_format,
                &muted,
                &format!("TC-IDE  /  {}", scene.file_name),
                D2D_RECT_F {
                    left: width * 0.42,
                    top: 15.0 * scale,
                    right: width * 0.7,
                    bottom: header_h,
                },
            )
        };

        let tree_items = std::iter::once("PROJECT".to_string())
            .chain(std::iter::once(format!("▾ {}", scene.project_name)))
            .chain(scene.project_entries.iter().cloned())
            .collect::<Vec<_>>();
        unsafe {
            Self::rounded(
                &target_base,
                if scene.workspace_search_focused {
                    &selection
                } else {
                    &raised
                },
                D2D_RECT_F {
                    left: 12.0 * scale,
                    top: header_h + 8.0 * scale,
                    right: tree_w - 12.0 * scale,
                    bottom: header_h + 40.0 * scale,
                },
                8.0 * scale,
            )
        };
        unsafe {
            Self::text(
                &target_base,
                &self.small_format,
                if scene.workspace_search.is_empty() {
                    &muted
                } else {
                    &text
                },
                if scene.workspace_search.is_empty() {
                    if scene.ui_chinese {
                        "搜索项目文件"
                    } else {
                        "Search project files"
                    }
                } else {
                    scene.workspace_search
                },
                D2D_RECT_F {
                    left: 22.0 * scale,
                    top: header_h + 16.0 * scale,
                    right: tree_w - 18.0 * scale,
                    bottom: header_h + 38.0 * scale,
                },
            )
        };
        for (index, item) in tree_items.iter().enumerate() {
            let delay = index as f32 * 0.045;
            let local = ((scene.animation_seconds - delay) / 0.28).clamp(0.0, 1.0);
            let eased = 1.0 - (1.0 - local).powi(3);
            let y = header_h + (58.0 + index as f32 * 35.0) * scale;
            let x = 18.0 * scale - (1.0 - eased) * 18.0 * scale;
            unsafe {
                Self::text(
                    &target_base,
                    &self.ui_format,
                    if item.trim_start().ends_with(scene.file_name) {
                        &bright
                    } else {
                        &text
                    },
                    item,
                    D2D_RECT_F {
                        left: x,
                        top: y,
                        right: tree_w - 12.0 * scale,
                        bottom: y + 28.0 * scale,
                    },
                )
            };
        }

        unsafe {
            Self::fill(
                &target_base,
                &raised,
                tree_w,
                header_h,
                editor_right,
                header_h + 38.0 * scale,
            )
        };
        unsafe {
            Self::text(
                &target_base,
                &self.ui_format,
                &bright,
                scene.file_name,
                D2D_RECT_F {
                    left: tree_w + 18.0 * scale,
                    top: header_h + 10.0 * scale,
                    right: editor_right,
                    bottom: header_h + 38.0 * scale,
                },
            )
        };

        for (index, value) in scene.lines.iter().take(max_lines).enumerate() {
            let y = code_top + index as f32 * line_step;
            if index == scene.caret_line {
                unsafe {
                    Self::fill(
                        &target_base,
                        &selection,
                        tree_w + 42.0 * scale,
                        y - 3.0 * scale,
                        editor_right - 12.0 * scale,
                        y + 21.0 * scale,
                    )
                };
            }
            if let Some(((start_line, start_column), (end_line, end_column))) = scene.selection {
                if index >= start_line && index <= end_line {
                    let left_column = if index == start_line { start_column } else { 0 };
                    let right_column = if index == end_line {
                        end_column
                    } else {
                        value.chars().count()
                    };
                    let left_cells = Self::visual_cells(value, left_column) as f32;
                    let right_cells = Self::visual_cells(value, right_column) as f32;
                    unsafe {
                        Self::fill(
                            &target_base,
                            &selected_text,
                            tree_w + (52.0 + left_cells * self.code_cell) * scale,
                            y - 2.0 * scale,
                            tree_w + (52.0 + right_cells * self.code_cell) * scale,
                            y + 21.0 * scale,
                        )
                    };
                }
            }
            unsafe {
                Self::text(
                    &target_base,
                    &self.small_format,
                    &muted,
                    &(index + 1).to_string(),
                    D2D_RECT_F {
                        left: tree_w + 12.0 * scale,
                        top: y,
                        right: tree_w + 39.0 * scale,
                        bottom: y + line_step,
                    },
                )
            };
            let edit_offset = scene
                .edit_age
                .filter(|_| index == scene.caret_line)
                .map(|age| (1.0 - (age / 0.16).clamp(0.0, 1.0)) * 7.0 * scale)
                .unwrap_or(0.0);
            let mut token_x = tree_w + 52.0 * scale + edit_offset;
            for token in editor_core::syntax_tokens(value, scene.file_name) {
                if token.kind == TokenKind::Comment {
                    unsafe {
                        Self::text(
                            &target_base,
                            &self.code_format,
                            &comment,
                            "●",
                            D2D_RECT_F {
                                left: token_x,
                                top: y,
                                right: token_x + 20.0 * scale,
                                bottom: y + line_step,
                            },
                        )
                    };
                    break;
                }
                let token_cells =
                    Self::visual_cells(&token.text, token.text.chars().count()) as f32;
                let token_brush = match token.kind {
                    TokenKind::Keyword => &keyword,
                    TokenKind::Literal => &literal,
                    TokenKind::Comment => &comment,
                    TokenKind::Plain => &text,
                };
                unsafe {
                    Self::text(
                        &target_base,
                        &self.code_format,
                        token_brush,
                        &token.text,
                        D2D_RECT_F {
                            left: token_x,
                            top: y,
                            right: editor_right - 10.0 * scale,
                            bottom: y + line_step,
                        },
                    )
                };
                token_x += token_cells * self.code_cell * scale;
            }
        }
        if scene.pointer_x as f32 >= tree_w {
            for (line_index, value) in scene.lines.iter().take(max_lines).enumerate() {
                let Some((marker_start, content_start, content_end)) =
                    editor_core::comment_bounds(value, scene.file_name)
                else {
                    continue;
                };
                let expanded = scene.expanded_comments.contains(&line_index);
                let editing = scene.editing_comment == Some(line_index);
                let y = code_top + line_index as f32 * line_step - 5.0 * scale;
                let bubble_right = editor_right - 14.0 * scale;
                let bubble_left = (bubble_right - 286.0 * scale).max(tree_w + 130.0 * scale);
                let bubble_height = if expanded { 112.0 } else { 34.0 } * scale;
                let marker_column = value[..marker_start].chars().count();
                let anchor_x = tree_w
                    + (52.0 + Self::visual_cells(value, marker_column) as f32 * self.code_cell)
                        * scale;
                unsafe {
                    Self::fill(
                        &target_base,
                        if editing { &accent } else { &line },
                        anchor_x + 10.0 * scale,
                        y + 15.0 * scale,
                        bubble_left,
                        y + 16.5 * scale,
                    );
                    Self::rounded(
                        &target_base,
                        if editing { &selection } else { &raised },
                        D2D_RECT_F {
                            left: bubble_left,
                            top: y,
                            right: bubble_right,
                            bottom: y + bubble_height,
                        },
                        9.0 * scale,
                    );
                    Self::text(
                        &target_base,
                        &self.small_format,
                        if editing { &accent } else { &muted },
                        if expanded { "⌄  Edit" } else { ">  Edit" },
                        D2D_RECT_F {
                            left: bubble_left + 10.0 * scale,
                            top: y + 7.0 * scale,
                            right: bubble_left + 76.0 * scale,
                            bottom: y + 30.0 * scale,
                        },
                    );
                    Self::text(
                        &target_base,
                        &self.small_format,
                        &text,
                        value[content_start..content_end].trim(),
                        D2D_RECT_F {
                            left: bubble_left + 78.0 * scale,
                            top: y + 7.0 * scale,
                            right: bubble_right - 10.0 * scale,
                            bottom: y + bubble_height - 6.0 * scale,
                        },
                    );
                }
            }
        }
        let caret_cells = scene
            .lines
            .get(scene.caret_line)
            .map(|line| Self::visual_cells(line, scene.caret_column))
            .unwrap_or(0);
        let caret_x = tree_w + (52.0 + caret_cells as f32 * self.code_cell) * scale;
        let caret_y = code_top + scene.caret_line as f32 * line_step;
        if (scene.animation_seconds * 2.0) as u32 & 1 == 0 {
            unsafe {
                Self::fill(
                    &target_base,
                    &bright,
                    caret_x,
                    caret_y,
                    caret_x + 1.5 * scale,
                    caret_y + 21.0 * scale,
                )
            };
        }
        if !scene.ime_composition.is_empty() {
            unsafe {
                Self::text(
                    &target_base,
                    &self.code_format,
                    &bright,
                    scene.ime_composition,
                    D2D_RECT_F {
                        left: caret_x + 2.0 * scale,
                        top: caret_y,
                        right: editor_right - 10.0 * scale,
                        bottom: caret_y + line_step,
                    },
                )
            };
            unsafe {
                Self::fill(
                    &target_base,
                    &accent,
                    caret_x + 2.0 * scale,
                    caret_y + 21.0 * scale,
                    caret_x
                        + (2.0
                            + Self::visual_cells(
                                scene.ime_composition,
                                scene.ime_composition.chars().count(),
                            ) as f32
                                * self.code_cell)
                            * scale,
                    caret_y + 23.0 * scale,
                )
            };
        }
        if !scene.completions.is_empty() && scene.ime_composition.is_empty() {
            let popup_top = caret_y + 27.0 * scale;
            let popup_width = 230.0 * scale;
            let popup_height = (scene.completions.len() as f32 * 28.0 + 10.0) * scale;
            unsafe {
                Self::rounded(
                    &target_base,
                    &raised,
                    D2D_RECT_F {
                        left: caret_x,
                        top: popup_top,
                        right: (caret_x + popup_width).min(editor_right - 8.0 * scale),
                        bottom: popup_top + popup_height,
                    },
                    9.0 * scale,
                )
            };
            for (index, candidate) in scene.completions.iter().enumerate() {
                if index == 0 {
                    unsafe {
                        Self::rounded(
                            &target_base,
                            &selection,
                            D2D_RECT_F {
                                left: caret_x + 5.0 * scale,
                                top: popup_top + (5.0 + index as f32 * 28.0) * scale,
                                right: (caret_x + popup_width - 5.0 * scale)
                                    .min(editor_right - 12.0 * scale),
                                bottom: popup_top + (31.0 + index as f32 * 28.0) * scale,
                            },
                            6.0 * scale,
                        )
                    };
                }
                unsafe {
                    Self::text(
                        &target_base,
                        &self.code_format,
                        if index == 0 { &bright } else { &text },
                        candidate,
                        D2D_RECT_F {
                            left: caret_x + 12.0 * scale,
                            top: popup_top + (8.0 + index as f32 * 28.0) * scale,
                            right: (caret_x + popup_width).min(editor_right),
                            bottom: popup_top + (34.0 + index as f32 * 28.0) * scale,
                        },
                    )
                };
            }
        }

        if let Some(focused) = scene.hovered_symbol {
            let value = scene
                .lines
                .get(focused.line)
                .map(String::as_str)
                .unwrap_or("");
            let left = tree_w
                + (49.0 + Self::visual_cells(value, focused.start) as f32 * self.code_cell) * scale;
            let right = tree_w
                + (55.0 + Self::visual_cells(value, focused.end) as f32 * self.code_cell) * scale;
            let top = code_top + focused.line as f32 * line_step - 3.0 * scale;
            unsafe {
                Self::outline(
                    &target_base,
                    &accent,
                    D2D_RECT_F {
                        left,
                        top,
                        right,
                        bottom: top + 25.0 * scale,
                    },
                    5.0 * scale,
                    1.5 * scale,
                )
            };
        }

        unsafe {
            Self::text(
                &target_base,
                &self.ui_format,
                &bright,
                if scene.ui_chinese {
                    "调用层级"
                } else {
                    "CALL HIERARCHY"
                },
                D2D_RECT_F {
                    left: editor_right + 18.0 * scale,
                    top: header_h + 14.0 * scale,
                    right: width,
                    bottom: header_h + 42.0 * scale,
                },
            )
        };
        let scope_rect = D2D_RECT_F {
            left: width - 150.0 * scale,
            top: header_h + 12.0 * scale,
            right: width - 18.0 * scale,
            bottom: header_h + 42.0 * scale,
        };
        unsafe { Self::rounded(&target_base, &selection, scope_rect, 8.0 * scale) };
        unsafe {
            Self::text(
                &target_base,
                &self.small_format,
                &accent,
                scene.graph_scope_label,
                D2D_RECT_F {
                    left: scope_rect.left + 12.0 * scale,
                    top: scope_rect.top + 5.0 * scale,
                    right: scope_rect.right - 8.0 * scale,
                    bottom: scope_rect.bottom,
                },
            )
        };
        let search_rect = D2D_RECT_F {
            left: editor_right + 18.0 * scale,
            top: header_h + 46.0 * scale,
            right: width - 18.0 * scale,
            bottom: header_h + 78.0 * scale,
        };
        unsafe {
            Self::rounded(
                &target_base,
                if scene.graph_search_focused {
                    &selection
                } else {
                    &raised
                },
                search_rect,
                8.0 * scale,
            )
        };
        let search_placeholder = if scene.ui_chinese {
            "搜索符号…"
        } else {
            "search symbols…"
        };
        let search_text = if scene.graph_search.is_empty() {
            search_placeholder
        } else {
            scene.graph_search
        };
        unsafe {
            Self::text(
                &target_base,
                &self.small_format,
                if scene.graph_search.is_empty() {
                    &muted
                } else {
                    &bright
                },
                search_text,
                D2D_RECT_F {
                    left: search_rect.left + 12.0 * scale,
                    top: search_rect.top + 6.0 * scale,
                    right: search_rect.right - 10.0 * scale,
                    bottom: search_rect.bottom,
                },
            )
        };
        let path_progress = (scene.animation_seconds / 0.65).clamp(0.0, 1.0);
        let path_x = graph_core::path_x(editor_right, scale);
        let first_y = header_h + graph_core::FIRST_ROW_TOP * scale;
        let row_height = graph_core::ROW_HEIGHT * scale;
        let last_y = first_y + scene.graph_nodes.len().saturating_sub(1) as f32 * row_height;
        if !scene.graph_nodes.is_empty() {
            unsafe {
                Self::fill(
                    &target_base,
                    &accent,
                    path_x,
                    first_y,
                    path_x + 2.0 * scale,
                    first_y + (last_y - first_y) * path_progress,
                )
            };
        }
        for (index, node) in scene.graph_nodes.iter().enumerate() {
            let y = first_y + index as f32 * row_height;
            let indent = graph_core::card_left(editor_right, node.depth, scale);
            let active = scene.graph_nodes.len() <= 1
                || path_progress >= index as f32 / scene.graph_nodes.len().saturating_sub(1) as f32;
            let selected = scene
                .graph_selected
                .map(|selected| selected == scene.graph_first_index + index)
                .unwrap_or(false);
            unsafe {
                Self::rounded(
                    &target_base,
                    if active { &selection } else { &raised },
                    D2D_RECT_F {
                        left: indent,
                        top: y - 12.0 * scale,
                        right: width - 18.0 * scale,
                        bottom: y + 28.0 * scale,
                    },
                    8.0 * scale,
                )
            };
            // Restrained differentiation per relation kind.
            let color = if selected {
                &accent
            } else {
                match node.edge {
                    Some(graph_core::GraphEdge::Declaration) => &bright,
                    Some(graph_core::GraphEdge::Caller) => &text,
                    Some(graph_core::GraphEdge::Callee) => &accent,
                    Some(graph_core::GraphEdge::Weak) => &comment,
                    Some(graph_core::GraphEdge::Inferred) => &muted,
                    None => match node.level {
                        graph_core::GraphLevel::Class => &keyword,
                        graph_core::GraphLevel::Occurrence | graph_core::GraphLevel::Folder => {
                            &muted
                        }
                        _ => {
                            if active {
                                &bright
                            } else {
                                &muted
                            }
                        }
                    },
                }
            };
            let marker = graph_core::marker_glyph(node);
            if !marker.is_empty() {
                let (marker_left, _) = graph_core::marker_span(editor_right, node.depth, scale);
                unsafe {
                    Self::text(
                        &target_base,
                        &self.small_format,
                        &accent,
                        marker,
                        D2D_RECT_F {
                            left: marker_left,
                            top: y,
                            right: marker_left + graph_core::MARKER_WIDTH * scale,
                            bottom: y + 25.0 * scale,
                        },
                    )
                };
            }
            unsafe {
                Self::text(
                    &target_base,
                    &self.small_format,
                    color,
                    &node.caption(),
                    D2D_RECT_F {
                        left: graph_core::caption_left(editor_right, node.depth, scale),
                        top: y,
                        right: (width - 190.0 * scale).max(
                            graph_core::caption_left(editor_right, node.depth, scale)
                                + 40.0 * scale,
                        ),
                        bottom: y + 25.0 * scale,
                    },
                )
            };
            if !node.detail.is_empty() {
                unsafe {
                    Self::text(
                        &target_base,
                        &self.small_format,
                        &muted,
                        &node.detail,
                        D2D_RECT_F {
                            left: width - 190.0 * scale,
                            top: y + 2.0 * scale,
                            right: width - 22.0 * scale,
                            bottom: y + 25.0 * scale,
                        },
                    )
                };
            }
        }
        unsafe {
            Self::text(
                &target_base,
                &self.small_format,
                &muted,
                scene.graph_summary,
                D2D_RECT_F {
                    left: editor_right + 18.0 * scale,
                    top: body_bottom - 34.0 * scale,
                    right: width - 12.0 * scale,
                    bottom: body_bottom - 8.0 * scale,
                },
            )
        };

        unsafe {
            Self::fill(
                &target_base,
                &raised,
                0.0,
                height - terminal_h,
                width,
                height,
            )
        };
        let terminal_label = if scene.terminal_open {
            let base = if scene.ui_chinese {
                "v  终端"
            } else {
                "v  TERMINAL"
            };
            if scene.terminal_scrolled > 0 {
                format!("{base}  ↑{}", scene.terminal_scrolled)
            } else {
                base.to_string()
            }
        } else if scene.ui_chinese {
            ">  终端".to_string()
        } else {
            ">  TERMINAL".to_string()
        };
        unsafe {
            Self::text(
                &target_base,
                &self.ui_format,
                &bright,
                &terminal_label,
                D2D_RECT_F {
                    left: 16.0 * scale,
                    top: height - terminal_h + 7.0 * scale,
                    right: 200.0 * scale,
                    bottom: height,
                },
            )
        };
        unsafe {
            Self::text(
                &target_base,
                &self.small_format,
                &accent,
                "Build     Run",
                D2D_RECT_F {
                    left: 160.0 * scale,
                    top: height - terminal_h + 9.0 * scale,
                    right: 310.0 * scale,
                    bottom: height - terminal_h + 32.0 * scale,
                },
            )
        };
        if terminal_progress > 0.08 {
            let output_top = terminal_panel.body_top;
            let terminal_right = terminal_panel.list_left;
            unsafe {
                Self::fill(
                    &target_base,
                    &line,
                    terminal_right,
                    height - terminal_h + 34.0 * scale,
                    terminal_right + 1.0 * scale,
                    height,
                );
                Self::text(
                    &target_base,
                    &self.ui_format,
                    &accent,
                    "+  New        ↗  Detach",
                    D2D_RECT_F {
                        left: terminal_right + 15.0 * scale,
                        top: output_top,
                        right: width - 8.0 * scale,
                        bottom: output_top + 28.0 * scale,
                    },
                );
            }
            for (index, name) in scene
                .terminal_names
                .iter()
                .take(terminal_core::PANEL_VISIBLE_ROWS)
                .enumerate()
            {
                let top = output_top + (34.0 + index as f32 * 30.0) * scale;
                if index == scene.active_terminal {
                    unsafe {
                        Self::rounded(
                            &target_base,
                            &selection,
                            D2D_RECT_F {
                                left: terminal_right + 8.0 * scale,
                                top,
                                right: width - 8.0 * scale,
                                bottom: top + 27.0 * scale,
                            },
                            6.0 * scale,
                        )
                    };
                }
                unsafe {
                    Self::text(
                        &target_base,
                        &self.small_format,
                        if index == scene.active_terminal {
                            &bright
                        } else {
                            &muted
                        },
                        name,
                        D2D_RECT_F {
                            left: terminal_right + 15.0 * scale,
                            top: top + 5.0 * scale,
                            right: width - 8.0 * scale,
                            bottom: top + 27.0 * scale,
                        },
                    )
                };
            }
            for (index, value) in scene.terminal_lines.iter().enumerate() {
                let row_top = output_top + index as f32 * terminal_panel.row_height;
                let line = scene.terminal_first_line + index;
                if let Some(((start_line, start_column), (end_line, end_column))) =
                    scene.terminal_selection
                {
                    if line >= start_line && line <= end_line {
                        let from = if line == start_line {
                            terminal_core::visual_cells(value, start_column)
                        } else {
                            0
                        };
                        let to = if line == end_line {
                            terminal_core::visual_cells(value, end_column)
                        } else {
                            value.chars().count()
                        };
                        if to > from {
                            let cell = self.code_cell * scale;
                            unsafe {
                                Self::rounded(
                                    &target_base,
                                    &selected_text,
                                    D2D_RECT_F {
                                        left: terminal_panel.body_left + from as f32 * cell,
                                        top: row_top,
                                        right: (terminal_panel.body_left + to as f32 * cell)
                                            .min(terminal_panel.body_right),
                                        bottom: row_top + terminal_panel.row_height,
                                    },
                                    3.0 * scale,
                                )
                            };
                        }
                    }
                }
                unsafe {
                    Self::text(
                        &target_base,
                        &self.code_format,
                        &text,
                        value,
                        D2D_RECT_F {
                            left: terminal_panel.body_left,
                            top: row_top,
                            right: terminal_panel.body_right,
                            bottom: row_top + terminal_panel.row_height,
                        },
                    )
                };
            }
            unsafe {
                Self::text(
                    &target_base,
                    &self.code_format,
                    &accent,
                    &format!("$ {}", scene.terminal_input),
                    D2D_RECT_F {
                        left: terminal_panel.body_left,
                        top: height - 30.0 * scale,
                        right: terminal_panel.body_right,
                        bottom: height,
                    },
                )
            };
        } else {
            unsafe {
                Self::text(
                    &target_base,
                    &self.small_format,
                    &muted,
                    scene.status,
                    D2D_RECT_F {
                        left: 160.0 * scale,
                        top: height - status_h + 7.0 * scale,
                        right: width,
                        bottom: height,
                    },
                )
            };
        }
        if scene.package_progress > 0.0 {
            let center_x = (tree_w + editor_right) * 0.5;
            let center_y = (header_h + body_bottom) * 0.5;
            let complete = !scene.package_running && scene.package_progress >= 1.0;
            for ring in 0..7 {
                let pulse = (scene.animation_seconds * 2.4 + ring as f32 * 0.37).sin() * 4.0;
                let radius = (22.0 + ring as f32 * 9.0 + pulse) * scale;
                let mut ellipse = D2D1_ELLIPSE::default();
                ellipse.point.X = center_x;
                ellipse.point.Y = center_y;
                ellipse.radiusX = radius * (1.0 + ring as f32 * 0.018);
                ellipse.radiusY = radius;
                unsafe {
                    target_base.DrawEllipse(
                        &ellipse,
                        if complete { &bright } else { &muted },
                        (1.0 + ring as f32 * 0.12) * scale,
                        None,
                    )
                };
            }
            for index in 0..18 {
                let source_x = tree_w + 35.0 * scale + (index % 6) as f32 * 74.0 * scale;
                let source_y = code_top + (index / 6) as f32 * 68.0 * scale;
                let travel = if complete {
                    (scene.animation_seconds * 1.8).fract()
                } else {
                    scene.package_progress.clamp(0.0, 1.0).powf(1.8)
                };
                let x = if complete {
                    center_x + (source_x - center_x) * travel
                } else {
                    source_x + (center_x - source_x) * travel
                };
                let y = if complete {
                    center_y + (source_y - center_y) * travel
                } else {
                    source_y + (center_y - source_y) * travel
                };
                let stretch = 5.0 + 24.0 * travel;
                unsafe {
                    Self::fill(
                        &target_base,
                        if complete { &bright } else { &text },
                        x - stretch * scale,
                        y - 1.5 * scale,
                        x + 5.0 * scale,
                        y + 1.5 * scale,
                    )
                };
            }
        }
        if scene.package_motion > 0.001 {
            let left = width - 326.0 * scale;
            let top = header_h + 8.0 * scale - (1.0 - scene.package_motion.min(1.0)) * 30.0 * scale;
            unsafe {
                Self::rounded(
                    &target_base,
                    &panel,
                    D2D_RECT_F {
                        left,
                        top,
                        right: width - 14.0 * scale,
                        bottom: top + 214.0 * scale,
                    },
                    14.0 * scale,
                )
            };
            let labels = [
                "1  Portable native artifact",
                "2  Portable ZIP",
                "3  MSIX (Windows SDK)",
            ];
            for (index, label) in labels.iter().enumerate() {
                let y = top + (48.0 + index as f32 * 42.0) * scale;
                unsafe {
                    Self::text(
                        &target_base,
                        &self.ui_format,
                        if scene.package_targets[index] {
                            &accent
                        } else {
                            &text
                        },
                        &format!(
                            "{}  {label}",
                            if scene.package_targets[index] {
                                "[x]"
                            } else {
                                "[ ]"
                            }
                        ),
                        D2D_RECT_F {
                            left: left + 18.0 * scale,
                            top: y,
                            right: width - 24.0 * scale,
                            bottom: y + 30.0 * scale,
                        },
                    )
                };
            }
            unsafe {
                Self::text(
                    &target_base,
                    &self.small_format,
                    &bright,
                    "Enter  Start packaging     Esc  Close",
                    D2D_RECT_F {
                        left: left + 18.0 * scale,
                        top: top + 178.0 * scale,
                        right: width - 24.0 * scale,
                        bottom: top + 206.0 * scale,
                    },
                )
            };
        }
        if scene.settings_motion > 0.001 {
            let panel_w = 390.0 * scale;
            let left = width - panel_w - 20.0 * scale;
            let top = (58.0 - (1.0 - scene.settings_motion.min(1.0)) * 34.0) * scale;
            unsafe {
                Self::rounded(
                    &target_base,
                    &panel,
                    D2D_RECT_F {
                        left,
                        top,
                        right: width - 20.0 * scale,
                        bottom: (top + 680.0 * scale).min(height - 20.0 * scale),
                    },
                    16.0 * scale,
                )
            };
            let settings = [
                (
                    24.0,
                    if scene.ui_chinese {
                        "设置".into()
                    } else {
                        "SETTINGS".into()
                    },
                ),
                (
                    78.0,
                    format!(
                        "L   Interface language: {}",
                        if scene.ui_chinese {
                            "简体中文"
                        } else {
                            "English"
                        }
                    ),
                ),
                (
                    132.0,
                    format!(
                        "T   Theme: {}",
                        if scene.light_theme { "Light" } else { "Dark" }
                    ),
                ),
                (
                    186.0,
                    format!("←→  Code font size: {}", 15 + scene.code_zoom),
                ),
                (
                    240.0,
                    format!(
                        "Font search: {}{}",
                        if scene.font_search.is_empty() {
                            "type to filter"
                        } else {
                            scene.font_search
                        },
                        if scene.font_search_focused { "  |" } else { "" }
                    ),
                ),
                (
                    464.0,
                    format!(
                        "B   Background: {}",
                        scene.background_path.unwrap_or("not selected")
                    ),
                ),
                (
                    518.0,
                    format!(
                        "O/P opacity {:.0}%   Z/X zoom {:.1}x",
                        scene.background_opacity * 100.0,
                        scene.background_zoom
                    ),
                ),
                (572.0, "C   Clear background".into()),
                (626.0, "Esc  Close settings".into()),
            ];
            for (index, (row_y, value)) in settings.iter().enumerate() {
                unsafe {
                    Self::text(
                        &target_base,
                        if index == 0 {
                            &self.ui_format
                        } else {
                            &self.small_format
                        },
                        if index == 0 {
                            &bright
                        } else if index < 4 {
                            &text
                        } else {
                            &muted
                        },
                        value,
                        D2D_RECT_F {
                            left: left + 22.0 * scale,
                            top: top + row_y * scale,
                            right: width - 38.0 * scale,
                            bottom: top + (row_y + 28.0) * scale,
                        },
                    )
                };
            }
            let needle = scene.font_search.trim().to_ascii_lowercase();
            for (visible, (index, family)) in scene
                .font_options
                .iter()
                .enumerate()
                .filter(|(_, family)| {
                    needle.is_empty() || family.to_ascii_lowercase().contains(&needle)
                })
                .take(5)
                .enumerate()
            {
                let row_top = top + (282.0 + visible as f32 * 34.0) * scale;
                if index == scene.code_font_family {
                    unsafe {
                        Self::rounded(
                            &target_base,
                            &selection,
                            D2D_RECT_F {
                                left: left + 18.0 * scale,
                                top: row_top - 4.0 * scale,
                                right: width - 34.0 * scale,
                                bottom: row_top + 28.0 * scale,
                            },
                            6.0 * scale,
                        )
                    };
                }
                unsafe {
                    Self::text(
                        &target_base,
                        &self.small_format,
                        if index == scene.code_font_family {
                            &bright
                        } else {
                            &text
                        },
                        &format!(
                            "{}{}",
                            if index == scene.code_font_family {
                                "✓  "
                            } else {
                                "   "
                            },
                            family
                        ),
                        D2D_RECT_F {
                            left: left + 28.0 * scale,
                            top: row_top,
                            right: width - 42.0 * scale,
                            bottom: row_top + 28.0 * scale,
                        },
                    )
                };
            }
        }
        if scene.extensions_motion > 0.001 {
            let panel_w = (920.0 * scale).min(width - 50.0 * scale);
            let panel_h = (610.0 * scale).min(height - 80.0 * scale);
            let left = (width - panel_w) * 0.5;
            let top = (58.0 - (1.0 - scene.extensions_motion.min(1.0)) * 34.0) * scale;
            unsafe {
                Self::rounded(
                    &target_base,
                    &panel,
                    D2D_RECT_F {
                        left,
                        top,
                        right: left + panel_w,
                        bottom: top + panel_h,
                    },
                    16.0 * scale,
                )
            };
            let categories = [
                "Languages",
                "Themes",
                "Debuggers",
                "Formatters",
                "AI / Agents",
            ];
            unsafe {
                Self::text(
                    &target_base,
                    &self.ui_format,
                    &bright,
                    if scene.ui_chinese {
                        "扩展"
                    } else {
                        "EXTENSIONS"
                    },
                    D2D_RECT_F {
                        left: left + 22.0 * scale,
                        top: top + 20.0 * scale,
                        right: left + panel_w,
                        bottom: top + 50.0 * scale,
                    },
                )
            };
            for (index, category) in categories.iter().enumerate() {
                let y = top + (78.0 + index as f32 * 44.0) * scale;
                if index == scene.extension_category {
                    unsafe {
                        Self::rounded(
                            &target_base,
                            &selection,
                            D2D_RECT_F {
                                left: left + 14.0 * scale,
                                top: y - 20.0 * scale,
                                right: left + 172.0 * scale,
                                bottom: y + 14.0 * scale,
                            },
                            7.0 * scale,
                        )
                    };
                }
                unsafe {
                    Self::text(
                        &target_base,
                        &self.small_format,
                        if index == scene.extension_category {
                            &bright
                        } else {
                            &text
                        },
                        category,
                        D2D_RECT_F {
                            left: left + 26.0 * scale,
                            top: y - 8.0 * scale,
                            right: left + 168.0 * scale,
                            bottom: y + 20.0 * scale,
                        },
                    )
                };
            }
            let content_left = left + 205.0 * scale;
            if scene.extension_category == 0 {
                unsafe {
                    Self::rounded(
                        &target_base,
                        if scene.extension_search_focused {
                            &selection
                        } else {
                            &raised
                        },
                        D2D_RECT_F {
                            left: content_left,
                            top: top + 72.0 * scale,
                            right: left + panel_w - 24.0 * scale,
                            bottom: top + 114.0 * scale,
                        },
                        8.0 * scale,
                    );
                    Self::text(
                        &target_base,
                        &self.small_format,
                        &text,
                        if scene.extension_search.is_empty() {
                            "Search programming languages"
                        } else {
                            scene.extension_search
                        },
                        D2D_RECT_F {
                            left: content_left + 14.0 * scale,
                            top: top + 83.0 * scale,
                            right: left + panel_w - 36.0 * scale,
                            bottom: top + 110.0 * scale,
                        },
                    );
                }
                let needle = scene.extension_search.trim().to_ascii_lowercase();
                for (visible, (index, language)) in scene
                    .language_options
                    .iter()
                    .enumerate()
                    .filter(|(_, language)| {
                        needle.is_empty() || language.to_ascii_lowercase().contains(&needle)
                    })
                    .skip(scene.extension_scroll)
                    .take(8)
                    .enumerate()
                {
                    let y = top + (142.0 + visible as f32 * 48.0) * scale;
                    if index == scene.selected_language {
                        unsafe {
                            Self::rounded(
                                &target_base,
                                &selection,
                                D2D_RECT_F {
                                    left: content_left,
                                    top: y - 22.0 * scale,
                                    right: content_left + 300.0 * scale,
                                    bottom: y + 16.0 * scale,
                                },
                                7.0 * scale,
                            )
                        };
                    }
                    let installed = scene
                        .language_installed
                        .get(index)
                        .copied()
                        .unwrap_or(false);
                    unsafe {
                        Self::text(
                            &target_base,
                            &self.small_format,
                            &text,
                            language,
                            D2D_RECT_F {
                                left: content_left + 12.0 * scale,
                                top: y - 9.0 * scale,
                                right: content_left + 160.0 * scale,
                                bottom: y + 18.0 * scale,
                            },
                        )
                    };
                    unsafe {
                        Self::text(
                            &target_base,
                            &self.small_format,
                            &accent,
                            Self::language_version(
                                index,
                                scene.language_versions.get(index).copied().unwrap_or(0),
                            ),
                            D2D_RECT_F {
                                left: content_left + 110.0 * scale,
                                top: y - 9.0 * scale,
                                right: content_left + 190.0 * scale,
                                bottom: y + 18.0 * scale,
                            },
                        )
                    };
                    unsafe {
                        Self::text(
                            &target_base,
                            &self.small_format,
                            if installed { &accent } else { &muted },
                            if installed {
                                "INSTALLED"
                            } else {
                                "BUILT-IN DETECTION"
                            },
                            D2D_RECT_F {
                                left: content_left + 240.0 * scale,
                                top: y - 9.0 * scale,
                                right: content_left + 390.0 * scale,
                                bottom: y + 18.0 * scale,
                            },
                        )
                    };
                }
                let version = Self::language_version(
                    scene.selected_language,
                    scene
                        .language_versions
                        .get(scene.selected_language)
                        .copied()
                        .unwrap_or(0),
                );
                unsafe {
                    Self::text(
                        &target_base,
                        &self.small_format,
                        &muted,
                        &format!("↑↓ Select  ·  V Version {version}  ·  Space Install/remove"),
                        D2D_RECT_F {
                            left: content_left,
                            top: top + panel_h - 48.0 * scale,
                            right: left + panel_w,
                            bottom: top + panel_h - 18.0 * scale,
                        },
                    )
                };
            } else {
                let detail = match scene.extension_category {
                    1 => "Dark and light semantic themes apply immediately.",
                    2 => "Native debugger adapters · click a tool to check the Windows runtime.",
                    3 => "Native formatter adapters · click a tool to check the Windows runtime.",
                    _ => {
                        "OpenAI-compatible endpoint, model and session-only API key configuration."
                    }
                };
                unsafe {
                    Self::text(
                        &target_base,
                        &self.ui_format,
                        &bright,
                        categories[scene.extension_category],
                        D2D_RECT_F {
                            left: content_left,
                            top: top + 82.0 * scale,
                            right: left + panel_w - 20.0 * scale,
                            bottom: top + 118.0 * scale,
                        },
                    )
                };
                unsafe {
                    Self::text(
                        &target_base,
                        &self.small_format,
                        &muted,
                        detail,
                        D2D_RECT_F {
                            left: content_left,
                            top: top + 132.0 * scale,
                            right: left + panel_w - 30.0 * scale,
                            bottom: top + 220.0 * scale,
                        },
                    )
                };
                if scene.extension_category == 1 {
                    for (index, label) in ["Dark", "Light"].iter().enumerate() {
                        let active =
                            (index == 0 && !scene.light_theme) || (index == 1 && scene.light_theme);
                        let row_top = top + (190.0 + index as f32 * 50.0) * scale;
                        unsafe {
                            Self::rounded(
                                &target_base,
                                if active { &selection } else { &raised },
                                D2D_RECT_F {
                                    left: content_left,
                                    top: row_top,
                                    right: content_left + 315.0 * scale,
                                    bottom: row_top + 40.0 * scale,
                                },
                                8.0 * scale,
                            );
                            Self::text(
                                &target_base,
                                &self.small_format,
                                if active { &bright } else { &text },
                                &format!("{}{}", label, if active { "    ACTIVE" } else { "" }),
                                D2D_RECT_F {
                                    left: content_left + 16.0 * scale,
                                    top: row_top + 10.0 * scale,
                                    right: content_left + 300.0 * scale,
                                    bottom: row_top + 36.0 * scale,
                                },
                            );
                        }
                    }
                }
                if matches!(scene.extension_category, 2 | 3) {
                    let items = if scene.extension_category == 2 {
                        ["LLDB", "GDB", "debugpy"]
                    } else {
                        ["Biome", "clang-format", "Ruff"]
                    };
                    for (index, label) in items.iter().enumerate() {
                        let row_top = top + (190.0 + index as f32 * 50.0) * scale;
                        unsafe {
                            Self::rounded(
                                &target_base,
                                &raised,
                                D2D_RECT_F {
                                    left: content_left,
                                    top: row_top,
                                    right: content_left + 360.0 * scale,
                                    bottom: row_top + 40.0 * scale,
                                },
                                8.0 * scale,
                            );
                            Self::text(
                                &target_base,
                                &self.small_format,
                                &text,
                                &format!("{label}    CHECK"),
                                D2D_RECT_F {
                                    left: content_left + 16.0 * scale,
                                    top: row_top + 10.0 * scale,
                                    right: content_left + 340.0 * scale,
                                    bottom: row_top + 36.0 * scale,
                                },
                            );
                        }
                    }
                }
                if scene.extension_category == 4 {
                    for (index, (label, value)) in [
                        ("ENDPOINT", scene.ai_endpoint),
                        ("MODEL", scene.ai_model),
                        (
                            "API KEY (session only)",
                            if scene.ai_key_present {
                                "****************"
                            } else {
                                ""
                            },
                        ),
                    ]
                    .iter()
                    .enumerate()
                    {
                        let label_top = top + (116.0 + index as f32 * 92.0) * scale;
                        unsafe {
                            Self::text(
                                &target_base,
                                &self.small_format,
                                &muted,
                                label,
                                D2D_RECT_F {
                                    left: content_left,
                                    top: label_top,
                                    right: content_left + 600.0 * scale,
                                    bottom: label_top + 24.0 * scale,
                                },
                            );
                            Self::rounded(
                                &target_base,
                                if scene.ai_field_focused == Some(index) {
                                    &selection
                                } else {
                                    &bg
                                },
                                D2D_RECT_F {
                                    left: content_left,
                                    top: label_top + 26.0 * scale,
                                    right: content_left + 600.0 * scale,
                                    bottom: label_top + 68.0 * scale,
                                },
                                7.0 * scale,
                            );
                            Self::text(
                                &target_base,
                                &self.small_format,
                                &text,
                                value,
                                D2D_RECT_F {
                                    left: content_left + 12.0 * scale,
                                    top: label_top + 36.0 * scale,
                                    right: content_left + 586.0 * scale,
                                    bottom: label_top + 64.0 * scale,
                                },
                            );
                        }
                    }
                    unsafe {
                        Self::rounded(
                            &target_base,
                            &accent,
                            D2D_RECT_F {
                                left: content_left,
                                top: top + 440.0 * scale,
                                right: content_left + 210.0 * scale,
                                bottom: top + 482.0 * scale,
                            },
                            8.0 * scale,
                        );
                        Self::text(
                            &target_base,
                            &self.small_format,
                            &bg,
                            if scene.ui_chinese {
                                "保存 API 配置"
                            } else {
                                "SAVE API SETTINGS"
                            },
                            D2D_RECT_F {
                                left: content_left + 28.0 * scale,
                                top: top + 451.0 * scale,
                                right: content_left + 200.0 * scale,
                                bottom: top + 478.0 * scale,
                            },
                        );
                    }
                }
            }
        }
        if let Some(kind) = scene.create_kind {
            let dialog_w = (520.0 * scale).min(width - 40.0 * scale);
            let dialog_h = if kind == "New project" { 350.0 } else { 230.0 } * scale;
            let left = (width - dialog_w) * 0.5;
            let top =
                (height - dialog_h) * 0.42 - (1.0 - scene.create_motion.min(1.0)) * 28.0 * scale;
            unsafe {
                Self::rounded(
                    &target_base,
                    &panel,
                    D2D_RECT_F {
                        left,
                        top,
                        right: left + dialog_w,
                        bottom: top + dialog_h,
                    },
                    16.0 * scale,
                )
            };
            unsafe {
                Self::text(
                    &target_base,
                    &self.ui_format,
                    &bright,
                    kind,
                    D2D_RECT_F {
                        left: left + 24.0 * scale,
                        top: top + 22.0 * scale,
                        right: left + dialog_w,
                        bottom: top + 52.0 * scale,
                    },
                )
            };
            unsafe {
                Self::rounded(
                    &target_base,
                    &selection,
                    D2D_RECT_F {
                        left: left + 24.0 * scale,
                        top: top + 70.0 * scale,
                        right: left + dialog_w - 24.0 * scale,
                        bottom: top + 112.0 * scale,
                    },
                    9.0 * scale,
                )
            };
            unsafe {
                Self::text(
                    &target_base,
                    &self.code_format,
                    &bright,
                    if scene.create_name.is_empty() {
                        "name"
                    } else {
                        scene.create_name
                    },
                    D2D_RECT_F {
                        left: left + 38.0 * scale,
                        top: top + 81.0 * scale,
                        right: left + dialog_w - 32.0 * scale,
                        bottom: top + 108.0 * scale,
                    },
                )
            };
            unsafe {
                Self::text(
                    &target_base,
                    &self.small_format,
                    if scene.create_error.is_empty() {
                        &muted
                    } else {
                        &literal
                    },
                    if scene.create_error.is_empty() {
                        if scene.ui_chinese {
                            "输入名称后点击确定，或使用 Enter / Esc"
                        } else {
                            "Enter a name, then click Create or Cancel"
                        }
                    } else {
                        scene.create_error
                    },
                    D2D_RECT_F {
                        left: left + 26.0 * scale,
                        top: top + 132.0 * scale,
                        right: left + dialog_w - 26.0 * scale,
                        bottom: top + 170.0 * scale,
                    },
                )
            };
            if kind == "New project" {
                unsafe {
                    Self::text(
                        &target_base,
                        &self.small_format,
                        &text,
                        &format!(
                            "↑↓ Language: {}    ←→ Environment: {}",
                            scene.create_language, scene.create_environment
                        ),
                        D2D_RECT_F {
                            left: left + 26.0 * scale,
                            top: top + 172.0 * scale,
                            right: left + dialog_w - 26.0 * scale,
                            bottom: top + 204.0 * scale,
                        },
                    )
                };
                unsafe {
                    Self::text(
                        &target_base,
                        &self.small_format,
                        &accent,
                        &format!("Version (click): {}", scene.create_version),
                        D2D_RECT_F {
                            left: left + 26.0 * scale,
                            top: top + 204.0 * scale,
                            right: left + dialog_w - 26.0 * scale,
                            bottom: top + 236.0 * scale,
                        },
                    )
                };
                unsafe {
                    Self::text(
                        &target_base,
                        &self.small_format,
                        &muted,
                        &format!("Location: {}", scene.create_location),
                        D2D_RECT_F {
                            left: left + 26.0 * scale,
                            top: top + 242.0 * scale,
                            right: left + dialog_w - 26.0 * scale,
                            bottom: top + 270.0 * scale,
                        },
                    )
                };
            }
            let button_top = top + dialog_h - 56.0 * scale;
            unsafe {
                Self::rounded(
                    &target_base,
                    &raised,
                    D2D_RECT_F {
                        left: left + 24.0 * scale,
                        top: button_top,
                        right: left + dialog_w * 0.5 - 8.0 * scale,
                        bottom: button_top + 36.0 * scale,
                    },
                    8.0 * scale,
                );
                Self::rounded(
                    &target_base,
                    &selection,
                    D2D_RECT_F {
                        left: left + dialog_w * 0.5 + 8.0 * scale,
                        top: button_top,
                        right: left + dialog_w - 24.0 * scale,
                        bottom: button_top + 36.0 * scale,
                    },
                    8.0 * scale,
                );
                Self::text(
                    &target_base,
                    &self.small_format,
                    &text,
                    if scene.ui_chinese { "取消" } else { "Cancel" },
                    D2D_RECT_F {
                        left: left + 86.0 * scale,
                        top: button_top + 9.0 * scale,
                        right: left + dialog_w * 0.5,
                        bottom: button_top + 34.0 * scale,
                    },
                );
                Self::text(
                    &target_base,
                    &self.small_format,
                    &bright,
                    if scene.ui_chinese { "确定" } else { "Create" },
                    D2D_RECT_F {
                        left: left + dialog_w * 0.5 + 82.0 * scale,
                        top: button_top + 9.0 * scale,
                        right: left + dialog_w - 28.0 * scale,
                        bottom: button_top + 34.0 * scale,
                    },
                );
            }
        }
        if let Err(error) = unsafe { target_base.EndDraw(None, None) } {
            self.target = None;
            return Err(error);
        }
        Ok(())
    }
}
