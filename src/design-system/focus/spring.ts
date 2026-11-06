/**
 * A critically damped spring — ADR-0042's "alive, never bouncing".
 *
 * Damping ratio 1 is the fastest a spring can reach its target without overshooting it.
 * The exact solution is used rather than a numerical step, so a slow frame can never
 * make it overshoot either: x(t) = (x0 + (v0 + ωx0)·t)·e^(−ωt), relative to the target.
 */

export interface Spring {
  x: number;
  v: number;
}

/** ω chosen so a jump is within 1% of its target after ~200 ms: (1 + ωt)e^(−ωt) = 0.01 at ωt ≈ 6.64. */
export const OMEGA = 33;

/** Advance `s` toward `target` by `dt` seconds. */
export function step(s: Spring, target: number, dt: number, omega = OMEGA): Spring {
  const x0 = s.x - target;
  const k = s.v + omega * x0;
  const e = Math.exp(-omega * dt);
  return { x: target + (x0 + k * dt) * e, v: (s.v - omega * k * dt) * e };
}
