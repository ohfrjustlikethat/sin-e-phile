import { forwardRef, type ReactNode } from "react";

/**
 * A big round icon button with its word underneath (Phase 16, the author: "TV style"): the title page's Download,
 * Watchlist, Watched and Streams. `on` fills the circle, so Watchlist and Watched show their state at a glance.
 * The word says the state too ("On watchlist", "Downloading 42%"), for anyone who cannot tell a fill.
 */
interface Props {
  id?: string;
  label: string;
  icon: ReactNode;
  on?: boolean;
  disabled?: boolean;
  onClick: () => void;
  /** For a toggle: whether it is pressed. Left off for a plain action. */
  pressed?: boolean;
  "aria-haspopup"?: "dialog";
}

export const RoundAction = forwardRef<HTMLButtonElement, Props>(function RoundAction(
  { id, label, icon, on = false, disabled, onClick, pressed, ...rest },
  ref,
) {
  return (
    <button
      ref={ref}
      id={id}
      type="button"
      onClick={onClick}
      disabled={disabled}
      aria-pressed={pressed}
      aria-label={label}
      {...rest}
      className="group flex w-24 flex-col items-center gap-2.5 rounded-sm py-1 text-ink disabled:opacity-50"
    >
      <span
        aria-hidden
        className={[
          "grid h-16 w-16 place-items-center rounded-full border transition-colors duration-[var(--dur-standard)]",
          on
            ? "border-ink bg-ink text-[color:var(--base)]"
            : "border-line-strong text-ink group-hover:border-ink group-disabled:group-hover:border-line-strong",
        ].join(" ")}
      >
        {icon}
      </span>
      <span className="text-center text-[13px] leading-tight text-ink-muted group-hover:text-ink">{label}</span>
    </button>
  );
});

const stroke = { stroke: "currentColor", strokeWidth: 1.7, strokeLinecap: "round", strokeLinejoin: "round" } as const;

/** The four icons, 26 px, drawn as lines to sit with the type. */
export const ICONS = {
  download: (
    <svg width="26" height="26" viewBox="0 0 26 26" fill="none">
      <path d="M13 5v11M8 11.5l5 5 5-5M6 20.5h14" {...stroke} />
    </svg>
  ),
  watchlist: (on: boolean) =>
    on ? (
      <svg width="26" height="26" viewBox="0 0 26 26" fill="none">
        <path d="M7 13.5l4 4 8-9" {...stroke} />
      </svg>
    ) : (
      <svg width="26" height="26" viewBox="0 0 26 26" fill="none">
        <path d="M13 6v14M6 13h14" {...stroke} />
      </svg>
    ),
  watched: (
    <svg width="26" height="26" viewBox="0 0 26 26" fill="none">
      <path d="M3.5 13S7 6.5 13 6.5 22.5 13 22.5 13 19 19.5 13 19.5 3.5 13 3.5 13z" {...stroke} />
      <circle cx="13" cy="13" r="3" {...stroke} />
    </svg>
  ),
  streams: (
    <svg width="26" height="26" viewBox="0 0 26 26" fill="none">
      <path d="M6 8h14M6 13h14M6 18h9" {...stroke} />
    </svg>
  ),
};
