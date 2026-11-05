import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Button, Input, Toggle } from "@/design-system";
import { commands, type AddonPreview, type CatalogueView, type SourceInfo } from "@/lib/ipc";

/**
 * Settings → Sources — SPEC.md Phase 6.
 *
 * §4's lesson from Stremio: "source setup is one paste-a-URL field with plain-language
 * help. Never a raw JSON editor as the primary path." So every claim an addon's
 * manifest makes is shown here as a sentence, before anything is stored, and nothing
 * here ever displays a manifest.
 *
 * No address is pre-filled anywhere on this screen. The app ships no sources and no
 * catalogue (§2.1); the placeholders use a reserved example domain.
 */

/** The generated commands resolve to `{status, data | error}`; a query wants a throw. */
async function ok<T>(p: Promise<{ status: "ok"; data: T } | { status: "error"; error: string }>) {
  const r = await p;
  if (r.status === "error") throw new Error(r.error);
  return r.data;
}

const KEY = ["sources"];

export function SourcesSection() {
  const client = useQueryClient();
  const refresh = () => client.invalidateQueries({ queryKey: KEY });
  const view = useQuery({ queryKey: KEY, queryFn: () => ok(commands.sourcesOverview()) });

  const toggle = useMutation({
    mutationFn: ({ id, on }: { id: string; on: boolean }) =>
      ok(commands.sourcesSetEnabled(id, on)),
    onSettled: refresh,
  });
  const reorder = useMutation({
    mutationFn: (ids: string[]) => ok(commands.sourcesReorder(ids)),
    onSettled: refresh,
  });
  const remove = useMutation({
    mutationFn: (id: string) => ok(commands.addonRemove(id)),
    onSettled: refresh,
  });

  const sources = view.data?.sources ?? [];
  const move = (i: number, by: -1 | 1) => {
    const ids = sources.map((s) => s.id);
    const j = i + by;
    const [a, b] = [ids[i], ids[j]];
    if (a === undefined || b === undefined) return;
    ids[i] = b;
    ids[j] = a;
    reorder.mutate(ids);
  };

  return (
    <>
      <Section title="Sources">
        <p className="mb-4 text-xs leading-relaxed text-ink-faint">
          Where films come from. Every search asks the sources that are on, in this order,
          and none of them waits for another. Out of the box that is your own files and the
          Internet Archive's public-domain films — nothing else is included.
        </p>
        {view.isPending && <p className="text-sm text-ink-muted">Reading sources…</p>}
        {view.isError && <p className="text-sm text-danger">{String(view.error.message)}</p>}
        <ol aria-label="Sources, in the order they are asked">
          {sources.map((s, i) => (
            <SourceRow
              key={s.id}
              source={s}
              first={i === 0}
              last={i === sources.length - 1}
              busy={toggle.isPending || reorder.isPending || remove.isPending}
              onToggle={(on) => toggle.mutate({ id: s.id, on })}
              onMove={(by) => move(i, by)}
              onRemove={() => remove.mutate(s.id)}
            />
          ))}
        </ol>
        {(view.data?.refused_addons.length ?? 0) > 0 && (
          <p className="mt-3 text-xs text-danger">
            {view.data!.refused_addons.length === 1 ? "One installed source" : "Some installed sources"}{" "}
            no longer passed the safety checks and {view.data!.refused_addons.length === 1 ? "was" : "were"} not
            loaded. Add {view.data!.refused_addons.length === 1 ? "it" : "them"} again from the author's address.
          </p>
        )}
      </Section>

      <AddSource onAdded={refresh} />
      <YourLinks />
    </>
  );
}

function SourceRow({
  source, first, last, busy, onToggle, onMove, onRemove,
}: {
  source: SourceInfo;
  first: boolean;
  last: boolean;
  busy: boolean;
  onToggle: (on: boolean) => void;
  onMove: (by: -1 | 1) => void;
  onRemove: () => void;
}) {
  return (
    <li className="border-b border-line-subtle py-2 last:border-b-0">
      <Toggle
        checked={source.enabled}
        onChange={onToggle}
        disabled={busy}
        label={source.name}
        description={
          source.addon_host
            ? `${source.blurb ? source.blurb + " " : ""}Sends your searches to ${source.addon_host}.`
            : source.blurb
        }
      />
      <div className="flex items-center justify-between gap-4 pb-1">
        <p className="text-[11px] text-ink-muted">
          {source.health}
          {source.last_error && <span className="text-ink-faint"> Last problem: {source.last_error}.</span>}
        </p>
        <div className="flex shrink-0 gap-1">
          <Button size="sm" variant="ghost" disabled={busy || first} onClick={() => onMove(-1)}
            aria-label={`Ask ${source.name} earlier`}>
            Earlier
          </Button>
          <Button size="sm" variant="ghost" disabled={busy || last} onClick={() => onMove(1)}
            aria-label={`Ask ${source.name} later`}>
            Later
          </Button>
          {source.addon_host && (
            <Button size="sm" variant="ghost" disabled={busy} onClick={onRemove}
              aria-label={`Remove ${source.name}`}>
              Remove
            </Button>
          )}
        </div>
      </div>
    </li>
  );
}

