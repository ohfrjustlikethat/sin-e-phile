import { useEffect, useRef, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { PosterCard, type PosterFilm } from "@/design-system";
import { commands } from "@/lib/ipc";

/**
 * A title's card with its poster, when the person chose a source for posters (ADR-0048): their TMDB key, or the
 * keyless poster service switched on. The poster is asked for only once the card is near the screen, once per
 * session; until it arrives, and whenever there is none, the designed text card stands (ADR-0013).
 */

export const ARTWORK = ["artwork-settings"] as const;

/** Whether this profile has any source of posters. One query for every card. */
export function useArtworkOn(): boolean {
  const s = useQuery({
    queryKey: ARTWORK,
    queryFn: async () => {
      const r = await commands.artworkSettings();
      return r.status === "ok" ? r.data : { posters: false, tmdb: false };
    },
    staleTime: 60_000,
  });
  return s.data ? s.data.posters || s.data.tmdb : false;
}

/** True once `el` comes within a screen of the viewport (and stays true). */
function useNear(el: React.RefObject<HTMLElement | null>): boolean {
  const [near, setNear] = useState(false);
  useEffect(() => {
    if (near) return;
    const node = el.current;
    if (!node || typeof IntersectionObserver === "undefined") {
      setNear(true);
      return;
    }
    const seen = new IntersectionObserver(
      (entries) => {
        if (entries.some((e) => e.isIntersecting)) setNear(true);
      },
      { rootMargin: "100% 100%" },
    );
    seen.observe(node);
    return () => seen.disconnect();
  }, [el, near]);
  return near;
}

export function TitleCard({
  id,
  film,
  size,
  onOpen,
}: {
  id: number;
  film: Omit<PosterFilm, "artwork">;
  size?: "sm" | "md" | "lead";
  onOpen: () => void;
}) {
  const on = useArtworkOn();
  const box = useRef<HTMLDivElement>(null);
  const near = useNear(box);
  const poster = useQuery({
    queryKey: ["poster", id],
    queryFn: async () => {
      const r = await commands.poster(id);
      return r.status === "ok" ? r.data : null;
    },
    enabled: on && near,
    staleTime: Infinity,
  });
  return (
    // `data-focus-key`: coming back to this page puts focus back on this card (src/lib/scrollMemory.ts).
    <div ref={box} data-focus-key={`title-${id}`}>
      <PosterCard
        size={size ?? "md"}
        film={on && poster.data ? { ...film, artwork: poster.data } : film}
        onOpen={onOpen}
      />
    </div>
  );
}
