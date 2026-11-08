import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Button, Input } from "@/design-system";
import { commands } from "@/lib/ipc";

/**
 * TMDB, with this profile's own key (SPEC.md §14, ADR-0013, ADR-0027; Phase 16). Optional:
 * the app is complete without it. With it, "New releases" starts with what is in cinemas
 * and on the air this week, which the offline catalogue cannot know. None ships with the
 * app; the key is the viewer's, under TMDB's terms, and removing it removes what it fetched.
 */

async function ok<T>(p: Promise<{ status: "ok"; data: T } | { status: "error"; error: string }>) {
  const r = await p;
  if (r.status === "error") throw new Error(r.error);
  return r.data;
}

export const TMDB = ["tmdb-status"] as const;

export function TmdbSection() {
  const client = useQueryClient();
  const status = useQuery({ queryKey: TMDB, queryFn: () => ok(commands.tmdbStatus()) });
  const [key, setKey] = useState("");
  const changed = () => {
    void client.invalidateQueries({ queryKey: TMDB });
    void client.invalidateQueries({ queryKey: ["home"] });
  };
  const save = useMutation({
    mutationFn: () => ok(commands.setTmdbKey(key)),
    onSuccess: () => {
      setKey("");
      changed();
    },
  });
  const remove = useMutation({ mutationFn: () => ok(commands.clearTmdbKey()), onSuccess: changed });

  return (
    <section aria-label="TMDB" className="mb-12">
      <h2 className="mb-3.5 text-[11px] font-semibold uppercase tracking-[0.12em] text-ink-faint">TMDB</h2>
      <p className="mb-3 max-w-[62ch] text-[13px] text-ink-muted">
        Optional. With your own free TMDB key, New releases starts with what is in cinemas and on the air this week.
        No key comes with the app; yours is kept on this PC, for this profile only.
      </p>
      {status.data?.has_key ? (
        <div className="flex items-center gap-3">
          <span className="text-sm text-ink-muted" role="status">
            Your key is saved.{status.data.fetched_on ? ` What is showing now was last fetched ${status.data.fetched_on}.` : ""}
          </span>
          <Button variant="ghost" size="sm" onClick={() => remove.mutate()}>
            Remove key
          </Button>
        </div>
      ) : (
        <>
          <p className="mb-3 text-[12px] text-ink-muted">
            <button type="button" className="underline hover:text-ink" onClick={() => void commands.openSignUp("tmdb")}>
              Get a free key
            </button>{" "}
            (a TMDB account, then its API page), and paste the API Key here. It is checked as you save it.
          </p>
          <div className="flex items-end gap-3">
            <div className="flex-1">
              <Input
                label="TMDB API key"
                value={key}
                onChange={(e) => setKey(e.target.value)}
                {...(save.isError ? { error: save.error.message } : {})}
                autoComplete="off"
                spellCheck={false}
              />
            </div>
            <Button variant="secondary" onClick={() => save.mutate()} disabled={!key.trim() || save.isPending}>
              Save key
            </Button>
          </div>
        </>
      )}
    </section>
  );
}
