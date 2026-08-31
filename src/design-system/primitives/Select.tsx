import { useId } from "react";
import type { SelectHTMLAttributes } from "react";

interface Option {
  value: string;
  label: string;
}

interface Props extends Omit<SelectHTMLAttributes<HTMLSelectElement>, "className"> {
  label?: string;
  options: Option[];
  className?: string;
}

/**
 * A native `<select>`, deliberately.
 *
 * A custom listbox would need its own focus management, typeahead and screen
 * reader semantics, all of which the platform already does correctly. The chevron
 * is drawn as an inline SVG background so the control still looks like the rest
 * of the system.
 */
export function Select({ label, options, className = "", ...rest }: Props) {
  const id = useId();
  return (
    <div className={["flex flex-col gap-1.5", className].join(" ")}>
      {label && <label htmlFor={id} className="label">{label}</label>}
      <select
        id={id}
        className={[
          "h-10 appearance-none rounded-sm border border-line-interactive bg-surface",
          "pl-3 pr-9 font-ui text-[13px] text-ink transition-colors",
          "duration-[var(--dur-standard)] ease-[var(--ease-standard)]",
          "hover:border-line-strong disabled:cursor-not-allowed disabled:opacity-45",
        ].join(" ")}
        style={{
          backgroundImage:
            "url(\"data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='12' height='12' viewBox='0 0 12 12'><path d='M2 4l4 4 4-4' fill='none' stroke='%239a948c' stroke-width='1.4'/></svg>\")",
          backgroundRepeat: "no-repeat",
          backgroundPosition: "right 10px center",
        }}
        {...rest}
      >
        {options.map((o) => (
          <option key={o.value} value={o.value}>{o.label}</option>
        ))}
      </select>
    </div>
  );
}
