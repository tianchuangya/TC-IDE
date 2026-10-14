//! Shared motion model.
//!
//! Every animated element in the interface — overlays, the terminal drawer, tree
//! rows, edit highlights — moves with the same damped spring. The two backends
//! had their own copies of the integration step with slightly different numbers,
//! so the same gesture felt different depending on the platform. Keeping the
//! spring, its settling rule and the easing helpers here means one tested
//! behaviour and one place to tune it.
#![allow(dead_code)] // The backends adopt the pieces they need as they migrate.

use std::collections::BTreeMap;

/// A damped spring driven by a fixed frame step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spring {
    pub value: f32,
    pub velocity: f32,
    /// Stiffness of the pull towards the target.
    pub stiffness: f32,
    /// Damping applied every step.
    pub damping: f32,
    /// How far the value may overshoot the target before it is clamped.
    pub overshoot: f32,
}

/// Distance and speed below which a spring is considered settled.
pub const SETTLE_DISTANCE: f32 = 0.0008;
/// Speed below which a spring is considered settled.
pub const SETTLE_SPEED: f32 = 0.0008;

/// Motion profile used by overlay panels.
pub const OVERLAY: SpringProfile = SpringProfile {
    stiffness: 300.0,
    damping: 19.0,
    overshoot: 0.04,
    settle_distance: 0.0008,
    settle_speed: 0.0008,
};

/// Motion profile used by tree and drawer motion, which is a little softer.
pub const DRAWER: SpringProfile = SpringProfile {
    stiffness: 235.0,
    damping: 17.0,
    overshoot: 0.08,
    settle_distance: 0.001,
    settle_speed: 0.01,
};

/// Numbers that define a spring's feel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpringProfile {
    pub stiffness: f32,
    pub damping: f32,
    pub overshoot: f32,
    pub settle_distance: f32,
    pub settle_speed: f32,
}

impl Spring {
    /// A spring at rest at `value`.
    pub fn new(value: f32) -> Self {
        Self::with_profile(value, OVERLAY)
    }

    /// A spring with a specific feel.
    pub fn with_profile(value: f32, profile: SpringProfile) -> Self {
        Self {
            value,
            velocity: 0.0,
            stiffness: profile.stiffness,
            damping: profile.damping,
            overshoot: profile.overshoot,
        }
    }

    /// Advance towards a target by one frame.
    ///
    /// The step is the same semi implicit integration the backends used, so the
    /// motion of existing animations does not change.
    pub fn step(&mut self, target: f32, dt: f32) {
        let dt = if dt.is_finite() && dt > 0.0 {
            dt.min(0.1)
        } else {
            0.0
        };
        self.velocity += (target - self.value) * self.stiffness * dt;
        self.velocity *= (-self.damping * dt).exp();
        self.value += self.velocity * dt;
        self.value = self.value.clamp(
            target.min(0.0) - self.overshoot,
            target.max(1.0) + self.overshoot,
        );
        if (target - self.value).abs() < SETTLE_DISTANCE && self.velocity.abs() < SETTLE_SPEED {
            self.value = target;
            self.velocity = 0.0;
        }
    }

    /// Whether the spring has come to rest on its target.
    pub fn is_settled(&self, target: f32) -> bool {
        (target - self.value).abs() < f32::EPSILON && self.velocity.abs() < f32::EPSILON
    }

    /// Reset to a value without motion.
    pub fn settle_at(&mut self, value: f32) {
        self.value = value;
        self.velocity = 0.0;
    }

    /// Progress towards a target in `0.0..=1.0`, for drawing.
    pub fn progress(&self, target: f32) -> f32 {
        if target >= 0.5 {
            self.value.clamp(0.0, 1.0)
        } else {
            (1.0 - self.value).clamp(0.0, 1.0)
        }
    }
}

/// Step a value and velocity pair towards a target.
///
/// Kept for the call sites that store the two numbers separately.
pub fn step(value: &mut f32, velocity: &mut f32, target: f32, dt: f32, profile: SpringProfile) {
    let mut spring = Spring {
        value: *value,
        velocity: *velocity,
        stiffness: profile.stiffness,
        damping: profile.damping,
        overshoot: profile.overshoot,
    };
    spring.step(target, dt);
    if (target - spring.value).abs() < profile.settle_distance
        && spring.velocity.abs() < profile.settle_speed
    {
        spring.value = target;
        spring.velocity = 0.0;
    }
    *value = spring.value;
    *velocity = spring.velocity;
}

/// Where one row of a list is drawn while the list is changing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RowSpring {
    /// Fractional row index the row is drawn at.
    pub position: f32,
    pub velocity: f32,
    /// Fractional row index the layout gave the row this frame.
    pub target: f32,
    /// Whether the layout placed the row during the last frame.
    pub seen: bool,
}

