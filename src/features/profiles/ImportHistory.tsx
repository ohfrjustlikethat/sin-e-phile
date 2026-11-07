import { useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Button, Input } from "@/design-system";
import { commands, type ImportView } from "@/lib/ipc";

/**
 * Import your history (SPEC.md Phase 14): a Letterboxd, Trakt, IMDb or MyAnimeList
 * export, recorded for the profile watching. Windows' file picker, or a path typed in (a
 * pad user cannot drive the native dialog; every field takes the TV keyboard). What could
 * not be placed is listed in words, never guessed at.
 */

async function ok<T>(p: Promise<{ status: "ok"; data: T } | { status: "error"; error: string }>) {
  const r = await p;
  if (r.status === "error") throw new Error(r.error);
  return r.data;
}

/** "Letterboxd: 592 of 600 films found. 345 ratings, 543 watches and 49 on your watchlist." */
export function summary(v: ImportView): string {
  const parts = [
    v.ratings ? `${v.ratings} rating${v.ratings === 1 ? "" : "s"}` : null,
    v.watches ? `${v.watches} watch${v.watches === 1 ? "" : "es"}` : null,
    v.watchlist ? `${v.watchlist} on your watchlist` : null,
  ].filter(Boolean) as string[];
  const recorded =
    parts.length === 0
      ? "Nothing new to record."
      : `${parts.length > 1 ? `${parts.slice(0, -1).join(", ")} and ${parts.at(-1)}` : parts[0]}.`;
  return `${v.from}: ${v.matched} of ${v.entries} found. ${recorded[0]?.toUpperCase()}${recorded.slice(1)}`;
}

export function ImportHistory({ onImported }: { onImported?: (v: ImportView) => void }) {
  const client = useQueryClient();
  const [path, setPath] = useState("");
  const [showMissing, setShowMissing] = useState(false);
  const run = useMutation({
    mutationFn: (p: string) => ok(commands.importHistory(p)),
    onSuccess: (v) => {
      setPath("");
      void client.invalidateQueries();
      onImported?.(v);
    },
  });
  const pick = async () => {
    const chosen = await ok(commands.pickImportFile());
    if (chosen) run.mutate(chosen);
  };

  const v = run.data;
  return (
    <div className="flex flex-col gap-3">
      <p className="text-[13px] leading-relaxed text-ink-muted">
        From Letterboxd (the export .zip), Trakt, IMDb (your ratings) or MyAnimeList and AniList. Your ratings,
        what you have watched and when, and your watchlist come across; nothing leaves this PC.
      </p>
      <div className="flex items-end gap-2">
        <Button onClick={() => void pick()} disabled={run.isPending}>
          {run.isPending ? "Importing…" : "Choose an export…"}
        </Button>
        <div className="flex-1">
          <Input
            label="Or its path"
            placeholder="D:\Downloads\letterboxd-export.zip"
            value={path}
            onChange={(e) => setPath(e.target.value)}
          />
        </div>
        <Button variant="secondary" onClick={() => run.mutate(path.trim())} disabled={!path.trim() || run.isPending}>
          Import
        </Button>
      </div>
      {run.isError && (
        <p className="text-sm text-danger" role="alert">
          {run.error.message}
        </p>
      )}
      {v && (
        <div role="status">
          <p className="text-sm text-ink">{summary(v)}</p>
          {v.unmatched_total > 0 && (
            <>
              <Button size="sm" variant="ghost" onClick={() => setShowMissing((s) => !s)}>
                {showMissing ? "Hide" : "Show"} the {v.unmatched_total} not found
              </Button>
              {showMissing && (
                <ul className="mt-2 max-h-56 overflow-auto text-[13px] text-ink-muted">
                  {v.unmatched.map((u, i) => (
                    <li key={i} className="border-b border-line-subtle py-1">
                      <span className="text-ink">{u.title}</span>
                      {u.year ? ` (${u.year})` : ""} — {u.why}
                    </li>
                  ))}
                </ul>
              )}
            </>
          )}
        </div>
      )}
    </div>
  );
}
