//! Background image: how the picture is placed behind the interface and where
//! its crop box sits.
//!
//! Both backends let the user move and zoom the background with numeric
//! controls, and both had their own copy of the cover-and-offset arithmetic
//! that decides what part of the picture is on screen. The arithmetic, the crop
//! rectangle it implies and the drag that moves or resizes that rectangle are
//! pure functions here, so the crop box can be dragged instead of nudged with
//! numbers and the result is the same on every platform.
//!
//! Linux and Windows both use the same placement, hit testing and drag model;
//! their native renderers only draw the preview and forward pointer movement.
#![allow(dead_code)]

/// Smallest allowed zoom.
pub const MIN_ZOOM: f64 = 1.0;
/// Largest allowed zoom.
pub const MAX_ZOOM: f64 = 3.0;
/// Normalised offsets reach this far in either direction.
pub const MAX_OFFSET: f64 = 1.0;

/// A finite number, or the fallback when the value cannot be used.
fn finite(value: f64, fallback: f64) -> f64 {
    if value.is_finite() {
        value
    } else {
        fallback
    }
}

/// A rectangle in image or view coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Rect {
    /// A rectangle from its corner and size, with non negative sizes.
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x: finite(x, 0.0),
            y: finite(y, 0.0),
            width: finite(width, 0.0).max(0.0),
            height: finite(height, 0.0).max(0.0),
        }
    }

    pub fn right(&self) -> f64 {
        self.x + self.width
    }

    pub fn bottom(&self) -> f64 {
        self.y + self.height
    }

    /// Whether a point is inside, borders included.
    pub fn contains(&self, point: (f64, f64)) -> bool {
        point.0 >= self.x
            && point.0 <= self.right()
            && point.1 >= self.y
            && point.1 <= self.bottom()
    }

    /// The rectangle moved by a delta, kept inside `bounds`.
    pub fn moved(&self, dx: f64, dy: f64, bounds: &Rect) -> Rect {
        let x = (self.x + dx).clamp(bounds.x, (bounds.right() - self.width).max(bounds.x));
        let y = (self.y + dy).clamp(bounds.y, (bounds.bottom() - self.height).max(bounds.y));
        Rect { x, y, ..*self }
    }
}

/// Where the picture sits and how much of it is on screen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    /// Scale applied to the image, cover scale times the zoom.
    pub scale: f64,
    /// Size of the drawn picture.
    pub drawn: (f64, f64),
    /// How far the drawn picture can be moved inside the view.
    pub excess: (f64, f64),
    /// Top left corner of the drawn picture, relative to the view.
    pub origin: (f64, f64),
}

impl Placement {
    /// The part of the image the view shows, in image coordinates.
    pub fn crop(&self, view: (f64, f64), image: (f64, f64)) -> Rect {
        let scale = self.scale.max(f64::MIN_POSITIVE);
        let x = (-self.origin.0 / scale).clamp(0.0, image.0);
        let y = (-self.origin.1 / scale).clamp(0.0, image.1);
        let width = (view.0 / scale).clamp(0.0, image.0 - x);
        let height = (view.1 / scale).clamp(0.0, image.1 - y);
        Rect::new(x, y, width, height)
    }

    /// The image point under a point of the drawn picture.
    ///
    /// The crop box is dragged in a preview of the picture, so a pointer has to
    /// travel back into image coordinates.
    pub fn unproject(&self, point: (f64, f64)) -> (f64, f64) {
        let scale = self.scale.max(f64::MIN_POSITIVE);
        (
            (finite(point.0, 0.0) - self.origin.0) / scale,
            (finite(point.1, 0.0) - self.origin.1) / scale,
        )
    }

    /// Where a rectangle of the image ends up on screen, in view coordinates.
    pub fn project(&self, rect: &Rect) -> Rect {
        Rect::new(
            self.origin.0 + rect.x * self.scale,
            self.origin.1 + rect.y * self.scale,
            rect.width * self.scale,
            rect.height * self.scale,
        )
    }
}

