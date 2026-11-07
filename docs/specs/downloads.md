# Downloads (Phase 13)

A queue that survives restarts, moved by whichever backend is available, filing every
finished download into the library as a confirmed match. An advisor on each film's page says
whether to stream or download, in one sentence. Measured numbers are in
`docs/eval-results.md` (Phase 13).

## 1. The queue — `crates/downloads/src/queue.rs`, `manager.rs`

A download is a row in `downloads` (migration 0023): title, `source`, optional `.torrent`
bytes, the backend and its id for it, priority, state, sizes, an optional per-download limit
(KiB/s), an optional `not_before` (ISO 8601 UTC, which orders as text), the advisor's
sentence as `reason`, and the file it made.

States: `queued → active → done`, with `paused` (only by the user), `failed` and `cancelled`.

`plan(open, now, max_active)` is pure: the runnable (queued, and `not_before` passed),
highest priority first then oldest, fill up to `max_active`; a running download outranked by a
waiting one is held (paused, back to queued). `Manager::tick` (once a second, from
`src-tauri/src/commands/downloads.rs` `downloads_loop`) follows what runs, files what
finished, then applies the plan.

- **Restart**: an active download its backend no longer holds is added again into the same
  folder; the pieces on disk are checked, not fetched again.
- **A changed limit** (`set_limit`): librqbit fixes a torrent's limit when it is added, so a
  running download is removed from its backend (files kept) and queued; the next tick adds it
  with the new limit.
- **Tonight** (`schedule`): a running download given a later start is paused in its backend
  and queued; at the time, it carries on.
- **Global limits** are the engine's (Settings → Network), applied live by `set_torrent_settings`.

## 2. Backends — `backend.rs`, `qbittorrent.rs` (ADR-0029)

`Backend`: `name`, `available`, `accepts`, `holds`, `add`, `progress`, `pause`, `resume`,
`remove`, `video_file`. A download uses its own backend if it is available and accepts it,
otherwise the first that does: a configured qBittorrent that is not running falls back to
librqbit (E3).

- **librqbit** (`Rqbit`): every download in `Downloads/<title>/` in the data folder. A torrent
  the film page already started for streaming (D61) is moved there with its pieces (remove
  keeping files, rename the folder, add again) instead of starting over.
- **qBittorrent** (the user's own, never bundled): Web API v2, form-encoded, logs in on 403,
  `stop`/`start` with a fallback to the older `pause`/`resume`. Given a link (`link`: the
  source's magnet, or one built from the hash and the source's trackers) and the same per-title
  folder librqbit uses, with `contentLayout=NoSubfolder`, so a download lands at the same path
  either way. `torrents/add` is taken on 200, or 202 with JSON counts from 5.2 (`took`). A
  transport only: the endpoints it uses are listed at the top of the file, and `tools/guard`
  rule `qbittorrent-content` rejects any call to its search, RSS or tracker endpoints (E4).
  Tested against a stand-in of every version's answers (`tests/qbittorrent.rs`) and, on demand,
  against a real qBittorrent in isolated instances (`tests/qbittorrent_real.rs`, E2).

`source` for a torrent found by a source: `infohash:<hex>|<tracker>|<tracker>`
(`source_for` / `source_of`). Trackers come from the source at run time; the app ships none.

## 3. Filing — `Manager::file`

The downloads folder is registered as a library folder; the finished file is recorded like
any scanned file. A download that knew its catalogue item is matched to it, **confirmed**, with
no review (E6); one that did not is identified like any other file.

## 4. The advisor — `advise.rs`, `measure.rs`

Inputs: the chosen copy's size, the catalogue runtime, the swarm (three seconds of the
engine's rate on the torrent the page already started, the later half kept: `swarm_from`),
free disk within the size budget (`storage::room`), this connection's mean rate by local hour
(`measure::fold`, a setting folded once a minute while anything downloads), and taste
(`Taste::neutral()` until Phase 15).

- **Stall probability**: throughput taken as normal, its spread widened for a thin swarm
  (`THIN_SWARM / √peers`), against the rate that keeps the buffer from running dry:
  `bytes / (runtime + buffer)` × `HEADROOM` 1.05.
- **Order**: a copy on this PC wins; nobody sharing → queue it; no room → stream, and say so;
  under 10% stall → stream (unless a likely favourite worth keeping); a large file on a slow
  evening with a much faster night → tonight; otherwise download, with how long.
- One sentence, under 160 characters, always (E8). The other choice is always one press away.

Judged on ten loopback swarm conditions chosen after the model's last change: 9 of 10 (E1).

## 5. Storage — `storage.rs`

`stored` lists finished downloads, watched first (`ProfileRepository::finished`: a completed
watch, or a position past 95%), then largest, then oldest. `clear` deletes the download's own
folder (subtitles included; never the downloads folder, and only the file when it lives
outside it), its library row, and its download row. The budget (`downloads.budget_gib`) caps
the room the advisor sees.

## 6. The surface

- **Masthead**: "Downloads" with the number in progress, when there is one.
- **Downloads page** (`src/features/downloads/DownloadsScreen.tsx`): each download's state in
  one line (percent, rate, people, time left), its reason, pause/resume, sooner/later,
  tonight, a speed limit, cancel (asks first); what is kept and clearing it (asks first); how
  many at once, the size budget, the user's qBittorrent and a check that it answers.
- **Film page** (`src/features/detail/DownloadAdvice.tsx`): the sentence and Download (or
  Download tonight), under Play; then "Downloading." and See downloads, with focus following.
- **Notices** (`DownloadNotices.tsx`): a toast when a download finishes or stops. Windows
  notifications wait for Phase 20 (D78).

IPC (`src-tauri/src/commands/downloads.rs`, thin): `downloads`, `download_advice`,
`download_title`, `download_pause`, `download_resume`, `download_cancel`, `download_priority`,
`download_limit`, `download_tonight`, `downloads_settings`, `set_downloads_settings`,
`set_qbittorrent`, `qbittorrent_check`, `downloads_storage`, `download_clear`,
`downloads_folder`. Events: `downloads-changed` (each tick), `download-finished`,
`download-failed` (the title), `library-changed`.
