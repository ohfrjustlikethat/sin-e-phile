/**
 * One row of choices under Play: "Audio" or "Quality" (SPEC.md Phase 9).
 *
 * Buttons, not a `<select>`: a native select opens a Windows popup that the pad's focus
 * system cannot reach, and a remote has to be able to change the language (ADR-0041).
 * Each option is a focus stop, so the TV focus ring glides across them (ADR-0042).
 */

export interface Choice {
  value: string | null;
  label: string;
}

interface Props {
  label: string;
  choices: Choice[];
  value: string | null;
  onChange: (value: string | null) => void;
}

export function ChoiceRow({ label, choices, value, onChange }: Props) {
  return (
    <div role="radiogroup" aria-label={label} className="flex items-baseline gap-6">
      <span className="label w-20 shrink-0">{label}</span>
      <div className="flex flex-wrap gap-x-5 gap-y-2">
        {choices.map((c) => {
          const on = c.value === value;
          return (
            <button
              key={c.value ?? "auto"}
              type="button"
              role="radio"
              aria-checked={on}
              onClick={() => onChange(c.value)}
              className={[
                "border-b pb-0.5 font-ui text-[14px] transition-colors",
                "duration-[var(--dur-standard)] ease-[var(--ease-standard)]",
                on ? "border-ink text-ink" : "border-transparent text-ink-muted hover:text-ink",
              ].join(" ")}
            >
              {c.label}
            </button>
          );
        })}
      </div>
    </div>
  );
}