/// Cover the view with the image and apply the zoom and the offsets.
///
/// `offset` is normalised: `-1.0` shows the top (or left) edge, `0.0` centres
/// the picture and `1.0` shows the bottom (or right) edge. Offsets only move the
/// picture where there is something to show.
pub fn placement(image: (f64, f64), view: (f64, f64), zoom: f64, offset: (f64, f64)) -> Placement {
    let image = (finite(image.0, 1.0).max(1.0), finite(image.1, 1.0).max(1.0));
    let view = (finite(view.0, 0.0).max(0.0), finite(view.1, 0.0).max(0.0));
    let zoom = finite(zoom, MIN_ZOOM).clamp(MIN_ZOOM, MAX_ZOOM);
    let cover = (view.0 / image.0).max(view.1 / image.1);
    let scale = (if cover.is_finite() && cover > 0.0 {
        cover
    } else {
        1.0
    }) * zoom;
    let drawn = (image.0 * scale, image.1 * scale);
    let excess = ((drawn.0 - view.0).max(0.0), (drawn.1 - view.1).max(0.0));
    let origin = (
        -excess.0 * (finite(offset.0, 0.0).clamp(-MAX_OFFSET, MAX_OFFSET) + 1.0) * 0.5,
        -excess.1 * (finite(offset.1, 0.0).clamp(-MAX_OFFSET, MAX_OFFSET) + 1.0) * 0.5,
    );
    Placement {
        scale,
        drawn,
        excess,
        origin,
    }
}

/// Fit the whole image inside a preview, centred with letterboxing when the
/// aspect ratios differ. Unlike [`placement`], this never crops the source.
pub fn preview_placement(image: (f64, f64), view: (f64, f64)) -> Placement {
    let image = (finite(image.0, 1.0).max(1.0), finite(image.1, 1.0).max(1.0));
    let view = (finite(view.0, 0.0).max(0.0), finite(view.1, 0.0).max(0.0));
    let scale = (view.0 / image.0).min(view.1 / image.1);
    let scale = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    };
    let drawn = (image.0 * scale, image.1 * scale);
    Placement {
        scale,
        drawn,
        excess: (0.0, 0.0),
        origin: ((view.0 - drawn.0) * 0.5, (view.1 - drawn.1) * 0.5),
    }
}

/// Which part of the crop box a pointer grabbed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handle {
    Inside,
    Left,
    Right,
    Top,
    Bottom,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl Handle {
    /// Columns the handle moves: `-1`, `0` or `1`.
    pub fn columns(self) -> i8 {
        match self {
            Handle::Left | Handle::TopLeft | Handle::BottomLeft => -1,
            Handle::Right | Handle::TopRight | Handle::BottomRight => 1,
            _ => 0,
        }
    }

    /// Rows the handle moves: `-1`, `0` or `1`.
    pub fn rows(self) -> i8 {
        match self {
            Handle::Top | Handle::TopLeft | Handle::TopRight => -1,
            Handle::Bottom | Handle::BottomLeft | Handle::BottomRight => 1,
            _ => 0,
        }
    }

    /// Whether the handle resizes instead of moving the whole box.
    pub fn resizes(self) -> bool {
        self != Handle::Inside
    }
}

/// Which handle sits under `point`, for a crop box drawn at `box_on_screen`.
///
/// `grab` is how close to a border or corner counts as a grab, in the same
/// units as the rectangle.
pub fn handle_at(box_on_screen: &Rect, point: (f64, f64), grab: f64) -> Option<Handle> {
    if !box_on_screen.contains(point) {
        return None;
    }
    let near_left = (point.0 - box_on_screen.x).abs() <= grab;
    let near_right = (point.0 - box_on_screen.right()).abs() <= grab;
    let near_top = (point.1 - box_on_screen.y).abs() <= grab;
    let near_bottom = (point.1 - box_on_screen.bottom()).abs() <= grab;
    Some(match (near_left, near_right, near_top, near_bottom) {
        (true, _, true, _) => Handle::TopLeft,
        (_, true, true, _) => Handle::TopRight,
        (true, _, _, true) => Handle::BottomLeft,
        (_, true, _, true) => Handle::BottomRight,
        (true, _, _, _) => Handle::Left,
        (_, true, _, _) => Handle::Right,
        (_, _, true, _) => Handle::Top,
        (_, _, _, true) => Handle::Bottom,
        _ => Handle::Inside,
    })
}

