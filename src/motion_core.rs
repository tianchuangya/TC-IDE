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

/// What produced an edit highlight.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditKind {
    Insert,
    Delete,
    Paste,
    Undo,
    Redo,
}

impl EditKind {
    /// Whether an edit of this kind takes over the highlight of the previous
    /// one, so that typing a word stays a single animation instead of one per
    /// character.
    fn continues(self, previous: EditKind) -> bool {
        matches!(
            (previous, self),
            (EditKind::Insert, EditKind::Insert) | (EditKind::Delete, EditKind::Delete)
        )
    }
}

/// One span that animates after an edit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EditPulse {
    pub kind: EditKind,
    pub line: usize,
    /// Byte range inside the line, in the coordinates of the edited document.
    pub start: usize,
    pub end: usize,
    /// Seconds since the edit.
    pub age: f32,
}

impl EditPulse {
    /// Highlight amount: `1.0` right after the edit, `0.0` when it is over.
    pub fn amount(&self) -> f32 {
        (1.0 - self.age / EditPulses::DURATION).clamp(0.0, 1.0)
    }

    /// The same amount eased out, for drawing.
    pub fn eased(&self) -> f32 {
        ease_out_cubic(self.amount())
    }

    /// Whether the edit removed text instead of adding it.
    pub fn removes(&self) -> bool {
        self.kind == EditKind::Delete
    }
}

/// Highlights of the edits of the last fraction of a second.
///
/// A paste of a whole block is one span rather than one per character, and a
/// stream of keystrokes keeps at most [`EditPulses::LIMIT`] highlights alive, so
/// fast typing never piles up animations that lag behind the text.
#[derive(Clone, Debug, Default)]
pub struct EditPulses {
    pulses: Vec<EditPulse>,
}

impl EditPulses {
    /// How long one highlight lasts.
    pub const DURATION: f32 = 0.26;
    /// How many highlights are kept at once.
    pub const LIMIT: usize = 4;
    /// Edits closer together than this on the same line become one highlight.
    pub const COALESCE: f32 = 0.12;

    /// No highlights yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record an edit, in the coordinates of the edited document.
    pub fn push(&mut self, kind: EditKind, line: usize, start: usize, end: usize) {
        let (start, end) = (start.min(end), start.max(end));
        if let Some(last) = self.pulses.last_mut() {
            let touching = start <= last.end.saturating_add(1) && last.start <= end + 1;
            if last.line == line
                && last.age <= Self::COALESCE
                && kind.continues(last.kind)
                && touching
            {
                last.start = last.start.min(start);
                last.end = last.end.max(end);
                last.age = 0.0;
                return;
            }
        }
        if self.pulses.len() >= Self::LIMIT {
            self.pulses.remove(0);
        }
        self.pulses.push(EditPulse {
            kind,
            line,
            start,
            end,
            age: 0.0,
        });
    }

    /// Age the highlights and drop the finished ones.
    ///
    /// Unlike a spring, an age follows the real frame time: a long frame ends
    /// the highlight instead of stretching it.
    pub fn step(&mut self, dt: f32) {
        let dt = if dt.is_finite() && dt > 0.0 { dt } else { 0.0 };
        for pulse in &mut self.pulses {
            pulse.age += dt;
        }
        self.pulses.retain(|pulse| pulse.age < Self::DURATION);
    }

    /// The newest highlight on `line`, which is the one to draw.
    pub fn on_line(&self, line: usize) -> Option<&EditPulse> {
        self.pulses.iter().rev().find(|pulse| pulse.line == line)
    }

    /// The highlight of the most recent edit, whichever line it touched.
    ///
    /// A backend animates the edit that just happened, and that is not always
    /// the line the caret ended on: pressing Enter records the edit on the
    /// line above the new caret.
    pub fn newest(&self) -> Option<&EditPulse> {
        self.pulses.last()
    }

    /// Every live highlight, oldest first.
    pub fn pulses(&self) -> &[EditPulse] {
        &self.pulses
    }

    /// Highlights currently alive.
    pub fn len(&self) -> usize {
        self.pulses.len()
    }

    /// Whether no highlight is alive.
    pub fn is_empty(&self) -> bool {
        self.pulses.is_empty()
    }

