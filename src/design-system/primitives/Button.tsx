import type { ButtonHTMLAttributes, ReactNode } from "react";

/**
 * Button — SPEC.md §9, Phase 2.
 *
 * Four variants, and the distinction that matters is `primary` vs everything
 * else: **oxblood marks intent** (§9.1), so `primary` is the one action a screen
 * is actually asking for. Two primary buttons on a screen means one of them is
 * wrong.
 *
 * `danger` uses `--danger` with a text label, never colour alone (§9.1), and
 * `--danger` and `--oxblood` must never appear adjacent — so a destructive
 * confirm dialog uses `danger` + `secondary`, never `danger` + `primary`.
 *
 * Radius is `--radius-sm` (2px) or none. §9.0 permits exactly two values in the
 * whole system and this is one of them; there is no large-radius option.
 */

export type ButtonVariant = "primary" | "secondary" | "ghost" | "danger";
export type ButtonSize = "sm" | "md" | "lg";

interface Props extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, "className"> {
  variant?: ButtonVariant;
  size?: ButtonSize;
  loading?: boolean;
  /** Leading glyph. Never an emoji — §9.0 bans them as icons. */
  icon?: ReactNode;
  children?: ReactNode;
  className?: string;
}

const VARIANT: Record<ButtonVariant, string> = {
  primary:
    "bg-oxblood text-ink border border-oxblood hover:bg-oxblood-hover hover:border-oxblood-hover active:bg-oxblood-deep",
  secondary:
    "bg-transparent text-ink border border-line-interactive hover:bg-raised hover:border-line-strong",
  ghost:
    "bg-transparent text-ink-muted border border-transparent hover:bg-raised hover:text-ink",
  danger:
    "bg-transparent text-danger border border-danger hover:bg-danger hover:text-ink",
};

const SIZE: Record<ButtonSize, string> = {
  sm: "h-8 px-3 text-[11px] tracking-[0.1em]",
  md: "h-10 px-5 text-[11.5px] tracking-[0.12em]",
  lg: "h-12 px-8 text-[12px] tracking-[0.14em]",
};

export function Button({
  variant = "secondary",
  size = "md",
  loading = false,
  icon,
  children,
  disabled,
  className = "",
  ...rest
}: Props) {
  return (
    <button
      type="button"
      disabled={disabled || loading}
      aria-busy={loading || undefined}
      className={[
        "inline-flex items-center justify-center gap-2 rounded-sm font-ui font-semibold uppercase",
        "transition-colors duration-[var(--dur-standard)] ease-[var(--ease-standard)]",
        "disabled:cursor-not-allowed disabled:opacity-45",
        VARIANT[variant],
        SIZE[size],
        className,
      ].join(" ")}
      {...rest}
    >
      {loading ? <Spinner /> : icon}
      {children}
    </button>
  );
}

/** Inline spinner. Rotation only — no glow, no gradient (§9.0). */
function Spinner() {
  return (
    <svg
      className="h-3.5 w-3.5 animate-spin"
      viewBox="0 0 16 16"
      fill="none"
      aria-hidden
    >
      <circle cx="8" cy="8" r="6" stroke="currentColor" strokeWidth="2" opacity="0.25" />
      <path d="M14 8a6 6 0 00-6-6" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
    </svg>
  );
}
