//! Packaging animation: how code is pulled into the build core and how the
//! artifacts come back out.
//!
//! Both backends show the same gesture while a package is built: the sources on
//! screen stretch and fall into a core in the middle, and the finished
//! artifacts fly back out of it. The numbers that shape that gesture — how long
//! the absorb takes, where a source is at a given moment, how much its trail
//! stretches, where the artifacts land — used to live inside the renderers, so
//! the two platforms drifted apart. They are pure functions here, which also
//! means the motion can be asserted in a unit test instead of being watched.
//!
//! The backends adopt the phase they show: both pull their sources in with
//! [`absorbed`], while the emitting core is drawn by the Linux backend today,
//! because the Windows packaging panel binds its progress to the real build
//! stages instead of a fixed phase.
#![allow(dead_code)]

/// Absorb time of a small project with no target selected.
const ABSORB_BASE: f32 = 1.35;
/// Extra absorb time per source line, so a larger project reads as heavier.
const ABSORB_PER_LINE: f32 = 0.006;
/// Extra absorb time per selected package target.
const ABSORB_PER_TARGET: f32 = 0.12;
/// Absorb time is clamped to this range.
const ABSORB_RANGE: (f32, f32) = (1.5, 5.5);

/// Curve of the pull: sources accelerate as they approach the core.
const ABSORB_ACCELERATION: f32 = 2.35;
/// Length of the motion trail at full stretch, relative to the distance the
/// source travels, so the smear reads the same on any window size or DPI.
const TRAIL_FRACTION: f32 = 0.10;
/// Lateral wobble of a travelling source, relative to the same distance.
const SWIRL_FRACTION: f32 = 0.032;
/// Fraction of the phase after which every source has arrived.
const ABSORB_ARRIVAL: f32 = 0.92;
/// Spread of the artifacts of the emitting core.
const EMIT_ACCELERATION: f32 = 2.6;
/// Delay between two artifacts leaving the core, as a fraction of the phase.
const EMIT_STAGGER: f32 = 0.08;
/// Shorter trail than this is not drawn: a one pixel smear reads as noise.
/// The caller compares it in its own units, so it scales the value.
pub const MIN_TRAIL: f32 = 3.0;

