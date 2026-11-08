import { useEffect, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Button, Input, Select, Slider, Toggle } from "@/design-system";
import { commands, type SubtitleSettings, type SubtitleSettingsView } from "@/lib/ipc";
import { languageName } from "@/lib/playbackChoice";

/**
 * Settings → Subtitles — SPEC.md Phase 10.
 *
 * Which languages you read, whether subtitles show when the film is already in one of
 * them, how plain subtitles look (an ASS file keeps its own styling), and OpenSubtitles:
 * your own key, and how much of today's small download quota is left.
 */

async function ok<T>(p: Promise<{ status: "ok"; data: T } | { status: "error"; error: string }>) {
  const r = await p;
  if (r.status === "error") throw new Error(r.error);
  return r.data;
}

const KEY = ["subtitle-settings"];
const FONTS = ["Segoe UI", "Arial", "Verdana", "Georgia", "Calibri"].map((f) => ({ value: f, label: f }));

/** "en, French, de" → ["en", "fr", "de"]: codes or English names, in order. */
export function parseLanguages(text: string): string[] {
  const byName = new Map<string, string>();
  for (const code of ["en", "fr", "de", "es", "it", "ja", "hi", "pt", "nl", "ko", "zh", "ru", "ar", "sv", "pl", "tr"]) {
    byName.set(languageName(code).toLowerCase(), code);
  }
  const out: string[] = [];
  for (const raw of text.split(/[,\s]+/)) {
    const t = raw.trim().toLowerCase();
    if (!t) continue;
    const code = byName.get(t) ?? (/^[a-z]{2,3}$/.test(t) ? t : null);
    if (code && !out.includes(code)) out.push(code);
  }
  return out;
}

export function SubtitlesSection() {
  const client = useQueryClient();
  const view = useQuery({ queryKey: KEY, queryFn: () => ok(commands.subtitleSettings()) });
  if (!view.data) {
    return (
      <Section title="Subtitles">
        <p className="text-sm text-ink-muted">
          {view.isError ? String(view.error.message) : "Reading subtitle settings…"}
        </p>
      </Section>
    );
  }
  return <Form view={view.data} onSaved={() => client.invalidateQueries({ queryKey: KEY })} />;
}

function Form({ view, onSaved }: { view: SubtitleSettingsView; onSaved: () => void }) {
  const [s, setS] = useState<SubtitleSettings>(view.settings);
  const [languages, setLanguages] = useState(view.settings.languages.join(", "));
  const [saved, setSaved] = useState(false);
  useEffect(() => setSaved(false), [s, languages]);
  const set = (patch: Partial<SubtitleSettings>) => setS((old) => ({ ...old, ...patch }));

  const save = useMutation({
    mutationFn: () => ok(commands.setSubtitleSettings({ ...s, languages: parseLanguages(languages) })),
    onSuccess: () => {
      setSaved(true);
      onSaved();
    },
  });
  const parsed = parseLanguages(languages);

  return (
    <Section title="Subtitles">
      <Input
        label="Languages you read, in order"
        value={languages}
        onChange={(e) => setLanguages(e.target.value)}
        hint={parsed.length ? parsed.map((c) => languageName(c)).join(" → ") : "None: subtitles stay off"}
      />
      <div className="mt-3">
        <Toggle
          checked={s.always}
          onChange={(always) => set({ always })}
          label="Show subtitles even when the film is in one of these"
          description="Off: a film in a language you read plays without them, and a foreign one gets them."
        />
      </div>

      <div className="mt-4 grid grid-cols-2 gap-x-6 gap-y-3">
        <Select label="Font" options={FONTS} value={s.font} onChange={(e) => set({ font: e.target.value })} />
        <Toggle checked={s.background} onChange={(background) => set({ background })} label="Dark box behind the text" />
        <Slider label="Size" min={30} max={80} value={s.size} onChange={(size) => set({ size })} display={String(s.size)} />
        <Slider
          label="Height"
          min={60}
          max={100}
          value={s.position}
          onChange={(position) => set({ position })}
          display={s.position >= 98 ? "Bottom" : `${100 - s.position}% up`}
        />
      </div>
      <p className="mt-1.5 text-[11px] leading-relaxed text-ink-faint">
        For plain subtitles. A styled ASS/SSA file keeps the look it was made with. G and H (or the
        bumpers) move subtitles a tenth of a second while watching, and the film remembers it.
      </p>

      <div className="mt-4 flex items-center gap-3">
        <Button variant="secondary" onClick={() => save.mutate()} disabled={save.isPending}>
          {save.isPending ? "Saving…" : "Save"}
        </Button>
        {save.isError && <span className="text-xs text-danger">{save.error.message}</span>}
        {saved && (
          <span className="text-xs text-success" role="status">
            Saved.
          </span>
        )}
      </div>

      <OpenSubtitlesKey view={view} onChanged={onSaved} />
    </Section>
  );
}

function OpenSubtitlesKey({ view, onChanged }: { view: SubtitleSettingsView; onChanged: () => void }) {
  const [key, setKey] = useState("");
  const add = useMutation({
    mutationFn: () => ok(commands.setOpensubtitlesKey(key)),
    onSuccess: () => {
      setKey("");
      onChanged();
    },
  });
  const remove = useMutation({ mutationFn: () => ok(commands.clearOpensubtitlesKey()), onSuccess: onChanged });
  const left = Math.max(0, Math.min(view.budget - view.used_today, view.server_remaining ?? view.budget));

  return (
    <div className="mt-6 border-t border-line-subtle pt-4">
      <p className="text-[13px] text-ink">OpenSubtitles</p>
      <p className="mt-0.5 mb-3 text-[11px] leading-relaxed text-ink-faint">
        Only when a film has no subtitles of its own, or beside it, in your languages. Uses your own
        free account&apos;s key; none comes with the app.
      </p>
      {view.has_key ? (
        <div className="flex items-center gap-3">
          <span className="text-sm text-ink-muted" role="status">
            Your key is saved. {left} of {view.budget} downloads left today.
          </span>
          <Button variant="ghost" size="sm" onClick={() => remove.mutate()}>
            Remove key
          </Button>
        </div>
      ) : (
        <>
        <p className="mb-3 text-[12px] text-ink-muted">
          A free OpenSubtitles account gives you a key in a minute.{" "}
          <button type="button" className="underline hover:text-ink" onClick={() => void commands.openSignUp("opensubtitles")}>
            Get a free key
          </button>
          , then paste it here. It is checked as you save it.
        </p>
        <div className="flex items-end gap-3">
          <div className="flex-1">
            <Input
              label="API key"
              value={key}
              onChange={(e) => setKey(e.target.value)}
              {...(add.isError ? { error: add.error.message } : {})}
              autoComplete="off"
              spellCheck={false}
            />
          </div>
          <Button variant="secondary" onClick={() => add.mutate()} disabled={!key.trim() || add.isPending}>
            Save key
          </Button>
        </div>
        </>
      )}
    </div>
  );
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <section className="mb-9" aria-label={title}>
      <h2 className="mb-3.5 text-[11px] font-semibold uppercase tracking-[0.12em] text-ink-faint">{title}</h2>
      {children}
    </section>
  );
}
