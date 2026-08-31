/**
 * Loading placeholder.
 *
 * A quiet opacity pulse, not a sweeping shimmer gradient — §9.0 bans gradients as
 * decoration, and a shimmer is exactly that. `prefers-reduced-motion` stops the
 * pulse via the global rule in tokens.css.
 */
export function Skeleton({
  className = "", rounded = true,
}: {
  className?: string;
  rounded?: boolean;
}) {
  return (
    <div
      aria-hidden
      className={[
        "animate-pulse bg-surface",
        rounded ? "rounded-sm" : "",
        className,
      ].join(" ")}
    />
  );
}

/** Spinner for indeterminate waits. Rotation only. */
export function Spinner({ size = 16, label = "Loading" }: { size?: number; label?: string }) {
  return (
    <svg
      role="status"
      aria-label={label}
      width={size}
      height={size}
      viewBox="0 0 16 16"
      fill="none"
      className="animate-spin text-ink-muted"
    >
      <circle cx="8" cy="8" r="6" stroke="currentColor" strokeWidth="1.6" opacity="0.25" />
      <path d="M14 8a6 6 0 00-6-6" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" />
    </svg>
  );
}
