import { useEffect, useRef } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Button } from "@/design-system";
import { commands } from "@/lib/ipc";
import { useUi } from "@/lib/store";
import { size } from "@/features/downloads/format";
import { DOWNLOADS } from "@/features/downloads/useDownloads";

/**
 * Stream or download? — SPEC.md Phase 13, under Play on a film's page.
 *
 * The advisor measures the swarm the page already started (about three seconds), then
 * says which is better in one sentence. The other choice is always one press away: Play
 * stays the page's one primary button, and Download sits beside the sentence whatever it
 * recommends. Nothing shows for a copy that is not a torrent (a web stream is not
 * downloaded) or one already on this PC.
 */

async function ok<T>(p: Promise<{ status: "ok"; data: T } | { status: "error"; error: string }>) {
  const r = await p;
  if (r.status === "error") throw new Error(r.error);
  return r.data;
}

const DOING: Record<string, string> = {
  queued: "Queued to download",
  active: "Downloading",
  paused: "Download paused",
  done: "Downloaded",
};

export function DownloadAdvice({ id, plan }: { id: number; plan: string }) {
  const client = useQueryClient();
  const setDownloadsOpen = useUi((s) => s.setDownloadsOpen);
  const key = ["advice", id, plan];
  const advice = useQuery({ queryKey: key, queryFn: () => ok(commands.downloadAdvice(id)), staleTime: 60_000 });
  const add = useMutation({
    mutationFn: (tonight: boolean) => ok(commands.downloadTitle(id, tonight, advice.data?.sentence ?? null)),
    onSuccess: () => {
      void client.invalidateQueries({ queryKey: key });
      void client.invalidateQueries({ queryKey: DOWNLOADS });
    },
  });

  // Download pressed: its button gives way to "See downloads", and focus follows rather
  // than falling to the page, so a pad user is still somewhere (ADR-0041).
  const see = useRef<HTMLButtonElement>(null);
  const queued = advice.data?.download != null;
  useEffect(() => {
    const at = document.activeElement;
    if (queued && (!at || at === document.body)) see.current?.focus();
  }, [queued]);

  if (advice.isPending) {
    return (
      <p className="mt-3 text-[13px] text-ink-faint" role="status">
        Measuring how well it would stream…
      </p>
    );
  }
  const a = advice.data;
  if (!a || a.choice === "play-local") return null;

  if (a.download) {
    return (
      <div className="mt-4 flex items-center gap-4">
        <p className="text-[13px] text-ink-muted" role="status">
          {DOING[a.download.state] ?? "Downloading"}
          {a.download.state === "done" ? " to this PC." : "."}
        </p>
        <Button ref={see} size="sm" variant="ghost" onClick={() => setDownloadsOpen(true)}>
          See downloads
        </Button>
      </div>
    );
  }

  const tonight = a.choice === "download-tonight";
  return (
    <div className="mt-4 max-w-[72ch]">
      <p className="text-[13px] text-ink-muted" aria-label="Stream or download">
        {a.sentence}
      </p>
      <div className="mt-3 flex items-center gap-2">
        {tonight && (
          <Button size="sm" onClick={() => add.mutate(true)} disabled={add.isPending}>
            Download tonight
          </Button>
        )}
        <Button
          size="sm"
          variant={tonight || a.choice === "stream" ? "ghost" : "secondary"}
          onClick={() => add.mutate(false)}
          disabled={add.isPending}
        >
          {tonight ? "Download now" : "Download"}
          {a.size_bytes ? ` · ${size(a.size_bytes)}` : ""}
        </Button>
        {add.isError && <span className="text-xs text-danger">{add.error.message}</span>}
      </div>
    </div>
  );
}
