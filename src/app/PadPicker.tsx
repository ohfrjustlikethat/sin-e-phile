import { useCallback, useEffect, useRef, useState } from "react";
import { setControlValue } from "@/lib/gamepad";

/**
 * A select's options as a list the pad can walk (Phase 16, ADR-0041).
 *
 * The app's selects are native `<select>`s, which keyboard and mouse handle well. A pad
 * cannot: A clicks the element, and a click from a script does not open its list, so the
 * VPN adapter, the subtitle font and how many downloads run at once could not be changed
 * without a keyboard. With a pad, A on a select opens this instead, as a TV's settings
 * open a list: focus on the current choice, the D-pad moves (the dialog holds it), A
 * chooses, B leaves without changing anything. The select is then set exactly as a person
 * choosing would set it, so its own `onChange` runs, and focus goes back to it.
 */

let open: ((select: HTMLSelectElement) => void) | null = null;
let close: (() => boolean) | null = null;

/** Show `select`'s options in the pad's list. Returns false when no list is mounted. */
export function openPadPicker(select: HTMLSelectElement): boolean {
  if (!open) return false;
  open(select);
  return true;
}

/** Close the list if it is open, choosing nothing. Returns whether it was. */
export function closePadPicker(): boolean {
  return close?.() ?? false;
}

/** What a select is called: its label, else its own `aria-label`. */
function nameOf(select: HTMLSelectElement): string {
  return select.labels?.[0]?.textContent?.trim() || select.getAttribute("aria-label") || "Choose";
}

export function PadPicker() {
  const [select, setSelect] = useState<HTMLSelectElement | null>(null);
  const list = useRef<HTMLDivElement>(null);

  const finish = useCallback(
    (chosen?: string) => {
      if (!select) return;
      if (chosen !== undefined && chosen !== select.value) setControlValue(select, chosen);
      setSelect(null);
      select.focus({ preventScroll: true });
    },
    [select],
  );

  useEffect(() => {
    open = setSelect;
    return () => {
      open = null;
    };
  }, []);

  useEffect(() => {
    close = () => {
      if (!select) return false;
      finish();
      return true;
    };
    if (!select) return;
    list.current?.querySelector<HTMLElement>('[aria-checked="true"]:not([disabled])')?.focus({ preventScroll: true });
    // Escape from a keyboard leaves the list, and goes no further: the screen beneath (a
    // first-run step, Settings) must not take it as its own "back".
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      e.preventDefault();
      e.stopImmediatePropagation();
      finish();
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [select, finish]);

  if (!select) return null;
  const name = nameOf(select);
  return (
    <div className="fixed inset-0 z-[60] grid place-items-center" style={{ background: "var(--scrim)" }}>
      <div role="dialog" aria-modal="true" aria-label={name} className="w-full max-w-sm rounded-sm border border-line bg-overlay">
        <header className="border-b border-line-subtle px-6 py-4">
          <h2 className="label text-ink">{name}</h2>
        </header>
        <div ref={list} role="radiogroup" aria-label={name} className="flex max-h-[60vh] flex-col overflow-y-auto py-2">
          {[...select.options].map((o) => {
            const current = o.value === select.value;
            return (
              <button
                key={o.value}
                type="button"
                role="radio"
                aria-checked={current}
                disabled={o.disabled}
                onClick={() => finish(o.value)}
                className={[
                  "flex items-baseline justify-between gap-4 px-6 py-2.5 text-left text-[15px]",
                  current ? "text-ink" : "text-ink-muted hover:text-ink",
                  "disabled:opacity-45",
                ].join(" ")}
              >
                <span>{o.label || o.text}</span>
                {current && <span className="label text-ink-faint">Chosen</span>}
              </button>
            );
          })}
        </div>
      </div>
    </div>
  );
}