/// Result of dragging the crop box.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CropEdit {
    /// New crop rectangle in image coordinates.
    pub crop: Rect,
    /// Zoom the crop implies, so the box fills the view.
    pub zoom: f64,
    /// Normalised offsets that put the box where it was dragged.
    pub offset: (f64, f64),
}

/// Drag the crop box by `delta` view pixels, or resize it through `handle`.
///
/// Resizing keeps the box inside the image, keeps it at least `minimum` wide and
/// tall, and reports the zoom that shows exactly what the box covers.
pub fn drag_crop(
    crop: &Rect,
    handle: Handle,
    delta: (f64, f64),
    image: (f64, f64),
    view: (f64, f64),
    minimum: f64,
) -> CropEdit {
    let image = (finite(image.0, 1.0).max(1.0), finite(image.1, 1.0).max(1.0));
    let view = (finite(view.0, 0.0).max(0.0), finite(view.1, 0.0).max(0.0));
    let delta = (finite(delta.0, 0.0), finite(delta.1, 0.0));
    let minimum = finite(minimum, 1.0).max(1.0).min(image.0).min(image.1);
    let mut next = *crop;

    if handle.resizes() {
        let columns = handle.columns();
        let rows = handle.rows();
        if columns < 0 {
            let left = (crop.x + delta.0).clamp(0.0, crop.right() - minimum);
            next.width = crop.right() - left;
            next.x = left;
        } else if columns > 0 {
            let right = (crop.right() + delta.0).clamp(crop.x + minimum, image.0);
            next.width = right - crop.x;
        }
        if rows < 0 {
            let top = (crop.y + delta.1).clamp(0.0, crop.bottom() - minimum);
            next.height = crop.bottom() - top;
            next.y = top;
        } else if rows > 0 {
            let bottom = (crop.bottom() + delta.1).clamp(crop.y + minimum, image.1);
            next.height = bottom - crop.y;
        }
    } else {
        let bounds = Rect::new(0.0, 0.0, image.0, image.1);
        next = crop.moved(delta.0, delta.1, &bounds);
    }

    // The box should fill the view, so the zoom follows from its size and the
    // offsets put its centre in the middle of the view.
    let cover_x = view.0 / next.width.max(f64::MIN_POSITIVE);
    let cover_y = view.1 / next.height.max(f64::MIN_POSITIVE);
    let zoom = cover_x.max(cover_y).clamp(MIN_ZOOM, MAX_ZOOM);
    let scale = zoom;
    let drawn = (image.0 * scale, image.1 * scale);
    let excess = ((drawn.0 - view.0).max(0.0), (drawn.1 - view.1).max(0.0));
    let centre = (next.x + next.width * 0.5, next.y + next.height * 0.5);
    let offset = (
        if excess.0 > 0.0 {
            (1.0 - 2.0 * centre.0 * scale / excess.0).clamp(-MAX_OFFSET, MAX_OFFSET)
        } else {
            0.0
        },
        if excess.1 > 0.0 {
            (1.0 - 2.0 * centre.1 * scale / excess.1).clamp(-MAX_OFFSET, MAX_OFFSET)
        } else {
            0.0
        },
    );
    CropEdit {
        crop: next,
        zoom,
        offset,
    }
}

