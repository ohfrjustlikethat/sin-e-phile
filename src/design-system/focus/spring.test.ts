import { describe, expect, it } from "vitest";
import { step, type Spring } from "./spring";

/** ADR-0042: the ring glides, settles in about 200 ms, and never overshoots. */

function run(from: number, to: number, ms: number, frame = 1000 / 60): number[] {
  let s: Spring = { x: from, v: 0 };
  const xs: number[] = [];
  for (let t = 0; t < ms; t += frame) {
    s = step(s, to, frame / 1000);
    xs.push(s.x);
  }
  return xs;
}

describe("critically damped spring", () => {
  it("settles within 1% in about 200 ms", () => {
    const xs = run(0, 100, 220);
    expect(Math.abs(100 - xs.at(-1)!)).toBeLessThan(1);
  });

  it("never overshoots and never turns back", () => {
    const xs = run(0, 100, 600);
    expect(Math.max(...xs)).toBeLessThanOrEqual(100);
    xs.slice(1).forEach((x, i) => expect(x).toBeGreaterThanOrEqual(xs[i]!));
  });

  it("is still moving partway through, so the eye sees a glide", () => {
    const xs = run(0, 100, 60);
    expect(xs.at(-1)!).toBeGreaterThan(20);
    expect(xs.at(-1)!).toBeLessThan(90);
  });

  it("a slow frame does not make it overshoot", () => {
    expect(step({ x: 0, v: 0 }, 100, 0.5).x).toBeLessThanOrEqual(100);
  });
});