    /// Forget every highlight, for example when another document is opened.
    pub fn clear(&mut self) {
        self.pulses.clear();
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

    #[test]
    fn an_edit_highlight_fades_out_and_is_dropped() {
        let mut pulses = EditPulses::new();
        pulses.push(EditKind::Paste, 4, 10, 210);
        assert_eq!(pulses.len(), 1);
        let newest = pulses
            .on_line(4)
            .copied()
            .expect("the paste is highlighted");
        assert_eq!((newest.start, newest.end), (10, 210));
        assert_eq!(newest.amount(), 1.0);
        assert_eq!(newest.eased(), 1.0);

        pulses.step(EditPulses::DURATION / 2.0);
        let half = pulses.on_line(4).expect("still fading");
        assert!((half.amount() - 0.5).abs() < 1.0e-6, "{}", half.amount());
        assert!(half.eased() > 0.5, "the fade eases out");

        for _ in 0..40 {
            pulses.step(1.0 / 60.0);
        }
        assert!(pulses.is_empty(), "a finished highlight is dropped");
        assert!(pulses.on_line(4).is_none());
    }

    #[test]
    fn typing_a_word_stays_one_highlight_and_keeps_its_span() {
        let mut pulses = EditPulses::new();
        for step in 0..12 {
            let at = 20 + step;
            pulses.push(EditKind::Insert, 2, at, at + 1);
            pulses.step(1.0 / 60.0);
        }
        assert_eq!(pulses.len(), 1, "one keystroke is not one animation");
        let pulse = pulses.on_line(2).copied().expect("the word is highlighted");
        assert_eq!((pulse.start, pulse.end), (20, 32));
        assert!(
            pulse.age <= 1.0 / 60.0 + 1.0e-6,
            "the fade restarts with every character: age {}",
            pulse.age
        );

        // A keystroke somewhere else on the line is a highlight of its own.
        for _ in 0..5 {
            pulses.step(1.0 / 60.0);
        }
        pulses.push(EditKind::Insert, 2, 60, 61);
        assert_eq!(
            pulses.len(),
            2,
            "a distant edit is not folded into the word"
        );

        // And once the highlight has faded, an edit starts a fresh one.
        for _ in 0..40 {
            pulses.step(1.0 / 60.0);
        }
        assert!(pulses.is_empty());
        pulses.push(EditKind::Insert, 2, 80, 81);
        assert_eq!(pulses.len(), 1);
        assert_eq!(pulses.on_line(2).map(|pulse| pulse.start), Some(80));
    }

    #[test]
    fn a_burst_of_edits_never_piles_up() {
        let mut pulses = EditPulses::new();
        for index in 0..50 {
            pulses.push(EditKind::Paste, index % 7, 0, 400);
        }
        assert_eq!(pulses.len(), EditPulses::LIMIT);
        assert!(
            pulses.on_line(49 % 7).is_some(),
            "the newest edit is always highlighted"
        );
        assert_eq!(pulses.pulses().len(), EditPulses::LIMIT);
    }

    #[test]
    fn undo_and_redo_are_highlights_of_their_own() {
        let mut pulses = EditPulses::new();
        pulses.push(EditKind::Insert, 3, 0, 5);
        pulses.push(EditKind::Undo, 3, 5, 5);
        pulses.push(EditKind::Redo, 3, 5, 12);
        assert_eq!(
            pulses.len(),
            3,
            "an undo is not folded into the typing before it"
        );
        assert_eq!(
            pulses.on_line(3).map(|pulse| pulse.kind),
            Some(EditKind::Redo)
        );
        // A deletion collapses to the caret and is marked as a removal.
        pulses.push(EditKind::Delete, 5, 8, 8);
        let deletion = pulses.on_line(5).copied().expect("delete highlight");
        assert!(deletion.removes());
        assert_eq!((deletion.start, deletion.end), (8, 8));
    }

    #[test]
    fn a_broken_frame_time_does_not_advance_a_highlight() {
        let mut pulses = EditPulses::new();
        pulses.push(EditKind::Insert, 0, 0, 3);
        pulses.step(f32::NAN);
        pulses.step(-1.0);
        assert_eq!(pulses.on_line(0).map(|pulse| pulse.age), Some(0.0));
        pulses.step(10.0);
        assert!(
            pulses.is_empty(),
            "a long frame finishes the highlight instead of freezing it"
        );
        pulses.push(EditKind::Insert, 1, 0, 1);
        pulses.clear();
        assert!(pulses.is_empty());
    }

    #[test]
    fn the_macos_self_check_expectations_hold_here() {
        // The macOS self-check asserts exactly this about the edit highlight,
        // and the runner is the only place it otherwise runs, so the same
        // expectations are pinned here where they are cheap to check.
        let mut pulses = EditPulses::new();
        pulses.push(EditKind::Insert, 0, 0, 1);
        assert!(pulses.on_line(0).is_some(), "an edit is recorded");
        let lifted = pulses.on_line(0).map(|p| p.eased()).unwrap_or(0.0);
        assert!(lifted > 0.0, "the line lifts");

        // After the duration the highlight is gone.
        for _ in 0..(EditPulses::DURATION * 70.0) as usize {
            pulses.step(1.0 / 60.0);
        }
        assert_eq!(pulses.on_line(0).map(|p| p.eased()), None, "it fades out");

        // A paste is recorded as a paste.
        let mut pulses = EditPulses::new();
        pulses.push(EditKind::Paste, 0, 0, 5);
        assert_eq!(pulses.on_line(0).map(|p| p.kind), Some(EditKind::Paste));
    }

    #[test]
    fn a_newline_records_its_edit_on_the_line_it_started_from() {
        use crate::motion_core::{EditKind, EditPulses};
        // What pressing Enter at the end of a document does: the edit is
        // recorded against the line the caret was on, then the caret moves to
        // the new line. Lifting the caret's line would show nothing.
        let mut pulses = EditPulses::new();
        pulses.push(EditKind::Insert, 0, 3, 4); // recorded on line 0
        let caret_line = 1; // the caret ends on the new line
        assert!(
            pulses.on_line(caret_line).is_none(),
            "the caret line has no pulse"
        );
        let pulse = pulses.on_line(0).expect("the edit line has one");
        assert_eq!(pulse.line, 0, "the pulse names the line it touched");
        assert!(pulse.eased() > 0.0);
    }

    #[test]
    fn the_newest_highlight_is_the_edit_that_just_happened() {
        // Pressing Enter records the edit on the line the caret was on, then
        // the caret moves down. A backend that asked for the caret's line would
        // find nothing, which is what the macOS check caught.
        let mut pulses = EditPulses::new();
        assert!(pulses.newest().is_none(), "nothing has happened yet");
        pulses.push(EditKind::Insert, 0, 3, 4);
        let newest = pulses.newest().expect("an edit was recorded");
        assert_eq!(newest.line, 0, "the edit names the line it touched");
        // The caret moved to the next line, which has no highlight.
        assert!(pulses.on_line(1).is_none());
        assert!(pulses.newest().is_some(), "the newest one is still there");

        // A later edit becomes the newest one.
        pulses.push(EditKind::Paste, 4, 0, 5);
        assert_eq!(pulses.newest().map(|pulse| pulse.line), Some(4));
        assert_eq!(
            pulses.newest().map(|pulse| pulse.kind),
            Some(EditKind::Paste)
        );
        // And it goes away with the rest once its time is up.
        for _ in 0..(EditPulses::DURATION * 70.0) as usize {
            pulses.step(1.0 / 60.0);
        }
        assert!(pulses.newest().is_none(), "the highlight faded out");
    }
}

/// How far into an edit one character of it is, in `0.0..=1.0`.
///
/// An edit that inserts several characters should not arrive as one block:
/// typing `let` should read as three keystrokes, and a paste should read as
/// one action that is longer. Each character is given a slice of the pulse's
/// life, spread across a fraction of it so a long run still settles quickly
/// rather than dribbling in for a second.
///
/// Returns `1.0` for a character whose turn has come and gone, so a caller can
/// treat it as settled.
pub fn character_progress(index: usize, count: usize, age: f32, duration: f32) -> f32 {
    // Nothing to animate, or no time to animate it in: both mean the text is
    // simply there, which is better than leaving it invisible for ever.
    if count == 0 || duration <= 0.0 || !age.is_finite() {
        return 1.0;
    }
    if count == 1 {
        // A single character is the whole edit, so it rises with the pulse.
        return settled_fraction(age, duration);
    }
    // The characters share the first part of the pulse; the rest is the fade,
    // which runs for the edit as a whole.
    let spread = duration * CHARACTER_SPREAD;
    let each = spread / count as f32;
    let start = each * index as f32;
    let elapsed = age - start;
    if elapsed <= 0.0 {
        return 0.0;
    }
    settled_fraction(elapsed, CHARACTER_RISE)
}

/// Fraction of a run of characters that is spent bringing them in, rather
/// than fading the whole edit out.
pub const CHARACTER_SPREAD: f32 = 0.6;
/// How long one character takes to settle, in seconds.
pub const CHARACTER_RISE: f32 = 0.05;

/// How settled something is after `age` of a `duration` long rise.
fn settled_fraction(age: f32, duration: f32) -> f32 {
    if duration <= 0.0 || !age.is_finite() {
        return 1.0;
    }
    (age / duration).clamp(0.0, 1.0)
}

/// The lift of one character as it arrives, in layout units.
///
/// A character comes up from below rather than fading in place, so a run of
/// them reads as being written. Returns two offsets: how far it still has to
/// travel, and its opacity, so a caller can draw both without repeating the
/// arithmetic.
pub fn character_arrival(progress: f32, travel: f32) -> (f32, f32) {
    let progress = if progress.is_finite() {
        progress.clamp(0.0, 1.0)
    } else {
        1.0
    };
    // Ease out: most of the distance is covered early, so the tail is short
    // and a fast typist is not left waiting for characters to land.
    let eased = 1.0 - (1.0 - progress) * (1.0 - progress);
    let offset = travel * (1.0 - eased);
    let opacity = 0.35 + 0.65 * eased;
    (offset, opacity)
}

#[cfg(test)]
mod character_tests {
    use super::*;

