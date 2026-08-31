import { useEffect, useMemo, useRef, useState } from "react";

/**
 * Command palette — SPEC.md §3.1: "`Ctrl+K` opens a command palette that searches
 * media, actions, and settings."
 *
 * **This is the shell, not the search.** Phase 5 supplies real semantic results;
 * the point of building it now is that the keyboard infrastructure — the global
 * shortcut, focus capture, focus restoration, arrow navigation, the ARIA combobox
 * wiring — is the part that gets retrofitted badly if it is left until later, and
 * §9.4 makes full keyboard navigation a Phase 2 requirement rather than a Phase 27
 * job.
 *
 * Filtering here is a plain substring match over whatever commands it is given.
 * That is deliberately dumb: replacing it with the Phase 5 engine should be a
 * change to one function, not a rewrite of the component.
 */

export interface Command {
  id: string;
  label: string;
  /** "Action", "Settings", "Film" — grouped under this in the list. */
  group: string;
  /** Shown right-aligned, e.g. "Ctrl+," or a film's year. */
  hint?: string;
  run: () => void;
}

export function CommandPalette({
  commands,
  open,
  onOpenChange,
}: {
  commands: Command[];
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const [query, setQuery] = useState("");
  const [index, setIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);
  const restoreTo = useRef<HTMLElement | null>(null);

  // Ctrl+K from anywhere. Registered on the document because the palette must
  // open regardless of what currently has focus.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        onOpenChange(!open);
      }
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [open, onOpenChange]);

  useEffect(() => {
    if (open) {
      restoreTo.current = document.activeElement as HTMLElement | null;
      setQuery("");
      setIndex(0);
      // After paint, or the input is not yet in the document to focus.
      requestAnimationFrame(() => inputRef.current?.focus());
    } else {
      restoreTo.current?.focus();
    }
  }, [open]);

  const results = useMemo(() => {
    const q = query.trim().toLowerCase();
    const matched = q
      ? commands.filter(
          (c) => c.label.toLowerCase().includes(q) || c.group.toLowerCase().includes(q),
        )
      : commands;
    // Group while preserving the order commands were given in, so the list does
    // not reshuffle as the user types.
    const groups = new Map<string, Command[]>();
    for (const c of matched) {
      const list = groups.get(c.group) ?? [];
      list.push(c);
      groups.set(c.group, list);
    }
    return { flat: matched, groups: [...groups.entries()] };
  }, [commands, query]);

  useEffect(() => {
    if (index >= results.flat.length) setIndex(0);
  }, [results.flat.length, index]);

  // Keep the highlighted row in view when arrowing past the fold.
  useEffect(() => {
    listRef.current
      ?.querySelector<HTMLElement>('[data-active="true"]')
      ?.scrollIntoView({ block: "nearest" });
  }, [index]);

  if (!open) return null;

  const activeId = results.flat[index]?.id;

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Escape") {
      e.preventDefault();
      onOpenChange(false);
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      setIndex((i) => (i + 1) % Math.max(1, results.flat.length));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setIndex((i) => (i - 1 + results.flat.length) % Math.max(1, results.flat.length));
    } else if (e.key === "Enter") {
      e.preventDefault();
      const chosen = results.flat[index];
      if (chosen) {
        onOpenChange(false);
        chosen.run();
      }
    }
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-start justify-center pt-[14vh]"
      style={{ background: "var(--scrim)" }}
      onMouseDown={(e) => e.target === e.currentTarget && onOpenChange(false)}
    >
      <div
        className="w-full max-w-[560px] overflow-hidden rounded-sm border border-line bg-overlay"
        onKeyDown={onKeyDown}
      >
        <div className="flex items-center gap-3 border-b border-line-subtle px-4">
          <span className="spine shrink-0">⌘K</span>
          <input
            ref={inputRef}
            value={query}
            onChange={(e) => {
              setQuery(e.target.value);
              setIndex(0);
            }}
            placeholder="Search films, actions, settings"
            role="combobox"
            aria-expanded
            aria-controls="cmdk-list"
            aria-activedescendant={activeId ? `cmdk-${activeId}` : undefined}
            className="h-12 w-full bg-transparent font-ui text-[14px] text-ink outline-none placeholder:text-ink-faint"
          />
        </div>

        <div
          id="cmdk-list"
          ref={listRef}
          role="listbox"
          aria-label="Commands"
          className="max-h-[340px] overflow-y-auto py-1.5"
        >
          {results.flat.length === 0 && (
            <p className="px-4 py-6 text-center text-[12.5px] text-ink-faint">
              Nothing matches “{query}”.
            </p>
          )}

          {results.groups.map(([group, items]) => (
            <div key={group}>
              <div className="label px-4 pb-1 pt-3">{group}</div>
              {items.map((c) => {
                const i = results.flat.indexOf(c);
                const active = i === index;
                return (
                  <div
                    key={c.id}
                    id={`cmdk-${c.id}`}
                    role="option"
                    aria-selected={active}
                    data-active={active}
                    onMouseEnter={() => setIndex(i)}
                    onMouseDown={(e) => {
                      e.preventDefault();
                      onOpenChange(false);
                      c.run();
                    }}
                    className={[
                      "flex cursor-pointer items-center justify-between px-4 py-2 font-ui text-[13px]",
                      active ? "bg-raised text-ink" : "text-ink-muted",
                    ].join(" ")}
                  >
                    <span className="flex items-center gap-2.5">
                      {active && <span className="h-3 w-0.5 bg-oxblood-bright" />}
                      <span className={active ? "" : "ml-[10px]"}>{c.label}</span>
                    </span>
                    {c.hint && <span className="font-mono text-[10.5px] text-ink-faint">{c.hint}</span>}
                  </div>
                );
              })}
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