/// Slides rows to the place the layout gives them.
///
/// Folding a folder removes the rows below it from the list, and a list drawn
/// from row indices alone snapped those rows up by a whole row height. The
/// shift keeps one spring per row key, so a row that keeps its identity slides
/// to its new place while the rows around it do the same. A row the layout did
/// not place for a frame is forgotten by [`RowShift::prune`], so a long session
/// only carries springs for the rows on screen.
#[derive(Clone, Debug, Default)]
pub struct RowShift {
    rows: BTreeMap<String, RowSpring>,
}

impl RowShift {
    /// An empty shift.
    pub fn new() -> Self {
        Self::default()
    }

    /// Point `key` at `target` and return the position to draw it at.
    ///
    /// A key seen for the first time starts on its target: a row that appears
    /// because a folder opened should not fly in from somewhere else.
    pub fn place(&mut self, key: &str, target: f32, dt: f32) -> f32 {
        let row = self.rows.entry(key.to_string()).or_insert(RowSpring {
            position: target,
            velocity: 0.0,
            target,
            seen: true,
        });
        row.target = target;
        row.seen = true;
        let dt = if dt.is_finite() && dt > 0.0 {
            dt.min(0.1)
        } else {
            0.0
        };
        row.velocity += (target - row.position) * DRAWER.stiffness * dt;
        row.velocity *= (-DRAWER.damping * dt).exp();
        row.position += row.velocity * dt;
        if (target - row.position).abs() < DRAWER.settle_distance
            && row.velocity.abs() < DRAWER.settle_speed
        {
            row.position = target;
            row.velocity = 0.0;
        }
        row.position
    }

    /// The position `key` was last placed at, without stepping it.
    pub fn position(&self, key: &str) -> Option<f32> {
        self.rows.get(key).map(|row| row.position)
    }

    /// Drop every row the layout did not place during the last frame.
    pub fn prune(&mut self) {
        self.rows.retain(|_, row| {
            let seen = row.seen;
            row.seen = false;
            seen
        });
    }

    /// Whether every followed row sits on the place the layout gave it.
    pub fn settled(&self) -> bool {
        self.rows
            .values()
            .all(|row| row.position == row.target && row.velocity == 0.0)
    }

    /// Rows currently followed.
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Whether no row is followed.
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Forget every row, for example when the project changes.
    pub fn clear(&mut self) {
        self.rows.clear();
    }
}

/// Cubic ease out, for one shot transitions.
pub fn ease_out_cubic(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

/// Cubic ease in out, for continuous transitions.
pub fn ease_in_out_cubic(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
    }
}