    #[test]
    fn a_typed_run_arrives_one_character_at_a_time() {
        // Typing `let` should read as three keystrokes, so the second
        // character must still be waiting when the first has started.
        let duration = EditPulses::DURATION;
        let first = character_progress(0, 3, 0.0, duration);
        let second = character_progress(1, 3, 0.0, duration);
        let third = character_progress(2, 3, 0.0, duration);
        assert_eq!(first, 0.0, "the first has not started either");
        assert_eq!(second, 0.0);
        assert_eq!(third, 0.0);

        // A little later the first is moving and the last has not begun.
        let early = 0.01;
        let first = character_progress(0, 3, early, duration);
        let third = character_progress(2, 3, early, duration);
        assert!(first > 0.0, "the first character has started");
        assert_eq!(third, 0.0, "the last is still waiting");
        // An earlier character is always at least as far along as a later
        // one, at every moment: the run arrives in the order it was typed.
        for age in [0.0, 0.01, 0.02, 0.05, 0.08, 0.13, 0.26] {
            for index in 1..3 {
                let earlier = character_progress(index - 1, 3, age, duration);
                let later = character_progress(index, 3, age, duration);
                assert!(
                    earlier >= later,
                    "at {age}s character {index} was ahead of {}: {earlier} < {later}",
                    index - 1
                );
            }
        }
    }