/** Paste an address, see what it says it does, confirm. Or browse a catalogue first. */
function AddSource({ onAdded }: { onAdded: () => void }) {
  const [address, setAddress] = useState("");
  const [catalogueAddress, setCatalogueAddress] = useState("");
  const [preview, setPreview] = useState<AddonPreview | null>(null);
  const [catalogue, setCatalogue] = useState<CatalogueView | null>(null);
  const [added, setAdded] = useState<string | null>(null);

  const look = useMutation({
    mutationFn: (url: string) => ok(commands.addonPreview(url)),
    onSuccess: (p) => {
      setPreview(p);
      setAdded(null);
    },
  });
  const install = useMutation({
    mutationFn: (pasted: string) => ok(commands.addonInstall(pasted)),
    onSuccess: () => {
      setAdded(preview?.name ?? "The source");
      setPreview(null);
      setAddress("");
      onAdded();
    },
  });
  const open = useMutation({
    mutationFn: (url: string) => ok(commands.catalogueOpen(url)),
    onSuccess: setCatalogue,
  });

  return (
    <Section title="Add a source">
      <p className="mb-4 text-xs leading-relaxed text-ink-faint">
        A source is a service someone else runs that knows where films can be watched. Paste
        the address its author gives you. You will see what it says it does before anything
        is added, and it only ever receives what you search for.
      </p>
      <form
        className="flex items-end gap-2"
        onSubmit={(e) => {
          e.preventDefault();
          if (address.trim()) look.mutate(address.trim());
        }}
      >
        <Input
          className="flex-1"
          label="Source address"
          placeholder="https://example.com/addon/manifest.json"
          value={address}
          onChange={(e) => setAddress(e.target.value)}
          {...(look.isError ? { error: look.error.message } : {})}
          spellCheck={false}
        />
        <Button type="submit" disabled={!address.trim() || look.isPending}>
          {look.isPending ? "Looking…" : "Look at it"}
        </Button>
      </form>

      {added && <p className="mt-3 text-sm text-success">{added} was added and is on.</p>}

      {preview && (
        <Confirm
          preview={preview}
          busy={install.isPending}
          {...(install.isError ? { error: install.error.message } : {})}
          onConfirm={() => install.mutate(preview.pasted)}
          onCancel={() => setPreview(null)}
        />
      )}

      <form
        className="mt-6 flex items-end gap-2"
        onSubmit={(e) => {
          e.preventDefault();
          if (catalogueAddress.trim()) open.mutate(catalogueAddress.trim());
        }}
      >
        <Input
          className="flex-1"
          label="Or browse a catalogue"
          hint="A list of sources someone recommends. None is built in."
          placeholder="https://example.com/catalogue.json"
          value={catalogueAddress}
          onChange={(e) => setCatalogueAddress(e.target.value)}
          {...(open.isError ? { error: open.error.message } : {})}
          spellCheck={false}
        />
        <Button type="submit" variant="secondary" disabled={!catalogueAddress.trim() || open.isPending}>
          {open.isPending ? "Opening…" : "Open"}
        </Button>
      </form>

      {catalogue && (
        <div className="mt-4">
          <p className="label mb-2">{catalogue.name}</p>
          {catalogue.addons.length === 0 && (
            <p className="text-sm text-ink-muted">This catalogue lists nothing.</p>
          )}
          <ul>
            {catalogue.addons.map((a) => (
              <li key={a.manifest}
                className="flex items-baseline justify-between gap-4 border-b border-line-subtle py-2.5 last:border-b-0">
                <div className="min-w-0">
                  <p className="text-[13px] text-ink">{a.name}</p>
                  {a.description && <p className="text-[11px] text-ink-faint">{a.description}</p>}
                </div>
                <Button size="sm" variant="secondary" disabled={look.isPending}
                  onClick={() => {
                    setAddress(a.manifest);
                    look.mutate(a.manifest);
                  }}>
                  Look at it
                </Button>
              </li>
            ))}
          </ul>
        </div>
      )}
    </Section>
  );
}

