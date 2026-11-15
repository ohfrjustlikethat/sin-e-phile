import { useQuery } from "@tanstack/react-query";
import { create } from "zustand";
import { persist } from "zustand/middleware";
import { commands } from "@/lib/ipc";
import { openSettingsAt } from "@/lib/openSettings";
import { profileStorage } from "@/lib/profileStorage";
import { setupLines, setupProgress, showSetup } from "./setup";

/**
 * "Finish setting up", at the top of Home until it is done or hidden (SPEC.md Appendix F,
 * I38). Each line opens the Settings section that settles it. The rules are in `setup.ts`.
 */

async function ok<T>(p: Promise<{ status: "ok"; data: T } | { status: "error"; error: string }>) {
  const r = await p;
  if (r.status === "error") throw new Error(r.error);
  return r.data;
}

/** Hidden by the viewer: each profile its own, on this machine. */
export const useSetupCard = create<{ hidden: boolean; hide: () => void }>()(
  persist((set) => ({ hidden: false, hide: () => set({ hidden: true }) }), {
    name: "sinephile.setup-card",
    storage: profileStorage,
  }),
);

export function SetupCard() {
  const hidden = useSetupCard((s) => s.hidden);
  const hide = useSetupCard((s) => s.hide);
  // The same keys as the Settings sections, so a change there is seen here at once.
  const folders = useQuery({ queryKey: ["library-folders"], queryFn: () => ok(commands.libraryFolders()) });
  const sources = useQuery({ queryKey: ["sources"], queryFn: () => ok(commands.sourcesOverview()) });
  const subtitles = useQuery({ queryKey: ["subtitle-settings"], queryFn: () => ok(commands.subtitleSettings()) });
  const network = useQuery({ queryKey: ["torrent-settings"], queryFn: () => ok(commands.torrentSettings()) });
  // Until every fact is in, say nothing: a card that flashes "0 of 4" and then goes is noise.
  if (!folders.data || !sources.data || !subtitles.data || !network.data) return null;

  const lines = setupLines({
    folders: folders.data.length,
    addons: sources.data.sources.filter((s) => s.addon_host !== null).length,
    subtitlesKey: subtitles.data.has_key,
    adapterBound: network.data.bind_interface !== null,
  });
  if (!showSetup(lines, hidden)) return null;

  return (
    <section aria-label="Finish setting up" className="grid" style={{ gridTemplateColumns: "var(--index-col) 1fr" }}>
      <div className="border-r border-line-subtle" />
      <div className="min-w-0 pt-2 pr-14 pl-8">
        <header className="flex items-baseline gap-4">
          <h2 className="font-display text-[15px] font-extrabold uppercase tracking-[-0.01em] text-ink">
            Finish setting up
          </h2>
          <span className="h-px flex-1 bg-line-subtle" />
          <span className="label">{setupProgress(lines)}</span>
          <button type="button" className="label hover:text-ink" onClick={hide}>
            Hide
          </button>
        </header>
        <ul className="mt-4 max-w-[900px] rounded-sm border border-line-subtle bg-surface">
          {lines.map((l) => (
            <li key={l.key} className="border-b border-line-subtle last:border-b-0">
              <button
                type="button"
                onClick={() => openSettingsAt(l.section)}
                className="grid w-full grid-cols-[1.5rem_14rem_1fr_auto] items-baseline gap-x-4 px-4 py-3 text-left hover:bg-raised"
              >
                <span aria-hidden className={l.done ? "text-success" : "text-ink-faint"}>
                  {l.done ? "✓" : "·"}
                </span>
                <span className={`text-[15px] ${l.done ? "text-ink-muted" : "text-ink"}`}>{l.title}</span>
                <span className="text-[13px] leading-relaxed text-ink-muted">{l.detail}</span>
                <span className="label text-ink-faint">{l.done ? "Done" : l.optional ? "Optional" : "Set up"}</span>
              </button>
            </li>
          ))}
        </ul>
      </div>
    </section>
  );
}
