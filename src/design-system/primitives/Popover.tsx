import { useEffect, useRef } from "react";
import type { ReactNode } from "react";

/**
 * Popover — a non-modal panel anchored to its trigger.
 *
 * Unlike Dialog it does NOT trap focus, because it is not blocking. It closes on
 * Escape and on a click outside, which is what a keyboard and a mouse user each
 * expect.
 */
export function Popover({
  open, onClose, children, align = "start",
}: {
  open: boolean;
  onClose: () => void;
  children: ReactNode;
  align?: "start" | "end";
}) {
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    const onDown = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) onClose();
    };
    document.addEventListener("keydown", onKey);
    document.addEventListener("mousedown", onDown);
    return () => {
      document.removeEventListener("keydown", onKey);
      document.removeEventListener("mousedown", onDown);
    };
  }, [open, onClose]);

  if (!open) return null;
  return (
    <div
      ref={ref}
      className={[
        "absolute top-[calc(100%+6px)] z-40 min-w-[200px] rounded-sm border border-line bg-overlay py-1.5",
        align === "end" ? "right-0" : "left-0",
      ].join(" ")}
    >
      {children}
    </div>
  );
}

export function PopoverItem({
  children, onClick, selected,
}: {
  children: ReactNode;
  onClick?: () => void;
  selected?: boolean;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={[
        "block w-full px-3.5 py-2 text-left font-ui text-[12.5px] transition-colors",
        selected ? "text-ink" : "text-ink-muted hover:bg-raised hover:text-ink",
      ].join(" ")}
    >
      {children}
    </button>
  );
}
