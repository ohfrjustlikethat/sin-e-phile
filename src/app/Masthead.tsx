import { useEffect, useState } from "react";
import { DESTINATIONS, type Destination, useUi } from "@/lib/store";

/**
 * The masthead — SPEC.md §3.1 and §9.4, ADR-0037.
 *
 * ONE bar is both the window's title bar and the app's navigation. It replaced a left
 * rail plus a separate title strip, which together gave the app the silhouette of an
 * admin dashboard: MUBI and the Criterion Channel, the two references §9.0 names
 * first, both navigate from a single horizontal masthead and give the width to the
 * work.
 *
 * - The destinations are WORDS, not icons. An icon row is the dashboard tell.
 * - Search is a labelled button showing its key. It used to be reachable only by `/`,
 *   `Ctrl+F` and the palette, none of which appear on screen.
 * - Oxblood marks the active destination and nothing else here (§9.1: intent only).
 * - Empty space is the drag region, so the bar still moves the window.
 *
 * Downloads and the profile avatar join the right-hand group in Phases 13 and 14. A
 * button that opens nothing would be a placeholder, so they are not here yet.
 */

export const LABELS: Record<Destination, string> = {
  home: "Home",
  films: "Films",
  tv: "TV Shows",
  watchlist: "Watchlist",
  live: "Live",
};

/** True only inside the Tauri shell. See the note on `useWindowControls`. */
function inTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export function Masthead({
  searching,
  onSearchChange,
}: {
  searching: boolean;
  onSearchChange: (open: boolean) => void;
}) {
  const { destination, settingsOpen, setDestination, setSettingsOpen } = useUi();
  const controls = useWindowControls();

  const go = (d: Destination) => {
    setSettingsOpen(false);
    onSearchChange(false);
    setDestination(d);
  };

  return (
    <header
      data-tauri-drag-region
      className="flex h-14 shrink-0 select-none items-stretch border-b border-line-subtle bg-void"
    >
      <button
        type="button"
        onClick={() => go("home")}
        aria-label="sin·e·phile — Home"
        className="flex items-center gap-2.5 pr-8 pl-6"
      >
        <span className="h-3 w-3 rounded-sm bg-oxblood" aria-hidden />
        <span className="font-display text-[17px] font-extrabold tracking-[-0.035em] text-ink">
          sin·e·phile
        </span>
      </button>

      <nav aria-label="Primary" className="flex">
        <ul className="flex">
          {DESTINATIONS.map((d) => {
            // Nothing is active while search or settings is showing: a tab claiming
            // "Films" behind a page of search results says you are somewhere you are not.
            const active = !searching && !settingsOpen && destination === d;
            return (
              <li key={d} className="flex">
                <button
                  type="button"
                  onClick={() => go(d)}
                  aria-current={active ? "page" : undefined}
                  className={`relative px-4 text-[13px] transition-colors duration-[var(--dur-standard)] ${
                    active ? "text-ink" : "text-ink-muted hover:text-ink"
                  }`}
                >
                  {LABELS[d]}
                  {/* Sits ON the bar's bottom rule, so the active tab reads as part of
                      the page below it. */}
                  <span
                    aria-hidden
                    className={`absolute inset-x-4 -bottom-px h-0.5 bg-oxblood-bright transition-opacity duration-[var(--dur-standard)] ${
                      active ? "opacity-100" : "opacity-0"
                    }`}
                  />
                </button>
              </li>
            );
          })}
        </ul>
      </nav>

      <div data-tauri-drag-region className="flex-1" />

      <div className="flex items-center gap-2 pr-3">
        <button
          type="button"
          onClick={() => {
            setSettingsOpen(false);
            onSearchChange(true);
          }}
          aria-pressed={searching}
          className={`flex h-8 items-center gap-2.5 rounded-sm border px-3 text-[13px] transition-colors duration-[var(--dur-standard)] ${
            searching
              ? "border-oxblood-bright text-ink"
              : "border-line text-ink-muted hover:border-line-strong hover:text-ink"
          }`}
        >
          <svg width="14" height="14" viewBox="0 0 14 14" fill="none" aria-hidden>
            <circle cx="6" cy="6" r="4.5" stroke="currentColor" strokeWidth="1.3" />
            <path d="M9.5 9.5L13 13" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" />
          </svg>
          Search
          <kbd className="rounded-sm border border-line px-1.5 font-mono text-[11px] leading-[18px] text-ink-faint">
            /
          </kbd>
        </button>

        <button
          type="button"
          onClick={() => {
            onSearchChange(false);
            setSettingsOpen(true);
          }}
          aria-label="Settings"
          aria-pressed={settingsOpen}
          className={`grid h-8 w-8 place-items-center transition-colors duration-[var(--dur-standard)] ${
            settingsOpen ? "text-ink" : "text-ink-muted hover:text-ink"
          }`}
        >
          <svg
            width="18" height="18" viewBox="0 0 20 20" fill="none" stroke="currentColor"
            strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round" aria-hidden
          >
            <path d="M10 12.5a2.5 2.5 0 100-5 2.5 2.5 0 000 5z" />
            <path d="M16 10a6 6 0 00-.1-1l1.6-1.2-1.5-2.6-1.9.7A6 6 0 0012.4 4L12 2H8l-.4 2a6 6 0 00-1.7 1l-1.9-.8-1.5 2.6L4.1 9a6 6 0 000 2l-1.6 1.2 1.5 2.6 1.9-.7a6 6 0 001.7 1L8 18h4l.4-2a6 6 0 001.7-1l1.9.7 1.5-2.6L15.9 11c.1-.3.1-.7.1-1z" />
          </svg>
        </button>
      </div>

      <div className="flex border-l border-line-subtle">
        <WindowButton label="Minimise" onClick={controls.minimize} disabled={!controls.shell}>
          <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden>
            <path d="M0 5h10" stroke="currentColor" strokeWidth="1" />
          </svg>
        </WindowButton>
        <WindowButton
          label={controls.maximized ? "Restore" : "Maximise"}
          onClick={controls.toggleMaximize}
          disabled={!controls.shell}
        >
          {controls.maximized ? (
            <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden>
              <path
                d="M2.5 2.5h5v5h-5z M0.5 0.5h7v1 M8.5 1.5v6h-6"
                fill="none" stroke="currentColor" strokeWidth="1"
              />
            </svg>
          ) : (
            <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden>
              <rect x="0.5" y="0.5" width="9" height="9" fill="none" stroke="currentColor" strokeWidth="1" />
            </svg>
          )}
        </WindowButton>
        <WindowButton label="Close" danger onClick={controls.close} disabled={!controls.shell}>
          <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden>
            <path d="M0 0l10 10M10 0L0 10" stroke="currentColor" strokeWidth="1" />
          </svg>
        </WindowButton>
      </div>
    </header>
  );
}

