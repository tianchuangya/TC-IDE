/**
 * The only three kinds of motion the interface is allowed to use.
 *
 * Values are the ones the handoff fixes and the ones the native backend uses
 * in `src/motion_core.rs`, so the two products feel the same. A new curve for
 * a new control is exactly what the handoff forbids, so this module exposes
 * roles rather than raw constants.
 */

export type MotionRole = 'overlay' | 'drawer' | 'follow';

interface SpringProfile {
  stiffness: number;
  damping: number;
  overshoot: number;
}

export const profiles: Record<MotionRole, SpringProfile> = {
  overlay: { stiffness: 300, damping: 19, overshoot: 0.04 },
  drawer: { stiffness: 235, damping: 17, overshoot: 0.08 },
  follow: { stiffness: 620, damping: 34, overshoot: 0 },
};

/** Distance a role starts from before arriving, in logical pixels. */
export const travel: Record<MotionRole, number> = {
  overlay: 12,
  drawer: 24,
  follow: 0,
};

/** Below this distance and speed a spring is put straight on its target. */
export const SETTLE_DISTANCE = 0.0008;
export const SETTLE_SPEED = 0.0008;

/** Largest step the integrator accepts, so a long frame cannot explode it. */
export const MAX_DT = 0.1;

export interface SpringState {
  value: number;
  velocity: number;
}

export function newSpring(value = 0): SpringState {
  return { value, velocity: 0 };
}

/**
 * Advance a spring by one frame with a semi implicit integrator.
 *
 * The order matters: velocity is updated from the current value, damped, and
 * only then applied. Doing it the other way round is what makes a stiff
 * spring oscillate or blow up.
 */
export function stepSpring(spring: SpringState, target: number, dt: number, role: MotionRole): void {
  const profile = profiles[role];
  const clamped = Number.isFinite(dt) ? Math.min(Math.max(dt, 0), MAX_DT) : 0;
  if (clamped === 0) {
    return;
  }
  spring.velocity += (target - spring.value) * profile.stiffness * clamped;
  spring.velocity *= Math.exp(-profile.damping * clamped);
  spring.value += spring.velocity * clamped;
  if (Math.abs(target - spring.value) < SETTLE_DISTANCE && Math.abs(spring.velocity) < SETTLE_SPEED) {
    spring.value = target;
    spring.velocity = 0;
  }
}

// The three roles get their own bounds so the header region stays valid, and
// so a later edit stops at a limit rather than producing an out of range value.

/**
 * Opacity of something arriving at `progress`.
 *
 * The alpha comes up faster than the travel finishes, so nothing is drawn
 * faintly for long; the floor keeps an arriving element visible instead of
 * invisible for its first frames.
 */
export function arrivalOpacity(progress: number): number {
  const safe = Number.isFinite(progress) ? Math.min(Math.max(progress, 0), 1) : 1;
  return Math.min(Math.max(0.25 + 0.75 * safe, 0), 1);
}

/** Remaining travel for a role at `progress`. */
export function arrivalOffset(role: MotionRole, progress: number): number {
  const safe = Number.isFinite(progress) ? Math.min(Math.max(progress, 0), 1) : 1;
  return travel[role] * (1 - safe);
}

/** How long a reduced motion fade takes, per the handoff. */
export const REDUCED_MOTION_MS = 80;

/**
 * A running animation, so a caller does not have to keep the bookkeeping.
 *
 * Only position-like values are animated; animating layout sizes each frame is
 * what the handoff forbids because it forces a reflow.
 */
export class Animated {
  private readonly springs = new Map<string, SpringState>();

  constructor(private readonly role: MotionRole) {}

  state(key: string): SpringState {
    let spring = this.springs.get(key);
    if (!spring) {
      spring = newSpring(0);
      this.springs.set(key, spring);
    }
    return spring;
  }

  /** Step every tracked value towards its target. Returns whether any moved. */
  step(targets: Record<string, number>, dt: number, role: MotionRole = this.role): boolean {
    let moving = false;
    for (const [key, target] of Object.entries(targets)) {
      const spring = this.state(key);
      const before = spring.value;
      stepSpring(spring, target, dt, role);
      if (spring.value !== before) {
        moving = true;
      }
    }
    return moving;
  }

  /** Forget values that are no longer drawn, so the map cannot grow forever. */
  retain(keys: readonly string[]): void {
    const keep = new Set(keys);
    for (const key of [...this.springs.keys()]) {
      if (!keep.has(key)) {
        this.springs.delete(key);
      }
    }
  }
}
