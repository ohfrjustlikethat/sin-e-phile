# The streaming scheduler

**Status:** implemented (Phase 7) · **Decides:** [ADR-0003](../adr/0003-librqbit-over-libtorrent.md) (librqbit in-process)
**Code:** `crates/torrent/` — `engine.rs` (the engine and its policies), `server.rs` (the local streaming server), `select.rs` (which file), `play.rs` (resolver → player) · vendored `crates/vendor/librqbit-dualstack-sockets/` (Windows adapter binding)
**Tests:** `crates/torrent/tests/` — `swarm.rs`, `buffer.rs`, `binding.rs`, all against a real two-node swarm on loopback

How a torrent becomes something a player can treat as an ordinary file, and why it
does not starve while it does.

---

## 1. The shape

```
                  ┌──────────────────────── sin-e-phile process ─────────────────────────┐
                  │                                                                        │
 libmpv  ──HTTP──▶│ StreamServer (127.0.0.1, random port, /<token>/<route>)               │
 (player)  Range  │      │ one request = one fresh reader, seeked to the range start       │
                  │      ▼                                                                 │
                  │ TorrentStream source ── finished? ──yes──▶ the file on disk            │
                  │      │ no                                                              │
                  │      ▼                                                                 │
                  │ wait_for_peers (dead swarm → clear error after N s)                    │
                  │      ▼                                                                 │
                  │ librqbit FileStream ── its read position defines ──┐                  │
                  │                                                     ▼                  │
                  │               ┌──────── piece picker, in this order ────────┐          │
                  │               │ 1. steal from a peer 10× slower              │          │
                  │               │ 2. PRIORITY: pieces in the 32 MiB window     │◀─ every  │
                  │               │    ahead of each open stream's position      │   open   │
                  │               │ 3. the normal queue (rarest-first-ish)       │   stream │
                  │               │ 4. steal from a peer 3× slower               │          │
                  │               └──────────────────────────────────────────────┘          │
                  │                     │                                                  │
                  │      sockets ── IP_UNICAST_IF ──▶ only the chosen adapter (VPN)        │
                  └────────────────────────────────────────────────────────────────────────┘
```

## 2. Where the scheduling actually happens

Spike B (Phase 1, `docs/eval-results.md`) read librqbit 9's picker and found most of the
"deadline-driven piece prioritisation window" the spec asks for already there:

- Each open `FileStream` maintains a **32 MiB lookahead window** from its read
  position, converted to piece indices. Those pieces are the picker's **priority** set,
  taken ahead of the normal queue.
- **A seek moves the window at once**: the next piece request is computed from the new
  position, so re-prioritisation costs one round of requests, not a timer.
- Multiple open streams are interleaved fairly, so a player opening a second
  connection for a seek does not lock out the first.
- Everything outside the windows comes from the normal queue — the **background fetch**
  that keeps the swarm healthy and the download finishing.

So Phase 7 drives and measures that scheduler rather than writing a second one. What
this crate adds is the part librqbit leaves to the application:

| Concern | Where | Rule |
|---|---|---|
| A seekable file for the player | `server.rs` | Loopback only; random 32-character token per launch; `Range` (`a-b`, `a-`, `-n`) → `206` + `Content-Range`; past the end → `416`; one request per connection |
| A dead swarm | `engine.rs` | A magnet's metadata wait and a stream's first-peer wait are both bounded by `dead_swarm_after` (30 s default), then a plain message: "nobody is sharing this" |
| Seeding | `engine.rs` | Stops at completion by default: the engine pauses a finished torrent, and a finished file is served from disk |
| Which file | `select.rs` | Videos only; never a sample, trailer or extra unless nothing else exists; the episode asked for or nothing |
| VPN safety | vendored sockets | Bound to an adapter, a socket's traffic cannot leave by another; an unknown adapter stops the engine starting |

## 3. The bandwidth floor (E3)

"Sequential mode never starves — the buffer ahead of the playhead is maintained under a
documented bandwidth floor." The floor, as measured by `tests/buffer.rs`:

> **Available bandwidth of at least 1.5× the stream's bitrate, with a 2-second start-up
> buffer.** Above it, no chunk reaches the player more than 250 ms late.

The test plays a 2 MiB/s "film" in 256 KiB chunks, each due at a fixed moment, over a
loopback swarm whose seeder is capped:

| Seeder capped at | Worst lateness | |
|---|---|---|
| 1.5× the bitrate | **0 ns** | never waited |
| 0.7× the bitrate | **2.27 s** | starved — the negative control |

Why 1.5× is enough: the window is 32 MiB, sixteen seconds of a 2 MiB/s stream, so once it
has filled the player has sixteen seconds of slack against a slow patch. Why 1.0× is not:
at exactly the bitrate nothing ever accumulates ahead of the playhead, and any jitter in
piece arrival becomes a stall.

The window being a compile-time constant in librqbit is a known limit (Spike B): a
high-bitrate 4K stream at 60 Mbit/s has only about four seconds in 32 MiB. Phase 9's
source scoring prefers candidates whose measured swarm throughput clears the floor for
their own bitrate.

## 4. Measured on a real swarm (E1, E2, E6)

See `docs/eval-results.md`, "Phase 7". Method: `cargo run --release -p sinephile-torrent
--example bench -- <.torrent> <scratch>` against a legal, well-seeded Linux distribution
torrent supplied at run time; `python tools/bench/qbittorrent_compare.py` runs qBittorrent
on the same torrent, in a throwaway profile, the same way.

## 5. What is not done here

- **No webseeds.** librqbit does not support BEP-19, so the Internet Archive's torrents
  (which rely on webseeds) are played over HTTP instead (Spike B, Phase 6).
- **The window size is fixed** at 32 MiB (above).
- **Time to first frame** is Phase 8's to measure, with the player. Phase 7 measures the
  bytes a player would need.
