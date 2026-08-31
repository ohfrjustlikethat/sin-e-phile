import { useId } from "react";

interface Props {
  value: number;
  min?: number;
  max?: number;
  step?: number;
  onChange: (next: number) => void;
  label: string;
  /** Rendered beside the label — e.g. "48%", "-120 ms". */
  display?: string;
}

/**
 * Slider, on a native `range` input.
 *
 * Native gives keyboard control (arrows, home/end, page up/down) for free, which
 * a div-based slider has to reimplement and usually gets wrong. The track and
 * thumb are restyled; the semantics are the platform's.
 */
export function Slider({ value, min = 0, max = 100, step = 1, onChange, label, display }: Props) {
  const id = useId();
  const pct = ((value - min) / (max - min)) * 100;
  return (
    <div className="flex flex-col gap-2 py-2">
      <div className="flex items-baseline justify-between">
        <label htmlFor={id} className="label">{label}</label>
        {display && <span className="font-mono text-[11px] text-ink-muted">{display}</span>}
      </div>
      <input
        id={id}
        type="range"
        min={min}
        max={max}
        step={step}
        value={value}
        onChange={(e) => onChange(Number(e.target.value))}
        className="sinephile-range h-4 w-full cursor-pointer appearance-none bg-transparent"
        style={{ ["--pct" as string]: `${pct}%` }}
      />
    </div>
  );
}
