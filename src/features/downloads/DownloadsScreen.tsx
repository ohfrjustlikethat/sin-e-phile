import { useEffect, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Badge, Button, EmptyState, Input, ProgressBar, Select } from "@/design-system";
import { commands, type DownloadRow, type DownloadsSettingsView, type StoredView } from "@/lib/ipc";
import { useUi } from "@/lib/store";
import { age, fraction, inProgress, n, rate, reorder, size, status } from "./format";
import { DOWNLOADS, STORAGE, useDownloads } from "./useDownloads";

/**
 * Downloads — SPEC.md Phase 13.
 *
 * What is downloading and why (the advisor's sentence is kept as the reason), with the
 * controls a queue needs: pause, order, tonight, a speed limit, cancel. Below it, what
 * finished downloads keep on this PC, watched first and largest first, so clearing room
 * is one press per film. Then how many run at once, the size budget, and the user's own
 * qBittorrent (ADR-0029: a transport only).
 *
 * Every control is a button, select or field, so the pad reaches all of it through the
 * focus system; B goes back (Escape).
 */

async function ok<T>(p: Promise<{ status: "ok"; data: T } | { status: "error"; error: string }>) {
  const r = await p;
  if (r.status === "error") throw new Error(r.error);
  return r.data;
}

const SETTINGS = ["downloads-settings"];

const LIMITS = [
  { value: "", label: "No limit" },
  { value: "512", label: "0.5 MB/s" },
  { value: "1024", label: "1 MB/s" },
  { value: "2048", label: "2 MB/s" },
  { value: "5120", label: "5 MB/s" },
  { value: "10240", label: "10 MB/s" },
];

export function DownloadsScreen() {
  const setDownloadsOpen = useUi((s) => s.setDownloadsOpen);
  const list = useDownloads();
  const storage = useQuery({ queryKey: STORAGE, queryFn: () => ok(commands.downloadsStorage()) });

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      const t = e.target as HTMLElement | null;
      // Escape in a field leaves the field first.
      if (t && (t.tagName === "INPUT" || t.tagName === "SELECT")) return;
      setDownloadsOpen(false);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [setDownloadsOpen]);

  // Something just finished: the storage list changes too.
  const doneCount = list.data?.filter((r) => r.download.state === "done").length ?? 0;
  const client = useQueryClient();
  useEffect(() => {
    void client.invalidateQueries({ queryKey: STORAGE });
  }, [doneCount, client]);

  const rows = inProgress(list.data ?? []);
  const speed = rows.reduce((s, r) => s + n(r.speed), 0);
  const running = rows.filter((r) => r.download.state === "active").length;
  const summary = [
    running > 0 ? `${running} downloading` : null,
    speed > 0 ? rate(speed) : null,
    storage.data?.free_bytes != null ? `${size(storage.data.free_bytes)} free` : null,
  ]
    .filter(Boolean)
    .join(" · ");

  return (
    <article className="max-w-[1100px] px-16 pt-12 pb-24">
      <button type="button" onClick={() => setDownloadsOpen(false)} className="label mb-10 hover:text-ink">
        ← Back
      </button>
      <h1 className="font-serif text-[56px] leading-none tracking-[-0.015em] text-ink">Downloads</h1>
      {summary && (
        <p className="label mt-5" role="status">
          {summary}
        </p>
      )}

      <section aria-label="Downloading" className="mt-12">
        {list.isError && <p className="text-sm text-danger">{list.error.message}</p>}
        {rows.length === 0 && list.isSuccess ? (
          <EmptyState
            title="Nothing downloading"
            body="Download from a film's page. The page says whether it would stream smoothly or is better downloaded first."
          />
        ) : (
          <ol className="flex flex-col">
            {rows.map((r, i) => (
              <DownloadItem
                key={r.download.id}
                row={r}
                first={i === 0}
                last={i === rows.length - 1}
                move={(delta) => reorder(rows, i, delta)}
              />
            ))}
          </ol>
        )}
      </section>

      {storage.data && storage.data.items.length > 0 && (
        <Kept
          items={storage.data.items}
          used={n(storage.data.used_bytes)}
          budget={storage.data.budget_bytes}
        />
      )}

      <Settings />
    </article>
  );
}

/** Several priority changes as one action: the first error, if any. */
async function setPriorities(
  changes: Array<[number, number]>,
): Promise<{ status: "ok"; data: null } | { status: "error"; error: string }> {
  for (const [id, priority] of changes) {
    const r = await commands.downloadPriority(id, priority);
    if (r.status === "error") return r;
  }
  return { status: "ok", data: null };
}

function useAct() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: (p: Promise<{ status: "ok"; data: unknown } | { status: "error"; error: string }>) => ok(p),
    onSettled: () => {
      void client.invalidateQueries({ queryKey: DOWNLOADS });
      void client.invalidateQueries({ queryKey: STORAGE });
    },
  });
}

