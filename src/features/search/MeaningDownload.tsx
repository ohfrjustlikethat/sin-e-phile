import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { commands, type AssetPlan, type AssetProgress } from "@/lib/ipc";
import { Button, ProgressBar } from "@/design-system";

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

export function MeaningDownload({ onDone }: { onDone?: () => void }) {
  const [plan, setPlan] = useState<AssetPlan | null>(null);
  const [progress, setProgress] = useState<AssetProgress | null>(null);
  const [running, setRunning] = useState(false);
  const [failed, setFailed] = useState<string | null>(null);

  useEffect(() => {
    void commands.optionalAssets().then(setPlan);
  }, []);

  useEffect(() => {
    const unlisten = listen<AssetProgress>("asset-progress", (event) =>
      setProgress(event.payload),
    );
    return () => {
      void unlisten.then((off) => off());
    };
  }, []);

  if (!plan || plan.complete) return null;

  const outstanding = plan.assets.filter((a) => a.state !== "present");
  const unusable = plan.assets.filter((a) => a.state === "unusable");

  async function start() {
    setRunning(true);
    setFailed(null);
    const result = await commands.downloadOptionalAssets();
    setRunning(false);
    if (result.status === "error") {
      setFailed(result.error);
      return;
    }
    setPlan(await commands.optionalAssets());
    onDone?.();
  }

  return (
    <section className="rounded border border-line-subtle bg-surface p-5">
      <h2 className="text-base font-medium">Search by meaning</h2>
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

      <div className="mt-5 flex items-center gap-4">
        <Button onClick={start} disabled={running}>
          {running
            ? progress?.phase === "building"
              ? "Building…"
              : "Downloading…"
            : `Download ${megabytes(plan.outstanding_bytes)}`}
        </Button>
        {!running && (
          <span className="text-[13.5px] text-ink-muted">
            Optional. Nothing is downloaded until you choose to.
          </span>
        )}
      </div>

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