/** Every claim the manifest makes, as a sentence. The one primary action on the screen. */
function Confirm({
  preview, busy, error, onConfirm, onCancel,
}: {
  preview: AddonPreview;
  busy: boolean;
  error?: string | undefined;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  return (
    <div role="region" aria-label={`About ${preview.name}`}
      className="mt-4 rounded-md border border-line bg-surface p-5">
      <p className="text-[17px] text-ink">
        {preview.name} <span className="text-[13px] text-ink-faint">version {preview.version}</span>
      </p>
      {preview.description && (
        <p className="mt-2 whitespace-pre-line text-sm leading-relaxed text-ink-muted">
          {preview.description}
        </p>
      )}
      <ul className="mt-3 space-y-1 text-[13px] text-ink">
        <li>{preview.finds}</li>
        {preview.delivers.map((d) => <li key={d}>{d}</li>)}
        <li>
          Your searches are sent to <span className="font-mono text-[12px]">{preview.host}</span>.
        </li>
        {preview.replaces && <li className="text-ink-muted">This updates the version you already have.</li>}
      </ul>
      {error && <p className="mt-3 text-sm text-danger">{error}</p>}
      <div className="mt-5 flex gap-2">
        <Button variant="primary" onClick={onConfirm} disabled={busy}>
          {busy ? "Adding…" : preview.replaces ? "Update source" : "Add source"}
        </Button>
        <Button variant="secondary" onClick={onCancel} disabled={busy}>Cancel</Button>
      </div>
    </div>
  );
}

/** The direct HTTP backend's list: links typed in, or a pasted playlist. */
function YourLinks() {
  const client = useQueryClient();
  const links = useQuery({ queryKey: ["links"], queryFn: () => ok(commands.linksList()) });
  const refresh = () => client.invalidateQueries({ queryKey: ["links"] });
  const [title, setTitle] = useState("");
  const [year, setYear] = useState("");
  const [url, setUrl] = useState("");
  const [playlist, setPlaylist] = useState("");

  const add = useMutation({
    mutationFn: () => {
      const y = Number.parseInt(year, 10);
      return ok(commands.linksAdd(title, Number.isFinite(y) ? y : null, url));
    },
    onSuccess: () => {
      setTitle("");
      setYear("");
      setUrl("");
      refresh();
    },
  });
  const importList = useMutation({
    mutationFn: () => ok(commands.linksImport(playlist)),
    onSuccess: () => {
      setPlaylist("");
      refresh();
    },
  });
  const remove = useMutation({
    mutationFn: (id: number) => ok(commands.linksRemove(id)),
    onSettled: refresh,
  });

  return (
    <Section title="Your links">
      <p className="mb-4 text-xs leading-relaxed text-ink-faint">
        Web addresses of films you can watch, matched to the catalogue by title and year.
      </p>
      <form
        className="grid grid-cols-[1fr_5rem] gap-2"
        onSubmit={(e) => {
          e.preventDefault();
          add.mutate();
        }}
      >
        <Input label="Title" value={title} onChange={(e) => setTitle(e.target.value)} />
        <Input label="Year" inputMode="numeric" value={year} onChange={(e) => setYear(e.target.value)} />
        <Input className="col-span-2" label="Address" placeholder="https://example.com/film.mp4"
          value={url} onChange={(e) => setUrl(e.target.value)} spellCheck={false}
          {...(add.isError ? { error: add.error.message } : {})} />
        <div className="col-span-2">
          <Button type="submit" variant="secondary" disabled={!title.trim() || !url.trim() || add.isPending}>
            Add link
          </Button>
        </div>
      </form>

      <form
        className="mt-5"
        onSubmit={(e) => {
          e.preventDefault();
          importList.mutate();
        }}
      >
        <label htmlFor="playlist" className="label">Or paste a playlist (M3U)</label>
        <textarea
          id="playlist"
          rows={3}
          value={playlist}
          onChange={(e) => setPlaylist(e.target.value)}
          spellCheck={false}
          className="mt-1.5 w-full resize-y rounded-sm border border-line bg-void px-3 py-2 font-mono text-[12px] text-ink focus-visible:border-oxblood-bright focus-visible:outline-none"
        />
        {importList.isError && <p className="mt-1 text-xs text-danger">{importList.error.message}</p>}
        {importList.isSuccess && (
          <p className="mt-1 text-xs text-success">Added {importList.data} {importList.data === 1 ? "link" : "links"}.</p>
        )}
        <Button type="submit" variant="secondary" className="mt-2" disabled={!playlist.trim() || importList.isPending}>
          Import
        </Button>
      </form>

      {(links.data?.length ?? 0) > 0 && (
        <ul className="mt-5" aria-label="Your links">
          {links.data!.map((l) => (
            <li key={l.id}
              className="flex items-baseline justify-between gap-4 border-b border-line-subtle py-2 last:border-b-0">
              <div className="min-w-0">
                <p className="text-[13px] text-ink">
                  {l.title}
                  {l.year && <span className="text-ink-faint"> {l.year}</span>}
                </p>
                <p className="truncate font-mono text-[11px] text-ink-faint">{l.url}</p>
              </div>
              <Button size="sm" variant="ghost" onClick={() => remove.mutate(l.id)}
                aria-label={`Remove ${l.title}`}>
                Remove
              </Button>
            </li>
          ))}
        </ul>
      )}
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
