import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Button, Input } from "@/design-system";
import { commands, type LibraryFolderView, type LibraryScanEvent } from "@/lib/ipc";
import { useUi } from "@/lib/store";

/**
 * Settings → Library — SPEC.md Phase 12.
 *
 * The folders the app looks in, how many films and episodes each holds, and a scan's
 * progress as it happens. A folder is chosen with Windows' own picker, or typed — the
 * picker cannot be driven with a controller, and the TV keyboard can fill the field.
 * Changes in a folder are picked up as they happen; Rescan is for the impatient.
 */

async function ok<T>(p: Promise<{ status: "ok"; data: T } | { status: "error"; error: string }>) {
  const r = await p;
  if (r.status === "error") throw new Error(r.error);
  return r.data;
}

const KEY = ["library-folders"];
const REVIEW = ["library-review-count"];

/** "D:\Media\Films\" -> "Films"; a drive alone stays as it is ("E:\"). */
export function folderName(path: string): string {
  const parts = path.split(/[\\/]+/).filter(Boolean);
  return parts.length > 1 ? parts[parts.length - 1]! : path;
}

/** "Identifying 120 of 900", "Looking… 3,400 files so far". */
export function scanLine(e: LibraryScanEvent | undefined): string | null {
  if (!e) return null;
  switch (e.phase) {
    case "walking":
      return `Looking… ${e.done.toLocaleString()} files so far`;
    case "identifying":
      return `Identifying ${e.done.toLocaleString()} of ${e.total.toLocaleString()}`;
    case "failed":
      return "The scan stopped. Rescan to try again.";
    default:
      return null;
  }
}

export function LibrarySection() {
  const client = useQueryClient();
  const folders = useQuery({ queryKey: KEY, queryFn: () => ok(commands.libraryFolders()) });
  const review = useQuery({ queryKey: REVIEW, queryFn: () => ok(commands.libraryReviewCount()) });
  const [progress, setProgress] = useState<Record<number, LibraryScanEvent>>({});
  const [typed, setTyped] = useState("");
  const [error, setError] = useState<string | null>(null);
  const setReviewOpen = useUi((s) => s.setReviewOpen);

  useEffect(() => {
    const refresh = () => {
      void client.invalidateQueries({ queryKey: KEY });
      void client.invalidateQueries({ queryKey: REVIEW });
    };
    const scanned = listen<LibraryScanEvent>("library-scan", (e) => {
      setProgress((p) => ({ ...p, [e.payload.folder_id]: e.payload }));
      if (e.payload.phase === "done" || e.payload.phase === "failed") refresh();
    });
    const changed = listen<number>("library-changed", refresh);
    return () => {
      void scanned.then((off) => off());
      void changed.then((off) => off());
    };
  }, [client]);

  const add = useMutation({
    mutationFn: (path: string) => ok(commands.addLibraryFolder(path)),
    onSuccess: () => {
      setTyped("");
      setError(null);
      void client.invalidateQueries({ queryKey: KEY });
    },
    onError: (e) => setError(e.message),
  });
  const remove = useMutation({
    mutationFn: (id: number) => ok(commands.removeLibraryFolder(id)),
    onSuccess: () => {
      void client.invalidateQueries({ queryKey: KEY });
      void client.invalidateQueries({ queryKey: REVIEW });
    },
  });
  const rescan = useMutation({ mutationFn: (id: number) => ok(commands.rescanLibraryFolder(id)) });

  async function choose() {
    const picked = await ok(commands.pickLibraryFolder());
    if (picked) add.mutate(picked);
  }

  const list = folders.data ?? [];
  return (
    <section className="mb-9" aria-label="Library">
      <h2 className="mb-3.5 text-[11px] font-semibold uppercase tracking-[0.12em] text-ink-faint">Library</h2>
      <p className="mb-4 text-sm text-ink-muted">
        Folders of films and episodes on this PC. Each file is identified from its name, its folders and its
        length; anything uncertain waits for you rather than being guessed.
      </p>

      {list.length === 0 && !folders.isPending && (
        <p className="mb-4 text-sm text-ink-faint">No folders yet. Add the one your films live in.</p>
      )}
      <ul className="mb-4 flex flex-col">
        {list.map((f) => (
          <FolderRow
            key={f.id}
            folder={f}
            line={scanLine(progress[f.id])}
            onRescan={() => rescan.mutate(f.id)}
            onRemove={() => remove.mutate(f.id)}
          />
        ))}
      </ul>

      {(review.data ?? 0) > 0 && (
        <div className="mb-5 flex items-center justify-between gap-4 rounded-md border border-line px-4 py-3">
          <p className="text-sm text-ink">
            {review.data} {review.data === 1 ? "file needs" : "files need"} a quick check.
          </p>
          <Button variant="secondary" size="sm" onClick={() => setReviewOpen(true)}>
            Check them
          </Button>
        </div>
      )}

      <div className="flex flex-wrap items-end gap-2">
        <Button variant="secondary" onClick={() => void choose()} disabled={add.isPending}>
          Add a folder…
        </Button>
        <form
          className="flex flex-1 items-end gap-2"
          onSubmit={(e) => {
            e.preventDefault();
            if (typed.trim()) add.mutate(typed.trim());
          }}
        >
          <Input
            label="Or type its path"
            value={typed}
            onChange={(e) => setTyped(e.target.value)}
            placeholder="D:\Films"
            className="flex-1"
          />
          <Button type="submit" variant="ghost" disabled={!typed.trim() || add.isPending}>
            Add
          </Button>
        </form>
      </div>
      {error && <p className="mt-2 text-sm text-danger">{error}</p>}
    </section>
  );
}

function FolderRow(p: {
  folder: LibraryFolderView;
  line: string | null;
  onRescan: () => void;
  onRemove: () => void;
}) {
  const { folder } = p;
  const status = p.line ?? `${folder.files.toLocaleString()} ${folder.files === 1 ? "file" : "files"}`;
  return (
    <li className="flex items-center justify-between gap-4 border-t border-line-subtle py-3">
      <div className="min-w-0">
        {/* The folder's own name first: a long path truncated at the end hides it. */}
        <p className="truncate text-sm text-ink">{folderName(folder.path)}</p>
        <p className="truncate text-xs text-ink-faint" title={folder.path}>
          {folder.path}
        </p>
        <p className="mt-0.5 text-xs text-ink-muted" aria-live="polite">
          {status}
        </p>
      </div>
      <div className="flex shrink-0 gap-1">
        <Button variant="ghost" size="sm" onClick={p.onRescan} aria-label={`Rescan ${folder.path}`}>
          Rescan
        </Button>
        <Button variant="ghost" size="sm" onClick={p.onRemove} aria-label={`Remove ${folder.path}`}>
          Remove
        </Button>
      </div>
    </li>
  );
}
