import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands, type Capability, type HardwareProfile, type Tier } from "@/lib/ipc";
import { useUi } from "@/lib/store";

/**
 * Settings — SPEC.md Phase 1, subtask 1.9.
 *
 * §8: "The Settings screen shows the detected tier and exactly which features it
 * enables, in plain language. **This is a nice UI moment, not an apology.**"
 *
 * So this screen states what the machine is and what that means, without hedging
 * and without implying the user has bought the wrong computer.
 */
export function SettingsScreen() {
  const setSettingsOpen = useUi((s) => s.setSettingsOpen);
  const queryClient = useQueryClient();

  const profile = useQuery({
    queryKey: ["hardware-profile"],
    queryFn: () => commands.getHardwareProfile(),
  });

  const dataDir = useQuery({
    queryKey: ["data-dir"],
    queryFn: () => commands.getDataDir(),
  });

  const override = useMutation({
    mutationFn: (tier: Tier | null) => commands.setTierOverride(tier),
    onSuccess: (updated) => queryClient.setQueryData(["hardware-profile"], updated),
  });

  return (
    <div className="mx-auto max-w-3xl px-10 py-10">
      <header className="mb-9 flex items-start justify-between">
        <div>
          <h1 className="text-[26px] font-semibold tracking-[-0.02em]">Settings</h1>
          <p className="mt-1 text-sm text-ink-muted">
            Everything here stays on this machine.
          </p>
        </div>
        <button
          type="button"
          onClick={() => setSettingsOpen(false)}
          className="rounded-md border border-line px-3.5 py-2 text-sm text-ink-muted transition-colors hover:bg-raised hover:text-ink"
        >
          Done
        </button>
      </header>

      {profile.isPending && <Skeleton />}
      {profile.isError && (
        <p className="text-sm text-danger">
          Could not read the hardware profile: {String(profile.error)}
        </p>
      )}
      {profile.data && (
        <HardwareSection
          profile={profile.data}
          onOverride={(t) => override.mutate(t)}
          pending={override.isPending}
        />
      )}

      <CapabilitiesSection />

      <Section title="Data">
        <Row label="Location">
          <code className="text-xs text-ink-muted">{dataDir.data ?? "…"}</code>
        </Row>
        <p className="mt-3 text-xs leading-relaxed text-ink-faint">
          All application data lives beside the executable. Move the folder to another
          machine or a USB stick and everything comes with it — history, watchlist and
          taste model included.
        </p>
      </Section>

      <Section title="Privacy">
        <p className="text-xs leading-relaxed text-ink-faint">
          No telemetry, no analytics, no crash reporting to a server. Crash reports are
          written to <code className="text-ink-muted">data/crashes/</code> and sent nowhere.
          The only network requests are to services you configure yourself, and the
          application works with no API key at all.
        </p>
      </Section>
    </div>
  );
}

/** §8: Settings shows "exactly which features it enables, in plain language". */
const CAPABILITY_COPY: Array<{ id: Capability; label: string; fallback: string }> = [
  {
    id: "local_document_embedding",
    label: "Embed the catalogue on this machine",
    fallback: "Embeddings are downloaded instead — search works the same.",
  },
  {
    id: "vad_subtitle_alignment",
    label: "Align subtitles against the audio",
    fallback: "Subtitles are matched by hash, with a nudge remembered per file.",
  },
  {
    id: "binge_detection",
    label: "Detect intros and credits",
    fallback: "Next episode still autoplays, just without skip-intro.",
  },
  {
    id: "high_res_playback",
    label: "Play above 1080p by default",
    fallback: "Capped at 1080p by default. You can override this per source.",
  },
  {
    id: "face_recognition",
    label: "Recognise faces in the pause overlay",
    fallback: "The pause overlay shows the full cast list instead.",
  },
  {
    id: "local_transcription",
    label: "Generate subtitles locally",
    fallback: "Subtitles come from the file, sidecars, or online providers.",
  },
  {
    id: "background_pre_embedding",
    label: "Pre-embed the catalogue in the background",
    fallback: "Embedding happens on demand, which is slightly slower to warm up.",
  },
  {
    id: "full_motion",
    label: "Full motion design",
    fallback: "Hover previews and background effects are reduced to keep 60fps.",
  },
];

/**
 * Reads capabilities through `has_capability` — the ONLY sanctioned way to ask
 * about hardware (§8). Note what this section does NOT do: it never says
 * "unavailable" and stops. Every disabled row states what happens instead,
 * because §8 requires each gated feature to degrade to something good.
 */