function DownloadItem({
  row,
  first,
  last,
  move,
}: {
  row: DownloadRow;
  first: boolean;
  last: boolean;
  move: (delta: -1 | 1) => Array<[number, number]>;
}) {
  const d = row.download;
  const act = useAct();
  const openDetail = useUi((s) => s.openDetail);
  const [confirm, setConfirm] = useState(false);
  const tonight = d.not_before !== null && new Date(d.not_before) > new Date();
  const paused = d.state === "paused" || d.state === "failed";

  return (
    <li className="border-t border-line-subtle py-6 last:border-b">
      <div className="flex items-baseline justify-between gap-6">
        {d.media_item_id !== null ? (
          <button
            type="button"
            onClick={() => openDetail(d.media_item_id as number)}
            className="text-left font-serif text-[26px] leading-tight text-ink hover:text-ink-muted"
          >
            {d.title}
          </button>
        ) : (
          <h2 className="font-serif text-[26px] leading-tight text-ink">{d.title}</h2>
        )}
        <span className="shrink-0 text-sm text-ink-muted">
          {d.total_bytes ? size(d.total_bytes) : ""}
          {d.backend === "qbittorrent" ? " · in qBittorrent" : ""}
        </span>
      </div>
      <p className={`mt-2 text-sm ${d.state === "failed" ? "text-danger" : "text-ink-muted"}`} role="status">
        {status(row)}
      </p>
      <div className="mt-3">
        <ProgressBar value={fraction(row)} label={`${d.title} downloaded`} />
      </div>
      {d.reason && <p className="mt-3 max-w-[72ch] text-[13px] text-ink-faint">{d.reason}</p>}

      <div className="mt-4 flex flex-wrap items-center gap-2">
        {paused ? (
          <Button size="sm" onClick={() => act.mutate(commands.downloadResume(d.id))}>
            {d.state === "failed" ? "Try again" : "Resume"}
          </Button>
        ) : (
          <Button size="sm" onClick={() => act.mutate(commands.downloadPause(d.id))}>
            Pause
          </Button>
        )}
        <Button
          size="sm"
          variant="ghost"
          disabled={first}
          aria-label={`Move ${d.title} up`}
          onClick={() => act.mutate(setPriorities(move(-1)))}
        >
          Sooner
        </Button>
        <Button
          size="sm"
          variant="ghost"
          disabled={last}
          aria-label={`Move ${d.title} down`}
          onClick={() => act.mutate(setPriorities(move(1)))}
        >
          Later
        </Button>
        <Button
          size="sm"
          variant="ghost"
          aria-pressed={tonight}
          onClick={() => act.mutate(commands.downloadTonight(d.id, !tonight))}
        >
          {tonight ? "Start now" : "Tonight"}
        </Button>
        <div className="w-36">
          <Select
            aria-label={`Speed limit for ${d.title}`}
            options={LIMITS}
            value={d.limit_kib?.toString() ?? ""}
            onChange={(e) =>
              act.mutate(commands.downloadLimit(d.id, e.target.value ? Number(e.target.value) : null))
            }
          />
        </div>
        <span className="flex-1" />
        {confirm ? (
          <>
            <span className="text-sm text-ink-muted">Cancel, and delete what it fetched?</span>
            <Button size="sm" variant="danger" onClick={() => act.mutate(commands.downloadCancel(d.id, true))}>
              Cancel download
            </Button>
            <Button size="sm" variant="ghost" onClick={() => setConfirm(false)}>
              Keep it
            </Button>
          </>
        ) : (
          <Button size="sm" variant="ghost" onClick={() => setConfirm(true)}>
            Cancel…
          </Button>
        )}
      </div>
      {act.isError && <p className="mt-2 text-xs text-danger">{act.error.message}</p>}
    </li>
  );
}

function Kept({ items, used, budget }: { items: StoredView[]; used: number; budget: number | null }) {
  return (
    <section aria-label="Kept on this PC" className="mt-16">
      <h2 className="label mb-2 border-t border-line-strong pt-3">Kept on this PC</h2>
      <p className="mb-4 text-sm text-ink-muted">
        {size(used)}
        {budget ? ` of the ${size(budget)} you set aside` : " in downloads"}. Watched films come first.
      </p>
      {budget ? (
        <div className="mb-6 max-w-md">
          <ProgressBar value={used / budget} label="Space set aside, used" size="thick" />
        </div>
      ) : null}
      <ul className="flex flex-col">
        {items.map((s) => (
          <KeptItem key={s.download.id} item={s} />
        ))}
      </ul>
    </section>
  );
}

