import { useEffect, useRef, useState } from "react";
import { claimButtons, BUTTON, moveFocus } from "@/lib/gamepad";
import { useHints } from "@/design-system/focus/FocusLayer";
import { commands } from "@/lib/ipc";
import { useRecentSearches } from "@/lib/recentSearches";
import { COLUMNS, PAGES, complete, completions, edit, type Text } from "./keyboardText";

/**
 * The TV keyboard — ADR-0041, as ADR-0043 locked it.
 *
 * A controller cannot type, so every text field gets this when the last input was a pad:
 * the Netflix-TV grid, docked on the left so the page (and search results, updating as
 * you type) stays in view on the right. Keyboard and mouse users never see it.
 *
 * - A types the focused key; the D-pad moves through the grid like anything else.
 * - X deletes, Y types a space, LT/RT move the cursor, LB/RB step through the pages
 *   (abc, ABC, symbols — API keys need case and punctuation), Menu is Done, B closes.
 * - On the search field: recent searches before typing, then title suggestions and word
 *   completions from the catalogue.
 *
 * Voice arrives with Phase 20 (ADR-0043); its key is not shown until it works.
 */

/** What a key does to the text at its end. Kept for callers that type without a cursor. */
export function applyKey(value: string, key: string): string {
  return edit({ value, cursor: value.length }, key).value;
}

/** Set a React-controlled input's value the way typing would, so `onChange` fires. */
function write(input: HTMLInputElement, t: Text) {
  const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")?.set;
  setter?.call(input, t.value);
  input.dispatchEvent(new Event("input", { bubbles: true }));
}

const isTextField = (el: EventTarget | null): el is HTMLInputElement =>
  el instanceof HTMLInputElement && ["text", "search", "password", "url", ""].includes(el.type);

const padMode = () => document.documentElement.dataset["input"] === "pad";

const SEARCH_FIELD = "Search the catalogue";
const isSearchField = (el: HTMLInputElement) => el.getAttribute("aria-label") === SEARCH_FIELD;

/** Set while the keyboard is open, so B can close it rather than leave the screen. */
let closeOpen: (() => void) | null = null;

/** Close the keyboard if it is open. Returns whether it was. */
export function closePadKeyboard(): boolean {
  if (!closeOpen) return false;
  closeOpen();
  return true;
}

interface Suggestion {
  id: number;
  title: string;
  year: number | null;
}

