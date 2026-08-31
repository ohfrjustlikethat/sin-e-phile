import type { ButtonHTMLAttributes, ReactNode } from "react";

/**
 * A square button carrying only a glyph.
 *
 * `label` is REQUIRED and becomes `aria-label`. An icon-only control with no
 * accessible name is invisible to a screen reader, and §9.4 makes full keyboard
 * and assistive navigation a Phase 2 requirement rather than a Phase 27 retrofit.
 */
interface Props extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, "className"> {
  label: string;
  children: ReactNode;
  size?: "sm" | "md";
  active?: boolean;
  className?: string;
}

export function IconButton({
  label, children, size = "md", active = false, className = "", ...rest
}: Props) {
  return (
    <button
      type="button"
      aria-label={label}
      aria-pressed={rest.onClick && active ? true : undefined}
      title={label}
      className={[
        "grid place-items-center rounded-sm border border-transparent transition-colors",
        "duration-[var(--dur-standard)] ease-[var(--ease-standard)]",
        "disabled:cursor-not-allowed disabled:opacity-45",
        active ? "text-ink bg-raised" : "text-ink-muted hover:text-ink hover:bg-raised",
        size === "sm" ? "h-8 w-8" : "h-10 w-10",
        className,
      ].join(" ")}
      {...rest}
    >
      {children}
    </button>
  );
}
