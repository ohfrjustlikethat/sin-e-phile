import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands, type SubtitleOfferView, type SubtitleSettings } from "@/lib/ipc";

/**
 * The subtitles in the player (SPEC.md Phase 16, ADR-0045): how they look, changed live and
 * kept for the profile; finding them on OpenSubtitles and choosing one, as VLC's VLSub
 * does; loading a file from disk; aligning the showing one again. Every control is a
 * button, so the D-pad reaches all of it (ADR-0041).
 */

async function ok<T>(p: Promise<{ status: "ok"; data: T } | { status: "error"; error: string }>) {
  const r = await p;
  if (r.status === "error") throw new Error(r.error);
  return r.data;
}

export const SUB_SETTINGS = ["subtitle-settings"] as const;

export const COLOURS: [string, string][] = [
  ["#FFFFFF", "White"],
  ["#FFD400", "Yellow"],
  ["#7FE7FF", "Cyan"],
  ["#B8F27A", "Green"],
];
export const FONTS = ["Segoe UI", "Arial", "Verdana", "Georgia", "Calibri"];

/** The next font in the list, wrapping round. */
export function nextFont(font: string): string {
  const i = FONTS.indexOf(font);
  return FONTS[(i + 1) % FONTS.length] ?? FONTS[0]!;
}

/** "English · Film.2019.BluRay · timed for this file". */
export function offerLine(o: SubtitleOfferView): string {
  const lang = new Intl.DisplayNames(undefined, { type: "language" }).of(o.language) ?? o.language;
  return [lang, o.release, o.hash_match ? "timed for this file" : null].filter(Boolean).join(" · ");
}

const row = "flex w-full items-center justify-between gap-4 rounded-sm px-2 py-1.5 text-left text-[14px] hover:bg-raised";
const small = "h-8 min-w-8 rounded-sm border border-line-strong px-2 text-[13px] text-ink hover:bg-raised";