    #[test]
    fn every_character_settles_before_the_edit_is_over() {
        // A run must not still be arriving when the highlight has already
        // faded, or a fast typist sees characters land after the edit ended.
        let duration = EditPulses::DURATION;
        for count in 1..=12 {
            for index in 0..count {
                let settled = character_progress(index, count, duration, duration);
                assert_eq!(
                    settled, 1.0,
                    "{index} of {count} had not settled by the end"
                );
            }
        }
    }

    #[test]
    fn a_single_character_is_the_whole_edit() {
        // One keystroke has nothing to spread across, so it rises with the
        // pulse rather than being held back.
        let duration = EditPulses::DURATION;
        assert_eq!(character_progress(0, 1, 0.0, duration), 0.0);
        let half = character_progress(0, 1, duration / 2.0, duration);
        assert!((half - 0.5).abs() < 0.01, "{half}");
        assert_eq!(character_progress(0, 1, duration, duration), 1.0);
    }

    #[test]
    fn an_empty_run_and_a_strange_duration_are_handled() {
        // Guards against a caller passing nothing, or a duration that would
        // divide by zero.
        assert_eq!(character_progress(0, 0, 0.0, 1.0), 1.0);
        assert_eq!(character_progress(5, 0, 0.0, 1.0), 1.0);
        assert_eq!(character_progress(0, 3, 0.0, 0.0), 1.0);
        assert_eq!(character_progress(0, 3, f32::NAN, 1.0), 1.0);
        // A negative age is before the edit, so nothing has started. A caller
        // never passes one, but reporting "arrived" would make a character
        // appear early rather than late, which is the worse of the two.
        assert_eq!(character_progress(0, 3, -1.0, 1.0), 0.0);
    }

