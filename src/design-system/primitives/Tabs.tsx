interface Tab {
  id: string;
  label: string;
}

/**
 * Tabs.
 *
 * Underlined with a 2px oxblood rule rather than a filled pill: §9.1 keeps
 * oxblood off large fills, and a rule is also the catalogue-ish move (ADR-0024).
 *
 * Arrow keys move between tabs, which is what the WAI-ARIA tabs pattern expects
 * and what a keyboard user will try.
 */
export function Tabs({
  tabs, active, onChange,
}: {
  tabs: Tab[];
  active: string;
  onChange: (id: string) => void;
}) {
  const move = (dir: 1 | -1) => {
    const i = tabs.findIndex((t) => t.id === active);
    const next = tabs[(i + dir + tabs.length) % tabs.length];
    if (next) onChange(next.id);
  };

  return (
    <div
      role="tablist"
      className="flex gap-1 border-b border-line-subtle"
      onKeyDown={(e) => {
        if (e.key === "ArrowRight") { e.preventDefault(); move(1); }
        if (e.key === "ArrowLeft") { e.preventDefault(); move(-1); }
      }}
    >
      {tabs.map((t) => {
        const on = t.id === active;
        return (
          <button
            key={t.id}
            role="tab"
            aria-selected={on}
            tabIndex={on ? 0 : -1}
            onClick={() => onChange(t.id)}
            className={[
              "relative px-4 py-2.5 font-ui text-[11.5px] font-semibold uppercase tracking-[0.12em]",
              "transition-colors duration-[var(--dur-standard)]",
              on ? "text-ink" : "text-ink-faint hover:text-ink-muted",
            ].join(" ")}
          >
            {t.label}
            {on && <span className="absolute inset-x-0 -bottom-px h-0.5 bg-oxblood-bright" />}
          </button>
        );
      })}
    </div>
  );
}