/// Zoom applied by one step of the zoom control, clamped to the allowed range.
pub fn zoomed(zoom: f64, steps: i32) -> f64 {
    (finite(zoom, MIN_ZOOM) + steps as f64 * 0.1).clamp(MIN_ZOOM, MAX_ZOOM)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(left: f64, right: f64) -> bool {
        (left - right).abs() < 1.0e-9
    }

    #[test]
    fn a_picture_covers_the_view_and_only_moves_where_there_is_something_to_show() {
        // A wide picture in a square view: the height is covered, so only the
        // horizontal axis can be moved.
        let placed = placement((2000.0, 1000.0), (800.0, 800.0), 1.0, (0.0, 0.0));
        assert!(close(placed.scale, 0.8));
        assert!(close(placed.drawn.0, 1600.0) && close(placed.drawn.1, 800.0));
        assert!(close(placed.excess.0, 800.0) && close(placed.excess.1, 0.0));
        assert!(close(placed.origin.0, -400.0) && close(placed.origin.1, -0.0));

        let left = placement((2000.0, 1000.0), (800.0, 800.0), 1.0, (-1.0, 0.0));
        assert!(
            close(left.origin.0, 0.0),
            "the left edge is flush with the view"
        );
        let right = placement((2000.0, 1000.0), (800.0, 800.0), 1.0, (1.0, 1.0));
        assert!(close(right.origin.0, -800.0), "the right edge is flush too");
        assert!(
            close(right.origin.1, 0.0),
            "an axis without excess cannot move"
        );

        // Zooming in adds excess, and the crop shrinks accordingly.
        let zoomed_in = placement((2000.0, 1000.0), (800.0, 800.0), 2.0, (0.0, 0.0));
        assert!(close(zoomed_in.drawn.0, 3200.0) && close(zoomed_in.drawn.1, 1600.0));
        let full = placed.crop((800.0, 800.0), (2000.0, 1000.0));
        let closer = zoomed_in.crop((800.0, 800.0), (2000.0, 1000.0));
        assert!(
            close(full.height, 1000.0),
            "at cover scale the whole height is visible"
        );
        assert!(
            close(closer.height, 500.0),
            "twice the zoom shows half of it"
        );
        assert!(closer.width < full.width);
    }

    #[test]
    fn a_preview_contains_and_centres_the_whole_picture() {
        let wide = preview_placement((2000.0, 1000.0), (400.0, 300.0));
        assert!(close(wide.scale, 0.2));
        assert_eq!(wide.drawn, (400.0, 200.0));
        assert!(close(wide.origin.0, 0.0) && close(wide.origin.1, 50.0));

        let tall = preview_placement((1000.0, 2000.0), (400.0, 300.0));
        assert!(close(tall.scale, 0.15));
        assert_eq!(tall.drawn, (150.0, 300.0));
        assert!(close(tall.origin.0, 125.0) && close(tall.origin.1, 0.0));
    }

    #[test]
    fn broken_numbers_fall_back_to_a_visible_picture() {
        let placed = placement((0.0, 0.0), (0.0, 0.0), f64::NAN, (f64::NAN, 0.0));
        assert!(placed.scale.is_finite() && placed.scale > 0.0);
        assert!(placed.origin.0.is_finite() && placed.origin.1.is_finite());
        let placed = placement((100.0, 100.0), (400.0, 300.0), 99.0, (5.0, -5.0));
        assert!(close(placed.scale, MAX_ZOOM * 4.0));
        // The width fits exactly, so only the vertical axis can be moved, and a
        // wild offset is clamped to the last edge instead of showing nothing.
        let clamped = placement((100.0, 100.0), (400.0, 300.0), 1.0, (5.0, 5.0));
        assert!(close(clamped.origin.0, 0.0));
        assert!(close(clamped.origin.1, -100.0));
    }

    #[test]
    fn the_crop_box_reports_where_the_picture_is_on_screen() {
        let placed = placement((2000.0, 1000.0), (800.0, 800.0), 1.0, (0.0, 0.0));
        let crop = placed.crop((800.0, 800.0), (2000.0, 1000.0));
        let screen = placed.project(&crop);
        assert!(close(screen.x, 0.0) && close(screen.width, 800.0));
        assert!(close(screen.y, 0.0) && close(screen.height, 800.0));
    }

    #[test]
    fn a_pointer_finds_the_handle_it_grabbed() {
        let area = Rect::new(100.0, 50.0, 400.0, 300.0);
        assert_eq!(handle_at(&area, (300.0, 200.0), 12.0), Some(Handle::Inside));
        assert_eq!(handle_at(&area, (104.0, 200.0), 12.0), Some(Handle::Left));
        assert_eq!(handle_at(&area, (496.0, 200.0), 12.0), Some(Handle::Right));
        assert_eq!(handle_at(&area, (300.0, 54.0), 12.0), Some(Handle::Top));
        assert_eq!(handle_at(&area, (300.0, 346.0), 12.0), Some(Handle::Bottom));
        assert_eq!(handle_at(&area, (105.0, 55.0), 12.0), Some(Handle::TopLeft));
        assert_eq!(
            handle_at(&area, (495.0, 55.0), 12.0),
            Some(Handle::TopRight)
        );
        assert_eq!(
            handle_at(&area, (105.0, 345.0), 12.0),
            Some(Handle::BottomLeft)
        );
        assert_eq!(
            handle_at(&area, (495.0, 345.0), 12.0),
            Some(Handle::BottomRight)
        );
        assert_eq!(handle_at(&area, (10.0, 10.0), 12.0), None);
        assert!(!Handle::Inside.resizes());
        assert!(Handle::TopLeft.resizes());
        assert_eq!(Handle::BottomRight.columns(), 1);
        assert_eq!(Handle::BottomRight.rows(), 1);
    }

    #[test]
    fn moving_the_box_keeps_it_inside_the_picture() {
        let crop = Rect::new(400.0, 200.0, 800.0, 400.0);
        let edit = drag_crop(
            &crop,
            Handle::Inside,
            (100.0, 50.0),
            (2000.0, 1000.0),
            (800.0, 800.0),
            64.0,
        );
        assert_eq!(edit.crop.x, 500.0);
        assert_eq!(edit.crop.y, 250.0);
        assert_eq!(edit.crop.width, 800.0);

        let far = drag_crop(
            &crop,
            Handle::Inside,
            (10_000.0, -10_000.0),
            (2000.0, 1000.0),
            (800.0, 800.0),
            64.0,
        );
        assert!(close(far.crop.right(), 2000.0), "the box stops at the edge");
        assert!(close(far.crop.y, 0.0));
        assert!(edit.offset.0.abs() <= MAX_OFFSET && edit.offset.1.abs() <= MAX_OFFSET);
        assert!(edit.zoom >= MIN_ZOOM && edit.zoom <= MAX_ZOOM);
    }

    #[test]
    fn resizing_the_box_keeps_it_usable_and_reports_the_zoom_it_implies() {
        let crop = Rect::new(400.0, 200.0, 800.0, 400.0);
        let bigger = drag_crop(
            &crop,
            Handle::BottomRight,
            (200.0, 100.0),
            (2000.0, 1000.0),
            (800.0, 800.0),
            64.0,
        );
        assert_eq!((bigger.crop.x, bigger.crop.y), (400.0, 200.0));
        assert_eq!((bigger.crop.width, bigger.crop.height), (1000.0, 500.0));
        assert!(
            close(bigger.zoom, 1.6),
            "800/500 decides the zoom: {}",
            bigger.zoom
        );

        // Dragging a corner past the opposite one stops at the minimum size.
        let tiny = drag_crop(
            &crop,
            Handle::TopLeft,
            (10_000.0, 10_000.0),
            (2000.0, 1000.0),
            (800.0, 800.0),
            64.0,
        );
        assert!(tiny.crop.width >= 64.0 && tiny.crop.height >= 64.0);
        assert!(close(tiny.crop.right(), 1200.0) && close(tiny.crop.bottom(), 600.0));

        // And it never leaves the picture.
        let outside = drag_crop(
            &crop,
            Handle::Left,
            (-10_000.0, 0.0),
            (2000.0, 1000.0),
            (800.0, 800.0),
            64.0,
        );
        assert!(close(outside.crop.x, 0.0));
    }

    #[test]
    fn a_point_of_the_picture_maps_back_into_the_image() {
        let placed = placement((2000.0, 1000.0), (400.0, 400.0), 1.0, (0.0, 0.0));
        // The picture is drawn at 0.4 scale with 400 units of horizontal excess,
        // so the middle of the view sits at the middle of the visible height.
        assert!(close(placed.unproject((200.0, 200.0)).0, 1000.0));
        assert!(close(placed.unproject((200.0, 200.0)).1, 500.0));
        assert!(close(placed.unproject((0.0, 0.0)).0, 500.0));
        let rect = Rect::new(100.0, 100.0, 50.0, 25.0);
        let screen = placed.project(&rect);
        let back = placed.unproject((screen.x, screen.y));
        assert!(close(back.0, 100.0) && close(back.1, 100.0));
        assert!(placed.unproject((f64::NAN, f64::INFINITY)).0.is_finite());
    }

    #[test]
    fn the_zoom_control_steps_between_its_bounds() {
        assert!(close(zoomed(1.0, 1), 1.1));
        assert!(close(zoomed(1.0, -3), MIN_ZOOM));
        assert!(close(zoomed(MAX_ZOOM, 5), MAX_ZOOM));
    }
}

