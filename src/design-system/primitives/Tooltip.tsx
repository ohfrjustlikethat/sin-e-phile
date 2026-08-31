import { useState } from "react";
import type { ReactNode } from "react";

/**
 * Tooltip.
 *
 * Opens on hover AND on focus, so it is reachable by keyboard. Delayed, so a
 * mouse crossing the screen does not leave a trail of popups.
 */
export function Tooltip({
  content, children, delay = 400,
}: {
  content: string;
  children: ReactNode;
  delay?: number;
}) {
  const [open, setOpen] = useState(false);
  const [timer, setTimer] = useState<number | null>(null);

  const show = () => {
    const t = window.setTimeout(() => setOpen(true), delay);
    setTimer(t);
  };
  const hide = () => {
    if (timer) window.clearTimeout(timer);
    setTimer(null);
    setOpen(false);
  };

  return (
    <span
      className="relative inline-flex"
      onMouseEnter={show}
      onMouseLeave={hide}
      onFocus={() => setOpen(true)}
      onBlur={hide}
    >
      {children}
      {open && (
        <span
          role="tooltip"
          className="pointer-events-none absolute bottom-[calc(100%+8px)] left-1/2 z-50 -translate-x-1/2 whitespace-nowrap rounded-sm border border-line bg-overlay px-2.5 py-1.5 font-ui text-[11px] text-ink"
        >
          {content}
        </span>
      )}
    </span>
  );
}
