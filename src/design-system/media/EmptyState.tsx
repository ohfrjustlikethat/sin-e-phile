import type { ReactNode } from "react";

/**
 * EmptyState.
 *
 * §4 forbids blank screens: every empty state is designed. And §8 requires a
 * gated feature to degrade to something GOOD — so this takes `whatInstead`,
 * which states what the user gets rather than only what they do not.
 *
 * `phase` exists because most of this app does not exist yet, and saying which
 * phase makes a screen real is more honest than an empty box implying it is
 * finished.
 */
export function EmptyState({
  title, body, whatInstead, phase, action,
}: {
  title: string;
  body: string;
  whatInstead?: string;
  phase?: string;
  action?: ReactNode;
}) {
  return (
    <div className="mx-auto flex h-full max-w-xl flex-col items-center justify-center gap-4 px-10 text-center">
      {phase && (
        <span className="rounded-sm border border-line px-2.5 py-1 font-mono text-[9.5px] uppercase tracking-[0.14em] text-ink-faint">
          {phase}
        </span>
      )}
      <h2 className="font-display text-[26px] font-extrabold uppercase tracking-[-0.035em] text-ink">
        {title}
      </h2>
      <p className="max-w-md text-[13.5px] leading-relaxed text-ink-muted">{body}</p>
      {whatInstead && (
        <p className="max-w-md border-t border-line-subtle pt-4 text-[12px] leading-relaxed text-ink-faint">
          {whatInstead}
        </p>
      )}
      {action && <div className="mt-2">{action}</div>}
    </div>
  );
}
