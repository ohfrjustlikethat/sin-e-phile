import { useEffect, useRef, useState } from "react";
import { moveFocus } from "@/lib/gamepad";

/**
 * The on-screen keyboard for a controller — ADR-0041.
 *
 * A controller cannot type, so without this the search field is a dead end and "with
 * only an Xbox controller, browse to a film" (Phase 8 E5) has no path. It appears when
 * a text field takes focus while the last input was a pad, and only then: keyboard and
 * mouse users never see it.
 *
 * The keys are ordinary buttons, so the D-pad reaches them through the same spatial
 * navigation as everything else, and A presses them. Each key writes into the field it
 * was opened for. Done (or B) closes it and moves focus down from the field — to the
 * results, which is where someone who has just typed wants to go.
 */

const ROWS = ["abcdefghi", "jklmnopqr", "stuvwxyz'", "1234567890"];

/** What a key does to the text. Pure, so it is tested without a DOM. */
export function applyKey(value: string, key: string): string {
  if (key === "backspace") return value.slice(0, -1);
  if (key === "space") return value.endsWith(" ") || value === "" ? value : `${value} `;
  return value + key;
}

/** Set a React-controlled input's value the way typing would, so `onChange` fires. */
function write(input: HTMLInputElement, value: string) {
  const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")?.set;
  setter?.call(input, value);
  input.dispatchEvent(new Event("input", { bubbles: true }));
}

const isTextField = (el: EventTarget | null): el is HTMLInputElement =>
  el instanceof HTMLInputElement && ["text", "search", ""].includes(el.type);

const padMode = () => document.documentElement.dataset["input"] === "pad";

/** Set while the keyboard is open, so B can close it rather than leave the screen. */
let closeOpen: (() => void) | null = null;

/** Close the keyboard if it is open. Returns whether it was. */
export function closePadKeyboard(): boolean {
  if (!closeOpen) return false;
  closeOpen();
  return true;
}

export function PadKeyboard() {
  const [target, setTarget] = useState<HTMLInputElement | null>(null);
  const panel = useRef<HTMLDivElement>(null);
  // The field just closed from: focusing it again (on the way down to the results)
  // must not reopen the keyboard.
  const dismissed = useRef<HTMLInputElement | null>(null);

  useEffect(() => {
    const onFocus = (e: FocusEvent) => {
      if (panel.current?.contains(e.target as Node)) return;
      if (isTextField(e.target) && padMode() && e.target !== dismissed.current) {
        setTarget(e.target);
        return;
      }
      if (e.target !== dismissed.current) dismissed.current = null;
    };
    document.addEventListener("focusin", onFocus);
    return () => document.removeEventListener("focusin", onFocus);
  }, []);

  useEffect(() => {
    if (!target) return;
    // Start on the first key, so the D-pad is already in the keyboard.
    panel.current?.querySelector<HTMLButtonElement>("button")?.focus();
    closeOpen = () => {
      dismissed.current = target;
      setTarget(null);
      target.focus();
      moveFocus("down");
    };
    return () => {
      closeOpen = null;
    };
  }, [target]);

  if (!target) return null;

  const press = (key: string) => write(target, applyKey(target.value, key));
  const key = (k: string, label = k) => (
    <button
      key={k}
      type="button"
      onClick={() => press(k)}
      className="h-12 min-w-12 border border-line bg-raised px-3 font-ui text-lg text-ink"
    >
      {label}
    </button>
  );

  return (
    <div
      ref={panel}
      role="group"
      aria-label="On-screen keyboard"
      className="fixed inset-x-0 bottom-0 z-50 flex flex-col items-center gap-2 border-t border-line bg-surface px-4 py-5"
    >
      <p className="label mb-1">Typing into {target.getAttribute("aria-label") ?? "the field"}</p>
      {ROWS.map((row) => (
        <div key={row} className="flex gap-2">
          {[...row].map((k) => key(k, k.toUpperCase()))}
        </div>
      ))}
      <div className="flex gap-2">
        {key("space", "Space")}
        {key("backspace", "Delete")}
        <button
          type="button"
          onClick={() => closeOpen?.()}
          className="h-12 border border-line-interactive bg-raised px-5 font-ui text-lg text-ink"
        >
          Done
        </button>
      </div>
    </div>
  );
}
