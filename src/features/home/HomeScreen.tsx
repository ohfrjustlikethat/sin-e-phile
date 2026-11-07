import { useQuery } from "@tanstack/react-query";
import { PosterCard, Rail, Skeleton } from "@/design-system";
import { commands, type HomeCard, type RailView } from "@/lib/ipc";
import { useUi } from "@/lib/store";

/**
 * Home, first version (SPEC.md Phase 14; Phase 18 overhauls it). Continue watching, where
 * your taste points, your watchlist, and what is widely loved: the last needs only the
 * offline catalogue, so Home is never empty. Typographic title cards: a default install has
 * no artwork, and §9.4 makes that state the designed one. The first rail leads with a
 * larger card, so the page is not a grid of identical tiles (§9.0).
 */

async function ok<T>(p: Promise<{ status: "ok"; data: T } | { status: "error"; error: string }>) {
  const r = await p;
  if (r.status === "error") throw new Error(r.error);
  return r.data;
}

export const HOME = ["home"];

const WHY: Record<string, string> = {
  continue: "Where you left off",
  taste: "From what you liked and what you said you love",
  watchlist: "What you saved",
  loved: "The films most people have seen and rated",
};

export function HomeScreen() {
  const home = useQuery({ queryKey: HOME, queryFn: () => ok(commands.home()) });
  if (home.isPending) {
    return (
      <div className="flex flex-col gap-10 px-16 pt-12">
        <Skeleton className="h-6 w-56" />
        <Skeleton className="h-[260px] w-full" />
      </div>
    );
  }
  if (home.isError) {
    return <p className="px-16 pt-12 text-sm text-ink-muted">{home.error.message}</p>;
  }
  return (
    <div className="flex flex-col gap-12 pt-10 pb-24">
      {home.data.map((r, i) => (
        <HomeRail key={r.key} rail={r} lead={i === 0} />
      ))}
    </div>
  );
}

function HomeRail({ rail, lead }: { rail: RailView; lead: boolean }) {
  const openDetail = useUi((s) => s.openDetail);
  return (
    <section aria-label={rail.title} className="px-16">
      <Rail<HomeCard>
        label={rail.title}
        why={WHY[rail.key] ?? ""}
        items={rail.items}
        keyOf={(c) => String(c.id)}
        itemWidth={194}
        leadWidth={lead ? 254 : 194}
        render={(c, i) => (
          <PosterCard
            size={lead && i === 0 ? "lead" : "md"}
            film={{
              id: String(c.id),
              title: c.title,
              year: c.year ?? 0,
              director: c.made_by ?? "",
              // Its place in the rail: № 001 is the first widely loved, not "Uncatalogued".
              spine: i + 1,
            }}
            onOpen={() => openDetail(c.id)}
          />
        )}
      />
    </section>
  );
}