export function PadKeyboard() {
  const [target, setTarget] = useState<HTMLInputElement | null>(null);
  const [text, setText] = useState<Text>({ value: "", cursor: 0 });
  const [page, setPage] = useState(0);
  const [suggestions, setSuggestions] = useState<Suggestion[]>([]);
  const recent = useRecentSearches((s) => s.recent);
  const remember = useRecentSearches((s) => s.remember);
  const panel = useRef<HTMLDivElement>(null);
  // The field just closed from: focusing it again (on the way down to the results)
  // must not reopen the keyboard.
  const dismissed = useRef<HTMLInputElement | null>(null);
  const isSearch = target?.getAttribute("aria-label") === SEARCH_FIELD;

  // The search field opens the keyboard as soon as it is focused (Y goes straight to
  // typing, as on Netflix). Any other field opens it on A: walking a settings page with
  // the D-pad must not pop a keyboard at every field it passes.
  useEffect(() => {
    const open = (field: HTMLInputElement) => {
      setTarget(field);
      setText({ value: field.value, cursor: field.value.length });
      setPage(0);
    };
    const onFocus = (e: FocusEvent) => {
      if (panel.current?.contains(e.target as Node)) return;
      if (isTextField(e.target) && padMode() && isSearchField(e.target) && e.target !== dismissed.current) {
        open(e.target);
        return;
      }
      if (e.target !== dismissed.current) dismissed.current = null;
    };
    const onClick = (e: MouseEvent) => {
      if (panel.current?.contains(e.target as Node)) return;
      if (isTextField(e.target) && padMode()) open(e.target);
    };
    document.addEventListener("focusin", onFocus);
    document.addEventListener("click", onClick);
    return () => {
      document.removeEventListener("focusin", onFocus);
      document.removeEventListener("click", onClick);
    };
  }, []);

  // Apply an edit to the field.
  const apply = (next: Text) => {
    if (!target) return;
    setText(next);
    write(target, next);
  };
  const press = (key: string) => {
    if (!target) return;
    // The field may have changed under us (a cleared search); start from what it holds.
    const now = target.value === text.value ? text : { value: target.value, cursor: target.value.length };
    apply(edit(now, key));
  };

  useEffect(() => {
    if (!target) return;
    // Start on the first key, so the D-pad is already in the keyboard.
    panel.current?.querySelector<HTMLButtonElement>("[data-key]")?.focus();
    const unhint = useHints.getState().push([
      ["A", "Type"],
      ["X", "Delete"],
      ["Y", "Space"],
      ["RB", "abc · ABC · #+="],
      ["Menu", "Done"],
      ["B", "Close"],
    ]);
    closeOpen = () => {
      dismissed.current = target;
      setTarget(null);
      setSuggestions([]);
      // After the keyboard has gone, or "down" from the field can land back in it. From
      // search, down to the results; from any other field, stay on it.
      const search = isSearchField(target);
      requestAnimationFrame(() => {
        target.focus();
        if (search) moveFocus("down");
      });
    };
    return () => {
      closeOpen = null;
      unhint();
    };
  }, [target]);

  // The pad's text buttons, while the keyboard is open.
  useEffect(() => {
    if (!target) return;
    return claimButtons((b) => {
      switch (b) {
        case BUTTON.X:
          press("backspace");
          return true;
        case BUTTON.Y:
          press("space");
          return true;
        case BUTTON.LT:
          press("left");
          return true;
        case BUTTON.RT:
          press("right");
          return true;
        case BUTTON.LB:
          setPage((p) => (p + PAGES.length - 1) % PAGES.length);
          return true;
        case BUTTON.RB:
          setPage((p) => (p + 1) % PAGES.length);
          return true;
        case BUTTON.MENU:
          done();
          return true;
        default:
          return false;
      }
    });
  });

  // Title suggestions for the search field, as you type.
  useEffect(() => {
    if (!isSearch || text.value.trim().length < 2) {
      setSuggestions([]);
      return;
    }
    let live = true;
    const t = window.setTimeout(() => {
      void commands.search(text.value, 6).then((r) => {
        if (live && r.status === "ok") setSuggestions(r.data.hits.map((h) => ({ id: h.id, title: h.title, year: h.year })));
      });
    }, 150);
    return () => {
      live = false;
      window.clearTimeout(t);
    };
  }, [isSearch, text.value]);

  // A shorter page (symbols) can remove the focused key: keep focus in the grid.
  useEffect(() => {
    if (!target || panel.current?.contains(document.activeElement)) return;
    const keysNow = panel.current?.querySelectorAll<HTMLButtonElement>("[data-key]");
    keysNow?.[keysNow.length - 1]?.focus();
  }, [page, target]);

  if (!target) return null;

  function done() {
    if (isSearch) remember(text.value);
    closeOpen?.();
  }

  const keys = [...PAGES[page]!.keys];
  const words = isSearch ? completions(text, suggestions.map((s) => s.title)) : [];
  const keyClass = "h-11 rounded-sm border border-line bg-raised font-ui text-[17px] text-ink";

  return (
    <div
      ref={panel}
      role="group"
      aria-label="On-screen keyboard"
      className="fixed top-0 bottom-0 left-0 z-50 flex w-[400px] flex-col gap-4 overflow-y-auto border-r border-line bg-surface px-6 py-8"
    >
      <p className="label">Typing into {target.getAttribute("aria-label") ?? "the field"}</p>
      <p className="min-h-8 font-ui text-[22px] leading-8 break-all text-ink" aria-live="polite">
        {text.value.slice(0, text.cursor)}
        <span className="inline-block h-6 w-0.5 translate-y-1 bg-ink" aria-hidden />
        {text.value.slice(text.cursor)}
      </p>

      {isSearch && text.value === "" && recent.length > 0 && (
        <section aria-label="Recent searches" className="flex flex-col">
          <p className="label mb-1 text-ink-faint">Recent</p>
          {recent.map((r) => (
            <button key={r} type="button" onClick={() => apply({ value: r, cursor: r.length })} className="py-1 text-left text-[15px] text-ink-muted hover:text-ink">
              {r}
            </button>
          ))}
        </section>
      )}

      <div className="grid gap-1.5" style={{ gridTemplateColumns: `repeat(${COLUMNS}, minmax(0, 1fr))` }}>
        {/* Keyed by position, so switching pages keeps the focused key focused. */}
        {keys.map((k, i) => (
          <button key={i} type="button" data-key aria-label={k} onClick={() => press(k)} className={keyClass}>
            {k}
          </button>
        ))}
      </div>
      <div className="grid grid-cols-6 gap-1.5">
        <button type="button" onClick={() => press("space")} className={`${keyClass} col-span-3`}>
          Space
        </button>
        <button type="button" aria-label="Delete" onClick={() => press("backspace")} className={`${keyClass} col-span-1`}>
          ⌫
        </button>
        <button
          type="button"
          aria-label={`Keys: ${PAGES[(page + 1) % PAGES.length]!.name}`}
          onClick={() => setPage((p) => (p + 1) % PAGES.length)}
          className={`${keyClass} col-span-2 text-[14px]`}
        >
          {PAGES[(page + 1) % PAGES.length]!.name}
        </button>
      </div>
      <button type="button" onClick={done} className="h-11 rounded-sm border border-line-interactive bg-raised text-[15px] text-ink">
        Done
      </button>

      {words.length > 0 && (
        <section aria-label="Complete the word" className="flex flex-wrap gap-2">
          {words.map((w) => (
            <button key={w} type="button" onClick={() => apply(complete(text, w))} className="rounded-sm border border-line px-3 py-1.5 text-[14px] text-ink">
              {w}
            </button>
          ))}
        </section>
      )}
      {suggestions.length > 0 && (
        <section aria-label="Suggestions" className="flex flex-col">
          <p className="label mb-1 text-ink-faint">Suggestions</p>
          {suggestions.slice(0, 5).map((s) => (
            <button
              key={s.id}
              type="button"
              onClick={() => apply({ value: s.title, cursor: s.title.length })}
              className="flex items-baseline justify-between gap-3 py-1 text-left text-[15px] text-ink hover:text-ink"
            >
              <span className="truncate">{s.title}</span>
              <span className="tabular-nums text-[12px] text-ink-faint">{s.year ?? ""}</span>
            </button>
          ))}
        </section>
      )}
    </div>
  );
}