function KeptItem({ item }: { item: StoredView }) {
  const act = useAct();
  const [confirm, setConfirm] = useState(false);
  const d = item.download;
  return (
    <li className="flex items-center gap-4 border-b border-line-subtle py-3">
      <span className="min-w-0 flex-1 truncate font-serif text-[19px] text-ink">{d.title}</span>
      {item.watched && <Badge tone="neutral">Watched</Badge>}
      <span className="w-20 text-right text-sm text-ink-muted">{size(n(item.bytes))}</span>
      <span className="w-28 text-right text-sm text-ink-faint">{age(n(item.age_days))}</span>
      {confirm ? (
        <>
          <Button size="sm" variant="danger" onClick={() => act.mutate(commands.downloadClear(d.id))}>
            Delete {size(n(item.bytes))}
          </Button>
          <Button size="sm" variant="ghost" onClick={() => setConfirm(false)}>
            Keep
          </Button>
        </>
      ) : (
        <Button size="sm" variant="ghost" aria-label={`Delete ${d.title}`} onClick={() => setConfirm(true)}>
          Delete…
        </Button>
      )}
    </li>
  );
}

/** Where downloads go and how many at once: the built-in engine, or the viewer's own
 * qBittorrent as a transport only (ADR-0029). Also the first run's Downloads step. */
export function Settings() {
  const view = useQuery({ queryKey: SETTINGS, queryFn: () => ok(commands.downloadsSettings()) });
  return (
    <section aria-label="Download settings" className="mt-16 max-w-xl">
      <h2 className="label mb-5 border-t border-line-strong pt-3">Settings</h2>
      {view.data ? <SettingsForm initial={view.data} /> : <p className="text-sm text-ink-muted">Reading settings…</p>}
    </section>
  );
}

function SettingsForm({ initial }: { initial: DownloadsSettingsView }) {
  const client = useQueryClient();
  const [atOnce, setAtOnce] = useState(initial.max_active.toString());
  const [budget, setBudget] = useState(initial.budget_gib?.toString() ?? "");
  const [address, setAddress] = useState(initial.qbittorrent_address ?? "");
  const [user, setUser] = useState(initial.qbittorrent_user ?? "");
  const [password, setPassword] = useState("");
  const [note, setNote] = useState<string | null>(null);

  const save = useMutation({
    mutationFn: async () => {
      const gib = Number.parseFloat(budget);
      await ok(commands.setDownloadsSettings(Number(atOnce), Number.isFinite(gib) && gib > 0 ? gib : null));
      await ok(commands.setQbittorrent(address, user, password === "" ? null : password));
    },
    onSuccess: () => {
      setPassword("");
      setNote("Saved.");
      void client.invalidateQueries({ queryKey: SETTINGS });
      void client.invalidateQueries({ queryKey: STORAGE });
    },
    onError: (e) => setNote(e.message),
  });
  const check = useMutation({
    mutationFn: () => ok(commands.qbittorrentCheck()),
    onSuccess: (reached) =>
      setNote(
        reached
          ? "qBittorrent answered, and downloads will go to it."
          : "qBittorrent did not answer. Downloads use the built-in engine until it does.",
      ),
    onError: (e) => setNote(e.message),
  });

  return (
    <div className="flex flex-col gap-4">
      <div className="grid grid-cols-2 gap-3">
        <Select
          label="Downloading at once"
          options={["1", "2", "3", "4", "5"].map((n) => ({ value: n, label: n }))}
          value={atOnce}
          onChange={(e) => setAtOnce(e.target.value)}
        />
        <Input
          label="Space to set aside, GB"
          inputMode="decimal"
          placeholder="The whole drive"
          value={budget}
          onChange={(e) => setBudget(e.target.value)}
        />
      </div>
      <p className="text-[11px] leading-relaxed text-ink-faint">
        Speed limits for all downloads together are in Settings → Network.
      </p>

      <h3 className="mt-4 text-[13px] text-ink">Your own qBittorrent (optional)</h3>
      <p className="-mt-2 text-[11px] leading-relaxed text-ink-faint">
        If you run qBittorrent with its Web UI on, downloads can go to it instead. Only the
        files are handed over; nothing else of qBittorrent&apos;s is read. If it is not running,
        the built-in engine carries on.
      </p>
      <Input
        label="Web UI address"
        placeholder="http://localhost:8080"
        value={address}
        onChange={(e) => setAddress(e.target.value)}
      />
      <div className="grid grid-cols-2 gap-3">
        <Input label="User name" value={user} onChange={(e) => setUser(e.target.value)} />
        <Input
          label="Password"
          type="password"
          placeholder={initial.qbittorrent_has_password ? "Saved; type to change" : ""}
          value={password}
          onChange={(e) => setPassword(e.target.value)}
        />
      </div>
      <div className="flex items-center gap-3">
        <Button onClick={() => save.mutate()} disabled={save.isPending}>
          {save.isPending ? "Saving…" : "Save"}
        </Button>
        {initial.qbittorrent_address && (
          <Button variant="ghost" onClick={() => check.mutate()} disabled={check.isPending}>
            {check.isPending ? "Checking…" : "Check qBittorrent"}
          </Button>
        )}
        {note && (
          <span className="text-xs text-ink-muted" role="status">
            {note}
          </span>
        )}
      </div>
    </div>
  );
}