/// A phase time as a fraction of the phase, safe against broken frame times.
fn phase(t: f32) -> f32 {
    if t.is_finite() {
        t.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

/// Seconds the absorb phase lasts.
///
/// The project size sets the weight and the selected targets add the work of
/// packing them, so a build never finishes its animation before the real
/// command has started and never drags on for a tiny project.
pub fn absorb_seconds(lines: usize, targets: usize) -> f32 {
    (ABSORB_BASE + lines as f32 * ABSORB_PER_LINE + targets as f32 * ABSORB_PER_TARGET)
        .clamp(ABSORB_RANGE.0, ABSORB_RANGE.1)
}

/// The part of the phase one source travels in, as `(start, end)` in `0.0..=1.0`.
///
/// Sources do not queue up: the further one starts from the core, the earlier it
/// leaves and the faster it moves, so they all arrive together and the absorb
/// reads as a single pull. `max_distance` is the distance of the source furthest
/// from the core, and `index` breaks ties between sources at the same distance.
pub fn absorb_window(distance: f32, max_distance: f32, index: usize, count: usize) -> (f32, f32) {
    let reach = if max_distance > 0.0 {
        (distance / max_distance).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let stagger = if count > 0 {
        (index % 8) as f32 / count.min(8) as f32
    } else {
        0.0
    };
    let start = (0.20 * (1.0 - reach) + 0.05 * stagger).clamp(0.0, 0.45);
    (start, ABSORB_ARRIVAL)
}

/// Where one source is while the core pulls it in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Travel {
    pub x: f32,
    pub y: f32,
    /// `0.0` when the source starts, `1.0` once it has been absorbed.
    pub progress: f32,
    /// How far behind the source its trail reaches, in layout units.
    pub trail: f32,
    /// Lateral wobble that fades out as the source arrives, in layout units.
    pub swirl: f32,
}

/// Position of the source at `t` of the absorb phase.
pub fn absorbed(
    source: (f32, f32),
    centre: (f32, f32),
    index: usize,
    count: usize,
    max_distance: f32,
    t: f32,
) -> Travel {
    let dx = source.0 - centre.0;
    let dy = source.1 - centre.1;
    let distance = (dx * dx + dy * dy).sqrt();
    let (start, end) = absorb_window(distance, max_distance, index, count);
    let span = (end - start).max(1.0e-3);
    let travelled = ((phase(t) - start) / span).clamp(0.0, 1.0);
    let progress = travelled.powf(ABSORB_ACCELERATION);
    // The smear and the wobble follow the distance the source has to cover, so
    // a source at the edge of a large window does not look like a short hop.
    let reach = distance.max(max_distance).max(1.0);
    let swirl = (progress * std::f32::consts::TAU * 2.6 + index as f32).sin()
        * reach
        * SWIRL_FRACTION
        * (1.0 - progress);
    Travel {
        x: source.0 + (centre.0 - source.0) * progress,
        y: source.1 + (centre.1 - source.1) * progress + swirl,
        progress,
        trail: progress.powf(3.0) * reach * TRAIL_FRACTION,
        swirl,
    }
}

/// Where one artifact of the emitting core is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Emission {
    pub x: f32,
    pub y: f32,
    /// `0.0` while it is still inside the core, `1.0` at its place.
    pub progress: f32,
}

/// Position of the artifact at `t` of the emit phase.
///
/// `count` is the number of artifacts, so the last one still lands on its place
/// when the phase ends instead of stopping short of it.
pub fn emitted(
    centre: (f32, f32),
    slot: (f32, f32),
    index: usize,
    count: usize,
    t: f32,
) -> Emission {
    let last = count.saturating_sub(1) as f32;
    let window = (1.0 - EMIT_STAGGER * last).max(1.0e-3);
    let spread = ((phase(t) - EMIT_STAGGER * index as f32) / window).clamp(0.0, 1.0);
    let progress = 1.0 - (1.0 - spread).powf(EMIT_ACCELERATION);
    Emission {
        x: centre.0 + slot.0 * progress,
        y: centre.1 + slot.1 * progress,
        progress,
    }
}

/// Places for `count` artifacts around the core.
///
/// The artifacts fan out in two columns so the icons never overlap each other,
/// and they stay above and below the centre so the caption under the core stays
/// readable.
pub fn emission_slots(count: usize, radius_x: f32, radius_y: f32) -> Vec<(f32, f32)> {
    let rows = count.div_ceil(2).max(1);
    (0..count)
        .map(|index| {
            let column = if index % 2 == 0 { -1.0 } else { 1.0 };
            let row = (index / 2) as f32;
            let y = if rows == 1 {
                0.0
            } else {
                -radius_y + row * (2.0 * radius_y / (rows - 1) as f32)
            };
            (column * radius_x, y)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_absorb_grows_with_the_project_and_the_targets() {
        assert_eq!(
            absorb_seconds(0, 0),
            ABSORB_RANGE.0,
            "an empty project sits on the lower bound"
        );
        assert!(absorb_seconds(400, 1) > absorb_seconds(400, 0));
        assert!(absorb_seconds(600, 4) > absorb_seconds(300, 4));
        assert_eq!(absorb_seconds(100_000, 9), ABSORB_RANGE.1);
        assert!(absorb_seconds(0, 0) >= ABSORB_RANGE.0);
    }

    #[test]
    fn every_source_arrives_together_and_the_far_ones_move_faster() {
        let centre: (f32, f32) = (500.0, 400.0);
        let near: (f32, f32) = (520.0, 400.0);
        let far: (f32, f32) = (40.0, 60.0);
        let (far_dx, far_dy) = (far.0 - centre.0, far.1 - centre.1);
        let max_distance = (far_dx * far_dx + far_dy * far_dy).sqrt();

        let near_early = absorbed(near, centre, 0, 2, max_distance, 0.25);
        let far_early = absorbed(far, centre, 1, 2, max_distance, 0.25);
        assert!(
            far_early.progress > near_early.progress,
            "the further source should leave earlier: {} vs {}",
            far_early.progress,
            near_early.progress
        );
        // The far source also has to move faster: it covers its longer way in a
        // window of the same length.
        let near_distance = ((near.0 - centre.0).powi(2) + (near.1 - centre.1).powi(2)).sqrt();
        let (near_start, near_end) = absorb_window(near_distance, max_distance, 0, 2);
        let (far_start, far_end) = absorb_window(max_distance, max_distance, 1, 2);
        let near_speed = near_distance / (near_end - near_start);
        let far_speed = max_distance / (far_end - far_start);
        assert!(
            far_speed > near_speed,
            "the further source should move faster: {far_speed} vs {near_speed}"
        );

        for source in [near, far] {
            let arrived = absorbed(source, centre, 0, 2, max_distance, ABSORB_ARRIVAL);
            assert_eq!(arrived.progress, 1.0);
            assert_eq!((arrived.x, arrived.y), centre);
            assert!(arrived.trail >= max_distance * TRAIL_FRACTION - 0.001);
        }
        assert_eq!(
            absorbed(near, centre, 0, 2, max_distance, 2.0).progress,
            1.0,
            "a source never leaves the core again"
        );
    }

    #[test]
    fn a_stretched_trail_is_only_drawn_once_it_is_wide_enough() {
        let centre = (0.0, 0.0);
        let source = (600.0, 0.0);
        let start = absorbed(source, centre, 0, 1, 600.0, 0.0);
        assert_eq!((start.x, start.y), source);
        assert_eq!(start.trail, 0.0);
        assert!(
            start.trail < MIN_TRAIL,
            "a source that has not moved has nothing to smear"
        );
        let middle = absorbed(source, centre, 0, 1, 600.0, 0.8);
        assert!(middle.trail > MIN_TRAIL);
        assert!(middle.trail < 600.0 * TRAIL_FRACTION * 1.01);
        assert!(middle.x < source.0 && middle.x > centre.0);
        assert!(
            middle.swirl.abs() <= 600.0 * SWIRL_FRACTION,
            "the wobble stays bounded: {}",
            middle.swirl
        );
    }

    #[test]
    fn a_broken_phase_time_cannot_throw_a_source_away() {
        let centre = (10.0, 10.0);
        let source = (100.0, 100.0);
        for t in [f32::NAN, -5.0, 0.0, 1.0, 40.0] {
            let travel = absorbed(source, centre, 3, 9, 0.0, t);
            assert!(
                travel.x.is_finite() && travel.y.is_finite() && travel.progress.is_finite(),
                "t={t} produced {travel:?}"
            );
            assert!((0.0..=1.0).contains(&travel.progress));
        }
    }

    #[test]
    fn artifacts_fan_out_to_their_own_place() {
        let slots = emission_slots(4, 200.0, 90.0);
        assert_eq!(slots.len(), 4);
        assert_eq!(slots[0], (-200.0, -90.0));
        assert_eq!(slots[1], (200.0, -90.0));
        assert_eq!(slots[2], (-200.0, 90.0));
        assert_eq!(slots[3], (200.0, 90.0));
        // An odd count still spreads over both columns without overlapping.
        let three = emission_slots(3, 100.0, 40.0);
        assert_eq!(three.len(), 3);
        assert!(three[0].0 < 0.0 && three[1].0 > 0.0 && three[2].0 < 0.0);
        assert_eq!(emission_slots(1, 100.0, 40.0), vec![(-100.0, 0.0)]);

        let centre = (500.0, 400.0);
        let slot = slots[3];
        let inside = emitted(centre, slot, 3, 4, 0.0);
        assert_eq!((inside.x, inside.y), centre);
        assert_eq!(inside.progress, 0.0);
        let settled = emitted(centre, slot, 3, 4, 1.0);
        assert_eq!((settled.x, settled.y), (500.0 + slot.0, 400.0 + slot.1));
        assert_eq!(settled.progress, 1.0);

        // The second artifact lags the first one and both stay on their axis.
        let first = emitted(centre, slots[0], 0, 4, 0.2);
        let second = emitted(centre, slots[2], 2, 4, 0.2);
        assert!(
            first.progress > second.progress,
            "the stagger should keep the artifacts apart: {first:?} {second:?}"
        );
        assert!((first.y - centre.1).abs() <= slot.1.abs());
        assert!(first.x <= centre.0);
    }
}