/// Which background adjustments mean anything right now.
///
/// Every panel that offers the background controls has to answer the same
/// question: with no picture chosen, clearing it, hiding it or zooming it does
/// nothing. The rule lives here so the panels disable the same controls instead
/// of each guessing, and so it can be tested without a window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BackgroundControls {
    /// Whether a picture is selected at all.
    pub has_image: bool,
    /// Whether the opacity can still be raised.
    pub opacity_up: bool,
    /// Whether the opacity can still be lowered.
    pub opacity_down: bool,
    /// Whether the picture can still be zoomed in.
    pub zoom_in: bool,
    /// Whether the picture can still be zoomed out.
    pub zoom_out: bool,
    /// Whether the picture can be cleared or hidden.
    pub clearable: bool,
}

/// The state of the background controls, from the preferences that drive them.
pub fn background_controls(has_image: bool, opacity: f32, zoom: f64) -> BackgroundControls {
    let has_image = has_image && opacity.is_finite() && zoom.is_finite();
    BackgroundControls {
        has_image,
        opacity_up: has_image && opacity < 1.0 - 1.0e-6,
        opacity_down: has_image && opacity > 0.0 + 1.0e-6,
        zoom_in: has_image && zoom < MAX_ZOOM - 1.0e-9,
        zoom_out: has_image && zoom > MIN_ZOOM + 1.0e-9,
        clearable: has_image,
    }
}

