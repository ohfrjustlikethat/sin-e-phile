import { useEffect, useRef } from "react";
import type { ReactNode } from "react";

/**
 * Modal dialog.
 *
 * §4 lists "modal dialogs for routine actions" as an anti-pattern, so this is for
 * the genuinely blocking cases only — a destructive confirm, a required choice.
 * Prefer inline and non-blocking everywhere else.
 *
 * Focus is trapped and restored, and Escape closes. Those are not niceties: a
 * modal that leaks focus to the page behind it is unusable by keyboard, and §9.4
 * makes keyboard completeness a Phase 2 requirement.
 */
export function Dialog({
  open, onClose, title, children, footer,
}: {
  open: boolean;
  onClose: () => void;
  title: string;
  children: ReactNode;
  footer?: ReactNode;
}) {
  const panelRef = useRef<HTMLDivElement>(null);
  const restoreTo = useRef<HTMLElement | null>(null);

  useEffect(() => {
    if (!open) return;
    restoreTo.current = document.activeElement as HTMLElement | null;

    const focusables = () =>
      Array.from(
        panelRef.current?.querySelectorAll<HTMLElement>(
          'button,[href],input,select,textarea,[tabindex]:not([tabindex="-1"])',
        ) ?? [],
      ).filter((el) => !el.hasAttribute("disabled"));

    focusables()[0]?.focus();

    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        onClose();
        return;
      }
      if (e.key !== "Tab") return;
      const items = focusables();
      if (items.length === 0) return;
      const first = items[0]!;
      const last = items[items.length - 1]!;
      if (e.shiftKey && document.activeElement === first) {
        e.preventDefault();
        last.focus();
      } else if (!e.shiftKey && document.activeElement === last) {
        e.preventDefault();
        first.focus();
      }
    };

    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("keydown", onKey);
      restoreTo.current?.focus();
    };
  }, [open, onClose]);

  if (!open) return null;

  return (
    <div className="fixed inset-0 z-50 grid place-items-center" style={{ background: "var(--scrim)" }}>
      <div
        ref={panelRef}
        role="dialog"
        aria-modal="true"
        aria-label={title}
        className="w-full max-w-md rounded-sm border border-line bg-overlay"
      >
        <header className="border-b border-line-subtle px-6 py-4">
          <h2 className="font-display text-[16px] font-extrabold uppercase tracking-[-0.02em] text-ink">
            {title}
          </h2>
        </header>
        <div className="px-6 py-5 text-[13px] leading-relaxed text-ink-muted">{children}</div>
        {footer && (
          <footer className="flex justify-end gap-2.5 border-t border-line-subtle px-6 py-4">
            {footer}
          </footer>
        )}
      </div>
    </div>
  );
}