    #[test]
    fn a_character_comes_up_from_below_and_reaches_its_place() {
        // The offset shrinks to nothing and the opacity rises to one, so a
        // settled character is drawn exactly where it belongs.
        let (offset, opacity) = character_arrival(0.0, 9.0);
        assert_eq!(offset, 9.0, "it starts below its place");
        assert!(opacity < 1.0, "and faint");
        let (offset, opacity) = character_arrival(1.0, 9.0);
        assert_eq!(offset, 0.0, "it lands exactly");
        assert_eq!(opacity, 1.0);
        // Halfway, most of the distance is already covered.
        let (offset, _) = character_arrival(0.5, 9.0);
        assert!(offset < 4.5, "the movement eases out: {offset}");
        assert!(offset > 0.0);
        // Out of range values are clamped rather than producing a jump.
        assert_eq!(character_arrival(-1.0, 9.0), (9.0, 0.35));
        assert_eq!(character_arrival(2.0, 9.0), (0.0, 1.0));
        assert_eq!(character_arrival(f32::NAN, 9.0), (0.0, 1.0));
    }
}

/// What a piece of interface motion is for.
///
/// Every panel in the Linux backend used one profile, so a dropdown, a
/// settings sheet, a terminal drawer and the graph all moved at the same
/// speed. They are not the same kind of thing: a menu that opens under the
/// pointer should feel immediate, while a drawer that takes a third of the
/// window can afford to be softer. Naming the role here means a backend asks
/// for the motion it means rather than reaching for whichever profile it
/// copied from the code next door.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MotionRole {
    /// A small panel that appears near what it belongs to: a menu, a popover.
    Overlay,
    /// A large panel that slides in: a drawer, a sidebar, the graph.
    Drawer,
    /// Something that follows a pointer or a caret.
    Follow,
}

impl MotionRole {
    pub const ALL: [MotionRole; 3] = [MotionRole::Overlay, MotionRole::Drawer, MotionRole::Follow];

    /// The spring this role moves with.
    pub fn profile(self) -> SpringProfile {
        match self {
            MotionRole::Overlay => OVERLAY,
            MotionRole::Drawer => DRAWER,
            // Following has to keep up without lagging behind the pointer, so
            // it is stiffer than either panel and barely overshoots.
            MotionRole::Follow => FOLLOW,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            MotionRole::Overlay => "overlay",
            MotionRole::Drawer => "drawer",
            MotionRole::Follow => "follow",
        }
    }
}

/// Motion profile for something that tracks a pointer or a caret.
pub const FOLLOW: SpringProfile = SpringProfile {
    stiffness: 620.0,
    damping: 34.0,
    overshoot: 0.0,
    settle_distance: 0.0004,
    settle_speed: 0.0004,
};

/// The distance a role travels as it arrives, in layout units.
///
/// A menu drops a short way from the bar it belongs to; a drawer travels
/// further because it covers more; something that follows does not travel at
/// all, because it is already where it belongs.
pub fn travel(role: MotionRole) -> f32 {
    match role {
        MotionRole::Overlay => 12.0,
        MotionRole::Drawer => 24.0,
        MotionRole::Follow => 0.0,
    }
}

