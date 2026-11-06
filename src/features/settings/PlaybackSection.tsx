import { Toggle } from "@/design-system";
import { usePlaybackChoice } from "@/lib/playbackChoice";

/**
 * Settings → Playback (SPEC.md Phase 9): the expert panel's switch. Ctrl+Shift+S on a
 * film page shows it for a moment; this keeps it on.
 */
export function PlaybackSection() {
  const details = usePlaybackChoice((s) => s.details);
  const setDetails = usePlaybackChoice((s) => s.setDetails);
  return (
    <section className="mb-9" aria-label="Playback">
      <h2 className="mb-3.5 text-[11px] font-semibold uppercase tracking-[0.12em] text-ink-faint">
        Playback
      </h2>
      <Toggle
        checked={details}
        onChange={setDetails}
        label="Show source details on film pages"
        description="Every copy your sources found, ranked, with each part of its score. Ctrl+Shift+S shows it once."
      />
    </section>
  );
}
