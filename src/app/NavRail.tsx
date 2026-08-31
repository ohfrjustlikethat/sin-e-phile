import { DESTINATIONS, type Destination, useUi } from "@/lib/store";

const LABELS: Record<Destination, string> = {
  home: "Home",
  films: "Films",
  tv: "TV Shows",
  watchlist: "Watchlist",
  live: "Live Channels",
};

/** Placeholder glyphs. Phase 2 replaces these with the real icon set. */
const ICONS: Record<Destination, React.ReactNode> = {
  home: <path d="M3 9.5L10 4l7 5.5V16a1 1 0 01-1 1h-4v-4H8v4H4a1 1 0 01-1-1V9.5z" />,
  films: <path d="M3 4h14v12H3V4zm0 3h14M3 13h14M7 4v12M13 4v12" />,
  tv: <path d="M3 6h14v9H3V6zm4-3l3 3 3-3" />,
  watchlist: <path d="M5 3h10v14l-5-3.5L5 17V3z" />,
  live: <path d="M10 13a3 3 0 100-6 3 3 0 000 6zM4.5 4.5a9 9 0 000 11M15.5 4.5a9 9 0 010 11" />,
};

/**
 * Left navigation rail (SPEC.md Phase 1, subtask 1.6; §9.4).
 *
 * 72px collapsed, 240px expanded, and it remembers which. All five destinations
 * exist from Phase 1 so the navigation shape never changes under the user, even
 * though most are placeholders until their phase.
 */
export function NavRail() {
  const { destination, railExpanded, setDestination, toggleRail, setSettingsOpen } = useUi();

  return (
    <nav
      aria-label="Primary"
      style={{ width: railExpanded ? "var(--rail-expanded)" : "var(--rail-collapsed)" }}
      className="flex shrink-0 flex-col border-r border-line-subtle bg-void transition-[width] duration-200 ease-[var(--ease-standard)]"
    >
      <ul className="flex flex-1 flex-col gap-1 p-3">
        {DESTINATIONS.map((d) => {
          const active = destination === d;
          return (
            <li key={d}>
              <button
                type="button"
                onClick={() => setDestination(d)}
                aria-current={active ? "page" : undefined}
                title={railExpanded ? undefined : LABELS[d]}
                className={[
                  "group relative flex h-11 w-full items-center gap-3 rounded-lg px-3 text-left text-sm transition-colors",
                  active
                    ? "bg-[var(--oxblood-wash)] text-ink"
                    : "text-ink-muted hover:bg-raised hover:text-ink",
                ].join(" ")}
              >
                {/* §9.1: oxblood marks intent — here, the active destination. */}
                {active && (
                  <span
                    aria-hidden
                    className="absolute left-0 top-1/2 h-5 w-[3px] -translate-y-1/2 rounded-r bg-oxblood-bright"
                  />
                )}
                <svg
                  width="20" height="20" viewBox="0 0 20 20" fill="none"
                  stroke="currentColor" strokeWidth="1.5" strokeLinecap="round"
                  strokeLinejoin="round" className="shrink-0"
                  aria-hidden
                >
                  {ICONS[d]}
                </svg>
                {railExpanded && <span className="truncate">{LABELS[d]}</span>}
              </button>
            </li>
          );
        })}
      </ul>

      <div className="flex flex-col gap-1 border-t border-line-subtle p-3">
        <RailButton
          expanded={railExpanded}
          label="Settings"
          onClick={() => setSettingsOpen(true)}
        >
          <path d="M10 12.5a2.5 2.5 0 100-5 2.5 2.5 0 000 5z" />
          <path d="M16 10a6 6 0 00-.1-1l1.6-1.2-1.5-2.6-1.9.7A6 6 0 0012.4 4L12 2H8l-.4 2a6 6 0 00-1.7 1l-1.9-.8-1.5 2.6L4.1 9a6 6 0 000 2l-1.6 1.2 1.5 2.6 1.9-.7a6 6 0 001.7 1L8 18h4l.4-2a6 6 0 001.7-1l1.9.7 1.5-2.6L15.9 11c.1-.3.1-.7.1-1z" />
        </RailButton>
        <RailButton
          expanded={railExpanded}
          label={railExpanded ? "Collapse" : "Expand"}
          onClick={toggleRail}
        >
          {railExpanded ? <path d="M12 5l-5 5 5 5" /> : <path d="M8 5l5 5-5 5" />}
        </RailButton>
      </div>
    </nav>
  );
}

function RailButton({
  expanded, label, onClick, children,
}: {
  expanded: boolean;
  label: string;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      title={expanded ? undefined : label}
      aria-label={label}
      className="flex h-10 w-full items-center gap-3 rounded-lg px-3 text-left text-sm text-ink-muted transition-colors hover:bg-raised hover:text-ink"
    >
      <svg
        width="20" height="20" viewBox="0 0 20 20" fill="none" stroke="currentColor"
        strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"
        className="shrink-0" aria-hidden
      >
        {children}
      </svg>
      {expanded && <span className="truncate">{label}</span>}
    </button>
  );
}
