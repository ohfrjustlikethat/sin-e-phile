import type { DownloadRow } from "@/lib/ipc";

/**
 * The Downloads page's words: sizes, rates, time left and what each download is doing,
 * the same units the advisor's sentence uses (MB is 1024², as Windows counts).
 */

const MB = 1024 * 1024;
const GB = 1024 * MB;

/** A Rust f64 arrives as `number | null` (NaN is sent as null): read as zero. */
export const n = (x: number | null | undefined): number => x ?? 0;

export function size(bytes: number): string {
  if (bytes >= GB) return `${(bytes / GB).toFixed(1)} GB`;
  if (bytes >= MB) return `${Math.round(bytes / MB)} MB`;
  return `${Math.max(1, Math.round(bytes / 1024))} KB`;
}

export function rate(bps: number): string {
  return bps >= MB ? `${(bps / MB).toFixed(1)} MB/s` : `${Math.round(bps / 1024)} KB/s`;
}

/** "about 8 minutes left", or null when there is no rate to judge by. */
export function timeLeft(remaining: number, bps: number): string | null {
  if (bps <= 0 || remaining <= 0) return null;
  const minutes = Math.ceil(remaining / bps / 60);
  if (minutes <= 1) return "about a minute left";
  if (minutes >= 90) return `about ${(minutes / 60).toFixed(1)} hours left`;
  return `about ${minutes} minutes left`;
}

/** "today", "yesterday", "3 days ago". */
export function age(days: number): string {
  const d = Math.floor(days);
  if (d <= 0) return "today";
  if (d === 1) return "yesterday";
  return `${d} days ago`;
}

/** Local "01:00" for a stored UTC start time. */
export function clockOf(iso: string): string {
  const t = new Date(iso);
  return `${String(t.getHours()).padStart(2, "0")}:${String(t.getMinutes()).padStart(2, "0")}`;
}

export function fraction(row: DownloadRow): number {
  const total = row.download.total_bytes;
  return total && total > 0 ? n(row.download.done_bytes) / total : 0;
}

/** What it is doing, in one line: "42% · 3.1 MB/s from 12 people · about 8 minutes left". */
export function status(row: DownloadRow, now: Date = new Date()): string {
  const d = row.download;
  const pct = `${Math.floor(fraction(row) * 100)}%`;
  switch (d.state) {
    case "active": {
      if (d.total_bytes === null) return "Finding the file…";
      const speed = n(row.speed);
      if (speed <= 0) return `${pct} · waiting for someone to share it`;
      const people = row.peers === 1 ? "1 person" : `${row.peers} people`;
      const left = timeLeft(d.total_bytes - n(d.done_bytes), speed);
      return [pct, `${rate(speed)} from ${people}`, left].filter(Boolean).join(" · ");
    }
    case "queued":
      if (d.not_before && new Date(d.not_before) > now) return `Starts at ${clockOf(d.not_before)}`;
      return n(d.done_bytes) > 0 ? `${pct} · waiting its turn` : "Waiting its turn";
    case "paused":
      return `Paused at ${pct}`;
    case "done":
      return "Downloaded";
    case "failed":
      return d.error ? `Stopped: ${d.error}` : "Stopped";
    default:
      return "";
  }
}

/** Running or waiting: what the masthead counts. */
export function inProgress(rows: DownloadRow[]): DownloadRow[] {
  return rows.filter((r) => ["active", "queued", "paused", "failed"].includes(r.download.state));
}
