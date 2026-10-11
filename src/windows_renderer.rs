use std::ffi::c_void;

use windows::core::{w, Interface, Result};
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Direct2D::Common::{
    D2D1_ALPHA_MODE_UNKNOWN, D2D1_COLOR_F, D2D1_PIXEL_FORMAT, D2D_RECT_F, D2D_SIZE_U,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1CreateFactory, ID2D1Factory, ID2D1HwndRenderTarget, ID2D1RenderTarget,
    ID2D1SolidColorBrush, D2D1_DRAW_TEXT_OPTIONS_NONE, D2D1_FACTORY_TYPE_SINGLE_THREADED,
    D2D1_FEATURE_LEVEL_DEFAULT, D2D1_HWND_RENDER_TARGET_PROPERTIES, D2D1_PRESENT_OPTIONS_NONE,
    D2D1_RENDER_TARGET_PROPERTIES, D2D1_RENDER_TARGET_TYPE_DEFAULT, D2D1_RENDER_TARGET_USAGE_NONE,
    D2D1_ROUNDED_RECT,
};
use windows::Win32::Graphics::DirectWrite::{
    DWriteCreateFactory, IDWriteFactory, IDWriteTextFormat, DWRITE_FACTORY_TYPE_SHARED,
    DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT_NORMAL,
    DWRITE_MEASURING_MODE_NATURAL,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_UNKNOWN;

pub struct WindowsScene<'a> {
    pub file_name: &'a str,
    pub lines: &'a [String],
    pub caret_line: usize,
    pub caret_column: usize,
    pub status: &'a str,
    pub terminal_lines: &'a [String],
    pub terminal_input: &'a str,
    pub terminal_open: bool,
    pub terminal_progress: f32,
    pub animation_seconds: f32,
    pub edit_age: Option<f32>,
}

pub struct DirectEditorRenderer {
    d2d_factory: ID2D1Factory,
    _dwrite_factory: IDWriteFactory,
    target: Option<ID2D1HwndRenderTarget>,
    code_format: IDWriteTextFormat,
    ui_format: IDWriteTextFormat,
    small_format: IDWriteTextFormat,
    dpi: f32,
}