/**
 * Minimise, maximise and close, wired only when the Tauri shell is present.
 *
 * WHY THE TAURI API IS LOADED LAZILY. `@tauri-apps/api` reads
 * `window.__TAURI_INTERNALS__.metadata` when a window handle is requested, and that
 * object does not exist outside the shell. Calling it at render time threw, the error
 * boundary caught it, and the whole app showed a crash screen in any plain browser,
 * including the headless one `tools/uiaudit` drives. So the controls are wired only
 * when the shell is present, and the bar still renders, inert, when it is not.
 */
function useWindowControls() {
  const [maximized, setMaximized] = useState(false);
  const [shell, setShell] = useState(inTauri());

  useEffect(() => {
    if (!shell) return;
    let cancelled = false;
    let cleanup: (() => void) | undefined;

    void import("@tauri-apps/api/window")
      .then(({ getCurrentWindow }) => {
        const win = getCurrentWindow();
        void win.isMaximized().then((m) => !cancelled && setMaximized(m));
        return win.onResized(() => {
          void win.isMaximized().then((m) => !cancelled && setMaximized(m));
        });
      })
      .then((unlisten) => {
        if (unlisten) cleanup = unlisten;
      })
      .catch(() => setShell(false));

    return () => {
      cancelled = true;
      cleanup?.();
    };
  }, [shell]);

  /** No-op outside the shell, so a click cannot throw. */
  const call = (method: "minimize" | "toggleMaximize" | "close") => () => {
    if (!shell) return;
    void import("@tauri-apps/api/window").then(({ getCurrentWindow }) => {
      void getCurrentWindow()[method]();
    });
  };

  return {
    shell,
    maximized,
    minimize: call("minimize"),
    toggleMaximize: call("toggleMaximize"),
    close: call("close"),
  };
}

function WindowButton({
  label,
  onClick,
  danger,
  disabled,
  children,
}: {
  label: string;
  onClick: () => void;
  danger?: boolean;
  disabled?: boolean;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      aria-label={label}
      onClick={onClick}
      disabled={disabled}
      className={[
        "grid h-full w-12 place-items-center text-ink-muted transition-colors",
        "disabled:cursor-default disabled:opacity-40",
        // §9.1: destructive actions use --danger, never oxblood, and the two are never
        // adjacent. Close is the only red thing in the masthead, and only on hover.
        danger
          ? "enabled:hover:bg-danger enabled:hover:text-ink"
          : "enabled:hover:bg-raised enabled:hover:text-ink",
      ].join(" ")}
    >
      {children}
    </button>
  );
}
