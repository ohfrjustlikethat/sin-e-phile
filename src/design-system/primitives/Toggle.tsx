import { useId } from "react";

interface Props {
  checked: boolean;
  onChange: (next: boolean) => void;
  label: string;
  description?: string;
  disabled?: boolean;
}

/**
 * Toggle.
 *
 * A real `<button role="switch">` rather than a styled checkbox, so `aria-checked`
 * is exact and the space/enter behaviour is the platform's.
 *
 * The track is square-cornered like everything else — §9.0 permits two radius
 * values and a pill would be a third. The knob is a 2px-radius block, which reads
 * as mechanical rather than as the usual soft switch, and suits the catalogue
 * direction (ADR-0024).
 */
export function Toggle({ checked, onChange, label, description, disabled }: Props) {
  const id = useId();
  return (
    <div className="flex items-start justify-between gap-6 py-2.5">
      <div className="min-w-0">
        <label htmlFor={id} className="block text-[13px] text-ink">{label}</label>
        {description && (
          <p className="mt-0.5 text-[11px] leading-relaxed text-ink-faint">{description}</p>
        )}
      </div>
      <button
        id={id}
        type="button"
        role="switch"
        aria-checked={checked}
        disabled={disabled}
        onClick={() => onChange(!checked)}
        className={[
          "relative h-5 w-9 shrink-0 rounded-sm border transition-colors",
          "duration-[var(--dur-standard)] ease-[var(--ease-standard)]",
          "disabled:cursor-not-allowed disabled:opacity-45",
          checked
            ? "border-oxblood bg-oxblood"
            : "border-line-interactive bg-surface hover:border-line-strong",
        ].join(" ")}
      >
        <span
          data-motion="scale"
          className={[
            "absolute top-0.5 h-3.5 w-3.5 rounded-sm transition-[left] duration-[var(--dur-standard)]",
            "ease-[var(--ease-standard)]",
            checked ? "left-[18px] bg-ink" : "left-0.5 bg-ink-faint",
          ].join(" ")}
        />
      </button>
    </div>
  );
}
