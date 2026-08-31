import { useId } from "react";
import type { InputHTMLAttributes } from "react";

/**
 * Text input.
 *
 * The border uses `--line-interactive`, not a decorative line: WCAG 1.4.11 asks
 * 3:1 of a boundary that identifies a component, and the contrast audit enforces
 * that pair. A hairline here would be a real accessibility defect, not a style
 * choice.
 */
interface Props extends Omit<InputHTMLAttributes<HTMLInputElement>, "className"> {
  label?: string;
  hint?: string;
  error?: string;
  className?: string;
}

export function Input({ label, hint, error, className = "", ...rest }: Props) {
  const id = useId();
  const describedBy = error ? `${id}-err` : hint ? `${id}-hint` : undefined;

  return (
    <div className={["flex flex-col gap-1.5", className].join(" ")}>
      {label && (
        <label htmlFor={id} className="label">
          {label}
        </label>
      )}
      <input
        id={id}
        aria-invalid={error ? true : undefined}
        aria-describedby={describedBy}
        className={[
          "h-10 rounded-sm border bg-surface px-3 font-ui text-[13px] text-ink",
          "placeholder:text-ink-faint transition-colors",
          "duration-[var(--dur-standard)] ease-[var(--ease-standard)]",
          "disabled:cursor-not-allowed disabled:opacity-45",
          error ? "border-danger" : "border-line-interactive hover:border-line-strong",
        ].join(" ")}
        {...rest}
      />
      {error ? (
        <span id={`${id}-err`} className="text-[11px] text-danger">{error}</span>
      ) : hint ? (
        <span id={`${id}-hint`} className="text-[11px] text-ink-faint">{hint}</span>
      ) : null}
    </div>
  );
}
