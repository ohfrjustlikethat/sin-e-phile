import { useQuery } from "@tanstack/react-query";
import { commands, type TastePick } from "@/lib/ipc";
import { useArtworkOn } from "@/features/home/TitleCard";

/**
 * The films picked in "What do you like?", shown back in the steps after it (Phase 16, the author: "imagery from
 * taste picks"): their posters when posters are on, a set initial otherwise. Up to six, the latest first.
 */
export function PickStrip({ picks }: { picks: TastePick[] }) {
  if (picks.length === 0) return null;
  const shown = [...picks].reverse().slice(0, 6);
  return (
    <div aria-label="Your picks" className="flex flex-col items-end gap-2">
      <span className="label">Your picks · {picks.length}</span>
      <ul className="flex gap-1.5">
        {shown.map((p) => (
          <li key={p.id}>
            <Thumb pick={p} />
          </li>
        ))}
      </ul>
    </div>
  );
}

function Thumb({ pick }: { pick: TastePick }) {
  const on = useArtworkOn();
  const poster = useQuery({
    queryKey: ["poster", pick.id],
    queryFn: async () => {
      const r = await commands.poster(pick.id);
      return r.status === "ok" ? r.data : null;
    },
    enabled: on,
    staleTime: Infinity,
  });
  const title = [pick.title, pick.year].filter(Boolean).join(", ");
  if (on && poster.data) {
    return <img src={poster.data} alt={title} title={title} className="h-[72px] w-[54px] rounded-sm object-cover" />;
  }
  return (
    <span
      role="img"
      aria-label={title}
      title={title}
      className="grid h-[72px] w-[54px] place-items-center rounded-sm border border-line-subtle bg-surface font-serif text-[26px] text-ink"
    >
      {[...pick.title.trim()][0]?.toUpperCase() ?? "?"}
    </span>
  );
}
