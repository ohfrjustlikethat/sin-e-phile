import { useEffect, useRef } from "react";
import { createPortal } from "react-dom";
import { useUi } from "@/lib/store";
import { useQuery } from "@tanstack/react-query";
import { commands, type CandidateView } from "@/lib/ipc";
import { languageName } from "@/lib/playbackChoice";
import { size } from "@/features/downloads/format";
import { ChoiceRow, type Choice } from "./ChoiceRow";

/**
 * Every stream the sources offered, from the right (Phase 16, the author's round 1): one section per add-on in the
 * order set in Settings, best first within each; each row with its resolution, HDR and codec as badges, its size
 * and seeders ("direct" for a web link), the full release name in small grey type, and its audio and subtitle
 * languages when it says them. Each row plays, and each one that can be downloaded downloads, exactly that stream.
 *
 * In sin·e·phile's own type and hairlines rather than Stremio's look. A dialog, so the D-pad stays inside it;
 * Escape and B close it, as the first step of the app's Back (`panelBack` in src/lib/store.ts).
 */

const RES: Record<string, string> = { "4K": "4K", "1080p": "1080p", "720p": "720p", SD: "SD" };

function badge(text: string, strong = false) {
  return (
    <span
      key={text}
      className={`rounded-sm border px-1.5 py-px font-mono text-[11px] leading-[16px] ${
        strong ? "border-ink text-ink" : "border-line-strong text-ink-muted"
      }`}
    >
      {text}
    </span>
  );
}

const lang = (code: string) => code.split("-")[0]!.toUpperCase();

interface Props {
  title: string;
  candidates: CandidateView[];
  audioChoices: Choice[];
  qualityChoices: Choice[];
  audio: string | null;
  quality: string | null;
  onAudio: (v: string | null) => void;
  onQuality: (v: string | null) => void;
  onPlay: (c: CandidateView) => void;
  onDownload: (c: CandidateView) => void;
  /** The index being downloaded or played just now, to show it is in hand. */
  busy: number | null;
  onClose: () => void;
}

export function StreamsPanel(p: Props) {
  const panel = useRef<HTMLDivElement>(null);
  // The same key and the same shape as Settings' add-on list (src/features/settings/SourcesSection.tsx): a cache
  // entry is shared, and this once stored a list of names under that key while Settings stored the overview, so
  // the panel broke on every title once Settings had been opened (the author's report).
  const sources = useQuery({
    queryKey: ["sources"],
    queryFn: async () => {
      const r = await commands.sourcesOverview();
      if (r.status === "error") throw new Error(r.error);
      return r.data;
    },
  });

  // The first step of the app's one Back (Escape, Backspace, B): closing this, not leaving the page under it.
  const close = useRef(p.onClose);
  close.current = p.onClose;
  useEffect(() => {
    useUi.setState({ panelBack: () => close.current() });
    return () => {
      useUi.setState({ panelBack: null });
    };
  }, []);
  useEffect(() => {
    panel.current?.querySelector<HTMLElement>("button")?.focus();
  }, []);

  // One section per add-on, in Settings' order; best first within each (the ranking's own order).
  const order = (sources.data?.sources ?? []).map((s) => s.name);
  const groups = new Map<string, CandidateView[]>();
  for (const c of p.candidates) groups.set(c.from, [...(groups.get(c.from) ?? []), c]);
  const named = [...groups.keys()].sort((a, b) => {
    const ia = order.indexOf(a);
    const ib = order.indexOf(b);
    return (ia < 0 ? Infinity : ia) - (ib < 0 ? Infinity : ib);
  });

  const content = (
    <div
      ref={panel}
      role="dialog"
      aria-modal="true"
      aria-label="Streams"
      className="pointer-events-auto absolute top-0 right-0 bottom-0 z-40 flex w-[min(620px,94%)] flex-col border-l border-line bg-base"
    >
      <div className="flex items-center gap-3 border-b border-line-subtle px-7 py-4">
        <button
          type="button"
          onClick={p.onClose}
          aria-label={`Back to ${p.title}`}
          className="-ml-2 flex h-11 items-center gap-3 rounded-sm px-2 text-[17px] text-ink-muted hover:text-ink"
        >
          <svg width="22" height="22" viewBox="0 0 22 22" fill="none" aria-hidden>
            <path d="M13.5 5L7.5 11l6 6" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
          </svg>
          <span className="max-w-[30ch] truncate">{p.title}</span>
        </button>
        <span className="flex-1" />
        <h2 className="label">
          Streams <span className="text-ink-faint">· {p.candidates.length}</span>
        </h2>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto px-7 pt-5 pb-10">
        <div className="mb-6 flex flex-col gap-3">
          {p.audioChoices.length > 1 && (
            <ChoiceRow label="Audio" choices={p.audioChoices} value={p.audio} onChange={p.onAudio} />
          )}
          {p.qualityChoices.length > 1 && (
            <ChoiceRow label="Quality" choices={p.qualityChoices} value={p.quality} onChange={p.onQuality} />
          )}
        </div>

        {named.map((name) => (
          <section key={name} aria-label={name} className="mb-8">
            <h3 className="label mb-2 border-t border-line-strong pt-3">
              {name} <span className="text-ink-faint">· {groups.get(name)!.length}</span>
            </h3>
            <ol className="flex flex-col">
              {groups.get(name)!.map((c) => (
                <Row key={c.index} c={c} busy={p.busy === c.index} onPlay={p.onPlay} onDownload={p.onDownload} />
              ))}
            </ol>
          </section>
        ))}
      </div>
    </div>
  );
  // Inside the app's frame, under the masthead (App's #panel-slot); in place where there is none (tests).
  const slot = document.getElementById("panel-slot");
  return slot ? createPortal(content, slot) : content;
}

