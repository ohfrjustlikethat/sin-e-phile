import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Toggle } from "@/design-system";
import { commands } from "@/lib/ipc";
import { toast } from "@/lib/toast";
import { ARTWORK } from "@/features/home/TitleCard";
import { SETTINGS_HEADING } from "./heading";

/**
 * Posters (ADR-0048, the author's "Artwork: both"): the keyless poster service, off unless switched on here or in
 * the first run, said plainly for what it sends; and TMDB's posters, which come with the profile's own key (the
 * TMDB section below). Without either, the cards are set in type, as designed.
 */
export function ArtworkSection() {
  const client = useQueryClient();
  const s = useQuery({
    queryKey: ARTWORK,
    queryFn: async () => {
      const r = await commands.artworkSettings();
      if (r.status === "error") throw new Error(r.error);
      return r.data;
    },
  });
  const set = useMutation({
    mutationFn: async (on: boolean) => {
      const r = await commands.setShowPosters(on);
      if (r.status === "error") throw new Error(r.error);
    },
    onSuccess: (_, on) => {
      void client.invalidateQueries({ queryKey: ARTWORK });
      void client.invalidateQueries({ queryKey: ["poster"] });
      toast(on ? "Posters on: they appear as titles come into view." : "Posters off: titles are set in type.");
    },
    onError: (e) => toast(`Not changed: ${e.message}`, "danger"),
  });
  return (
    <section aria-label="Posters" className="mb-12">
      <h2 className={SETTINGS_HEADING}>Posters</h2>
      <Toggle
        checked={s.data?.posters ?? false}
        disabled={!s.data || set.isPending}
        onChange={(on) => set.mutate(on)}
        label="Show posters"
        description={
          "Asks Stremio's poster images (images.metahub.space) for each title as it comes into view, by its IMDb " +
          "id: that service sees which titles you look at, and nothing else. Kept on this PC after the first time." +
          (s.data?.tmdb ? " Your TMDB key's posters come first." : "")
        }
      />
    </section>
  );
}
