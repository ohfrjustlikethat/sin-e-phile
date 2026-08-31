interface Props {
  /** 0-1. Playback or download progress. */
  value: number;
  /** 0-1. Buffered or downloaded ahead — drawn behind `value`. */
  buffered?: number;
  label?: string;
  size?: "thin" | "thick";
}

/**
 * Progress, with an optional buffered range behind it.
 *
 * The two-track shape exists because Phase 7 streams torrents: the user needs to
 * see how far the download runs ahead of the playhead, which a single bar cannot
 * express. Progress is `--oxblood-bright` because progress is intent (§9.1).
 */
export function ProgressBar({ value, buffered = 0, label, size = "thin" }: Props) {
  const clamp = (n: number) => Math.min(1, Math.max(0, n));
  return (
    <div
      role="progressbar"
      aria-valuenow={Math.round(clamp(value) * 100)}
      aria-valuemin={0}
      aria-valuemax={100}
      aria-label={label}
      className={["relative w-full bg-line", size === "thin" ? "h-[3px]" : "h-1"].join(" ")}
    >
      {buffered > 0 && (
        <div
          className="absolute inset-y-0 left-0 bg-line-strong"
          style={{ width: `${clamp(buffered) * 100}%` }}
        />
      )}
      <div
        className="absolute inset-y-0 left-0 bg-oxblood-bright"
        style={{ width: `${clamp(value) * 100}%` }}
      />
    </div>
  );
}