#[cfg(test)]
mod background_control_tests {
    use super::*;

    #[test]
    fn the_background_controls_report_what_can_be_done() {
        // Without a picture every control is inert, and the panel says so
        // instead of offering buttons that do nothing.
        let none = background_controls(false, 0.5, 1.0);
        assert!(!none.has_image);
        assert!(!none.opacity_up && !none.opacity_down);
        assert!(!none.zoom_in && !none.zoom_out);
        assert!(!none.clearable);

        // With a picture the adjustments depend on where they already are.
        let middle = background_controls(true, 0.5, 1.5);
        assert!(middle.has_image && middle.clearable);
        assert!(middle.opacity_up && middle.opacity_down);
        assert!(middle.zoom_in && middle.zoom_out);

        // At the limits only the other direction is offered.
        let full = background_controls(true, 1.0, MAX_ZOOM);
        assert!(!full.opacity_up && full.opacity_down);
        assert!(!full.zoom_in && full.zoom_out);

        let empty = background_controls(true, 0.0, MIN_ZOOM);
        assert!(empty.opacity_up && !empty.opacity_down);
        assert!(empty.zoom_in && !empty.zoom_out);
        assert!(empty.clearable, "a clear picture can still be cleared");

        // Nonsense numbers leave the controls inert rather than misbehaving.
        let broken = background_controls(true, f32::NAN, f64::NAN);
        assert!(!broken.has_image && !broken.clearable);
    }
}