/// How settled a motion is, expressed so a caller can draw with it.
///
/// Returns `(progress, offset, opacity)`: the clamped progress, how far the
/// thing still has to travel, and how opaque it should be. The opacity comes up
/// faster than the travel finishes, so nothing is drawn faintly for long.
pub fn arrival(role: MotionRole, value: f32) -> (f32, f32, f32) {
    let progress = if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        1.0
    };
    let offset = travel(role) * (1.0 - progress);
    let opacity = (0.25 + 0.75 * progress).clamp(0.0, 1.0);
    (progress, offset, opacity)
}

#[cfg(test)]
mod role_tests {
    use super::*;

    #[test]
    fn each_role_has_its_own_spring() {
        // The point of naming roles is that they do not all move the same way;
        // if two profiles matched, one of the roles would be pointless.
        let profiles = MotionRole::ALL.map(|role| role.profile());
        for (index, left) in profiles.iter().enumerate() {
            for (offset, right) in profiles.iter().enumerate().skip(index + 1) {
                assert_ne!(
                    (left.stiffness, left.damping),
                    (right.stiffness, right.damping),
                    "{:?} and {:?} move identically",
                    MotionRole::ALL[index],
                    MotionRole::ALL[offset]
                );
            }
        }
        assert_eq!(MotionRole::ALL.len(), 3);
    }

    #[test]
    fn a_following_motion_keeps_up_and_does_not_overshoot() {
        // Something that tracks a pointer and overshoots reads as lagging, so
        // it is stiffer than the panels and has no overshoot at all.
        let follow = MotionRole::Follow.profile();
        for role in [MotionRole::Overlay, MotionRole::Drawer] {
            let panel = role.profile();
            assert!(
                follow.stiffness > panel.stiffness,
                "follow should be stiffer than {}",
                role.label()
            );
            assert!(
                follow.settle_distance <= panel.settle_distance,
                "follow should settle at least as precisely as {}",
                role.label()
            );
        }
        assert_eq!(follow.overshoot, 0.0);
        // And it does not travel, because it is already where it belongs.
        assert_eq!(travel(MotionRole::Follow), 0.0);
    }

    #[test]
    fn a_drawer_travels_further_than_a_menu() {
        // A drawer covers more of the window, so it can afford to move more
        // than a menu that drops a short way from the bar.
        assert!(travel(MotionRole::Drawer) > travel(MotionRole::Overlay));
        assert!(travel(MotionRole::Overlay) > 0.0);
        // Every role has a label, for a diagnostic or a test name.
        for role in MotionRole::ALL {
            assert!(!role.label().is_empty());
        }
    }

    #[test]
    fn a_role_settles_to_its_place_and_full_strength() {
        for role in MotionRole::ALL {
            let (progress, offset, opacity) = arrival(role, 1.0);
            assert_eq!(progress, 1.0);
            assert_eq!(offset, 0.0, "{} has arrived", role.label());
            assert_eq!(opacity, 1.0, "{} is fully drawn", role.label());
            // At the start it is offset by the whole travel and faint.
            let (progress, offset, opacity) = arrival(role, 0.0);
            assert_eq!(progress, 0.0);
            assert_eq!(offset, travel(role));
            assert!(opacity < 1.0);
            // Halfway it is nearer its place than not, and mostly opaque.
            let (_, offset, opacity) = arrival(role, 0.5);
            assert!(offset <= travel(role) / 2.0);
            assert!(opacity > 0.5);
        }
    }

    #[test]
    fn a_broken_value_does_not_produce_a_broken_frame() {
        // A spring should never produce these, but drawing from a NaN would
        // put a panel somewhere unrepresentable rather than leaving it home.
        for role in MotionRole::ALL {
            assert_eq!(arrival(role, f32::NAN).0, 1.0);
            assert_eq!(arrival(role, f32::INFINITY).0, 1.0);
            assert_eq!(arrival(role, -5.0).0, 0.0);
            assert_eq!(arrival(role, 5.0).0, 1.0);
            for value in [-1.0f32, 0.0, 0.3, 0.7, 1.0, 2.0] {
                let (progress, offset, opacity) = arrival(role, value);
                assert!((0.0..=1.0).contains(&progress));
                assert!(offset >= 0.0 && offset <= travel(role));
                assert!((0.0..=1.0).contains(&opacity));
            }
        }
    }
}
