# Phase 13 — Learning notes

**Download it or stream it, and say which in one sentence; then a queue that keeps going
through restarts and files what it fetched into the library.** Four sections per ADR-0016 (A4).

---

## 1. What we built

- **A download queue** — `crates/downloads`: priorities, pause, tonight, per-download and
  global limits, a set number at once; it survives a restart and carries on from the pieces
  on disk.
- **Two backends** — librqbit in the app, or the user's own qBittorrent over its Web API, as
  a transport only. If qBittorrent is not running, librqbit takes the download.
- **Filing** — a finished download is in the library, matched and confirmed, with nothing
  for anyone to do.
- **The advisor** — on a film's page: the swarm measured for three seconds, the disk, this
  connection's speed by hour, and (from Phase 15) taste, giving one recommendation in one
  sentence. Right in 9 of 10 fresh trials.
- **The Downloads page** — progress, rate, people, time left, the reason it was queued, and
  every control; what is kept on this PC, watched first, and clearing it; settings. All of it
  by pad.
- **An engine bug, found by measuring** — every speed since Phase 7 was 8.4 times too low.

## 2. Why

- **A probability, not a threshold.** "1.8 MB/s against 2.0 needed" is not a yes or no: a
  swarm wobbles, and a thin one wobbles more. A stall probability says how likely a pause is,
  and the 10% line is where streaming stops being the better choice.
- **The buffer and the runtime matter.** A small shortfall on a 20-second clip is covered by
  the buffer; over two hours it is not. The first trials missed this; the model now asks for
  the rate that keeps the buffer from running dry.
- **Judge on conditions chosen after the last change.** Trials the model was tuned on cannot
  judge it, so a fresh set did (9/10), and both runs are recorded.
- **Measure what you are told.** The advisor's numbers looked plausible until a timed
  transfer disagreed with the engine's own speed by a factor of 8.4: a unit read as megabits
  that was mebibytes.
- **One queue, any backend.** The queue does not know which program moves the bytes, so a
  download files identically either way, and a missing qBittorrent is a fallback, not a
  failure.
- **A transport is all qBittorrent is.** Reading its search plugins or RSS would bring
  content sources into the app by the back door (§2.1), so the guard refuses the endpoints.
- **Keep the pieces.** Restarting, changing a limit, moving to tonight and turning a stream
  into a download all keep what is on disk; nothing is fetched twice.
- **Ask before deleting.** Cancel and Delete take two presses, and say what they will remove.

## 3. New concepts

- **A pure planner** — `crates/downloads/src/queue.rs:21` `plan`: the queue, the clock and
  the limit in, start/hold out; no I/O, so every rule is a unit test.
- **Ordering ISO times as text** — `queue.rs:26` (`t <= now`), and `crates/downloads/src/manager.rs:44`
  `iso_utc`: days-from-civil, no date library.
- **A trait object per backend** — `crates/downloads/src/backend.rs:34` `Backend` with
  `async_trait`; `manager.rs:111` `usable` picks one, falling back.
- **Interior mutability for a snapshot** — `manager.rs` `live: Mutex<HashMap<i64, Progress>>`,
  written by each tick, read by the page.
- **Moving a running torrent** — `backend.rs:156` `Rqbit::add`: remove keeping files, rename
  the folder, add again; the pieces are checked, not fetched.
- **Limits fixed at add time** — `manager.rs:282` `set_limit`: why a changed limit restarts.
- **Form-encoded HTTP and a login cookie** — `crates/downloads/src/qbittorrent.rs:61` `form`,
  `:142` `login`; `:193` `either` for an API renamed between versions.
- **A guard rule as a contract** — `tools/guard/guard.py:92` `qbittorrent-content`.
- **The normal distribution's CDF** — `crates/downloads/src/advise.rs:89` `normal_cdf`
  (Abramowitz and Stegun).
- **Stall probability** — `advise.rs:116`, with `:110` `needed_rate` (runtime plus buffer).
- **A running mean that forgets** — `crates/downloads/src/measure.rs:120` `fold`: the count is
  capped, so old samples fade.
- **Calling Win32 directly** — `measure.rs:8` `free_space` (`GetDiskFreeSpaceExW`) and `:59`
  `utc_offset_seconds` (`GetLocalTime` minus `GetSystemTime`), each `unsafe` block with its
  SAFETY note.
- **Local time without a timezone library** — `measure.rs:104` `tonight`.
- **Deleting only what is the download's** — `crates/downloads/src/storage.rs:86` `to_delete`.
- **A unit bug, and the test that pins it** — `crates/torrent/src/engine.rs:445`
  `MIB_PER_SECOND`; `crates/torrent/tests/swarm.rs` `the_reported_rate_is_the_rate_bytes_arrive_at`.
- **Keeping what the plan resolved** — `crates/playback/src/lib.rs` `PlayPlan::stream`, so
  Download fetches the same copy Play would have streamed.
- **React 19 refs as props** — `src/design-system/primitives/Button.tsx` `ref`; used by
  `src/features/detail/DownloadAdvice.tsx` to move focus to "See downloads".
- **Events from Rust to the page** — `src-tauri/src/commands/downloads.rs` `downloads_loop`
  emits; `src/features/downloads/useDownloads.ts` listens and refetches.
- **One press, one place** — `src/features/downloads/format.ts` `reorder`: renumbering instead
  of nudging a priority.

## 4. The questions

Written, not asked — Phase 13 is not a tier boundary (§10.10).

1. The swarm gives 1.8 MB/s and the film needs 2.0. Why is the advisor's answer a
   probability rather than "no"? What makes a swarm of one person less trustworthy than
   ten at the same speed?
2. A 20-second clip and a two-hour film are both streamed at 90% of their bitrate. One
   plays through, one pauses. Which, and what in `needed_rate` says so?
3. Every speed the app showed was 8.4 times too low for six phases. Why did no test catch
   it, and what kind of test did?
4. You lower a running download's limit. What happens to the torrent, why, and why is
   nothing downloaded twice?
5. qBittorrent is configured but closed. What happens to a queued download, and why may the
   app call `torrents/add` but never `search/start`?
