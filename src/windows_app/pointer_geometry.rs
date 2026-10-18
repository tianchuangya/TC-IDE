//! Geometry shared by Windows pointer hit testing and the background preview.

use super::*;

pub(super) fn background_preview_rect(area: &Rect, scale: f32) -> background_core::Rect {
    let left = area.right as f32 - 410.0 * scale;
    let top = 58.0 * scale;
    background_core::Rect::new(
        (left + 18.0 * scale) as f64,
        (top + 444.0 * scale) as f64,
        (358.0 * scale) as f64,
        (132.0 * scale) as f64,
    )
}

pub(super) fn background_view_size(app: &WindowsAppState, area: &Rect, scale: f32) -> (f64, f64) {
    let width = area.right as f32;
    let height = area.bottom as f32;
    let tree = (width * 0.19).max(220.0 * scale);
    let graph = (width * 0.27).max(300.0 * scale);
    let panel = terminal_core::panel_metrics(width, height, scale, app.terminal_motion);
    (
        (width - graph - tree).max(1.0) as f64,
        (height - panel.height - 86.0 * scale).max(1.0) as f64,
    )
}

pub(super) fn background_crop_on_screen(
    app: &WindowsAppState,
    area: &Rect,
    scale: f32,
    image: (f64, f64),
) -> (background_core::Rect, background_core::Placement) {
    let preview_rect = background_preview_rect(area, scale);
    let preview =
        background_core::preview_placement(image, (preview_rect.width, preview_rect.height));
    let view = background_view_size(app, area, scale);
    let placed = background_core::placement(
        image,
        view,
        app.background_zoom as f64,
        (
            app.background_offset_x as f64,
            app.background_offset_y as f64,
        ),
    );
    let crop = preview.project(&placed.crop(view, image));
    (
        background_core::Rect::new(
            preview_rect.x + crop.x,
            preview_rect.y + crop.y,
            crop.width,
            crop.height,
        ),
        preview,
    )
}