function CapabilitiesSection() {
  const caps = useQuery({
    queryKey: ["capabilities"],
    queryFn: async () => {
      const entries = await Promise.all(
        CAPABILITY_COPY.map(async (c) => [c.id, await commands.hasCapability(c.id)] as const),
      );
      return Object.fromEntries(entries) as Record<Capability, boolean>;
    },
  });

  return (
    <Section title="What this machine enables">
      {CAPABILITY_COPY.map((c) => {
        const on = caps.data?.[c.id] ?? false;
        return (
          <div key={c.id} className="border-b border-line-subtle py-3 last:border-b-0">
            <div className="flex items-baseline justify-between gap-4 text-sm">
              <span className={on ? "text-ink" : "text-ink-muted"}>{c.label}</span>
              <span className={on ? "shrink-0 text-success" : "shrink-0 text-ink-faint"}>
                {on ? "on" : "off"}
              </span>
            </div>
            {!on && (
              <p className="mt-1 text-xs leading-relaxed text-ink-faint">{c.fallback}</p>
            )}
          </div>
        );
      })}
    </Section>
  );
}

const TIER_COPY: Record<Tier, { name: string; blurb: string }> = {
  modest: {
    name: "Modest",
    blurb:
      "The full core experience. Software decode, 1080p by default, subtitles matched by hash with a nudge you only have to make once per file.",
  },
  standard: {
    name: "Standard",
    blurb:
      "Everything, plus 4K playback, subtitles aligned automatically against the audio, intro detection, and the full motion design.",
  },
  capable: {
    name: "Capable",
    blurb:
      "Everything, plus faces recognised in the pause overlay, optional locally-generated subtitles, and the catalogue embedded in the background.",
  },
};

const TIERS: Tier[] = ["modest", "standard", "capable"];

function HardwareSection({
  profile, onOverride, pending,
}: {
  profile: HardwareProfile;
  onOverride: (tier: Tier | null) => void;
  pending: boolean;
}) {
  const effective = TIER_COPY[profile.effective_tier];

  return (
    <>
      <Section title="Your machine">
        <div className="mb-5 rounded-xl border border-line bg-surface p-5">
          <div className="flex items-baseline gap-2.5">
            <span className="text-lg font-semibold text-ink">{effective.name}</span>
            {profile.overridden && (
              <span className="rounded-full border border-line px-2 py-0.5 text-[11px] text-ink-faint">
                set by you — detected {TIER_COPY[profile.detected_tier].name}
              </span>
            )}
          </div>
          <p className="mt-2 text-sm leading-relaxed text-ink-muted">{effective.blurb}</p>
        </div>

        <Row label="Processor">
          {profile.cpu_brand}
          <span className="text-ink-faint">
            {" "}
            · {profile.physical_cores} cores / {profile.logical_cores} threads
          </span>
        </Row>
        <Row label="Memory">{(profile.total_memory_mb / 1024).toFixed(1)} GB</Row>
        <Row label="Graphics">{profile.gpu_name ?? "not detected"}</Row>
        <Row label="Hardware video decode">
          {profile.hardware_decode ? (
            <span className="text-success">available</span>
          ) : (
            <span className="text-ink-muted">not available — using software decode</span>
          )}
        </Row>
      </Section>

      <Section title="Override the tier">
        <p className="mb-4 text-xs leading-relaxed text-ink-faint">
          Detection is deliberately cautious, so it can land a tier low. Change it if you
          disagree, or to see how the app behaves on weaker hardware.
        </p>
        <div className="flex flex-wrap gap-2">
          <OverrideButton
            active={!profile.overridden}
            disabled={pending}
            onClick={() => onOverride(null)}
          >
            Detect automatically
          </OverrideButton>
          {TIERS.map((t) => (
            <OverrideButton
              key={t}
              active={profile.overridden && profile.effective_tier === t}
              disabled={pending}
              onClick={() => onOverride(t)}
            >
              {TIER_COPY[t].name}
            </OverrideButton>
          ))}
        </div>
      </Section>
    </>
  );
}

function OverrideButton({
  active, disabled, onClick, children,
}: {
  active: boolean;
  disabled: boolean;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      aria-pressed={active}
      className={[
        "rounded-lg border px-4 py-2 text-sm transition-colors disabled:opacity-50",
        active
          ? "border-oxblood-bright bg-[var(--oxblood-wash)] text-ink"
          : "border-line text-ink-muted hover:bg-raised hover:text-ink",
      ].join(" ")}
    >
      {children}
    </button>
  );
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <section className="mb-9">
      <h2 className="mb-3.5 text-[11px] font-semibold uppercase tracking-[0.12em] text-ink-faint">
        {title}
      </h2>
      {children}
    </section>
  );
}

function Row({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="flex items-baseline justify-between gap-6 border-b border-line-subtle py-2.5 text-sm last:border-b-0">
      <span className="shrink-0 text-ink-muted">{label}</span>
      <span className="min-w-0 truncate text-right text-ink">{children}</span>
    </div>
  );
}

function Skeleton() {
  return (
    <div className="space-y-3" aria-busy>
      {[0, 1, 2].map((i) => (
        <div key={i} className="h-11 animate-pulse rounded-lg bg-surface" />
      ))}
    </div>
  );
}
