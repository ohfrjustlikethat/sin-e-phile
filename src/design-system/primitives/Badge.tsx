import type { ReactNode } from "react";

export type BadgeTone = "neutral" | "accent" | "success" | "warning" | "danger" | "info";

const TONE: Record<BadgeTone, string> = {
  neutral: "border-line-interactive text-ink-muted",
  accent: "border-oxblood text-oxblood-text",
  success: "border-success text-success",
  warning: "border-warning text-warning",
  danger: "border-danger text-danger",
  info: "border-info text-info",
};

/**
 * Badge — availability, quality, language, source.
 *
 * Outlined rather than filled. A filled badge would be a large colour fill, and
 * §9.1 reserves colour fills for intent; a row of filled badges would put more
 * colour on the chrome than on the artwork, which inverts the whole palette rule.
 */
export function Badge({
  tone = "neutral", children,
}: {
  tone?: BadgeTone;
  children: ReactNode;
}) {
  return (
    <span
      className={[
        "inline-flex items-center rounded-sm border px-2 py-[3px]",
        "font-ui text-[9.5px] font-medium uppercase tracking-[0.13em]",
        TONE[tone],
      ].join(" ")}
    >
      {children}
    </span>
  );
}