function Row({
  c,
  busy,
  onPlay,
  onDownload,
}: {
  c: CandidateView;
  busy: boolean;
  onPlay: (c: CandidateView) => void;
  onDownload: (c: CandidateView) => void;
}) {
  const res = RES[c.quality] ?? (c.quality === "unknown quality" ? null : c.quality);
  const codec = c.codec === "unknown" ? null : c.codec.toUpperCase();
  const health = c.direct ? "direct" : c.seeders !== null ? `${c.seeders.toLocaleString()} seeders` : null;
  const facts = [c.size_bytes ? size(c.size_bytes) : null, health].filter(Boolean).join(" · ");
  return (
    <li className="flex items-start gap-4 border-b border-line-subtle py-3.5">
      <div className="min-w-0 flex-1">
        <div className="flex flex-wrap items-center gap-1.5">
          {res && badge(res, true)}
          {c.hdr && badge("HDR")}
          {codec && badge(codec)}
          {facts && <span className="ml-1.5 text-[13px] text-ink-muted">{facts}</span>}
          {c.chosen && <span className="ml-1.5 text-[12px] text-ink">· Play picks this</span>}
        </div>
        <p className="mt-1.5 text-[12px] leading-snug break-all text-ink-faint">{c.title}</p>
        {(c.audio.length > 0 || c.subtitles.length > 0) && (
          <div className="mt-1.5 flex flex-wrap items-center gap-1 text-[11px] text-ink-faint">
            {c.audio.length > 0 && <span className="mr-0.5">Audio</span>}
            {c.audio.map((a) => (
              <span key={`a${a}`} title={languageName(a)}>
                {badge(lang(a))}
              </span>
            ))}
            {c.subtitles.length > 0 && <span className="mr-0.5 ml-2">Subtitles</span>}
            {c.subtitles.map((s) => (
              <span key={`s${s}`} title={languageName(s)}>
                {badge(lang(s))}
              </span>
            ))}
          </div>
        )}
        {(c.weak || c.sunk) && (
          <p className="mt-1 text-[12px] text-warning">{c.sunk ? `Probably not what it says: ${c.sunk}` : c.weak}</p>
        )}
      </div>
      <div className="flex shrink-0 items-center gap-1.5 pt-0.5">
        <button
          type="button"
          onClick={() => onPlay(c)}
          disabled={busy}
          aria-label={`Play ${res ?? ""} from ${c.from}: ${c.title}`.replace("  ", " ")}
          className="grid h-10 w-10 place-items-center rounded-full border border-line-strong text-ink hover:border-ink disabled:opacity-50"
        >
          <svg width="16" height="16" viewBox="0 0 16 16" fill="currentColor" aria-hidden>
            <path d="M5 3.5v9l7.5-4.5z" />
          </svg>
        </button>
        {c.downloadable && (
          <button
            type="button"
            onClick={() => onDownload(c)}
            disabled={busy}
            aria-label={`Download ${res ?? ""} from ${c.from}: ${c.title}`.replace("  ", " ")}
            className="grid h-10 w-10 place-items-center rounded-full border border-line-strong text-ink hover:border-ink disabled:opacity-50"
          >
            <svg width="18" height="18" viewBox="0 0 18 18" fill="none" aria-hidden>
              <path d="M9 3v8M5.5 7.5L9 11l3.5-3.5M4 14.5h10" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
            </svg>
          </button>
        )}
      </div>
    </li>
  );
}

