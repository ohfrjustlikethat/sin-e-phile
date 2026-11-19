import { Rail } from "@/design-system";
import { TitleCard } from "./TitleCard";
import { RecommendedCard } from "./RecommendedCard";
import type { HomeCard, RailView } from "@/lib/ipc";
import { useUi } from "@/lib/store";

/** Rails of recommendations, whose cards can be told "not for me" (Phase 18). */
const RECOMMENDED = new Set(["taste", "taste_more", "stretch", "bridge", "blind_spot", "canon", "deep_cut"]);

/**
 * One rail of title cards: Home's, and since Phase 16 the catalogue's under Films and TV Shows. `why` is the small
 * line under the rail's name ("the most voted on IMDb").
 */
export function TitleRail({ rail, lead, why }: { rail: RailView; lead: boolean; why: string }) {
  const openDetail = useUi((s) => s.openDetail);
  const recommended = RECOMMENDED.has(rail.key);
  return (
    // No padding of its own: the rail's grid holds the page margin and the index column
    // (ADR-0024). Padding here as well pushed the column in and cut the rail short (Phase 16).
    <section aria-label={rail.title}>
      <Rail<HomeCard>
        label={rail.title}
        why={why}
        items={rail.items}
        keyOf={(c) => String(c.id)}
        itemWidth={194}
        leadWidth={lead ? 254 : 194}
        render={(c, i) => {
          const card = {
            id: c.id,
            size: lead && i === 0 ? ("lead" as const) : ("md" as const),
            film: {
              id: String(c.id),
              title: c.title,
              year: c.year ?? 0,
              // Why it is here (a recommendation's reason, a new episode's number); otherwise who made it.
              director: c.note ?? c.made_by ?? "",
              // Its place in the rail: № 001 is the first widely loved, not "Uncatalogued".
              spine: i + 1,
              // Continue watching: how far into the film, or the show's episode (Phase 16).
              ...(c.progress !== null ? { progress: c.progress } : {}),
            },
            onOpen: () => openDetail(c.id),
          };
          return recommended ? <RecommendedCard title={c.title} {...card} /> : <TitleCard {...card} />;
        }}
      />
    </section>
  );
}
