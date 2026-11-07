import { describe, expect, it } from "vitest";
import type { DownloadRow } from "@/lib/ipc";
import { age, inProgress, rate, reorder, size, status, timeLeft } from "./format";

const MB = 1024 * 1024;

function row(over: Partial<DownloadRow["download"]>, speed: number | null = 0, peers = 0): DownloadRow {
  return {
    speed,
    peers,
    download: {
      id: 1, media_item_id: 7, title: "Sunrise", state: "active", backend: "librqbit", priority: 0,
      done_bytes: 0, total_bytes: null, limit_kib: null, not_before: null, reason: null, error: null,
      file_path: null, ...over,
    },
  };
}

describe("the Downloads page's words (SPEC.md Phase 13)", () => {
  it("says sizes and rates in the advisor's units", () => {
    expect(size(2.5 * 1024 * MB)).toBe("2.5 GB");
    expect(size(700 * MB)).toBe("700 MB");
    expect(size(10)).toBe("1 KB");
    expect(rate(3.14 * MB)).toBe("3.1 MB/s");
    expect(rate(300 * 1024)).toBe("300 KB/s");
  });

  it("says time left only when there is a rate to judge it by", () => {
    expect(timeLeft(60 * MB, MB)).toBe("about a minute left");
    expect(timeLeft(480 * MB, MB)).toBe("about 8 minutes left");
    expect(timeLeft(6000 * MB, MB)).toBe("about 1.7 hours left");
    expect(timeLeft(100, 0)).toBeNull();
  });

  it("says what each download is doing in one line", () => {
    expect(status(row({ total_bytes: 1000 * MB, done_bytes: 420 * MB }, 4 * MB, 12))).toBe(
      "42% · 4.0 MB/s from 12 people · about 3 minutes left",
    );
    expect(status(row({ total_bytes: 1000 * MB, done_bytes: 420 * MB }, 0, 0))).toBe(
      "42% · waiting for someone to share it",
    );
    expect(status(row({}))).toBe("Finding the file…");
    expect(status(row({ state: "paused", total_bytes: 100, done_bytes: 50 }))).toBe("Paused at 50%");
    expect(status(row({ state: "failed", error: "no download backend" }))).toBe("Stopped: no download backend");
    const tonight = row({ state: "queued", not_before: "2026-11-08T01:00:00Z" });
    expect(status(tonight, new Date("2026-11-07T20:00:00Z"))).toMatch(/^Starts at \d\d:\d\d$/);
    expect(status(tonight, new Date("2026-11-08T02:00:00Z"))).toBe("Waiting its turn");
    expect(status(row({ total_bytes: 100, done_bytes: null }, null, 1))).toBe("0% · waiting for someone to share it");
  });

  it("moves a download exactly one place, whatever the priorities were", () => {
    // Queue order: A (5), B (5), C (0), D (0).
    const rows = [
      row({ id: 1, priority: 5 }), row({ id: 2, priority: 5 }), row({ id: 3, priority: 0 }), row({ id: 4, priority: 0 }),
    ];
    // C up one: A, C, B, D numbered 4, 3, 2, 1; every priority changes.
    expect(reorder(rows, 2, -1)).toEqual([[1, 4], [3, 3], [2, 2], [4, 1]]);
    // Applying it and sorting by priority (desc, then id) gives exactly that order.
    const next = new Map(reorder(rows, 2, -1));
    const sorted = rows
      .map((r) => ({ id: r.download.id, p: next.get(r.download.id) ?? r.download.priority }))
      .sort((a, b) => b.p - a.p || a.id - b.id)
      .map((r) => r.id);
    expect(sorted).toEqual([1, 3, 2, 4]);
    expect(reorder(rows, 0, -1)).toEqual([]);
    expect(reorder(rows, 3, 1)).toEqual([]);
    // Already numbered: only the two that swap change.
    const numbered = [row({ id: 1, priority: 3 }), row({ id: 2, priority: 2 }), row({ id: 3, priority: 1 })];
    expect(reorder(numbered, 2, -1)).toEqual([[3, 2], [2, 1]]);
  });

  it("counts what is in progress, not what is done", () => {
    const rows = ["active", "queued", "paused", "failed", "done"].map((state) => row({ state }));
    expect(inProgress(rows)).toHaveLength(4);
    expect(age(0.4)).toBe("today");
    expect(age(1.2)).toBe("yesterday");
    expect(age(9)).toBe("9 days ago");
  });
});
