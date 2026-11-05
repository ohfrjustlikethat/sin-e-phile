import { useEffect, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Button, Input, Select, Toggle } from "@/design-system";
import { commands, type TorrentSettingsView } from "@/lib/ipc";

/**
 * Settings → Network — SPEC.md Phase 7.
 *
 * The torrent engine's limits and its two safety settings, in words. The adapter
 * binding is the one that matters most: bound to a VPN's adapter, the engine's traffic
 * stops if the VPN drops instead of leaving by the ordinary connection.
 */

async function ok<T>(p: Promise<{ status: "ok"; data: T } | { status: "error"; error: string }>) {
  const r = await p;
  if (r.status === "error") throw new Error(r.error);
  return r.data;
}

const KEY = ["torrent-settings"];

/** "" means no limit. */
function toLimit(text: string): number | null {
  const n = Number.parseInt(text, 10);
  return Number.isFinite(n) && n > 0 ? n : null;
}

export function NetworkSection() {
  const client = useQueryClient();
  const view = useQuery({ queryKey: KEY, queryFn: () => ok(commands.torrentSettings()) });
  return view.data ? (
    <Form
      initial={view.data}
      onSaved={() => client.invalidateQueries({ queryKey: KEY })}
    />
  ) : (
    <Section title="Network">
      <p className="text-sm text-ink-muted">
        {view.isError ? String(view.error.message) : "Reading network settings…"}
      </p>
    </Section>
  );
}

function Form({ initial, onSaved }: { initial: TorrentSettingsView; onSaved: () => void }) {
  const [down, setDown] = useState(initial.download_limit_kib?.toString() ?? "");
  const [up, setUp] = useState(initial.upload_limit_kib?.toString() ?? "");
  const [peers, setPeers] = useState(initial.peer_limit.toString());
  const [keepSeeding, setKeepSeeding] = useState(initial.keep_seeding);
  const [adapter, setAdapter] = useState(initial.bind_interface ?? "");
  const [saved, setSaved] = useState(false);
  useEffect(() => setSaved(false), [down, up, peers, keepSeeding, adapter]);

  const save = useMutation({
    mutationFn: () =>
      ok(
        commands.setTorrentSettings(
          toLimit(down),
          toLimit(up),
          Number.parseInt(peers, 10) || 128,
          keepSeeding,
          adapter || null,
        ),
      ),
    onSuccess: () => {
      setSaved(true);
      onSaved();
    },
  });

  const chosen = initial.adapters.find((a) => a.name === adapter);
  const options = [
    { value: "", label: "Any connection (the system decides)" },
    ...initial.adapters.map((a) => ({
      value: a.name,
      label: a.connected ? a.name : `${a.name} (not connected)`,
    })),
  ];
  if (adapter && !chosen) options.push({ value: adapter, label: `${adapter} (not found)` });

  return (
    <Section title="Network">
      <p className="mb-4 text-xs leading-relaxed text-ink-faint">
        How the torrent engine uses your connection. Nothing here affects streams from the
        Internet Archive or your own links.
      </p>

      <Select
        label="Use only this network adapter"
        options={options}
        value={adapter}
        onChange={(e) => setAdapter(e.target.value)}
      />
      <p className="mt-1.5 mb-5 text-[11px] leading-relaxed text-ink-faint">
        Choose your VPN&apos;s adapter and torrent traffic stops if the VPN drops, instead of
        quietly carrying on over your normal connection.
      </p>

      <Toggle
        checked={!keepSeeding}
        onChange={(stop) => setKeepSeeding(!stop)}
        label="Stop sharing when a download finishes"
        description="On by default. Sharing is uploading; with this off, finished torrents keep uploading to others."
      />

      <div className="mt-3 grid grid-cols-3 gap-2">
        <Input label="Download limit, KB/s" inputMode="numeric" placeholder="No limit"
          value={down} onChange={(e) => setDown(e.target.value)} />
        <Input label="Upload limit, KB/s" inputMode="numeric" placeholder="No limit"
          value={up} onChange={(e) => setUp(e.target.value)} />
        <Input label="Peers per torrent" inputMode="numeric"
          value={peers} onChange={(e) => setPeers(e.target.value)} />
      </div>

      <div className="mt-4 flex items-center gap-3">
        <Button variant="secondary" onClick={() => save.mutate()} disabled={save.isPending}>
          {save.isPending ? "Saving…" : "Save"}
        </Button>
        {save.isError && <span className="text-xs text-danger">{save.error.message}</span>}
        {saved && (
          <span className="text-xs text-success" role="status">
            Saved.{initial.engine_running ? " These apply the next time sin-e-phile starts." : ""}
          </span>
        )}
      </div>
    </Section>
  );
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <section className="mb-9">
      <h2 className="mb-3.5 text-[11px] font-semibold uppercase tracking-[0.12em] text-ink-faint">
        {title}
      </h2>
      {children}
    </section>
  );
}
