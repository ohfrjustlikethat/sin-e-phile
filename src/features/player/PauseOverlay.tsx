import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { commands, type PauseDetails, type PersonView, type TitleView } from "@/lib/ipc";
import { clock } from "./intents";

/**
 * The pause overlay — SPEC.md Phase 11's signature feature, laid out as ADR-0043 chose:
 * the dimmed frame on the left with the title and Resume, a panel from the right with the
 * cast, who made it, its rating and more like this. A cast member opens their filmography
 * in the panel; a title leaves the player for its page.
 *
 * Everything shown was fetched when playback started (`details`), so pausing only paints.
 */

const CREW_LABEL: Record<string, string> = {
  director: "Directed by",
  creator: "Created by",
  writer: "Written by",
  composer: "Music by",
  cinematographer: "Photographed by",
  editor: "Edited by",
};

interface Props {
  title: string;
  still: string | null;
  position: number | null;
  duration: number | null;
  chapter: string | null;
  details: PauseDetails | undefined;
  onResume: () => void;
  onOpen: (id: number) => void;
}

export function PauseOverlay({ title, still, position, duration, chapter, details, onResume, onOpen }: Props) {
  const resume = useRef<HTMLButtonElement>(null);
  const [person, setPerson] = useState<PersonView | null>(null);
  const [films, setFilms] = useState<TitleView[] | null>(null);

  // Phase 11 E1: from the pause to this frame on screen, measured in the real window.
  useLayoutEffect(() => {
    const started = (window as { __pausedAt?: number }).__pausedAt;
    requestAnimationFrame(() => {
      if (started !== undefined) (window as { __pauseMs?: number }).__pauseMs = performance.now() - started;
    });
    if (document.documentElement.dataset["input"] === "pad") resume.current?.focus();
  }, []);

  useEffect(() => {
    if (!person) return;
    let live = true;
    setFilms(null);
    void commands.filmography(person.person_id).then((r) => {
      if (live) setFilms(r.status === "ok" ? r.data : []);
    });
    return () => {
      live = false;
    };
  }, [person]);

  const left = position !== null && duration !== null ? Math.max(0, duration - position) : null;
  const crew = (details?.crew ?? []).reduce<Record<string, string[]>>((acc, c) => {
    const role = c.part ?? "";
    (acc[role] ??= []).push(c.name);
    return acc;
  }, {});

  return (
    <div className="absolute inset-0" role="dialog" aria-label="Paused">
      {still && <img src={still} alt="" className="absolute inset-0 h-full w-full object-contain transition-opacity duration-200" />}
      <div className="absolute inset-0 bg-black/60" />

      <div className="absolute top-[30%] left-[6%] flex max-w-[44%] flex-col gap-4">
        <p className="label text-ink-muted">
          Paused · {clock(position)} of {clock(duration)}
          {left !== null && ` · ${Math.round(left / 60)} min left`}
        </p>
        <h1 className="font-display text-[44px] leading-[1.05] font-semibold tracking-[-0.03em]">{title}</h1>
        {chapter && <p className="text-[15px] text-ink-muted">{chapter}</p>}
        <div>
          <button
            ref={resume}
            type="button"
            onClick={onResume}
            className="mt-2 rounded-sm border border-line-strong px-6 py-2.5 text-[12px] tracking-[0.12em] uppercase hover:bg-raised"
          >
            Resume
          </button>
        </div>
      </div>

      {/* Between the player's top and bottom bars, which stay while paused. */}
      <aside
        aria-label="About this film"
        className="absolute top-[68px] right-3 bottom-[96px] flex w-[min(440px,38%)] flex-col overflow-y-auto rounded-md bg-surface px-7 py-6"
      >
        {person ? (
          <section aria-label={`${person.name}'s films`}>
            <button type="button" onClick={() => setPerson(null)} className="label mb-5 hover:text-ink">
              ← Cast
            </button>
            <h2 className="font-display text-[24px] leading-tight font-semibold">{person.name}</h2>
            <ol className="mt-4 flex flex-col">
              {(films ?? []).map((f) => (
                <li key={f.id}>
                  <button
                    type="button"
                    onClick={() => onOpen(f.id)}
                    className="flex w-full items-baseline gap-3 border-t border-line-subtle py-2 text-left hover:bg-raised"
                  >
                    <span className="w-10 shrink-0 tabular-nums text-ink-faint">{f.year ?? ""}</span>
                    <span className="flex-1 text-[14px] text-ink">{f.title}</span>
                    {f.part && <span className="text-[12px] text-ink-faint">{f.part}</span>}
                  </button>
                </li>
              ))}
            </ol>
            {films?.length === 0 && <p className="text-sm text-ink-muted">Nothing else in the catalogue.</p>}
          </section>
        ) : (
          <>
            {(details?.cast.length ?? 0) > 0 && (
              <section aria-label="Cast">
                <h2 className="label mb-3">Cast</h2>
                <ul className="flex flex-col">
                  {details!.cast.map((c) => (
                    <li key={`${c.person_id}-${c.part ?? ""}`}>
                      <button
                        type="button"
                        onClick={() => setPerson(c)}
                        className="flex w-full items-baseline justify-between gap-4 py-1.5 text-left hover:text-ink"
                      >
                        <span className="text-[15px] text-ink">{c.name}</span>
                        {c.part && <span className="truncate text-[13px] text-ink-faint">{c.part}</span>}
                      </button>
                    </li>
                  ))}
                </ul>
              </section>
            )}
            {Object.keys(crew).length > 0 && (
              <section aria-label="Made by" className="mt-7 flex flex-col gap-1.5">
                {Object.entries(crew).map(([role, names]) => (
                  <p key={role} className="text-[13px] text-ink-muted">
                    <span className="text-ink-faint">{CREW_LABEL[role] ?? role}</span> {names.join(", ")}
                  </p>
                ))}
              </section>
            )}
            {(details?.ratings.length ?? 0) > 0 && (
              <section aria-label="Ratings" className="mt-6 flex gap-5">
                {details!.ratings.map((r) => (
                  <p key={r.source} className="text-[13px] text-ink-muted">
                    <span className="text-ink-faint">{r.source}</span>{" "}
                    <span className="tabular-nums text-ink">{(r.score ?? 0).toFixed(1)}</span>
                    {r.votes ? <span className="text-ink-faint"> · {r.votes.toLocaleString("en")} votes</span> : null}
                  </p>
                ))}
              </section>
            )}
            {(details?.more_like_this.length ?? 0) > 0 && (
              <section aria-label="More like this" className="mt-8">
                <h2 className="label mb-3">More like this</h2>
                <ul className="flex flex-col">
                  {details!.more_like_this.map((t) => (
                    <li key={t.id}>
                      <button
                        type="button"
                        onClick={() => onOpen(t.id)}
                        className="flex w-full items-baseline gap-3 py-1.5 text-left hover:text-ink"
                      >
                        <span className="flex-1 text-[14px] text-ink">{t.title}</span>
                        <span className="tabular-nums text-[12px] text-ink-faint">{t.year ?? ""}</span>
                      </button>
                    </li>
                  ))}
                </ul>
              </section>
            )}
          </>
        )}
      </aside>
    </div>
  );
}
