import { useCallback, useEffect, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { onEvent } from "@/lib/events";
import { commands, type AssetPlan, type AssetProgress } from "@/lib/ipc";
import { Button, ProgressBar } from "@/design-system";
import { CATALOGUE } from "@/features/settings/catalogueKey";

async function ok<T>(p: Promise<{ status: "ok"; data: T } | { status: "error"; error: string }>) {
  const r = await p;
  if (r.status === "error") throw new Error(r.error);
  return r.data;
}

/**
 * Consent for the optional downloads (ADR-0014, subtask 5.9).
 *
 * # What this screen is not allowed to do
 *
 * Start anything. ADR-0014's requirement is that the download is **consented to, with
 * the size shown, before it happens** — no background fetch, no "we'll just grab this".
 * So the size comes from pinned values the backend states up front, never from starting
 * a download to find out, and nothing is requested until the button is pressed.
 *
 * The one exception (ADR-0050, the author's decision): a **new** install's setup starts it
 * once the catalogue is searchable. In setup (`setup`) this says so, with the size, before
 * and while it happens; everywhere else it is the offer, with its button. While either is
 * running every copy of this shows that download (`running`), never a second.
 *
 * # Why it says what search does without it
 *
 * The honest framing is an upgrade, not a repair. Search already works: titles, people,
 * filters, typo tolerance. What is missing is the ability to describe a film rather than
 * name it. A screen that implied the app were broken until this finished would be
 * lying, and `SPEC.md` §8 is explicit that a degraded tier must be "diminished but
 * genuinely useful, never broken or empty".
 */
function megabytes(bytes: number): string {
  return `${Math.round(bytes / 1_048_576).toLocaleString()} MB`;
}

/**
 * What the progress bar is counting right now.
 *
 * There are two stages and they count different things. The download moves bytes; the
 * build puts titles into a graph and takes about half a minute, during which no bytes
 * move at all. Reporting the second as though it were the first would leave a bar
 * labelled in megabytes sitting still — which reads as a hang, and is why the backend
 * sends a `phase` rather than leaving this screen to infer one.
 */
function describe(progress: AssetProgress): { label: string; percent: number } {
  const percent =
    progress.total_bytes > 0 ? (progress.done_bytes / progress.total_bytes) * 100 : 0;

  if (progress.phase === "building") {
    return {
      percent,
      label: `Building the meaning index — ${progress.done_bytes.toLocaleString()} of ${progress.total_bytes.toLocaleString()} titles`,
    };
  }
  const of = progress.total_bytes > 0 ? ` of ${megabytes(progress.total_bytes)}` : "";
  return { percent, label: `${progress.name} — ${megabytes(progress.done_bytes)}${of}` };
}

export function MeaningDownload({ onDone, setup = false }: { onDone?: () => void; setup?: boolean }) {
  const [plan, setPlan] = useState<AssetPlan | null>(null);
  const [progress, setProgress] = useState<AssetProgress | null>(null);
  const [pressed, setPressed] = useState(false);
  const [failed, setFailed] = useState<string | null>(null);

  const reload = useCallback(() => {
    void Promise.resolve()
      .then(() => commands.optionalAssets())
      .then(setPlan)
      .catch(() => {});
  }, []);
  useEffect(reload, [reload]);

  useEffect(
    () =>
      onEvent<AssetProgress>("asset-progress", (p) => {
        setProgress(p);
        // Setup's download began after this was drawn: show it, not the button.
        setPlan((plan) => (plan && !plan.running ? { ...plan, running: true } : plan));
      }),
    [],
  );
  // However it ends, and whoever started it: show what is true now.
  useEffect(
    () =>
      onEvent<boolean>("meaning-ready", (works) => {
        setPressed(false);
        setProgress(null);
        reload();
        if (works) onDone?.();
      }),
    [reload, onDone],
  );

  if (!plan || plan.complete) return null;

  const outstanding = plan.assets.filter((a) => a.state !== "present");
  const unusable = plan.assets.filter((a) => a.state === "unusable");
  const running = pressed || plan.running;
  // Everything downloaded, the index not yet built (a build that stopped): the button builds, and says so.
  const size = megabytes(plan.outstanding_bytes);
  const action = plan.outstanding_bytes > 0 ? `Download ${size}` : "Build the index";
  const doing =
    plan.outstanding_bytes > 0
      ? `${setup ? "Downloading as part of setting up" : "Downloading"} · ${size}.`
      : "Building the index on this PC, about a minute.";

  async function start() {
    setPressed(true);
    setFailed(null);
    const result = await commands.downloadOptionalAssets();
    if (result.status === "error") {
      setPressed(false);
      setFailed(result.error);
      reload();
    }
    // Success arrives as `meaning-ready`, for this copy and every other.
  }

  return (
    <section aria-label="Search by meaning" className="rounded border border-line-subtle bg-surface p-5">
      {/* Not `text-base`: in this theme that is the page's background colour, and the heading was invisible. */}
      <h2 className="text-[16px] font-medium text-ink">Search by meaning</h2>
      <p className="mt-1 max-w-prose text-[13.5px] leading-relaxed text-ink-muted">
        Search works now — titles, people, directors, decades, and spelling you got
        slightly wrong. This adds the part that lets you <em>describe</em> a film instead:
        “a heist where the plan goes wrong”, “anime about loneliness in a big city”.
      </p>

      {/*
        Each file named individually rather than one total, because "367 MB" with no
        breakdown is a number a person has to simply trust. The model is a third of it
        and nobody would guess that from "meaning index".
      */}
      <ul className="mt-4 flex flex-col gap-1 text-[13.5px]">
        {outstanding.map((asset) => (
          <li key={asset.name} className="flex justify-between gap-6">
            <span>
              {asset.name}
              <span className="ml-2 text-ink-muted">{asset.purpose}</span>
            </span>
            <span className="shrink-0 tabular-nums text-ink-muted">
              {megabytes(asset.bytes)}
            </span>
          </li>
        ))}
      </ul>

      {unusable.length > 0 && (
        // A file that is present and wrong is worth saying out loud. The alternative is
        // a user wondering why a download they already did is being asked for again.
        <p className="mt-3 text-[13.5px] text-warning">
          A previously downloaded file cannot be used and will be replaced:{" "}
          {unusable[0]?.unusable_reason}
        </p>
      )}

      {setup && !running ? (
        <SetupStart
          size={size}
          installed={plan.outstanding_bytes === 0}
          action={action}
          onStart={() => void start()}
        />
      ) : running ? (
        <p className="mt-5 text-[13.5px] text-ink" role="status">
          {doing} Search works meanwhile.
        </p>
      ) : (
        <div className="mt-5 flex items-center gap-4">
          <Button onClick={start}>{action}</Button>
          <span className="text-[13.5px] text-ink-muted">
            {plan.outstanding_bytes > 0
              ? "Optional. Nothing is downloaded until you choose to."
              : "Everything it needs is on this PC; the index is built from it here."}
          </span>
        </div>
      )}

      {running && progress && (
        <div className="mt-4">
          <ProgressBar value={describe(progress).percent} />
          <p className="mt-2 text-[13.5px] text-ink-muted">{describe(progress).label}</p>
        </div>
      )}

      {failed && (
        <p className="mt-3 text-[13.5px] text-danger">
          The download did not finish: {failed}. Search continues to work without it.
        </p>
      )}
    </section>
  );
}

/**
 * In setup, before it runs (ADR-0050): a new install's catalogue is not searchable yet, and the download starts by
 * itself once it is, so this says so; once the catalogue is ready and nothing started it (an install set up before),
 * it is the button. The catalogue's status is read with the same key and shape as everywhere else.
 */
function SetupStart({
  size,
  installed,
  action,
  onStart,
}: {
  size: string;
  /** The installer carried the files (ADR-0053): nothing to download, only the index to build. */
  installed: boolean;
  action: string;
  onStart: () => void;
}) {
  const catalogue = useQuery({ queryKey: CATALOGUE, queryFn: () => ok(commands.catalogueStatus()) });
  if (catalogue.data && !catalogue.data.ready) {
    return (
      <p className="mt-5 text-[13.5px] text-ink">
        {installed
          ? "Comes with the app. Its index is built on this PC as soon as your catalogue can be searched."
          : `Downloads by itself, ${size}, as soon as your catalogue can be searched.`}
      </p>
    );
  }
  return (
    <div className="mt-5 flex items-center gap-4">
      <Button onClick={onStart}>{action}</Button>
      <span className="text-[13.5px] text-ink-muted">Optional. Search works without it.</span>
    </div>
  );
}
