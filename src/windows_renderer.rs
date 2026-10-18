use std::ffi::c_void;

use windows::core::{w, Interface, Result, PCWSTR};
use windows::Win32::Foundation::{GENERIC_READ, HWND};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D1_ALPHA_MODE_UNKNOWN, D2D1_COLOR_F, D2D1_PIXEL_FORMAT, D2D_RECT_F, D2D_SIZE_U,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1CreateFactory, ID2D1Bitmap, ID2D1Factory, ID2D1HwndRenderTarget, ID2D1RenderTarget,
    ID2D1SolidColorBrush, D2D1_ANTIALIAS_MODE_PER_PRIMITIVE, D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
    D2D1_DRAW_TEXT_OPTIONS_NONE, D2D1_ELLIPSE, D2D1_FACTORY_TYPE_SINGLE_THREADED,
    D2D1_FEATURE_LEVEL_DEFAULT, D2D1_HWND_RENDER_TARGET_PROPERTIES, D2D1_PRESENT_OPTIONS_NONE,
    D2D1_RENDER_TARGET_PROPERTIES, D2D1_RENDER_TARGET_TYPE_DEFAULT, D2D1_RENDER_TARGET_USAGE_NONE,
    D2D1_ROUNDED_RECT,
};
use windows::Win32::Graphics::DirectWrite::{
    DWriteCreateFactory, IDWriteFactory, IDWriteTextFormat, DWRITE_FACTORY_TYPE_SHARED,
    DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT_NORMAL,
    DWRITE_MEASURING_MODE_NATURAL, DWRITE_TEXT_METRICS,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_UNKNOWN;
use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory, GUID_WICPixelFormat32bppPBGRA, IWICImagingFactory,
    WICBitmapDitherTypeNone, WICBitmapPaletteTypeMedianCut, WICDecodeMetadataCacheOnLoad,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
};

use crate::background_core;
use crate::editor_core::{self, SymbolOccurrence, TokenKind};
use crate::graph_core;
use crate::package_motion;
use crate::terminal_core;
use crate::theme_core;

pub struct WindowsScene<'a> {
    pub file_name: &'a str,
    pub project_name: &'a str,
    pub project_entries: &'a [String],
    /// Row index each project entry is drawn at, so rows slide when a folder
    /// folds instead of jumping.
    pub tree_row_positions: &'a [f32],
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
    /// First visible column, in characters.
    pub scroll_columns: usize,
    /// First visible line and total line count of the document.
    pub first_line: usize,
    pub total_lines: usize,
    pub selection: Option<((usize, usize), (usize, usize))>,
    pub ime_composition: &'a str,
    pub completions: &'a [String],
    pub light_theme: bool,
    pub ui_chinese: bool,
    pub code_zoom: i32,
    pub code_font_family: usize,
    pub font_search: &'a str,
    pub font_search_focused: bool,
    pub font_options: &'a [&'a str],
    pub settings_motion: f32,
    /// Row the keyboard is on in the settings panel, when it is on one.
    pub settings_focus: Option<background_core::SettingsRow>,
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
    pub ai_prompt: &'a str,
    pub ai_prompt_focused: bool,
    pub ai_cache_summary: &'a str,
    /// Problems panel: diagnostics, debug state and breakpoints.
    pub problems_open: bool,
    pub problems: &'a [String],
    pub problems_selected: usize,
    pub problems_hint: &'a str,
    /// Real installation state of the selected language's tooling.
    pub language_tooling: &'a str,
    /// AI panel: conversation, input line, run state and the last error.
    pub chat_open: bool,
    pub chat_messages: &'a [(bool, String)],
    pub chat_input: &'a str,
    pub chat_running: bool,
    pub chat_error: &'a str,
    pub ai_field_focused: Option<usize>,
    pub symbols: &'a [SymbolOccurrence],
    /// Symbol retained while its relation route springs out after hover ends.
    pub relation_symbol: Option<&'a SymbolOccurrence>,
    pub relation_focus_motion: f32,
    /// Titles of the open documents.
    pub tabs: &'a [String],
    pub active_tab: usize,
    pub tab_dirty: Vec<bool>,
    /// `(line, is_error)` pairs the gutter marks.
    pub diagnostic_lines: &'a [(usize, bool)],
    /// Breakpoint lines, relative to the first visible line.
    pub breakpoint_lines: &'a [usize],
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
    pub comment_expansion: &'a [(usize, f32)],
    pub comment_bubbles_motion: f32,
    pub editing_comment: Option<usize>,
    pub pointer_x: i32,
    /// Whether the left mouse button is down while a control is hovered.
    pub pointer_pressed: bool,
    pub pointer_y: i32,
    /// Project tree totals, for its scrollbar.
    pub project_total: usize,
    pub project_scroll: usize,
    pub package_motion: f32,
    pub package_targets: [bool; 3],
    /// Whether the project can produce the executable the MSIX target wraps.
    pub msix_available: bool,
    /// File name of the configured MSIX signing certificate, if any.
    pub signing_certificate: &'a str,
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
    /// Fade of the highlight of the newest edit on the caret line.
    pub edit_pulse: Option<f32>,
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
    /// Pixel dimensions of the decoded background used by both drawing and
    /// the settings crop hit testing.
    pub fn background_size(&self) -> Option<(f64, f64)> {
        self.background_bitmap.as_ref().map(|bitmap| {
            let size = unsafe { bitmap.GetSize() };
            (size.width.max(1.0) as f64, size.height.max(1.0) as f64)
        })
    }
}

/// Advance of one character cell in the configured code font, in logical pixels.
///
/// DirectWrite measures the real glyphs, so the caret, the selection and the
/// relation rails line up with what is drawn instead of relying on a constant.
unsafe fn measure_code_cell(
    factory: &IDWriteFactory,
    format: &IDWriteTextFormat,
    code_zoom: i32,
    scale: f32,
) -> f32 {
    const SAMPLE: &str = "0123456789";
    let fallback = terminal_core::code_cell_width(code_zoom);
    let sample = SAMPLE.encode_utf16().collect::<Vec<_>>();
    let Ok(layout) = (unsafe { factory.CreateTextLayout(&sample, format, 4096.0, 128.0) }) else {
        return fallback;
    };
    let mut metrics = DWRITE_TEXT_METRICS::default();
    if unsafe { layout.GetMetrics(&mut metrics) }.is_err() {
        return fallback;
    }
    let width = metrics.widthIncludingTrailingWhitespace.max(metrics.width);
    if !width.is_finite() || width <= 0.0 {
        return fallback;
    }
    let cell = width / SAMPLE.chars().count() as f32 / scale.max(0.5);
    if !cell.is_finite() || !(2.0..40.0).contains(&cell) {
        return fallback;
    }
    cell
}

/// Settings row label in the interface language.
fn settings_row_text(scene: &WindowsScene<'_>, index: usize, fallback: &str) -> String {
    if !scene.ui_chinese {
        return fallback.to_string();
    }
    match index {
        0 => "设置".to_string(),
        1 => format!(
            "L   界面语言：{}",
            if scene.ui_chinese {
                "简体中文"
            } else {
                "English"
            }
        ),
        2 => format!(
            "T   主题：{}",
            if scene.light_theme {
                "浅色"
            } else {
                "深色"
            }
        ),
        3 => format!("←→  代码字号：{}", 15 + scene.code_zoom),
        4 => format!(
            "字体搜索：{}{}",
            if scene.font_search.is_empty() {
                "输入以筛选"
            } else {
                scene.font_search
            },
            if scene.font_search_focused { "  |" } else { "" }
        ),
        5 => format!("B   背景图：{}", scene.background_path.unwrap_or("未选择")),
        6 => format!(
            "O/P 不透明度 {:.0}%   Z/X 缩放 {:.1}x",
            scene.background_opacity * 100.0,
            scene.background_zoom
        ),
        7 => "C   清除背景".to_string(),
        8 => "Esc  关闭设置".to_string(),
        _ => fallback.to_string(),
    }
}

/// The state of one settings row: not every row can be used at any time.
///
/// Clearing the background, changing its opacity or zooming it mean nothing
/// while no picture is selected, so those rows are drawn disabled and say why
/// instead of quietly doing nothing.
fn settings_row_state(
    scene: &WindowsScene<'_>,
    index: usize,
    rect: D2D_RECT_F,
) -> theme_core::ControlState {
    // Whether a row can be used comes from the shared rule, so the Windows and
    // Linux panels disable the same controls for the same reason.
    let controls = background_core::background_controls(
        scene.background_path.is_some(),
        scene.background_opacity,
        scene.background_zoom as f64,
    );
    let enabled = match index {
        // Row 5 chooses the picture, 6 changes its opacity and zoom, 7 clears
        // it; the other rows are always usable.
        5 => true,
        6 => controls.has_image && (controls.opacity_up || controls.opacity_down),
        7 => controls.clearable,
        _ => true,
    };
    if !enabled {
        return theme_core::ControlState::Disabled;
    }
    if !hovering(
        scene.pointer_x,
        scene.pointer_y,
        rect.left,
        rect.top,
        rect.right,
        rect.bottom,
    ) {
        return theme_core::ControlState::Normal;
    }
    if scene.pointer_pressed {
        theme_core::ControlState::Pressed
    } else {
        theme_core::ControlState::Hovered
    }
}

/// Whether the pointer is inside a rectangle, for hover feedback.
fn hovering(pointer_x: i32, pointer_y: i32, left: f32, top: f32, right: f32, bottom: f32) -> bool {
    (pointer_x as f32) >= left
        && (pointer_x as f32) <= right
        && (pointer_y as f32) >= top
        && (pointer_y as f32) <= bottom
}

/// The brushes and geometry of one frame, handed to the region painters below.
///
/// `render` keeps the frame together as a readable sequence of scene regions;
/// each region draws itself against this shared state instead of living in one
/// several thousand line function.
struct ScenePaint<'a, 's> {
    renderer: &'a DirectEditorRenderer,
    target: &'a ID2D1RenderTarget,
    scene: &'s WindowsScene<'s>,
    accent: ID2D1SolidColorBrush,
    bg: ID2D1SolidColorBrush,
    bright: ID2D1SolidColorBrush,
    /// Quiet separator colour, used for the thin rules between regions.
    divider: ID2D1SolidColorBrush,
    literal: ID2D1SolidColorBrush,
    muted: ID2D1SolidColorBrush,
    panel: ID2D1SolidColorBrush,
    raised: ID2D1SolidColorBrush,
    selection: ID2D1SolidColorBrush,
    selected_text: ID2D1SolidColorBrush,
    text: ID2D1SolidColorBrush,
    height: f32,
    scale: f32,
    width: f32,
    header_h: f32,
    code_top: f32,
    body_bottom: f32,
    editor_right: f32,
    tree_w: f32,
    terminal_h: f32,
    terminal_panel: terminal_core::TerminalPanel,
    terminal_progress: f32,
    status_h: f32,
}

