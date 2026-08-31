import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";

/**
 * Custom title bar (SPEC.md Phase 1, subtask 1.5).
 *
 * `decorations: false` in tauri.conf.json removes the OS chrome, so dragging and
 * the window buttons become ours. `data-tauri-drag-region` is what makes an
 * element behave like a title bar.
 */
export function TitleBar() {
  const [maximized, setMaximized] = useState(false);
  const win = getCurrentWindow();

  useEffect(() => {
    let cancelled = false;
    void win.isMaximized().then((m) => !cancelled && setMaximized(m));
    const unlisten = win.onResized(() => {
      void win.isMaximized().then((m) => !cancelled && setMaximized(m));
    });
    return () => {
      cancelled = true;
      void unlisten.then((f) => f());
    };
  }, [win]);

  return (
    <div
      data-tauri-drag-region
      className="flex h-9 shrink-0 items-center justify-between border-b border-line-subtle bg-void select-none"
    >
      <div data-tauri-drag-region className="flex items-center gap-2.5 px-3.5">
        <div className="h-3 w-3 rounded-[3px] bg-oxblood" aria-hidden />
        <span className="text-[13px] font-medium tracking-tight text-ink-muted">
          sin-e-phile
        </span>
      </div>

      <div className="flex h-full">
        <WindowButton label="Minimise" onClick={() => void win.minimize()}>
          <svg width="10" height="10" viewBox="0 0 10 10"><path d="M0 5h10" stroke="currentColor" strokeWidth="1" /></svg>
        </WindowButton>
        <WindowButton label={maximized ? "Restore" : "Maximise"} onClick={() => void win.toggleMaximize()}>
          {maximized ? (
            <svg width="10" height="10" viewBox="0 0 10 10"><path d="M2.5 2.5h5v5h-5z M0.5 0.5h7v1 M8.5 1.5v6h-6" fill="none" stroke="currentColor" strokeWidth="1" /></svg>
          ) : (
            <svg width="10" height="10" viewBox="0 0 10 10"><rect x="0.5" y="0.5" width="9" height="9" fill="none" stroke="currentColor" strokeWidth="1" /></svg>
          )}
        </WindowButton>
        <WindowButton label="Close" danger onClick={() => void win.close()}>
          <svg width="10" height="10" viewBox="0 0 10 10"><path d="M0 0l10 10M10 0L0 10" stroke="currentColor" strokeWidth="1" /></svg>
        </WindowButton>
      </div>
    </div>
  );
}

function WindowButton({
  label,
  onClick,
  danger,
  children,
}: {
  label: string;
  onClick: () => void;
  danger?: boolean;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      aria-label={label}
      onClick={onClick}
      className={[
        "grid h-full w-12 place-items-center text-ink-muted transition-colors",
        // §9.1: destructive actions use --danger, never oxblood, and the two are
        // never adjacent. Close is the only red thing in the title bar.
        danger ? "hover:bg-danger hover:text-ink" : "hover:bg-raised hover:text-ink",
      ].join(" ")}
    >
      {children}
    </button>
  );
}
