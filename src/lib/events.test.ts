import { describe, expect, it, vi } from "vitest";

const listen = vi.fn();
vi.mock("@tauri-apps/api/event", () => ({ listen }));
const { onEvent } = await import("./events");

describe("onEvent", () => {
  it("is nothing to listen to outside the shell, not a crash", () => {
    // What @tauri-apps/api does in a plain browser: it throws before returning a promise.
    listen.mockImplementationOnce(() => {
      throw new TypeError("Cannot read properties of undefined (reading 'transformCallback')");
    });
    const stop = onEvent("downloads-changed", () => {});
    expect(() => stop()).not.toThrow();
  });

  it("passes the payload on, and stops even if stopped before it started", async () => {
    let handler: (e: { payload: unknown }) => void = () => {};
    const off = vi.fn();
    listen.mockImplementationOnce((_name: string, h: typeof handler) => {
      handler = h;
      return Promise.resolve(off);
    });
    const got: unknown[] = [];
    const stop = onEvent<string>("download-finished", (t) => got.push(t));
    stop();
    await Promise.resolve();
    await Promise.resolve();
    expect(off).toHaveBeenCalled();
    handler({ payload: "Sunrise" });
    expect(got).toEqual(["Sunrise"]);
  });
});