impl ScenePaint<'_, '_> {
    /// State of a control, from where the pointer is and whether it is down.
    fn control_state(
        &self,
        left: f32,
        top: f32,
        right: f32,
        bottom: f32,
        enabled: bool,
    ) -> theme_core::ControlState {
        if !enabled {
            return theme_core::ControlState::Disabled;
        }
        if !hovering(
            self.scene.pointer_x,
            self.scene.pointer_y,
            left,
            top,
            right,
            bottom,
        ) {
            return theme_core::ControlState::Normal;
        }
        if self.scene.pointer_pressed {
            theme_core::ControlState::Pressed
        } else {
            theme_core::ControlState::Hovered
        }
    }

    /// Paint the terminal panel region.
    fn draw_terminal_panel(&self) {
        if self.terminal_progress > 0.08 {
            let output_top = self.terminal_panel.body_top;
            let terminal_right = self.terminal_panel.list_left;
            unsafe {
                DirectEditorRenderer::fill(
                    self.target,
                    &self.divider,
                    terminal_right,
                    self.height - self.terminal_h + 34.0 * self.scale,
                    terminal_right + 1.0 * self.scale,
                    self.height,
                );
                DirectEditorRenderer::text(
                    self.target,
                    &self.renderer.ui_format,
                    &self.accent,
                    "+  New        ↗  Detach",
                    D2D_RECT_F {
                        left: terminal_right + 15.0 * self.scale,
                        top: output_top,
                        right: self.width - 8.0 * self.scale,
                        bottom: output_top + 28.0 * self.scale,
                    },
                );
            }
            for (index, name) in self
                .scene
                .terminal_names
                .iter()
                .take(terminal_core::PANEL_VISIBLE_ROWS)
                .enumerate()
            {
                let top = output_top + (34.0 + index as f32 * 30.0) * self.scale;
                if index == self.scene.active_terminal {
                    unsafe {
                        DirectEditorRenderer::rounded(
                            self.target,
                            &self.selection,
                            D2D_RECT_F {
                                left: terminal_right + 8.0 * self.scale,
                                top,
                                right: self.width - 8.0 * self.scale,
                                bottom: top + 27.0 * self.scale,
                            },
                            6.0 * self.scale,
                        )
                    };
                }
                unsafe {
                    DirectEditorRenderer::text(
                        self.target,
                        &self.renderer.small_format,
                        if index == self.scene.active_terminal {
                            &self.bright
                        } else {
                            &self.muted
                        },
                        name,
                        D2D_RECT_F {
                            left: terminal_right + 15.0 * self.scale,
                            top: top + 5.0 * self.scale,
                            right: self.width - 8.0 * self.scale,
                            bottom: top + 27.0 * self.scale,
                        },
                    )
                };
            }
            for (index, value) in self.scene.terminal_lines.iter().enumerate() {
                let row_top = output_top + index as f32 * self.terminal_panel.row_height;
                let line = self.scene.terminal_first_line + index;
                if let Some(((start_line, start_column), (end_line, end_column))) =
                    self.scene.terminal_selection
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
                            let cell = self.renderer.code_cell * self.scale;
                            unsafe {
                                DirectEditorRenderer::rounded(
                                    self.target,
                                    &self.selected_text,
                                    D2D_RECT_F {
                                        left: self.terminal_panel.body_left + from as f32 * cell,
                                        top: row_top,
                                        right: (self.terminal_panel.body_left + to as f32 * cell)
                                            .min(self.terminal_panel.body_right),
                                        bottom: row_top + self.terminal_panel.row_height,
                                    },
                                    3.0 * self.scale,
                                )
                            };
                        }
                    }
                }
                unsafe {
                    DirectEditorRenderer::text(
                        self.target,
                        &self.renderer.code_format,
                        &self.text,
                        value,
                        D2D_RECT_F {
                            left: self.terminal_panel.body_left,
                            top: row_top,
                            right: self.terminal_panel.body_right,
                            bottom: row_top + self.terminal_panel.row_height,
                        },
                    )
                };
            }
            unsafe {
                DirectEditorRenderer::text(
                    self.target,
                    &self.renderer.code_format,
                    &self.accent,
                    &format!("$ {}", self.scene.terminal_input),
                    D2D_RECT_F {
                        left: self.terminal_panel.body_left,
                        top: self.height - 30.0 * self.scale,
                        right: self.terminal_panel.body_right,
                        bottom: self.height,
                    },
                )
            };
        } else {
            unsafe {
                DirectEditorRenderer::text(
                    self.target,
                    &self.renderer.small_format,
                    &self.muted,
                    self.scene.status,
                    D2D_RECT_F {
                        left: 160.0 * self.scale,
                        top: self.height - self.status_h + 7.0 * self.scale,
                        right: self.width,
                        bottom: self.height,
                    },
                )
            };
        }
    }

    /// Paint the package progress region.
    fn draw_package_progress(&self) {
        if self.scene.package_progress > 0.0 {
            let center_x = (self.tree_w + self.editor_right) * 0.5;
            let center_y = (self.header_h + self.body_bottom) * 0.5;
            let complete = !self.scene.package_running && self.scene.package_progress >= 1.0;
            for ring in 0..7 {
                let pulse = (self.scene.animation_seconds * 2.4 + ring as f32 * 0.37).sin() * 4.0;
                let radius = (22.0 + ring as f32 * 9.0 + pulse) * self.scale;
                let mut ellipse = D2D1_ELLIPSE::default();
                ellipse.point.X = center_x;
                ellipse.point.Y = center_y;
                ellipse.radiusX = radius * (1.0 + ring as f32 * 0.018);
                ellipse.radiusY = radius;
                unsafe {
                    self.target.DrawEllipse(
                        &ellipse,
                        if complete { &self.bright } else { &self.muted },
                        (1.0 + ring as f32 * 0.12) * self.scale,
                        None,
                    )
                };
            }
            // The same absorb as the Linux backend: the sources furthest from
            // the core leave first, so they all arrive together.
            let centre = (center_x, center_y);
            let sources = (0..18)
                .map(|index| {
                    (
                        self.tree_w + 35.0 * self.scale + (index % 6) as f32 * 74.0 * self.scale,
                        self.code_top + (index / 6) as f32 * 68.0 * self.scale,
                    )
                })
                .collect::<Vec<_>>();
            let max_distance = sources
                .iter()
                .map(|(x, y)| ((x - centre.0).powi(2) + (y - centre.1).powi(2)).sqrt())
                .fold(0.0_f32, f32::max);
            let phase = if complete {
                1.0
            } else {
                self.scene.package_progress.clamp(0.0, 1.0)
            };
            let half = 1.5 * self.scale;
            for (index, source) in sources.iter().enumerate() {
                let travel = package_motion::absorbed(
                    *source,
                    centre,
                    index,
                    sources.len(),
                    max_distance,
                    phase,
                );
                if complete || travel.progress >= 1.0 {
                    // Once the build is done the sources have been absorbed;
                    // sending them back out as white blocks only added noise.
                    continue;
                }
                let head = if travel.progress > 0.85 {
                    &self.muted
                } else {
                    &self.text
                };
                unsafe {
                    if travel.trail > package_motion::MIN_TRAIL * self.scale {
                        let tail = travel.trail;
                        DirectEditorRenderer::fill(
                            self.target,
                            &self.muted,
                            travel.x - tail,
                            travel.y - 0.5 * self.scale,
                            travel.x,
                            travel.y + 0.5 * self.scale,
                        );
                    }
                    DirectEditorRenderer::fill(
                        self.target,
                        head,
                        travel.x - half,
                        travel.y - half,
                        travel.x + half,
                        travel.y + half,
                    )
                };
            }
        }
    }

    /// Paint the package overlay region.
    fn draw_package_overlay(&self) {
        if self.scene.package_motion > 0.001 {
            let left = self.width - 326.0 * self.scale;
            let top = self.header_h + 8.0 * self.scale
                - (1.0 - self.scene.package_motion.min(1.0)) * 30.0 * self.scale;
            unsafe {
                DirectEditorRenderer::rounded(
                    self.target,
                    &self.panel,
                    D2D_RECT_F {
                        left,
                        top,
                        right: self.width - 14.0 * self.scale,
                        bottom: top + 240.0 * self.scale,
                    },
                    14.0 * self.scale,
                )
            };
            let labels = if self.scene.ui_chinese {
                [
                    "1  原生可执行文件",
                    "2  Portable ZIP",
                    "3  MSIX（Windows SDK）",
                ]
            } else {
                [
                    "1  Portable native artifact",
                    "2  Portable ZIP",
                    "3  MSIX (Windows SDK)",
                ]
            };
            for (index, label) in labels.iter().enumerate() {
                let y = top + (48.0 + index as f32 * 42.0) * self.scale;
                // The MSIX row is greyed out for a project that cannot produce
                // the executable it wraps.
                let enabled = index != 2 || self.scene.msix_available;
                let state = self.control_state(
                    left + 10.0 * self.scale,
                    y - 6.0 * self.scale,
                    self.width - 20.0 * self.scale,
                    y + 30.0 * self.scale,
                    enabled,
                );
                let palette = theme_core::Palette::for_scheme(self.scene.light_theme);
                let visual = palette.control(state, self.scene.package_targets[index]);
                let (fill, label_color) = unsafe {
                    (
                        DirectEditorRenderer::brush(self.target, visual.fill, visual.opacity).ok(),
                        DirectEditorRenderer::brush(self.target, visual.text, 1.0).ok(),
                    )
                };
                if state != theme_core::ControlState::Normal {
                    if let Some(fill) = fill.as_ref() {
                        unsafe {
                            DirectEditorRenderer::rounded(
                                self.target,
                                fill,
                                D2D_RECT_F {
                                    left: left + 10.0 * self.scale,
                                    top: y - 6.0 * self.scale,
                                    right: self.width - 20.0 * self.scale,
                                    bottom: y + 30.0 * self.scale,
                                },
                                6.0 * self.scale,
                            )
                        };
                    }
                }
                unsafe {
                    DirectEditorRenderer::text(
                        self.target,
                        &self.renderer.ui_format,
                        if !enabled {
                            &self.muted
                        } else if self.scene.package_targets[index] {
                            &self.accent
                        } else {
                            label_color.as_ref().unwrap_or(&self.text)
                        },
                        &format!(
                            "{}  {label}",
                            if self.scene.package_targets[index] {
                                "[x]"
                            } else {
                                "[ ]"
                            }
                        ),
                        D2D_RECT_F {
                            left: left + 18.0 * self.scale,
                            top: y,
                            right: self.width - 24.0 * self.scale,
                            bottom: y + 30.0 * self.scale,
                        },
                    )
                };
            }
            let certificate = if self.scene.signing_certificate.is_empty() {
                if self.scene.ui_chinese {
                    "4  签名证书：未选择（点击选择）".to_string()
                } else {
                    "4  Signing certificate: none (click to choose)".to_string()
                }
            } else if self.scene.ui_chinese {
                format!("4  签名证书：{}", self.scene.signing_certificate)
            } else {
                format!("4  Signing certificate: {}", self.scene.signing_certificate)
            };
            unsafe {
                DirectEditorRenderer::text(
                    self.target,
                    &self.renderer.small_format,
                    if self.scene.signing_certificate.is_empty() {
                        &self.muted
                    } else {
                        &self.accent
                    },
                    &certificate,
                    D2D_RECT_F {
                        left: left + 18.0 * self.scale,
                        top: top + 172.0 * self.scale,
                        right: self.width - 24.0 * self.scale,
                        bottom: top + 198.0 * self.scale,
                    },
                );
                DirectEditorRenderer::text(
                    self.target,
                    &self.renderer.small_format,
                    &self.bright,
                    if self.scene.ui_chinese {
                        "Enter  开始打包     Esc  关闭"
                    } else {
                        "Enter  Start packaging     Esc  Close"
                    },
                    D2D_RECT_F {
                        left: left + 18.0 * self.scale,
                        top: top + 194.0 * self.scale,
                        right: self.width - 24.0 * self.scale,
                        bottom: top + 222.0 * self.scale,
                    },
                )
            };
        }
    }

    /// Paint the settings overlay region.
    fn draw_settings_overlay(&self) {
        if self.scene.settings_motion > 0.001 {
            let panel_w = 390.0 * self.scale;
            let left = self.width - panel_w - 20.0 * self.scale;
            let top = (58.0 - (1.0 - self.scene.settings_motion.min(1.0)) * 34.0) * self.scale;
            unsafe {
                DirectEditorRenderer::rounded(
                    self.target,
                    &self.panel,
                    D2D_RECT_F {
                        left,
                        top,
                        right: self.width - 20.0 * self.scale,
                        bottom: (top + 742.0 * self.scale).min(self.height - 12.0 * self.scale),
                    },
                    16.0 * self.scale,
                )
            };
            let settings = [
                (
                    24.0,
                    if self.scene.ui_chinese {
                        "设置".into()
                    } else {
                        "SETTINGS".into()
                    },
                ),
                (
                    78.0,
                    format!(
                        "L   Interface language: {}",
                        if self.scene.ui_chinese {
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
                        if self.scene.light_theme {
                            "Light"
                        } else {
                            "Dark"
                        }
                    ),
                ),
                (
                    186.0,
                    format!("←→  Code font size: {}", 15 + self.scene.code_zoom),
                ),
                (
                    240.0,
                    format!(
                        "Font search: {}{}",
                        if self.scene.font_search.is_empty() {
                            "type to filter"
                        } else {
                            self.scene.font_search
                        },
                        if self.scene.font_search_focused {
                            "  |"
                        } else {
                            ""
                        }
                    ),
                ),
                (
                    400.0,
                    format!(
                        "B   Background: {}",
                        self.scene.background_path.unwrap_or("not selected")
                    ),
                ),
                (
                    594.0,
                    format!(
                        "O/P opacity {:.0}%   Z/X zoom {:.1}x",
                        self.scene.background_opacity * 100.0,
                        self.scene.background_zoom
                    ),
                ),
                (640.0, "C   Clear background".into()),
                (686.0, "Esc  Close settings".into()),
            ];
            for (index, (row_y, value)) in settings.iter().enumerate() {
                // Rows 1..4 are clickable, so they light up under the pointer.
                let row_top = top + row_y * self.scale;
                let row_bottom = row_top + (if index == 5 { 34.0 } else { 54.0 }) * self.scale;
                // Every clickable row is one control: it can be pressed, it can
                // be disabled when it would do nothing, and it lifts as the
                // pointer crosses it, all from the shared control model.
                let clickable = (1..=9).contains(&index);
                let row_left = left + 16.0 * self.scale;
                let row_right = self.width - 30.0 * self.scale;
                let row_rect = D2D_RECT_F {
                    left: row_left,
                    top: row_top - 6.0 * self.scale,
                    right: row_right,
                    bottom: row_bottom - 6.0 * self.scale,
                };
                let state = if clickable {
                    settings_row_state(self.scene, index, row_rect)
                } else {
                    theme_core::ControlState::Disabled
                };
                let palette = theme_core::Palette::for_scheme(self.scene.light_theme);
                let visual = palette.control(state, false);
                // The fill and the text colour both come from the shared control
                // model, so a row looks the same whether it is pressed, hovered
                // or disabled.
                let (fill, row_text) = unsafe {
                    (
                        DirectEditorRenderer::brush(self.target, visual.fill, visual.opacity).ok(),
                        DirectEditorRenderer::brush(self.target, visual.text, 1.0).ok(),
                    )
                };
                let focused = clickable
                    && self
                        .scene
                        .settings_focus
                        .map(|row| row == background_core::SettingsRow::all()[index])
                        .unwrap_or(false);
                if clickable && (state != theme_core::ControlState::Normal || focused) {
                    if let Some(fill) = fill.as_ref() {
                        unsafe {
                            DirectEditorRenderer::rounded(
                                self.target,
                                fill,
                                row_rect,
                                6.0 * self.scale,
                            )
                        };
                    }
                }
                // The row the keyboard is on wears an accent ring, so Tab is
                // visible instead of silent.
                if focused {
                    unsafe {
                        DirectEditorRenderer::outline(
                            self.target,
                            &self.accent,
                            row_rect,
                            6.0 * self.scale,
                            2.0 * self.scale,
                        )
                    };
                }
                let row_enabled = clickable && state != theme_core::ControlState::Disabled;
                let _ = row_enabled;
                let (row_y, value) = (*row_y, settings_row_text(self.scene, index, value));
                unsafe {
                    DirectEditorRenderer::text(
                        self.target,
                        if index == 0 {
                            &self.renderer.ui_format
                        } else {
                            &self.renderer.small_format
                        },
                        if index == 0 {
                            &self.bright
                        } else if clickable {
                            row_text.as_ref().unwrap_or(&self.text)
                        } else {
                            &self.muted
                        },
                        &value,
                        D2D_RECT_F {
                            left: left + 22.0 * self.scale,
                            top: top + row_y * self.scale,
                            right: self.width - 38.0 * self.scale,
                            bottom: top + (row_y + 28.0) * self.scale,
                        },
                    )
                };
            }
            let needle = self.scene.font_search.trim().to_ascii_lowercase();
            for (visible, (index, family)) in self
                .scene
                .font_options
                .iter()
                .enumerate()
                .filter(|(_, family)| {
                    needle.is_empty() || family.to_ascii_lowercase().contains(&needle)
                })
                .take(3)
                .enumerate()
            {
                let row_top = top + (282.0 + visible as f32 * 34.0) * self.scale;
                if index == self.scene.code_font_family {
                    unsafe {
                        DirectEditorRenderer::rounded(
                            self.target,
                            &self.selection,
                            D2D_RECT_F {
                                left: left + 18.0 * self.scale,
                                top: row_top - 4.0 * self.scale,
                                right: self.width - 34.0 * self.scale,
                                bottom: row_top + 28.0 * self.scale,
                            },
                            6.0 * self.scale,
                        )
                    };
                }
                unsafe {
                    DirectEditorRenderer::text(
                        self.target,
                        &self.renderer.small_format,
                        if index == self.scene.code_font_family {
                            &self.bright
                        } else {
                            &self.text
                        },
                        &format!(
                            "{}{}",
                            if index == self.scene.code_font_family {
                                "✓  "
                            } else {
                                "   "
                            },
                            family
                        ),
                        D2D_RECT_F {
                            left: left + 28.0 * self.scale,
                            top: row_top,
                            right: self.width - 42.0 * self.scale,
                            bottom: row_top + 28.0 * self.scale,
                        },
                    )
                };
            }
            if let (Some(bitmap), Some(image)) = (
                self.renderer.background_bitmap.as_ref(),
                self.renderer.background_size(),
            ) {
                let preview = D2D_RECT_F {
                    left: left + 18.0 * self.scale,
                    top: top + 444.0 * self.scale,
                    right: self.width - 34.0 * self.scale,
                    bottom: top + 576.0 * self.scale,
                };
                let preview_size = (
                    (preview.right - preview.left) as f64,
                    (preview.bottom - preview.top) as f64,
                );
                let fitted = background_core::preview_placement(image, preview_size);
                let destination = D2D_RECT_F {
                    left: preview.left + fitted.origin.0 as f32,
                    top: preview.top + fitted.origin.1 as f32,
                    right: preview.left + fitted.origin.0 as f32 + fitted.drawn.0 as f32,
                    bottom: preview.top + fitted.origin.1 as f32 + fitted.drawn.1 as f32,
                };
                unsafe {
                    self.target.DrawBitmap(
                        bitmap,
                        Some(&destination),
                        0.86,
                        D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
                        None,
                    );
                }
                let view = (
                    (self.editor_right - self.tree_w).max(1.0) as f64,
                    (self.body_bottom - self.header_h - 38.0 * self.scale).max(1.0) as f64,
                );
                let placed = background_core::placement(
                    image,
                    view,
                    self.scene.background_zoom as f64,
                    (
                        self.scene.background_offset_x as f64,
                        self.scene.background_offset_y as f64,
                    ),
                );
                let crop = placed.crop(view, image);
                let crop = fitted.project(&crop);
                let crop = D2D_RECT_F {
                    left: preview.left + crop.x as f32,
                    top: preview.top + crop.y as f32,
                    right: preview.left + crop.right() as f32,
                    bottom: preview.top + crop.bottom() as f32,
                };
                let thickness = 2.0 * self.scale;
                unsafe {
                    DirectEditorRenderer::fill(
                        self.target,
                        &self.accent,
                        crop.left,
                        crop.top,
                        crop.right,
                        crop.top + thickness,
                    );
                    DirectEditorRenderer::fill(
                        self.target,
                        &self.accent,
                        crop.left,
                        crop.bottom - thickness,
                        crop.right,
                        crop.bottom,
                    );
                    DirectEditorRenderer::fill(
                        self.target,
                        &self.accent,
                        crop.left,
                        crop.top,
                        crop.left + thickness,
                        crop.bottom,
                    );
                    DirectEditorRenderer::fill(
                        self.target,
                        &self.accent,
                        crop.right - thickness,
                        crop.top,
                        crop.right,
                        crop.bottom,
                    );
                    for (x, y) in [
                        (crop.left, crop.top),
                        (crop.right, crop.top),
                        (crop.right, crop.bottom),
                        (crop.left, crop.bottom),
                    ] {
                        DirectEditorRenderer::fill(
                            self.target,
                            &self.accent,
                            x - 4.0 * self.scale,
                            y - 4.0 * self.scale,
                            x + 4.0 * self.scale,
                            y + 4.0 * self.scale,
                        );
                    }
                    DirectEditorRenderer::text(
                        self.target,
                        &self.renderer.small_format,
                        &self.muted,
                        if self.scene.ui_chinese {
                            "拖动取景框裁剪，拖角缩放"
                        } else {
                            "Drag frame to crop · corners resize"
                        },
                        D2D_RECT_F {
                            left: preview.left,
                            top: preview.top - 20.0 * self.scale,
                            right: preview.right,
                            bottom: preview.top,
                        },
                    );
                }
            }
        }
    }

    /// Paint the extensions overlay region.
    fn draw_extensions_overlay(&self) {
        if self.scene.extensions_motion > 0.001 {
            let panel_w = (920.0 * self.scale).min(self.width - 50.0 * self.scale);
            let panel_h = (610.0 * self.scale).min(self.height - 80.0 * self.scale);
            let left = (self.width - panel_w) * 0.5;
            let top = (58.0 - (1.0 - self.scene.extensions_motion.min(1.0)) * 34.0) * self.scale;
            unsafe {
                DirectEditorRenderer::rounded(
                    self.target,
                    &self.panel,
                    D2D_RECT_F {
                        left,
                        top,
                        right: left + panel_w,
                        bottom: top + panel_h,
                    },
                    16.0 * self.scale,
                )
            };
            let categories = if self.scene.ui_chinese {
                ["语言", "主题", "调试器", "格式化器", "AI / 代理"]
            } else {
                [
                    "Languages",
                    "Themes",
                    "Debuggers",
                    "Formatters",
                    "AI / Agents",
                ]
            };
            unsafe {
                DirectEditorRenderer::text(
                    self.target,
                    &self.renderer.ui_format,
                    &self.bright,
                    if self.scene.ui_chinese {
                        "扩展"
                    } else {
                        "EXTENSIONS"
                    },
                    D2D_RECT_F {
                        left: left + 22.0 * self.scale,
                        top: top + 20.0 * self.scale,
                        right: left + panel_w,
                        bottom: top + 50.0 * self.scale,
                    },
                )
            };
            let palette = theme_core::Palette::for_scheme(self.scene.light_theme);
            for (index, category) in categories.iter().enumerate() {
                let y = top + (78.0 + index as f32 * 44.0) * self.scale;
                // A category is a control like any other: the selected one stays
                // filled, and the one under the pointer lifts or presses.
                let selected = index == self.scene.extension_category;
                let state = self.control_state(
                    left + 14.0 * self.scale,
                    y - 20.0 * self.scale,
                    left + 172.0 * self.scale,
                    y + 14.0 * self.scale,
                    true,
                );
                let visual = palette.control(state, selected);
                let (fill, label) = unsafe {
                    (
                        DirectEditorRenderer::brush(self.target, visual.fill, visual.opacity).ok(),
                        DirectEditorRenderer::brush(
                            self.target,
                            if selected {
                                palette.bright
                            } else {
                                visual.text
                            },
                            1.0,
                        )
                        .ok(),
                    )
                };
                if selected || state != theme_core::ControlState::Normal {
                    if let Some(fill) = fill.as_ref() {
                        unsafe {
                            DirectEditorRenderer::rounded(
                                self.target,
                                fill,
                                D2D_RECT_F {
                                    left: left + 14.0 * self.scale,
                                    top: y - 20.0 * self.scale,
                                    right: left + 172.0 * self.scale,
                                    bottom: y + 14.0 * self.scale,
                                },
                                7.0 * self.scale,
                            )
                        };
                    }
                }
                unsafe {
                    DirectEditorRenderer::text(
                        self.target,
                        &self.renderer.small_format,
                        label.as_ref().unwrap_or(&self.text),
                        category,
                        D2D_RECT_F {
                            left: left + 26.0 * self.scale,
                            top: y - 8.0 * self.scale,
                            right: left + 168.0 * self.scale,
                            bottom: y + 20.0 * self.scale,
                        },
                    )
                };
            }
            let content_left = left + 205.0 * self.scale;
            if self.scene.extension_category == 0 {
                unsafe {
                    DirectEditorRenderer::rounded(
                        self.target,
                        if self.scene.extension_search_focused {
                            &self.selection
                        } else {
                            &self.raised
                        },
                        D2D_RECT_F {
                            left: content_left,
                            top: top + 72.0 * self.scale,
                            right: left + panel_w - 24.0 * self.scale,
                            bottom: top + 114.0 * self.scale,
                        },
                        8.0 * self.scale,
                    );
                    DirectEditorRenderer::text(
                        self.target,
                        &self.renderer.small_format,
                        &self.text,
                        if self.scene.extension_search.is_empty() {
                            "Search programming languages"
                        } else {
                            self.scene.extension_search
                        },
                        D2D_RECT_F {
                            left: content_left + 14.0 * self.scale,
                            top: top + 83.0 * self.scale,
                            right: left + panel_w - 36.0 * self.scale,
                            bottom: top + 110.0 * self.scale,
                        },
                    );
                }
                let needle = self.scene.extension_search.trim().to_ascii_lowercase();
                for (visible, (index, language)) in self
                    .scene
                    .language_options
                    .iter()
                    .enumerate()
                    .filter(|(_, language)| {
                        needle.is_empty() || language.to_ascii_lowercase().contains(&needle)
                    })
                    .skip(self.scene.extension_scroll)
                    .take(8)
                    .enumerate()
                {
                    let y = top + (142.0 + visible as f32 * 48.0) * self.scale;
                    if index == self.scene.selected_language {
                        unsafe {
                            DirectEditorRenderer::rounded(
                                self.target,
                                &self.selection,
                                D2D_RECT_F {
                                    left: content_left,
                                    top: y - 22.0 * self.scale,
                                    right: content_left + 300.0 * self.scale,
                                    bottom: y + 16.0 * self.scale,
                                },
                                7.0 * self.scale,
                            )
                        };
                    }
                    let installed = self
                        .scene
                        .language_installed
                        .get(index)
                        .copied()
                        .unwrap_or(false);
                    unsafe {
                        DirectEditorRenderer::text(
                            self.target,
                            &self.renderer.small_format,
                            &self.text,
                            language,
                            D2D_RECT_F {
                                left: content_left + 12.0 * self.scale,
                                top: y - 9.0 * self.scale,
                                right: content_left + 160.0 * self.scale,
                                bottom: y + 18.0 * self.scale,
                            },
                        )
                    };
                    unsafe {
                        DirectEditorRenderer::text(
                            self.target,
                            &self.renderer.small_format,
                            &self.accent,
                            DirectEditorRenderer::language_version(
                                index,
                                self.scene
                                    .language_versions
                                    .get(index)
                                    .copied()
                                    .unwrap_or(0),
                            ),
                            D2D_RECT_F {
                                left: content_left + 110.0 * self.scale,
                                top: y - 9.0 * self.scale,
                                right: content_left + 190.0 * self.scale,
                                bottom: y + 18.0 * self.scale,
                            },
                        )
                    };
                    unsafe {
                        DirectEditorRenderer::text(
                            self.target,
                            &self.renderer.small_format,
                            if installed { &self.accent } else { &self.muted },
                            if installed {
                                "INSTALLED"
                            } else {
                                "BUILT-IN DETECTION"
                            },
                            D2D_RECT_F {
                                left: content_left + 240.0 * self.scale,
                                top: y - 9.0 * self.scale,
                                right: content_left + 390.0 * self.scale,
                                bottom: y + 18.0 * self.scale,
                            },
                        )
                    };
                }
                let version = DirectEditorRenderer::language_version(
                    self.scene.selected_language,
                    self.scene
                        .language_versions
                        .get(self.scene.selected_language)
                        .copied()
                        .unwrap_or(0),
                );
                unsafe {
                    DirectEditorRenderer::text(
                        self.target,
                        &self.renderer.small_format,
                        &self.muted,
                        &format!("↑↓ Select  ·  V Version {version}  ·  Space Install/remove"),
                        D2D_RECT_F {
                            left: content_left,
                            top: top + panel_h - 48.0 * self.scale,
                            right: left + panel_w,
                            bottom: top + panel_h - 18.0 * self.scale,
                        },
                    )
                };
            } else {
                let detail = match self.scene.extension_category {
                    1 => "Dark and light semantic themes apply immediately.",
                    2 => "Native debugger adapters · click a tool to check the Windows runtime.",
                    3 => "Native formatter adapters · click a tool to check the Windows runtime.",
                    _ => {
                        "OpenAI-compatible endpoint, model and session-only API key configuration."
                    }
                };
                unsafe {
                    DirectEditorRenderer::text(
                        self.target,
                        &self.renderer.ui_format,
                        &self.bright,
                        categories[self.scene.extension_category],
                        D2D_RECT_F {
                            left: content_left,
                            top: top + 82.0 * self.scale,
                            right: left + panel_w - 20.0 * self.scale,
                            bottom: top + 118.0 * self.scale,
                        },
                    )
                };
                unsafe {
                    DirectEditorRenderer::text(
                        self.target,
                        &self.renderer.small_format,
                        &self.muted,
                        detail,
                        D2D_RECT_F {
                            left: content_left,
                            top: top + 132.0 * self.scale,
                            right: left + panel_w - 30.0 * self.scale,
                            bottom: top + 220.0 * self.scale,
                        },
                    )
                };
                if self.scene.extension_category == 1 {
                    for (index, label) in ["Dark", "Light"].iter().enumerate() {
                        let active = (index == 0 && !self.scene.light_theme)
                            || (index == 1 && self.scene.light_theme);
                        let row_top = top + (190.0 + index as f32 * 50.0) * self.scale;
                        unsafe {
                            DirectEditorRenderer::rounded(
                                self.target,
                                if active {
                                    &self.selection
                                } else {
                                    &self.raised
                                },
                                D2D_RECT_F {
                                    left: content_left,
                                    top: row_top,
                                    right: content_left + 315.0 * self.scale,
                                    bottom: row_top + 40.0 * self.scale,
                                },
                                8.0 * self.scale,
                            );
                            DirectEditorRenderer::text(
                                self.target,
                                &self.renderer.small_format,
                                if active { &self.bright } else { &self.text },
                                &format!(
                                    "{}{}",
                                    label,
                                    if active {
                                        if self.scene.ui_chinese {
                                            "    已启用"
                                        } else {
                                            "    ACTIVE"
                                        }
                                    } else {
                                        ""
                                    }
                                ),
                                D2D_RECT_F {
                                    left: content_left + 16.0 * self.scale,
                                    top: row_top + 10.0 * self.scale,
                                    right: content_left + 300.0 * self.scale,
                                    bottom: row_top + 36.0 * self.scale,
                                },
                            );
                        }
                    }
                }
                if self.scene.extension_category == 0 {
                    // The language page reports what is really installed.
                    unsafe {
                        DirectEditorRenderer::text(
                            self.target,
                            &self.renderer.small_format,
                            &self.muted,
                            self.scene.language_tooling,
                            D2D_RECT_F {
                                left: content_left,
                                top: top + 520.0 * self.scale,
                                right: left + panel_w - 24.0 * self.scale,
                                bottom: top + 548.0 * self.scale,
                            },
                        )
                    };
                }
                if matches!(self.scene.extension_category, 2 | 3) {
                    let items = if self.scene.extension_category == 2 {
                        ["LLDB", "GDB", "debugpy"]
                    } else {
                        ["Biome", "clang-format", "Ruff"]
                    };
                    for (index, label) in items.iter().enumerate() {
                        let row_top = top + (190.0 + index as f32 * 50.0) * self.scale;
                        unsafe {
                            DirectEditorRenderer::rounded(
                                self.target,
                                &self.raised,
                                D2D_RECT_F {
                                    left: content_left,
                                    top: row_top,
                                    right: content_left + 360.0 * self.scale,
                                    bottom: row_top + 40.0 * self.scale,
                                },
                                8.0 * self.scale,
                            );
                            DirectEditorRenderer::text(
                                self.target,
                                &self.renderer.small_format,
                                &self.text,
                                &format!(
                                    "{label}{}",
                                    if self.scene.ui_chinese {
                                        "    检测"
                                    } else {
                                        "    CHECK"
                                    }
                                ),
                                D2D_RECT_F {
                                    left: content_left + 16.0 * self.scale,
                                    top: row_top + 10.0 * self.scale,
                                    right: content_left + 340.0 * self.scale,
                                    bottom: row_top + 36.0 * self.scale,
                                },
                            );
                        }
                    }
                    // One real action per category, for the file that is open.
                    let action = if self.scene.extension_category == 2 {
                        format!("Start debug adapter for {}", self.scene.file_name)
                    } else {
                        format!("Format {}", self.scene.file_name)
                    };
                    let row_top = top + 348.0 * self.scale;
                    unsafe {
                        DirectEditorRenderer::rounded(
                            self.target,
                            &self.selection,
                            D2D_RECT_F {
                                left: content_left,
                                top: row_top,
                                right: content_left + 520.0 * self.scale,
                                bottom: row_top + 44.0 * self.scale,
                            },
                            8.0 * self.scale,
                        );
                        DirectEditorRenderer::text(
                            self.target,
                            &self.renderer.small_format,
                            &self.accent,
                            &action,
                            D2D_RECT_F {
                                left: content_left + 16.0 * self.scale,
                                top: row_top + 12.0 * self.scale,
                                right: content_left + 500.0 * self.scale,
                                bottom: row_top + 40.0 * self.scale,
                            },
                        );
                    }
                }
                if self.scene.extension_category == 4 {
                    for (index, (label, value)) in [
                        ("ENDPOINT", self.scene.ai_endpoint),
                        ("MODEL", self.scene.ai_model),
                        (
                            "API KEY (session only)",
                            if self.scene.ai_key_present {
                                "****************"
                            } else {
                                ""
                            },
                        ),
                    ]
                    .iter()
                    .enumerate()
                    {
                        let label_top = top + (116.0 + index as f32 * 92.0) * self.scale;
                        unsafe {
                            DirectEditorRenderer::text(
                                self.target,
                                &self.renderer.small_format,
                                &self.muted,
                                label,
                                D2D_RECT_F {
                                    left: content_left,
                                    top: label_top,
                                    right: content_left + 600.0 * self.scale,
                                    bottom: label_top + 24.0 * self.scale,
                                },
                            );
                            DirectEditorRenderer::rounded(
                                self.target,
                                if self.scene.ai_field_focused == Some(index) {
                                    &self.selection
                                } else {
                                    &self.bg
                                },
                                D2D_RECT_F {
                                    left: content_left,
                                    top: label_top + 26.0 * self.scale,
                                    right: content_left + 600.0 * self.scale,
                                    bottom: label_top + 68.0 * self.scale,
                                },
                                7.0 * self.scale,
                            );
                            DirectEditorRenderer::text(
                                self.target,
                                &self.renderer.small_format,
                                &self.text,
                                value,
                                D2D_RECT_F {
                                    left: content_left + 12.0 * self.scale,
                                    top: label_top + 36.0 * self.scale,
                                    right: content_left + 586.0 * self.scale,
                                    bottom: label_top + 64.0 * self.scale,
                                },
                            );
                        }
                    }
                    unsafe {
                        DirectEditorRenderer::rounded(
                            self.target,
                            &self.accent,
                            D2D_RECT_F {
                                left: content_left,
                                top: top + 440.0 * self.scale,
                                right: content_left + 210.0 * self.scale,
                                bottom: top + 482.0 * self.scale,
                            },
                            8.0 * self.scale,
                        );
                        DirectEditorRenderer::text(
                            self.target,
                            &self.renderer.small_format,
                            &self.bg,
                            if self.scene.ui_chinese {
                                "保存 API 配置"
                            } else {
                                "SAVE API SETTINGS"
                            },
                            D2D_RECT_F {
                                left: content_left + 28.0 * self.scale,
                                top: top + 451.0 * self.scale,
                                right: content_left + 200.0 * self.scale,
                                bottom: top + 478.0 * self.scale,
                            },
                        );
                    }
                    // Prompt plus the connection test and streamed send actions.
                    let prompt_top = top + 500.0 * self.scale;
                    unsafe {
                        DirectEditorRenderer::text(
                            self.target,
                            &self.renderer.small_format,
                            &self.muted,
                            if self.scene.ui_chinese {
                                "提问（回车发送，终端里 Ctrl+C 取消）"
                            } else {
                                "PROMPT (enter sends, Ctrl+C in the terminal cancels)"
                            },
                            D2D_RECT_F {
                                left: content_left,
                                top: prompt_top,
                                right: content_left + 600.0 * self.scale,
                                bottom: prompt_top + 24.0 * self.scale,
                            },
                        );
                        DirectEditorRenderer::rounded(
                            self.target,
                            if self.scene.ai_prompt_focused {
                                &self.selection
                            } else {
                                &self.bg
                            },
                            D2D_RECT_F {
                                left: content_left,
                                top: prompt_top + 26.0 * self.scale,
                                right: content_left + 600.0 * self.scale,
                                bottom: prompt_top + 68.0 * self.scale,
                            },
                            7.0 * self.scale,
                        );
                        DirectEditorRenderer::text(
                            self.target,
                            &self.renderer.small_format,
                            &self.text,
                            self.scene.ai_prompt,
                            D2D_RECT_F {
                                left: content_left + 12.0 * self.scale,
                                top: prompt_top + 36.0 * self.scale,
                                right: content_left + 586.0 * self.scale,
                                bottom: prompt_top + 64.0 * self.scale,
                            },
                        );
                        DirectEditorRenderer::rounded(
                            self.target,
                            &self.accent,
                            D2D_RECT_F {
                                left: content_left,
                                top: prompt_top + 78.0 * self.scale,
                                right: content_left + 210.0 * self.scale,
                                bottom: prompt_top + 120.0 * self.scale,
                            },
                            8.0 * self.scale,
                        );
                        DirectEditorRenderer::text(
                            self.target,
                            &self.renderer.small_format,
                            &self.bg,
                            if self.scene.ui_chinese {
                                "发送"
                            } else {
                                "SEND"
                            },
                            D2D_RECT_F {
                                left: content_left + 28.0 * self.scale,
                                top: prompt_top + 89.0 * self.scale,
                                right: content_left + 200.0 * self.scale,
                                bottom: prompt_top + 116.0 * self.scale,
                            },
                        );
                        DirectEditorRenderer::rounded(
                            self.target,
                            &self.raised,
                            D2D_RECT_F {
                                left: content_left + 226.0 * self.scale,
                                top: prompt_top + 78.0 * self.scale,
                                right: content_left + 466.0 * self.scale,
                                bottom: prompt_top + 120.0 * self.scale,
                            },
                            8.0 * self.scale,
                        );
                        DirectEditorRenderer::text(
                            self.target,
                            &self.renderer.small_format,
                            &self.bright,
                            if self.scene.ui_chinese {
                                "测试连接"
                            } else {
                                "TEST CONNECTION"
                            },
                            D2D_RECT_F {
                                left: content_left + 254.0 * self.scale,
                                top: prompt_top + 89.0 * self.scale,
                                right: content_left + 460.0 * self.scale,
                                bottom: prompt_top + 116.0 * self.scale,
                            },
                        );
                        DirectEditorRenderer::text(
                            self.target,
                            &self.renderer.small_format,
                            &self.muted,
                            self.scene.ai_cache_summary,
                            D2D_RECT_F {
                                left: content_left,
                                top: prompt_top + 130.0 * self.scale,
                                right: content_left + 600.0 * self.scale,
                                bottom: prompt_top + 156.0 * self.scale,
                            },
                        );
                    }
                }
            }
        }
    }

    /// Paint the problems panel region.
    fn draw_problems_panel(&self) {
        if self.scene.problems_open {
            let panel_w = (560.0 * self.scale).min(self.width - 60.0 * self.scale);
            let panel_h = (420.0 * self.scale).min(self.height - 120.0 * self.scale);
            let left = (self.width - panel_w) * 0.5;
            let top = 60.0 * self.scale;
            unsafe {
                DirectEditorRenderer::rounded(
                    self.target,
                    &self.panel,
                    D2D_RECT_F {
                        left,
                        top,
                        right: left + panel_w,
                        bottom: top + panel_h,
                    },
                    16.0 * self.scale,
                );
                DirectEditorRenderer::text(
                    self.target,
                    &self.renderer.ui_format,
                    &self.bright,
                    if self.scene.ui_chinese {
                        "问题与调试"
                    } else {
                        "PROBLEMS & DEBUG"
                    },
                    D2D_RECT_F {
                        left: left + 20.0 * self.scale,
                        top: top + 14.0 * self.scale,
                        right: left + panel_w - 20.0 * self.scale,
                        bottom: top + 42.0 * self.scale,
                    },
                );
            }
            let visible = ((panel_h - 96.0 * self.scale) / (24.0 * self.scale)) as usize;
            let first = self
                .scene
                .problems_selected
                .saturating_sub(visible.saturating_sub(1));
            for (row, line) in self
                .scene
                .problems
                .iter()
                .skip(first)
                .take(visible)
                .enumerate()
            {
                let index = first + row;
                let row_top = top + (52.0 + row as f32 * 24.0) * self.scale;
                let selected = index == self.scene.problems_selected;
                if selected {
                    unsafe {
                        DirectEditorRenderer::rounded(
                            self.target,
                            &self.selection,
                            D2D_RECT_F {
                                left: left + 12.0 * self.scale,
                                top: row_top - 3.0 * self.scale,
                                right: left + panel_w - 12.0 * self.scale,
                                bottom: row_top + 21.0 * self.scale,
                            },
                            5.0 * self.scale,
                        )
                    };
                }
                unsafe {
                    DirectEditorRenderer::text(
                        self.target,
                        &self.renderer.small_format,
                        if selected { &self.bright } else { &self.text },
                        line,
                        D2D_RECT_F {
                            left: left + 24.0 * self.scale,
                            top: row_top,
                            right: left + panel_w - 20.0 * self.scale,
                            bottom: row_top + 22.0 * self.scale,
                        },
                    )
                };
            }
            if self.scene.problems.is_empty() {
                unsafe {
                    DirectEditorRenderer::text(
                        self.target,
                        &self.renderer.small_format,
                        &self.muted,
                        if self.scene.ui_chinese {
                            "没有诊断或调试信息"
                        } else {
                            "Nothing reported yet"
                        },
                        D2D_RECT_F {
                            left: left + 24.0 * self.scale,
                            top: top + 60.0 * self.scale,
                            right: left + panel_w - 20.0 * self.scale,
                            bottom: top + 90.0 * self.scale,
                        },
                    )
                };
            }
            unsafe {
                DirectEditorRenderer::text(
                    self.target,
                    &self.renderer.small_format,
                    &self.muted,
                    self.scene.problems_hint,
                    D2D_RECT_F {
                        left: left + 20.0 * self.scale,
                        top: top + panel_h - 32.0 * self.scale,
                        right: left + panel_w - 20.0 * self.scale,
                        bottom: top + panel_h - 8.0 * self.scale,
                    },
                )
            };
        }
    }

    /// Paint the chat panel region.
    fn draw_chat_panel(&self) {
        if self.scene.chat_open {
            let panel_w = (520.0 * self.scale).min(self.width - 40.0 * self.scale);
            let panel_h = (560.0 * self.scale).min(self.height - 90.0 * self.scale);
            let left = (self.width - panel_w) * 0.5;
            let top = 54.0 * self.scale;
            unsafe {
                DirectEditorRenderer::rounded(
                    self.target,
                    &self.panel,
                    D2D_RECT_F {
                        left,
                        top,
                        right: left + panel_w,
                        bottom: top + panel_h,
                    },
                    16.0 * self.scale,
                );
                DirectEditorRenderer::text(
                    self.target,
                    &self.renderer.ui_format,
                    &self.bright,
                    if self.scene.ui_chinese {
                        "AI 面板"
                    } else {
                        "AI PANEL"
                    },
                    D2D_RECT_F {
                        left: left + 20.0 * self.scale,
                        top: top + 14.0 * self.scale,
                        right: left + panel_w - 20.0 * self.scale,
                        bottom: top + 42.0 * self.scale,
                    },
                );
            }
            // Newest turns first, so the answer stays visible while it streams.
            let mut message_y = top + 50.0 * self.scale;
            let line_limit = ((panel_h - 130.0 * self.scale) / (18.0 * self.scale)) as usize;
            let mut drawn = 0usize;
            for (is_user, message) in self
                .scene
                .chat_messages
                .iter()
                .rev()
                .filter(|(_, message)| !message.is_empty())
            {
                for line in message.lines().rev().take(4) {
                    if drawn >= line_limit {
                        break;
                    }
                    let prefix = if *is_user { "you  " } else { "ai   " };
                    unsafe {
                        DirectEditorRenderer::text(
                            self.target,
                            &self.renderer.small_format,
                            if *is_user { &self.muted } else { &self.text },
                            &format!("{prefix}{line}"),
                            D2D_RECT_F {
                                left: left + 20.0 * self.scale,
                                top: message_y,
                                right: left + panel_w - 20.0 * self.scale,
                                bottom: message_y + 20.0 * self.scale,
                            },
                        )
                    };
                    message_y += 18.0 * self.scale;
                    drawn += 1;
                }
                if drawn >= line_limit {
                    break;
                }
            }
            unsafe {
                DirectEditorRenderer::rounded(
                    self.target,
                    &self.raised,
                    D2D_RECT_F {
                        left: left + 16.0 * self.scale,
                        top: top + panel_h - 92.0 * self.scale,
                        right: left + panel_w - 16.0 * self.scale,
                        bottom: top + panel_h - 46.0 * self.scale,
                    },
                    8.0 * self.scale,
                );
                DirectEditorRenderer::text(
                    self.target,
                    &self.renderer.small_format,
                    &self.bright,
                    if self.scene.chat_input.is_empty() {
                        if self.scene.ui_chinese {
                            "输入问题，回车发送，Esc 取消"
                        } else {
                            "Ask something, Enter sends, Esc cancels"
                        }
                    } else {
                        self.scene.chat_input
                    },
                    D2D_RECT_F {
                        left: left + 28.0 * self.scale,
                        top: top + panel_h - 82.0 * self.scale,
                        right: left + panel_w - 28.0 * self.scale,
                        bottom: top + panel_h - 52.0 * self.scale,
                    },
                );
                let footer = if !self.scene.chat_error.is_empty() {
                    self.scene.chat_error
                } else if self.scene.chat_running {
                    if self.scene.ui_chinese {
                        "正在接收回答…（Esc 取消）"
                    } else {
                        "Receiving the answer… (Esc cancels)"
                    }
                } else if self.scene.ui_chinese {
                    "Ctrl+Shift+A 关闭面板"
                } else {
                    "Ctrl+Shift+A closes this panel"
                };
                DirectEditorRenderer::text(
                    self.target,
                    &self.renderer.small_format,
                    if self.scene.chat_error.is_empty() {
                        &self.muted
                    } else {
                        &self.literal
                    },
                    footer,
                    D2D_RECT_F {
                        left: left + 20.0 * self.scale,
                        top: top + panel_h - 38.0 * self.scale,
                        right: left + panel_w - 20.0 * self.scale,
                        bottom: top + panel_h - 10.0 * self.scale,
                    },
                );
            }
        }
    }

    /// Paint the create dialog region.
    fn draw_create_dialog(&self) {
        if let Some(kind) = self.scene.create_kind {
            let dialog_w = (520.0 * self.scale).min(self.width - 40.0 * self.scale);
            let dialog_h = if kind == "New project" { 350.0 } else { 230.0 } * self.scale;
            let left = (self.width - dialog_w) * 0.5;
            let top = (self.height - dialog_h) * 0.42
                - (1.0 - self.scene.create_motion.min(1.0)) * 28.0 * self.scale;
            unsafe {
                DirectEditorRenderer::rounded(
                    self.target,
                    &self.panel,
                    D2D_RECT_F {
                        left,
                        top,
                        right: left + dialog_w,
                        bottom: top + dialog_h,
                    },
                    16.0 * self.scale,
                )
            };
            unsafe {
                DirectEditorRenderer::text(
                    self.target,
                    &self.renderer.ui_format,
                    &self.bright,
                    kind,
                    D2D_RECT_F {
                        left: left + 24.0 * self.scale,
                        top: top + 22.0 * self.scale,
                        right: left + dialog_w,
                        bottom: top + 52.0 * self.scale,
                    },
                )
            };
            unsafe {
                DirectEditorRenderer::rounded(
                    self.target,
                    &self.selection,
                    D2D_RECT_F {
                        left: left + 24.0 * self.scale,
                        top: top + 70.0 * self.scale,
                        right: left + dialog_w - 24.0 * self.scale,
                        bottom: top + 112.0 * self.scale,
                    },
                    9.0 * self.scale,
                )
            };
            unsafe {
                DirectEditorRenderer::text(
                    self.target,
                    &self.renderer.code_format,
                    &self.bright,
                    if self.scene.create_name.is_empty() {
                        "name"
                    } else {
                        self.scene.create_name
                    },
                    D2D_RECT_F {
                        left: left + 38.0 * self.scale,
                        top: top + 81.0 * self.scale,
                        right: left + dialog_w - 32.0 * self.scale,
                        bottom: top + 108.0 * self.scale,
                    },
                )
            };
            unsafe {
                DirectEditorRenderer::text(
                    self.target,
                    &self.renderer.small_format,
                    if self.scene.create_error.is_empty() {
                        &self.muted
                    } else {
                        &self.literal
                    },
                    if self.scene.create_error.is_empty() {
                        if self.scene.ui_chinese {
                            "输入名称后点击确定，或使用 Enter / Esc"
                        } else {
                            "Enter a name, then click Create or Cancel"
                        }
                    } else {
                        self.scene.create_error
                    },
                    D2D_RECT_F {
                        left: left + 26.0 * self.scale,
                        top: top + 132.0 * self.scale,
                        right: left + dialog_w - 26.0 * self.scale,
                        bottom: top + 170.0 * self.scale,
                    },
                )
            };
            if kind == "New project" {
                unsafe {
                    DirectEditorRenderer::text(
                        self.target,
                        &self.renderer.small_format,
                        &self.text,
                        &format!(
                            "↑↓ Language: {}    ←→ Environment: {}",
                            self.scene.create_language, self.scene.create_environment
                        ),
                        D2D_RECT_F {
                            left: left + 26.0 * self.scale,
                            top: top + 172.0 * self.scale,
                            right: left + dialog_w - 26.0 * self.scale,
                            bottom: top + 204.0 * self.scale,
                        },
                    )
                };
                unsafe {
                    DirectEditorRenderer::text(
                        self.target,
                        &self.renderer.small_format,
                        &self.accent,
                        &format!("Version (click): {}", self.scene.create_version),
                        D2D_RECT_F {
                            left: left + 26.0 * self.scale,
                            top: top + 204.0 * self.scale,
                            right: left + dialog_w - 26.0 * self.scale,
                            bottom: top + 236.0 * self.scale,
                        },
                    )
                };
                unsafe {
                    DirectEditorRenderer::text(
                        self.target,
                        &self.renderer.small_format,
                        &self.muted,
                        &format!("Location: {}", self.scene.create_location),
                        D2D_RECT_F {
                            left: left + 26.0 * self.scale,
                            top: top + 242.0 * self.scale,
                            right: left + dialog_w - 26.0 * self.scale,
                            bottom: top + 270.0 * self.scale,
                        },
                    )
                };
            }
            let button_top = top + dialog_h - 56.0 * self.scale;
            unsafe {
                DirectEditorRenderer::rounded(
                    self.target,
                    &self.raised,
                    D2D_RECT_F {
                        left: left + 24.0 * self.scale,
                        top: button_top,
                        right: left + dialog_w * 0.5 - 8.0 * self.scale,
                        bottom: button_top + 36.0 * self.scale,
                    },
                    8.0 * self.scale,
                );
                DirectEditorRenderer::rounded(
                    self.target,
                    &self.selection,
                    D2D_RECT_F {
                        left: left + dialog_w * 0.5 + 8.0 * self.scale,
                        top: button_top,
                        right: left + dialog_w - 24.0 * self.scale,
                        bottom: button_top + 36.0 * self.scale,
                    },
                    8.0 * self.scale,
                );
                DirectEditorRenderer::text(
                    self.target,
                    &self.renderer.small_format,
                    &self.text,
                    if self.scene.ui_chinese {
                        "取消"
                    } else {
                        "Cancel"
                    },
                    D2D_RECT_F {
                        left: left + 86.0 * self.scale,
                        top: button_top + 9.0 * self.scale,
                        right: left + dialog_w * 0.5,
                        bottom: button_top + 34.0 * self.scale,
                    },
                );
                DirectEditorRenderer::text(
                    self.target,
                    &self.renderer.small_format,
                    &self.bright,
                    if self.scene.ui_chinese {
                        "确定"
                    } else {
                        "Create"
                    },
                    D2D_RECT_F {
                        left: left + dialog_w * 0.5 + 82.0 * self.scale,
                        top: button_top + 9.0 * self.scale,
                        right: left + dialog_w - 28.0 * self.scale,
                        bottom: button_top + 34.0 * self.scale,
                    },
                );
            }
        }
    }
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
        let code_cell =
            unsafe { measure_code_cell(&dwrite_factory, &code_format, code_zoom, scale) };
        Ok(Self {
            d2d_factory,
            _dwrite_factory: dwrite_factory,
            target: None,
            code_format,
            ui_format,
            small_format,
            code_cell,
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
        // One palette for both backends; the unit tests in theme_core hold it to
        // the contrast the interface needs.
        let palette = theme_core::Palette::for_scheme(scene.light_theme);
        let bg = unsafe { Self::brush(&target_base, palette.background, 1.0) }?;
        let panel = unsafe { Self::brush(&target_base, palette.panel, 1.0) }?;
        let raised = unsafe { Self::brush(&target_base, palette.raised, 1.0) }?;
        let line = unsafe { Self::brush(&target_base, palette.border, 1.0) }?;
        let text = unsafe { Self::brush(&target_base, palette.text, 1.0) }?;
        let muted = unsafe { Self::brush(&target_base, palette.muted, 1.0) }?;
        let bright = unsafe { Self::brush(&target_base, palette.bright, 1.0) }?;
        let accent = unsafe { Self::brush(&target_base, palette.accent, 1.0) }?;
        let selection = unsafe { Self::brush(&target_base, palette.selection, 1.0) }?;
        let selected_text = unsafe { Self::brush(&target_base, palette.selected_text, 0.46) }?;
        let keyword = unsafe { Self::brush(&target_base, palette.keyword, 1.0) }?;
        let syntax_type = unsafe { Self::brush(&target_base, palette.syntax_type, 1.0) }?;
        let function = unsafe { Self::brush(&target_base, palette.function, 1.0) }?;
        let constant = unsafe { Self::brush(&target_base, palette.constant, 1.0) }?;
        let syntax_variable = unsafe { Self::brush(&target_base, palette.variable, 1.0) }?;
        let syntax_parameter = unsafe { Self::brush(&target_base, palette.parameter, 1.0) }?;
        let syntax_property = unsafe { Self::brush(&target_base, palette.property, 1.0) }?;
        let syntax_macro = unsafe { Self::brush(&target_base, palette.macro_name, 1.0) }?;
        let syntax_module = unsafe { Self::brush(&target_base, palette.module, 1.0) }?;
        let preprocessor = unsafe { Self::brush(&target_base, palette.preprocessor, 1.0) }?;
        let literal = unsafe { Self::brush(&target_base, palette.literal, 1.0) }?;
        let comment = unsafe { Self::brush(&target_base, palette.comment, 1.0) }?;
        let width = width as f32;
        let height = height as f32;
        let scale = self.dpi / 96.0;
        let header_h = theme_core::chrome::TOP_BAR_HEIGHT * scale;
        let status_h = theme_core::chrome::STATUS_BAR_HEIGHT * scale;
        let terminal_progress = scene.terminal_progress.clamp(0.0, 1.0);
        let terminal_panel = terminal_core::panel_metrics(width, height, scale, terminal_progress);
        let terminal_h = terminal_panel.height;
        let body_bottom = height - terminal_h;
        let tree_w = (width * 0.19).max(220.0 * scale);
        let graph_w = (width * 0.27).max(300.0 * scale);
        let editor_right = width - graph_w;
        let code_top = header_h + 58.0 * scale;
        let line_step = 25.0 * scale;
        // Horizontal scroll shifts the whole code column; the gutter and the
        // tree stay put, and the column is clipped while it is drawn.
        let scroll_x = scene.scroll_columns as f32 * self.code_cell * scale;
        let code_origin = tree_w - scroll_x;
        let code_gutter = theme_core::chrome::CODE_GUTTER_WIDTH;
        // Breakpoints the user set, drawn as dots before the code column.
        for line in scene.breakpoint_lines.iter().take(200) {
            let top = code_top + *line as f32 * line_step;
            if top < header_h || top > body_bottom {
                continue;
            }
            unsafe {
                Self::fill(
                    &target_base,
                    &literal,
                    tree_w + 26.0 * scale,
                    top + 5.0 * scale,
                    tree_w + 34.0 * scale,
                    top + 13.0 * scale,
                )
            };
        }
        // Diagnostics from the language server, marked in the gutter.
        for (line, is_error) in scene.diagnostic_lines.iter().take(200) {
            let top = code_top + *line as f32 * line_step;
            if top < header_h || top > body_bottom {
                continue;
            }
            unsafe {
                Self::fill(
                    &target_base,
                    if *is_error { &literal } else { &accent },
                    tree_w + 34.0 * scale,
                    top + 5.0 * scale,
                    tree_w + 40.0 * scale,
                    top + 11.0 * scale,
                )
            };
        }
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
            let scrim_alpha =
                theme_core::editor_scrim_alpha(scene.light_theme, scene.background_opacity);
            let scrim = unsafe { Self::brush(&target_base, palette.background, scrim_alpha) }?;
            unsafe {
                Self::fill(
                    &target_base,
                    &scrim,
                    region.left,
                    region.top,
                    region.right,
                    region.bottom,
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

        if let Some(focused) = scene.relation_symbol {
            let reveal = scene.relation_focus_motion.clamp(0.0, 1.0);
            let focused_line = scene
                .lines
                .get(focused.line)
                .map(String::as_str)
                .unwrap_or("");
            let focused_left = code_origin
                + (49.0 + Self::visual_cells(focused_line, focused.start) as f32 * self.code_cell)
                    * scale;
            let focused_right = code_origin
                + (55.0 + Self::visual_cells(focused_line, focused.end) as f32 * self.code_cell)
                    * scale;
            let focused_top = code_top + focused.line as f32 * line_step - 3.0 * scale;
            let focused_center = (focused_left + focused_right) * 0.5;
            let outline_left = focused_center + (focused_left - focused_center) * reveal;
            let outline_right = focused_center + (focused_right - focused_center) * reveal;
            unsafe {
                Self::outline(
                    &target_base,
                    &accent,
                    D2D_RECT_F {
                        left: outline_left,
                        top: focused_top,
                        right: outline_right,
                        bottom: focused_top + 25.0 * scale,
                    },
                    5.0 * scale,
                    1.5 * scale,
                )
            };
            let route_x = editor_right - 13.0 * scale;
            let source_y = focused_top + 12.0 * scale;
            let source_end = focused_right + (route_x - focused_right) * reveal;
            unsafe {
                Self::fill(
                    &target_base,
                    &accent,
                    focused_right,
                    source_y,
                    source_end,
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
                let related_left = code_origin
                    + (49.0
                        + Self::visual_cells(line_value, related.start) as f32 * self.code_cell)
                        * scale;
                let related_right = code_origin
                    + (55.0 + Self::visual_cells(line_value, related.end) as f32 * self.code_cell)
                        * scale;
                let related_top = code_top + related.line as f32 * line_step - 3.0 * scale;
                let related_y = related_top + 12.0 * scale;
                let branch_reveal = ((reveal - 0.22) / 0.78).clamp(0.0, 1.0);
                let related_center = (related_left + related_right) * 0.5;
                let related_outline_left =
                    related_center + (related_left - related_center) * branch_reveal;
                let related_outline_right =
                    related_center + (related_right - related_center) * branch_reveal;
                let vertical_end = source_y + (related_y - source_y) * branch_reveal;
                let branch_end = route_x + (related_right - route_x) * branch_reveal;
                unsafe {
                    Self::outline(
                        &target_base,
                        &muted,
                        D2D_RECT_F {
                            left: related_outline_left,
                            top: related_top,
                            right: related_outline_right,
                            bottom: related_top + 25.0 * scale,
                        },
                        5.0 * scale,
                        1.0 * scale,
                    );
                    Self::fill(
                        &target_base,
                        &accent,
                        route_x,
                        source_y.min(vertical_end),
                        route_x + 1.5 * scale,
                        source_y.max(vertical_end) + 1.5 * scale,
                    );
                    Self::fill(
                        &target_base,
                        &accent,
                        branch_end,
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
        // The same three quiet quick actions as the Linux identity bar. Their
        // panels remain native to each backend, but the discoverability and
        // state treatment no longer change with the operating system.
        let quick_actions = [
            (
                width - 290.0 * scale,
                85.0 * scale,
                if scene.ui_chinese {
                    "设置"
                } else {
                    "Settings"
                },
                scene.settings_motion > 0.01,
            ),
            (
                width - 205.0 * scale,
                100.0 * scale,
                if scene.ui_chinese {
                    "扩展"
                } else {
                    "Extensions"
                },
                scene.extensions_motion > 0.01,
            ),
            (
                width - 105.0 * scale,
                100.0 * scale,
                if scene.package_running {
                    if scene.ui_chinese {
                        "打包中…"
                    } else {
                        "Packaging…"
                    }
                } else if scene.ui_chinese {
                    "打包  ›"
                } else {
                    "Package  ›"
                },
                scene.package_motion > 0.01,
            ),
        ];
        for (left, action_width, label, active) in quick_actions {
            let hovered = scene.pointer_y as f32 >= 8.0 * scale
                && scene.pointer_y as f32 <= 40.0 * scale
                && scene.pointer_x as f32 >= left
                && scene.pointer_x as f32 <= left + action_width;
            if hovered || active {
                unsafe {
                    Self::rounded(
                        &target_base,
                        if active { &selection } else { &panel },
                        D2D_RECT_F {
                            left: left + 4.0 * scale,
                            top: 8.0 * scale,
                            right: left + action_width - 4.0 * scale,
                            bottom: 40.0 * scale,
                        },
                        theme_core::chrome::CONTROL_RADIUS * scale,
                    )
                };
            }
            unsafe {
                Self::text(
                    &target_base,
                    &self.small_format,
                    if active { &accent } else { &muted },
                    label,
                    D2D_RECT_F {
                        left: left + 14.0 * scale,
                        top: 15.0 * scale,
                        right: left + action_width - 6.0 * scale,
                        bottom: header_h,
                    },
                )
            };
        }
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
            let row = index.saturating_sub(2);
            let position = if index < 2 {
                index as f32
            } else {
                scene
                    .tree_row_positions
                    .get(row)
                    .copied()
                    .unwrap_or(row as f32)
            };
            let y = header_h + (58.0 + position * 35.0) * scale;
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

        // The code column is clipped so horizontal scrolling never paints over
        // the gutter, the tree or the relation graph.
        let code_clip = D2D_RECT_F {
            left: tree_w + 42.0 * scale,
            top: code_top - 6.0 * scale,
            right: editor_right - 10.0 * scale,
            bottom: body_bottom,
        };
        unsafe { target_base.PushAxisAlignedClip(&code_clip, D2D1_ANTIALIAS_MODE_PER_PRIMITIVE) };
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
                            code_origin + (code_gutter + left_cells * self.code_cell) * scale,
                            y - 2.0 * scale,
                            code_origin + (code_gutter + right_cells * self.code_cell) * scale,
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
            // The newest edit on this line lifts the code while it fades, so a
            // keystroke, a paste, a deletion and an undo all animate the same
            // way instead of only typing doing something.
            let edit_offset = scene
                .edit_pulse
                .filter(|_| index == scene.caret_line)
                .map(|amount| amount * 7.0 * scale)
                .unwrap_or(0.0);
            let mut token_x = code_origin + code_gutter * scale + edit_offset;
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
                    TokenKind::Type => &syntax_type,
                    TokenKind::Function => &function,
                    TokenKind::Constant => &constant,
                    TokenKind::Variable => &syntax_variable,
                    TokenKind::Parameter => &syntax_parameter,
                    TokenKind::Property => &syntax_property,
                    TokenKind::Macro => &syntax_macro,
                    TokenKind::Module => &syntax_module,
                    TokenKind::Preprocessor => &preprocessor,
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
        // Project tree scrollbar, when the tree is longer than the column.
        {
            let visible_rows = (((body_bottom - (header_h + 58.0 * scale)) / (35.0 * scale))
                .floor()
                .max(1.0)) as usize;
            let total = scene.project_total.max(scene.project_entries.len());
            if total > visible_rows {
                let track_top = header_h + 58.0 * scale;
                let track_bottom = body_bottom - 12.0 * scale;
                let track = (track_bottom - track_top).max(24.0 * scale);
                let thumb = (track * visible_rows as f32 / total as f32).clamp(16.0 * scale, track);
                let max_scroll = total.saturating_sub(visible_rows).max(1);
                let offset = scene.project_scroll.min(max_scroll) as f32 / max_scroll as f32;
                let top = track_top + (track - thumb) * offset.clamp(0.0, 1.0);
                unsafe {
                    Self::rounded(
                        &target_base,
                        &line,
                        D2D_RECT_F {
                            left: tree_w - 7.0 * scale,
                            top: track_top,
                            right: tree_w - 3.0 * scale,
                            bottom: track_bottom,
                        },
                        2.0 * scale,
                    );
                    Self::rounded(
                        &target_base,
                        &muted,
                        D2D_RECT_F {
                            left: tree_w - 8.0 * scale,
                            top,
                            right: tree_w - 2.0 * scale,
                            bottom: top + thumb,
                        },
                        3.0 * scale,
                    );
                }
            }
        }
        let bubble_visibility = scene.comment_bubbles_motion.clamp(0.0, 1.0);
        if bubble_visibility > 0.001 {
            for (line_index, value) in scene.lines.iter().take(max_lines).enumerate() {
                let Some((marker_start, content_start, content_end)) =
                    editor_core::comment_bounds(value, scene.file_name)
                else {
                    continue;
                };
                let expansion = scene
                    .comment_expansion
                    .iter()
                    .find(|(line, _)| *line == line_index)
                    .map(|(_, value)| value.clamp(0.0, 1.0))
                    .unwrap_or(0.0);
                let expanded = scene.expanded_comments.contains(&line_index);
                let editing = scene.editing_comment == Some(line_index);
                let y = code_top + line_index as f32 * line_step - 5.0 * scale;
                let bubble_right = editor_right - 14.0 * scale;
                let settled_left = (bubble_right - 286.0 * scale).max(tree_w + 130.0 * scale);
                let bubble_left =
                    settled_left + (bubble_right - settled_left) * (1.0 - bubble_visibility);
                let bubble_height = (34.0 + 78.0 * expansion) * scale;
                let marker_column = value[..marker_start].chars().count();
                let anchor_x = code_origin
                    + (code_gutter
                        + Self::visual_cells(value, marker_column) as f32 * self.code_cell)
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
        let caret_x = code_origin + (code_gutter + caret_cells as f32 * self.code_cell) * scale;
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
        unsafe { target_base.PopAxisAlignedClip() };
        // Vertical scrollbar: only drawn when the document is taller than the view.
        let total_lines = scene.total_lines.max(1);
        if total_lines > max_lines.max(1) {
            let track_top = code_top;
            let track_bottom = body_bottom - 8.0 * scale;
            let track = (track_bottom - track_top).max(24.0 * scale);
            let visible = max_lines.min(total_lines) as f32 / total_lines as f32;
            let thumb = (track * visible).clamp(16.0 * scale, track);
            let scrollable = (track - thumb).max(1.0);
            let offset = scene
                .first_line
                .min(total_lines.saturating_sub(max_lines.max(1))) as f32
                / total_lines.saturating_sub(max_lines.max(1)).max(1) as f32;
            let top = track_top + scrollable * offset.clamp(0.0, 1.0);
            unsafe {
                Self::rounded(
                    &target_base,
                    &line,
                    D2D_RECT_F {
                        left: editor_right - 7.0 * scale,
                        top: track_top,
                        right: editor_right - 3.0 * scale,
                        bottom: track_bottom,
                    },
                    2.0 * scale,
                );
                Self::rounded(
                    &target_base,
                    &muted,
                    D2D_RECT_F {
                        left: editor_right - 8.0 * scale,
                        top,
                        right: editor_right - 2.0 * scale,
                        bottom: top + thumb,
                    },
                    3.0 * scale,
                );
            }
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
        // Document tabs across the editor header.
        {
            let tab_width = theme_core::chrome::TAB_WIDTH * scale;
            let tab_top = header_h + 4.0 * scale;
            let tab_bottom = header_h + theme_core::chrome::CONTROL_HEIGHT * scale;
            let mut tab_x = tree_w + 16.0 * scale;
            for (index, title) in scene.tabs.iter().enumerate() {
                if tab_x + tab_width > editor_right - 8.0 * scale {
                    break;
                }
                let active = index == scene.active_tab;
                unsafe {
                    Self::rounded(
                        &target_base,
                        if active { &selection } else { &raised },
                        D2D_RECT_F {
                            left: tab_x,
                            top: tab_top,
                            right: tab_x + tab_width - 6.0 * scale,
                            bottom: tab_bottom,
                        },
                        theme_core::chrome::CONTROL_RADIUS * scale,
                    )
                };
                if active {
                    let rail_left = tab_x + 5.0 * scale;
                    let rail_right = tab_x + tab_width - 11.0 * scale;
                    let rail_top = tab_bottom - theme_core::chrome::SEMANTIC_RAIL_HEIGHT * scale;
                    unsafe {
                        Self::fill(
                            &target_base,
                            &accent,
                            rail_left,
                            rail_top,
                            rail_right,
                            tab_bottom,
                        )
                    };
                    let (glint_left, glint_right) = crate::motion_core::flowing_segment(
                        rail_left,
                        rail_right,
                        scene.animation_seconds,
                        1.8,
                        0.18,
                    );
                    unsafe {
                        Self::fill(
                            &target_base,
                            &bright,
                            glint_left,
                            rail_top,
                            glint_right,
                            tab_bottom,
                        )
                    };
                }
                let dirty = scene.tab_dirty.get(index).copied().unwrap_or(false);
                unsafe {
                    Self::text(
                        &target_base,
                        &self.small_format,
                        if active {
                            &bright
                        } else if dirty {
                            &accent
                        } else {
                            &muted
                        },
                        title,
                        D2D_RECT_F {
                            left: tab_x + 10.0 * scale,
                            top: tab_top + 4.0 * scale,
                            right: tab_x + tab_width - 24.0 * scale,
                            bottom: tab_bottom,
                        },
                    );
                    Self::text(
                        &target_base,
                        &self.small_format,
                        &muted,
                        "x",
                        D2D_RECT_F {
                            left: tab_x + tab_width - 22.0 * scale,
                            top: tab_top + 4.0 * scale,
                            right: tab_x + tab_width - 8.0 * scale,
                            bottom: tab_bottom,
                        },
                    )
                };
                tab_x += tab_width;
            }
        }
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
        let paint = ScenePaint {
            renderer: self,
            target: &target_base,
            scene,
            accent,
            bg,
            bright,
            divider: line,
            literal,
            muted,
            panel,
            raised,
            selection,
            selected_text,
            text,
            height,
            scale,
            width,
            header_h,
            code_top,
            body_bottom,
            editor_right,
            tree_w,
            terminal_h,
            terminal_panel,
            terminal_progress,
            status_h,
        };
        paint.draw_terminal_panel();
        paint.draw_package_progress();
        paint.draw_package_overlay();
        paint.draw_settings_overlay();
        paint.draw_extensions_overlay();
        paint.draw_problems_panel();
        paint.draw_chat_panel();
        paint.draw_create_dialog();
        if let Err(error) = unsafe { target_base.EndDraw(None, None) } {
            self.target = None;
            return Err(error);
        }
        Ok(())
    }
}