/// A value that follows a target with a fixed time constant, for hover fades.
pub fn approach(value: f32, target: f32, dt: f32, rate: f32) -> f32 {
    let dt = if dt.is_finite() && dt > 0.0 {
        dt.min(0.1)
    } else {
        0.0
    };
    let blend = (rate * dt).clamp(0.0, 1.0);
    value + (target - value) * blend
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frames(steps: usize) -> f32 {
        1.0 / 60.0 * steps as f32
    }

    #[test]
    fn a_spring_reaches_its_target_and_stops() {
        let mut spring = Spring::new(0.0);
        for _ in 0..120 {
            spring.step(1.0, 1.0 / 60.0);
        }
        assert!(
            spring.is_settled(1.0),
            "value {} velocity {}",
            spring.value,
            spring.velocity
        );
        assert_eq!(spring.value, 1.0);

        // Closing behaves the same way.
        for _ in 0..120 {
            spring.step(0.0, 1.0 / 60.0);
        }
        assert!(spring.is_settled(0.0));
        assert_eq!(spring.value, 0.0);
    }

    #[test]
    fn motion_is_monotonic_in_time_and_bounded() {
        let mut spring = Spring::new(0.0);
        let mut previous = 0.0;
        let mut overshot = false;
        for step_index in 0..60 {
            spring.step(1.0, 1.0 / 60.0);
            assert!(
                spring.value >= previous - 0.05,
                "the spring jumped backwards at frame {step_index}"
            );
            if spring.value > 1.0 {
                overshot = true;
            }
            assert!(spring.value <= 1.0 + OVERLAY.overshoot);
            previous = spring.value;
        }
        assert!(
            overshot,
            "the spring is damped but should still settle smoothly"
        );
    }

    #[test]
    fn a_broken_frame_time_cannot_explode() {
        let mut spring = Spring::new(0.0);
        spring.step(1.0, f32::NAN);
        assert!(spring.value.is_finite());
        spring.step(1.0, -1.0);
        assert!(spring.value.is_finite());
        spring.step(1.0, 10.0);
        assert!(spring.value.is_finite() && spring.value.abs() <= 1.1);
    }

    #[test]
    fn progress_reports_open_and_closed_amounts() {
        let mut spring = Spring::new(0.0);
        assert_eq!(spring.progress(1.0), 0.0);
        spring.settle_at(1.0);
        assert_eq!(spring.progress(1.0), 1.0);
        assert_eq!(spring.progress(0.0), 0.0);
        spring.settle_at(0.4);
        assert!((spring.progress(1.0) - 0.4).abs() < 1e-6);
        assert!((spring.progress(0.0) - 0.6).abs() < 1e-6);
    }

    #[test]
    fn the_step_helper_matches_the_spring() {
        let mut value = 0.0;
        let mut velocity = 0.0;
        let mut spring = Spring::new(0.0);
        for _ in 0..30 {
            step(&mut value, &mut velocity, 1.0, 1.0 / 60.0, OVERLAY);
            spring.step(1.0, 1.0 / 60.0);
            assert!((value - spring.value).abs() < 1e-4);
        }
        assert!(
            value > 0.5,
            "the value should have moved towards the target"
        );
    }

    #[test]
    fn the_drawer_profile_is_softer_than_the_overlay_profile() {
        let mut overlay = Spring::with_profile(0.0, OVERLAY);
        let mut drawer = Spring::with_profile(0.0, DRAWER);
        overlay.step(1.0, frames(10));
        drawer.step(1.0, frames(10));
        assert!(
            overlay.value > drawer.value,
            "the overlay spring should move faster: {} vs {}",
            overlay.value,
            drawer.value
        );
    }

    #[test]
    fn easing_helpers_cover_their_range() {
        assert_eq!(ease_out_cubic(0.0), 0.0);
        assert_eq!(ease_out_cubic(1.0), 1.0);
        assert!(ease_out_cubic(0.5) > 0.5);
        assert_eq!(ease_in_out_cubic(0.0), 0.0);
        assert_eq!(ease_in_out_cubic(1.0), 1.0);
        assert!((ease_in_out_cubic(0.5) - 0.5).abs() < 1e-6);
        assert_eq!(ease_out_cubic(5.0), 1.0);

        let mut value = 0.0;
        for _ in 0..60 {
            value = approach(value, 1.0, 1.0 / 60.0, 12.0);
        }
        assert!(value > 0.99);
        assert_eq!(approach(0.0, 1.0, 0.0, 12.0), 0.0);
    }

    #[test]
    fn a_row_starts_on_its_place_and_slides_when_the_layout_moves_it() {
        let mut shift = RowShift::new();
        assert_eq!(shift.place("src/main.rs", 4.0, frames(1)), 4.0);
        assert!(shift.settled());

        // The folder above the row folded, so the layout now puts it at row 1.
        let first = shift.place("src/main.rs", 1.0, frames(1));
        assert!(
            first < 4.0 && first > 1.0,
            "the row should slide, not jump: {first}"
        );
        for _ in 0..120 {
            shift.place("src/main.rs", 1.0, frames(1));
        }
        assert_eq!(shift.position("src/main.rs"), Some(1.0));
        assert!(shift.settled());
    }

    #[test]
    fn rows_keep_their_own_spring_and_never_lag_for_long() {
        let mut shift = RowShift::new();
        shift.place("a", 0.0, frames(1));
        shift.place("b", 5.0, frames(1));
        // A fold removes two rows above `b`, and a search box scrolls `a` down.
        let mut last = 0.0;
        for _ in 0..30 {
            last = shift.place("a", 3.0, frames(1));
            shift.place("b", 2.0, frames(1));
        }
        assert!(
            (last - 3.0).abs() < 0.35,
            "half a second should be enough to travel three rows: {last}"
        );
        assert!(shift
            .position("b")
            .is_some_and(|value| (value - 2.0).abs() < 0.35));
        for _ in 0..120 {
            shift.place("a", 3.0, frames(1));
            shift.place("b", 2.0, frames(1));
        }
        assert_eq!(shift.position("a"), Some(3.0));
        assert_eq!(shift.position("b"), Some(2.0));
        assert!(shift.settled());
    }

    #[test]
    fn forgetting_rows_forgets_only_the_ones_the_layout_dropped() {
        let mut shift = RowShift::new();
        shift.place("kept", 0.0, frames(1));
        shift.place("folded", 1.0, frames(1));
        assert_eq!(shift.len(), 2);
        shift.prune();
        assert_eq!(shift.len(), 2, "a row that was placed stays known");

        shift.place("kept", 0.0, frames(1));
        shift.prune();
        assert_eq!(shift.len(), 1, "the folded row is forgotten");
        assert_eq!(shift.position("kept"), Some(0.0));
        assert_eq!(shift.position("folded"), None);

        shift.clear();
        assert!(shift.is_empty());
    }

    #[test]
    fn a_row_that_comes_back_starts_where_it_is_drawn() {
        let mut shift = RowShift::new();
        shift.place("a", 0.0, frames(1));
        shift.prune();
        shift.place("a", 0.0, frames(1));
        shift.place("b", 1.0, frames(1));
        shift.prune();
        // `b` disappeared with the folded folder and comes back when it opens.
        assert_eq!(shift.place("b", 1.0, frames(1)), 1.0);
    }

    #[test]
    fn a_broken_frame_time_leaves_the_rows_where_they_are() {
        let mut shift = RowShift::new();
        shift.place("row", 6.0, frames(1));
        assert_eq!(shift.place("row", 1.0, f32::NAN), 6.0);
        assert_eq!(shift.place("row", 1.0, -1.0), 6.0);
        assert!(shift.place("row", 1.0, 10.0).is_finite());
    }
}
