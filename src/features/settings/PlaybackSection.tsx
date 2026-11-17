import { Toggle } from "@/design-system";
import { qualityLabel, usePlaybackChoice } from "@/lib/playbackChoice";
import { ChoiceRow } from "@/features/detail/ChoiceRow";
import { SETTINGS_HEADING } from "./heading";

/**
 * Settings → Playback (SPEC.md Phase 9): the expert panel's switch. Ctrl+Shift+S on a
 * film page shows it for a moment; this keeps it on.
 */
export function PlaybackSection() {
  const details = usePlaybackChoice((s) => s.details);
  const setDetails = usePlaybackChoice((s) => s.setDetails);
  const autoplay = usePlaybackChoice((s) => s.autoplay);
  const setAutoplay = usePlaybackChoice((s) => s.setAutoplay);
  const preferred = usePlaybackChoice((s) => s.preferred);
  const setPreferred = usePlaybackChoice((s) => s.setPreferred);
  return (
    <section className="mb-9" aria-label="Playback">
      <h2 className={SETTINGS_HEADING}>
        Playback
      </h2>
      <ChoiceRow
        label="Preferred quality"
        choices={[
          { value: null, label: "Best for this PC" },
          ...["uhd", "full_hd", "hd", "sd"].map((q) => ({ value: q, label: qualityLabel(q) })),
        ]}
        value={preferred}
        onChange={setPreferred}
      />
      <p className="mt-2 mb-5 max-w-[62ch] text-[13px] text-ink-muted">
        Play picks the healthiest stream at this quality, then the nearest one that will start smoothly. 4K is
        picked only when it has a strong swarm behind it.
      </p>
      <Toggle
        checked={details}
        onChange={setDetails}
        label="Show source details on film pages"
        description="Every copy your sources found, ranked, with each part of its score. Ctrl+Shift+S shows it once."
      />
      <Toggle
        checked={autoplay}
        onChange={setAutoplay}
        label="Play the next episode automatically"
        description="A ten-second countdown at the end of an episode; any button stops it."
      />
    </section>
  );
}
