import { useEffect, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Button } from "@/design-system";
import { commands, type StatementsView } from "@/lib/ipc";
import { ChoiceRow, type Choice } from "@/features/detail/ChoiceRow";

/**
 * What you like, in your own terms (SPEC.md Phase 14): six statements that are not genres
 * (pace, endings, era, subtitles, length, form), and "what you love" in a sentence or two,
 * which is embedded into the taste. Below, the titles the taste points at so far: the
 * answers visibly do something.
 */

async function ok<T>(p: Promise<{ status: "ok"; data: T } | { status: "error"; error: string }>) {
  const r = await p;
  if (r.status === "error") throw new Error(r.error);
  return r.data;
}

type Axis = keyof StatementsView;

export const AXES: { axis: Axis; label: string; choices: [string, string, string] }[] = [
  { axis: "pace", label: "Pace", choices: ["Slow and patient", "Either", "Brisk"] },
  { axis: "ambiguity", label: "Endings", choices: ["Left open", "Either", "Tied up"] },
  { axis: "era", label: "Era", choices: ["Older films", "Any era", "Recent films"] },
  { axis: "subtitles", label: "Subtitles", choices: ["Rather not", "Sometimes", "Happily"] },
  { axis: "runtime", label: "Length", choices: ["Short", "Any length", "Long is welcome"] },
  { axis: "form", label: "Form", choices: ["Classical", "Either", "Experimental"] },
];

const EMPTY: StatementsView = { pace: null, ambiguity: null, era: null, subtitles: null, runtime: null, form: null };

export const TASTE = ["taste-input"];
export const PREVIEW = ["taste-preview"];

export function TasteInput({ onSaved }: { onSaved?: () => void }) {
  const client = useQueryClient();
  const input = useQuery({ queryKey: TASTE, queryFn: () => ok(commands.tasteInput()) });
  const preview = useQuery({ queryKey: PREVIEW, queryFn: () => ok(commands.tastePreview(8)) });
  const [statements, setStatements] = useState<StatementsView>(EMPTY);
  const [love, setLove] = useState("");
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    if (input.data) {
      setStatements(input.data.statements);
      setLove(input.data.love ?? "");
    }
  }, [input.data]);

  const save = useMutation({
    mutationFn: () => ok(commands.setTasteInput(statements, love.trim() || null)),
    onSuccess: () => {
      setSaved(true);
      void client.invalidateQueries({ queryKey: TASTE });
      void client.invalidateQueries({ queryKey: PREVIEW });
      onSaved?.();
    },
  });

  const set = (axis: Axis, v: string | null) => {
    setSaved(false);
    setStatements((s) => ({ ...s, [axis]: v === null ? null : Number(v) }));
  };

  return (
    <div className="flex flex-col gap-4">
      {AXES.map(({ axis, label, choices }) => {
        const options: Choice[] = choices.map((c, i) => ({ value: String(i - 1), label: c }));
        const v = statements[axis];
        return (
          <ChoiceRow
            key={axis}
            label={label}
            choices={options}
            value={v === null ? null : String(v)}
            onChange={(next) => set(axis, next === (v === null ? null : String(v)) ? null : next)}
          />
        );
      })}
      <label className="mt-2 flex flex-col gap-1.5">
        <span className="label">What you love, in your own words</span>
        <textarea
          value={love}
          onChange={(e) => {
            setSaved(false);
            setLove(e.target.value);
          }}
          maxLength={2000}
          rows={3}
          placeholder="Long takes, rain, people who say less than they mean."
          className="resize-y rounded-sm border border-line-interactive bg-surface px-3 py-2 text-[14px] text-ink placeholder:text-ink-faint"
        />
      </label>
      <div className="flex items-center gap-3">
        <Button onClick={() => save.mutate()} disabled={save.isPending}>
          {save.isPending ? "Saving…" : "Save"}
        </Button>
        {saved && (
          <span className="text-xs text-ink-muted" role="status">
            Saved.
          </span>
        )}
        {save.isError && <span className="text-xs text-danger">{save.error.message}</span>}
      </div>
      {preview.data && preview.data.length > 0 && (
        <section aria-label="Where your taste points" className="mt-4">
          <h3 className="label mb-2">Where your taste points so far</h3>
          <ul className="grid grid-cols-2 gap-x-8 gap-y-1.5 text-[14px]">
            {preview.data.map((p) => (
              <li key={p.id}>
                <span className="font-serif text-[16px] text-ink">{p.title}</span>
                <span className="ml-2 text-[12px] text-ink-faint">
                  {[p.year, p.made_by].filter(Boolean).join(" · ")}
                </span>
              </li>
            ))}
          </ul>
        </section>
      )}
    </div>
  );
}