export function SubtitleTools() {
  const client = useQueryClient();
  const settings = useQuery({ queryKey: SUB_SETTINGS, queryFn: () => ok(commands.subtitleSettings()) });
  const [said, setSaid] = useState<string | null>(null);
  const [finding, setFinding] = useState(false);

  // Saved for the profile and applied to the playing film at once (set_subtitle_settings).
  const save = useMutation({
    mutationFn: (s: SubtitleSettings) => ok(commands.setSubtitleSettings(s)),
    onSuccess: () => void client.invalidateQueries({ queryKey: SUB_SETTINGS }),
    onError: (e) => setSaid(e.message),
  });
  const offers = useQuery({
    queryKey: ["subtitle-offers"],
    queryFn: () => ok(commands.subtitleOffers()),
    enabled: finding,
    retry: false,
  });
  const choose = useMutation({
    mutationFn: (o: SubtitleOfferView) => ok(commands.subtitleChoose(o)),
    onSuccess: (note) => {
      setFinding(false);
      setSaid(`Showing the chosen subtitles (${note}).`);
    },
    onError: (e) => setSaid(e.message),
  });
  const load = useMutation({
    mutationFn: () => ok(commands.subtitleLoad()),
    onSuccess: (note) => setSaid(note ? `Showing the file (${note}).` : null),
    onError: (e) => setSaid(e.message),
  });
  const realign = useMutation({
    mutationFn: () => ok(commands.subtitleRealign()),
    onSuccess: (note) => setSaid(`Aligned again (${note}).`),
    onError: (e) => setSaid(e.message),
  });

  const s = settings.data?.settings;
  const change = (patch: Partial<SubtitleSettings>) => s && save.mutate({ ...s, ...patch });
  const colour = s?.colour ?? "#FFFFFF";
  const outline = s?.outline ?? 3;

  return (
    <div className="mt-2 flex flex-col gap-1" aria-label="Subtitle tools">
      <button type="button" className={row} onClick={() => setFinding((f) => !f)} aria-expanded={finding}>
        <span className="text-ink">Find subtitles</span>
        <span className="label text-ink-faint">OpenSubtitles</span>
      </button>
      {finding && (
        <div className="ml-2 flex flex-col border-l border-line-subtle pl-2" aria-label="Subtitles found">
          {offers.isPending && <p className="px-2 py-1.5 text-[13px] text-ink-muted">Searching…</p>}
          {offers.isError && <p className="px-2 py-1.5 text-[13px] text-ink-muted">{offers.error.message}</p>}
          {offers.data?.length === 0 && (
            <p className="px-2 py-1.5 text-[13px] text-ink-muted">Nothing in your languages for this file.</p>
          )}
          {offers.data?.slice(0, 12).map((o) => (
            <button key={o.file_id} type="button" className={row} onClick={() => choose.mutate(o)} disabled={choose.isPending}>
              <span className="text-[13px] text-ink">{offerLine(o)}</span>
            </button>
          ))}
        </div>
      )}
      <button type="button" className={row} onClick={() => load.mutate()}>
        <span className="text-ink">Load a file…</span>
      </button>
      <button type="button" className={row} onClick={() => realign.mutate()}>
        <span className="text-ink">Align again</span>
      </button>
      {said && (
        <p role="status" className="px-2 py-1 text-[12px] text-ink-muted">
          {said}
        </p>
      )}

      {s && (
        <section aria-label="Subtitle style" className="mt-3 flex flex-col gap-1.5">
          <h3 className="label px-2">Style</h3>
          <div className="flex items-center gap-2 px-2">
            <span className="flex-1 text-[13px] text-ink-muted">
              Size <span className="tabular-nums text-ink">{s.size}</span>
            </span>
            <button type="button" className={small} aria-label="Smaller subtitles" onClick={() => change({ size: Math.max(20, s.size - 4) })}>−</button>
            <button type="button" className={small} aria-label="Larger subtitles" onClick={() => change({ size: Math.min(100, s.size + 4) })}>+</button>
          </div>
          <div className="flex items-center gap-2 px-2">
            <span className="flex-1 text-[13px] text-ink-muted">
              Position <span className="tabular-nums text-ink">{s.position}</span>
            </span>
            <button type="button" className={small} aria-label="Subtitles higher" onClick={() => change({ position: Math.max(50, s.position - 2) })}>↑</button>
            <button type="button" className={small} aria-label="Subtitles lower" onClick={() => change({ position: Math.min(100, s.position + 2) })}>↓</button>
          </div>
          <div className="flex items-center gap-2 px-2" role="radiogroup" aria-label="Subtitle colour">
            <span className="flex-1 text-[13px] text-ink-muted">Colour</span>
            {COLOURS.map(([hex, name]) => (
              <button
                key={hex}
                type="button"
                role="radio"
                aria-checked={colour.toUpperCase() === hex}
                aria-label={name}
                onClick={() => change({ colour: hex })}
                className={`h-8 w-8 rounded-sm border ${colour.toUpperCase() === hex ? "border-ink" : "border-line-strong"}`}
              >
                <span aria-hidden className="text-[15px] font-semibold" style={{ color: hex }}>
                  A
                </span>
              </button>
            ))}
          </div>
          <div className="flex items-center gap-2 px-2">
            <span className="flex-1 text-[13px] text-ink-muted">
              Outline <span className="tabular-nums text-ink">{outline}</span>
            </span>
            <button type="button" className={small} aria-label="Thinner outline" onClick={() => change({ outline: Math.max(0, outline - 1) })}>−</button>
            <button type="button" className={small} aria-label="Thicker outline" onClick={() => change({ outline: Math.min(6, outline + 1) })}>+</button>
          </div>
          <button type="button" className={row} onClick={() => change({ background: !s.background })} aria-pressed={s.background}>
            <span className="text-ink">Box behind the words</span>
            <span className="label text-ink-faint">{s.background ? "On" : "Off"}</span>
          </button>
          <button type="button" className={row} onClick={() => change({ font: nextFont(s.font) })}>
            <span className="text-ink">Font</span>
            <span className="text-[13px] text-ink-muted">{s.font}</span>
          </button>
        </section>
      )}
    </div>
  );
}