impl DirectEditorRenderer {
    pub unsafe fn new(dpi: u32) -> Result<Self> {
        let d2d_factory: ID2D1Factory =
            unsafe { D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None) }?;
        let dwrite_factory: IDWriteFactory =
            unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED) }?;
        let scale = dpi.max(96) as f32 / 96.0;
        let code_format = unsafe {
            dwrite_factory.CreateTextFormat(
                w!("Cascadia Mono"),
                None,
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                15.0 * scale,
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
            dpi: dpi.max(96) as f32,
        })
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
        let bg = unsafe { Self::brush(&target_base, 0x000000, 1.0) }?;
        let panel = unsafe { Self::brush(&target_base, 0x1c1c1e, 1.0) }?;
        let raised = unsafe { Self::brush(&target_base, 0x2c2c2e, 1.0) }?;
        let line = unsafe { Self::brush(&target_base, 0x38383a, 1.0) }?;
        let text = unsafe { Self::brush(&target_base, 0xd1d1d6, 1.0) }?;
        let muted = unsafe { Self::brush(&target_base, 0x8e8e93, 1.0) }?;
        let bright = unsafe { Self::brush(&target_base, 0xf5f5f7, 1.0) }?;
        let accent = unsafe { Self::brush(&target_base, 0x0a84ff, 1.0) }?;
        let selection = unsafe { Self::brush(&target_base, 0x193451, 1.0) }?;
        let width = width as f32;
        let height = height as f32;
        let scale = self.dpi / 96.0;
        let header_h = 48.0 * scale;
        let status_h = 28.0 * scale;
        let terminal_progress = scene.terminal_progress.clamp(0.0, 1.0);
        let terminal_h = (28.0 + 214.0 * terminal_progress) * scale;
        let body_bottom = height - terminal_h;
        let tree_w = (width * 0.19).max(220.0 * scale);
        let graph_w = (width * 0.27).max(300.0 * scale);
        let editor_right = width - graph_w;

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

        let tree_items = ["PROJECT", "TC-IDE", "src", scene.file_name];
        for (index, item) in tree_items.iter().enumerate() {
            let delay = index as f32 * 0.045;
            let local = ((scene.animation_seconds - delay) / 0.28).clamp(0.0, 1.0);
            let eased = 1.0 - (1.0 - local).powi(3);
            let y = header_h + (38.0 + index as f32 * 35.0) * scale;
            let x = (18.0 + index.saturating_sub(1) as f32 * 14.0) * scale
                - (1.0 - eased) * 18.0 * scale;
            unsafe {
                Self::text(
                    &target_base,
                    &self.ui_format,
                    if index == 3 { &bright } else { &text },
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

        let code_top = header_h + 58.0 * scale;
        let line_step = 25.0 * scale;
        let max_lines = ((body_bottom - code_top) / line_step).max(0.0) as usize;
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
            let code_brush = if value.trim_start().starts_with("//") {
                &muted
            } else if value.contains("fn ") || value.contains("struct ") {
                &accent
            } else {
                &text
            };
            unsafe {
                Self::text(
                    &target_base,
                    &self.code_format,
                    code_brush,
                    value,
                    D2D_RECT_F {
                        left: tree_w + 52.0 * scale + edit_offset,
                        top: y,
                        right: editor_right - 10.0 * scale,
                        bottom: y + line_step,
                    },
                )
            };
        }
        let caret_x = tree_w + (52.0 + scene.caret_column as f32 * 8.9) * scale;
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

        unsafe {
            Self::text(
                &target_base,
                &self.ui_format,
                &bright,
                "CALL HIERARCHY",
                D2D_RECT_F {
                    left: editor_right + 18.0 * scale,
                    top: header_h + 14.0 * scale,
                    right: width,
                    bottom: header_h + 42.0 * scale,
                },
            )
        };
        let nodes = [
            "project  TC-IDE",
            "file  main.rs",
            "fn  render()",
            "fn  terminal()",
        ];
        let path_progress = (scene.animation_seconds / 0.65).clamp(0.0, 1.0);
        let path_x = editor_right + 30.0 * scale;
        let first_y = header_h + 86.0 * scale;
        let last_y = first_y + 3.0 * 58.0 * scale;
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
        for (index, node) in nodes.iter().enumerate() {
            let y = first_y + index as f32 * 58.0 * scale;
            let active = path_progress >= index as f32 / (nodes.len() - 1) as f32;
            unsafe {
                Self::rounded(
                    &target_base,
                    if active { &selection } else { &raised },
                    D2D_RECT_F {
                        left: path_x + 12.0 * scale,
                        top: y - 12.0 * scale,
                        right: width - 18.0 * scale,
                        bottom: y + 28.0 * scale,
                    },
                    8.0 * scale,
                )
            };
            unsafe {
                Self::text(
                    &target_base,
                    &self.small_format,
                    if active { &bright } else { &muted },
                    node,
                    D2D_RECT_F {
                        left: path_x + 26.0 * scale,
                        top: y,
                        right: width,
                        bottom: y + 25.0 * scale,
                    },
                )
            };
        }

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
        unsafe {
            Self::text(
                &target_base,
                &self.ui_format,
                &bright,
                if scene.terminal_open {
                    "v  TERMINAL"
                } else {
                    ">  TERMINAL"
                },
                D2D_RECT_F {
                    left: 16.0 * scale,
                    top: height - terminal_h + 7.0 * scale,
                    right: 150.0 * scale,
                    bottom: height,
                },
            )
        };
        if terminal_progress > 0.08 {
            let output_top = height - terminal_h + 38.0 * scale;
            for (index, value) in scene.terminal_lines.iter().rev().take(6).rev().enumerate() {
                unsafe {
                    Self::text(
                        &target_base,
                        &self.code_format,
                        &text,
                        value,
                        D2D_RECT_F {
                            left: 22.0 * scale,
                            top: output_top + index as f32 * 24.0 * scale,
                            right: width - 20.0 * scale,
                            bottom: output_top + (index as f32 + 1.0) * 24.0 * scale,
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
                        left: 22.0 * scale,
                        top: height - 30.0 * scale,
                        right: width - 20.0 * scale,
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
        if let Err(error) = unsafe { target_base.EndDraw(None, None) } {
            self.target = None;
            return Err(error);
        }
        Ok(())
    }
}
