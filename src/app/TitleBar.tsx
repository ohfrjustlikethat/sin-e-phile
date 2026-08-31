import { useEffect, useState } from "react";

/**
 * Custom title bar (SPEC.md Phase 1, subtask 1.5).
 *
 * `decorations: false` in tauri.conf.json removes the OS chrome, so dragging and
 * the window buttons become ours. `data-tauri-drag-region` makes an element
 * behave like a title bar.
 *
 * WHY THE TAURI API IS LOADED LAZILY. `@tauri-apps/api` reads
 * `window.__TAURI_INTERNALS__.metadata` when a window handle is requested, and
 * that object does not exist outside the Tauri shell. Calling it at render time
 * threw `Cannot read properties of undefined (reading 'metadata')`, which the
 * error boundary caught — so the whole app showed a crash screen in any plain
 * browser, including the headless one used to screenshot the design gallery and
 * any future component test that mounts `App`.
 *
 * A component that takes the entire app down when an environment API is absent is
 * fragile regardless of whether that environment is the normal one. So the window
 * controls are only wired up when the shell is actually present, and the bar still
 * renders — inert — when it is not.
 */

/** True only inside the Tauri shell. */
function inTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export function TitleBar() {
  const [maximized, setMaximized] = useState(false);
  const [shell, setShell] = useState(inTauri());

  useEffect(() => {
    if (!shell) return;
    let cancelled = false;
    let cleanup: (() => void) | undefined;

    // Dynamic import: even the module-level side effects should not run in a
    // browser that has no Tauri internals.
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

  return (
    <div
      data-tauri-drag-region
      className="flex h-9 shrink-0 select-none items-center justify-between border-b border-line-subtle bg-void"
    >
      <div data-tauri-drag-region className="flex items-center gap-2.5 px-3.5">
        <div className="h-3 w-3 rounded-sm bg-oxblood" aria-hidden />
        <span className="font-display text-[13px] font-semibold tracking-[-0.02em] text-ink-muted">
          sin·e·phile
        </span>
      </div>

      <div className="flex h-full">
        <WindowButton label="Minimise" onClick={call("minimize")} disabled={!shell}>
          <svg width="10" height="10" viewBox="0 0 10 10">
            <path d="M0 5h10" stroke="currentColor" strokeWidth="1" />
          </svg>
        </WindowButton>
        <WindowButton
          label={maximized ? "Restore" : "Maximise"}
          onClick={call("toggleMaximize")}
          disabled={!shell}
        >
          {maximized ? (
            <svg width="10" height="10" viewBox="0 0 10 10">
              <path
                d="M2.5 2.5h5v5h-5z M0.5 0.5h7v1 M8.5 1.5v6h-6"
                fill="none"
                stroke="currentColor"
                strokeWidth="1"
              />
            </svg>
          ) : (
            <svg width="10" height="10" viewBox="0 0 10 10">
              <rect x="0.5" y="0.5" width="9" height="9" fill="none" stroke="currentColor" strokeWidth="1" />
            </svg>
          )}
        </WindowButton>
        <WindowButton label="Close" danger onClick={call("close")} disabled={!shell}>
          <svg width="10" height="10" viewBox="0 0 10 10">
            <path d="M0 0l10 10M10 0L0 10" stroke="currentColor" strokeWidth="1" />
          </svg>
        </WindowButton>
      </div>
    </div>
  );
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
        // §9.1: destructive actions use --danger, never oxblood, and the two are
        // never adjacent. Close is the only red thing in the title bar.
        danger
          ? "enabled:hover:bg-danger enabled:hover:text-ink"
          : "enabled:hover:bg-raised enabled:hover:text-ink",
      ].join(" ")}
    >
      {children}
    </button>
  );
}
