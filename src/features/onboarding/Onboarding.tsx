import { useEffect, useRef, useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Button, Input } from "@/design-system";
import { commands } from "@/lib/ipc";
import { AVATAR_COLOURS, Avatar, colourOf, type AvatarColour } from "@/features/profiles/Avatar";
import { ImportHistory } from "@/features/profiles/ImportHistory";
import { TasteInput } from "@/features/profiles/TasteInput";
import { PROFILES, useProfiles } from "@/features/profiles/useProfiles";
import { LibrarySection } from "@/features/settings/LibrarySection";
import { SourcesSection } from "@/features/settings/SourcesSection";
import { TitleCards } from "./TitleCards";

/**
 * First run (SPEC.md Phase 14): three steps a non-technical person finishes in under two
 * minutes. 1, who this is for; 2, what they like (films to pick by name, statements that
 * are not genres, their own words, or an import); 3, where to watch (optional: the
 * Internet Archive works already, a folder of their own is one press). Every step can be
 * skipped and none is a dead end: skipping everything still lands on a Home with films on
 * it. No launch intro (ADR-0043): the first screen asks the first question.
 *
 * Pad: the D-pad moves, A chooses, B goes back a step.
 */

async function ok<T>(p: Promise<{ status: "ok"; data: T } | { status: "error"; error: string }>) {
  const r = await p;
  if (r.status === "error") throw new Error(r.error);
  return r.data;
}

const STEPS = ["Who's this for?", "What do you like?", "Where to watch"] as const;
type Way = "cards" | "words" | "import";

export function Onboarding({ onDone }: { onDone: () => void }) {
  const client = useQueryClient();
  const { active } = useProfiles();
  const [step, setStep] = useState(0);
  const [name, setName] = useState("");
  const [colour, setColour] = useState<AvatarColour>("sage");
  const [way, setWay] = useState<Way>("cards");
  const first = useRef<HTMLDivElement>(null);

  // The profile made on install is "You": its name is the first question.
  useEffect(() => {
    if (active && !name) {
      setName(active.name === "You" ? "" : active.name);
      setColour(colourOf(active.avatar_colour ?? "sage"));
    }
  }, [active]); // the name is filled once, from the profile

  // Each step starts with focus on its first control, for the pad.
  useEffect(() => {
    first.current?.querySelector<HTMLElement>("input, button, textarea")?.focus();
  }, [step, way]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const t = e.target as HTMLElement | null;
      if (e.key !== "Escape" || (t && ["INPUT", "TEXTAREA"].includes(t.tagName))) return;
      setStep((s) => Math.max(0, s - 1));
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const saveProfile = useMutation({
    mutationFn: async () => {
      if (active && name.trim()) await ok(commands.updateProfile(active.id, name.trim(), colour, active.is_kids));
    },
    onSuccess: () => {
      void client.invalidateQueries({ queryKey: PROFILES });
      setStep(1);
    },
  });

  const finish = useMutation({
    mutationFn: () => ok(commands.finishOnboarding()),
    onSuccess: () => {
      void client.invalidateQueries();
      onDone();
    },
  });

  const next = () => {
    if (step === 0) saveProfile.mutate();
    else if (step < STEPS.length - 1) setStep(step + 1);
    else finish.mutate();
  };

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label="Setting up sin·e·phile"
      className="fixed inset-0 z-50 overflow-auto bg-base text-ink"
    >
      <div className="mx-auto flex min-h-full max-w-[880px] flex-col px-10 py-14">
        <p className="label" aria-label={`Step ${step + 1} of ${STEPS.length}`}>
          {step + 1} of {STEPS.length}
        </p>
        <h1 className="mt-3 font-display text-[44px] font-extrabold leading-none tracking-[-0.03em]">
          {STEPS[step]}
        </h1>

        <div ref={first} className="mt-10 flex-1">
          {step === 0 && (
            <div className="flex flex-col gap-6">
              <div className="flex items-end gap-5">
                <Avatar name={name || "?"} colour={colour} size="lg" />
                <div className="w-80">
                  <Input
                    label="Your name"
                    placeholder="What should we call you?"
                    value={name}
                    maxLength={40}
                    onChange={(e) => setName(e.target.value)}
                  />
                </div>
              </div>
              <div role="radiogroup" aria-label="Colour" className="flex gap-2">
                {AVATAR_COLOURS.map((c) => (
                  <button
                    key={c}
                    type="button"
                    role="radio"
                    aria-checked={colour === c}
                    aria-label={c}
                    onClick={() => setColour(c)}
                    className={`rounded-sm p-0.5 ${colour === c ? "outline outline-1 outline-ink" : ""}`}
                  >
                    <Avatar name={name || "?"} colour={c} />
                  </button>
                ))}
              </div>
              <p className="max-w-[60ch] text-[13px] leading-relaxed text-ink-muted">
                Everyone in the house can have their own profile later, each with their own taste, history and
                watchlist.
              </p>
            </div>
          )}

          {step === 1 && (
            <div className="flex flex-col gap-6">
              <div className="flex gap-2" role="tablist" aria-label="Ways to tell us">
                {(
                  [
                    ["cards", "Pick films you love"],
                    ["words", "Say what you like"],
                    ["import", "Import from Letterboxd, Trakt, IMDb"],
                  ] as const
                ).map(([w, label]) => (
                  <Button
                    key={w}
                    size="sm"
                    variant={way === w ? "secondary" : "ghost"}
                    role="tab"
                    aria-selected={way === w}
                    onClick={() => setWay(w)}
                  >
                    {label}
                  </Button>
                ))}
              </div>
              {way === "cards" && <TitleCards />}
              {way === "words" && <TasteInput />}
              {way === "import" && <ImportHistory />}
            </div>
          )}

          {step === 2 && (
            <div className="flex flex-col gap-2">
              <p className="mb-6 max-w-[62ch] text-[13px] leading-relaxed text-ink-muted">
                Optional. Films from the Internet Archive play already, and anything on this PC can be added from a
                folder. A source someone gave you is an address to paste. You can do all of this later in Settings.
              </p>
              <LibrarySection />
              <SourcesSection />
            </div>
          )}
        </div>

        {/* Always on screen: a long step scrolls, and Continue never goes below the fold. */}
        <div className="sticky bottom-0 mt-12 flex items-center gap-3 border-t border-line-subtle bg-base py-6">
          {step > 0 && (
            <Button variant="ghost" onClick={() => setStep(step - 1)}>
              Back
            </Button>
          )}
          <span className="flex-1" />
          <Button variant="ghost" onClick={() => (step === STEPS.length - 1 ? finish.mutate() : setStep(step + 1))}>
            {step === STEPS.length - 1 ? "Skip, and start" : "Skip"}
          </Button>
          <Button variant="primary" onClick={next} disabled={saveProfile.isPending || finish.isPending}>
            {step === STEPS.length - 1 ? "Start watching" : "Continue"}
          </Button>
        </div>
      </div>
    </div>
  );
}
