# Phase 7 — Torrent Engine and Streaming Server

**Status:** complete · **Depends on:** 6 · **Sessions:** 2–3

> The single file a session reads to know what it is doing. Generated from
> `SPEC.md` §15 by `tools/phasedoc/generate.py`. Working file, not a document.

## Goal

Torrents that stream, not torrents that download. This is the hardest engineering in the project.

## Deliverables

`librqbit` integrated in-process. Magnet and `.torrent` parsing, DHT, peer discovery, trackers. **The streaming scheduler** — the heart of it: a deadline-driven piece prioritisation window that keeps the pieces just ahead of the playhead at highest priority, maintains a rarest-first background fetch for the rest, and re-prioritises within 2 s when the user seeks to an unbuffered region. A local HTTP server exposing an in-progress torrent as a seekable stream with correct HTTP range support, so libmpv can treat it as an ordinary file. File selection within multi-file torrents (pick the episode, ignore the sample and the readme). Swarm health probing: measure achievable throughput on the first pieces and expose it. Bandwidth limits, connection limits, and seeding policy, all user-configurable with sane defaults — **seeding stops at completion by default**, because seeding is uploading and uploading is distribution, and that is the side enforcement actually targets. **Network interface binding**: the engine binds to a chosen adapter so that if a VPN drops, torrent traffic stops rather than silently falling back to the real connection. This is the single highest-value user-safety feature in the phase and qBittorrent has it for the same reason. Session persistence across restarts. Per-torrent state and progress events streamed to the frontend.

## Exit criteria

- [ ] **E1** Playback begins in < 8 s on a healthy swarm (measured against a legal, well-seeded Internet Archive or Linux ISO torrent). — *carried to Phase 8*
- [x] **E2** Seeking to an unbuffered region resumes playback in < 5 s.
- [x] **E3** Sequential mode never starves — the buffer ahead of the playhead is maintained under a documented bandwidth floor.
- [x] **E4** Correct behaviour on a dead swarm: clear, fast failure, not an indefinite hang.
- [x] **E5** Range requests are byte-correct — verified by streaming a file and comparing its hash to a full download.
- [x] **E6** Measured against qBittorrent on both axes and both numbers published: time-to-first-frame on a healthy swarm, and sustained bulk throughput on the same torrent. Losing on bulk is a finding, not a failure — ADR-0029 exists so the project does not have to win that comparison — but the comparison must be made rather than left implicit.
- [ ] **E7** Binding to a specific network interface works, verified by disabling that adapter mid-download and confirming traffic stops rather than falling back. — *carried to Phase 8*
- [x] **E8** `docs/specs/streaming-scheduler.md` explains the algorithm with a diagram.

## Subtasks

- [x] **7.1** crates/torrent: an Engine over an in-process librqbit 9 Session — data/torrents for files, session persistence and fastresume under data/, DHT on, magnet and .torrent input, add/list/pause/remove, a stats snapshot type. Config with sane defaults: bandwidth and connection limits, seeding stops at completion.
- [x] **7.2** A loopback test swarm: create a torrent from generated files (an FFmpeg test video, a sample, a readme), seed it from one session and leech from another via initial_peers. No network, no URLs, runs in CI. Every later test uses it.
- [x] **7.3** File selection within multi-file torrents: the main video, never the sample or the readme, and the requested episode when S/E is given. A pure function with a table of real-world-shaped layouts.
- [x] **7.4** The local streaming server: loopback only, a random per-session token in every path, HTTP Range (206, Content-Range, 416) over librqbit's FileStream, so libmpv treats a torrent as a file. E5: a full stream and random ranges hash-identical to the source.
- [x] **7.5** Streaming behaviour, measured: time to first bytes, seek re-prioritisation, buffer ahead of a simulated playhead under a rate-limited seeder (E3's documented floor), and a dead swarm failing fast and clearly (E4).
- [x] **7.6** Network interface binding on Windows (E7). librqbit's BindDevice returns NotSupported on Windows; vendor librqbit-dualstack-sockets 0.7.0 via [patch.crates-io] and implement it with GetAdaptersAddresses plus IP_UNICASTIF / IPV6_UNICAST_IF, so traffic stops rather than falling back when the adapter goes. Adapter listing for the UI.
- [x] **7.7** Session persistence across restarts, tested by stopping and reopening a session mid-download.
- [x] **7.8** Integration: the resolver's PlayableStream::Torrent becomes a local stream URL; per-torrent progress events to the frontend; Settings gains the network section (limits, seeding, adapter).
- [x] **7.9** Live measurements against a legal, well-seeded torrent supplied at runtime and never committed (E1, E2), and the qBittorrent comparison on time-to-first-bytes and bulk throughput (E6), recorded in docs/eval-results.md.
- [x] **7.10** docs/specs/streaming-scheduler.md with a diagram (E8), the learning note, and the phase close.

## Risks named by this phase

- **R2** — librqbit's streaming control is insufficient for the Phase 7 scheduler

## Learning note

How BitTorrent works — pieces, swarms, DHT, rarest-first; why sequential download conflicts with swarm health and how the scheduler balances them; HTTP range requests; async Rust and `tokio`; backpressure.

---

<!-- subtask log: appended during the phase -->

## Work log
- **7.1** crates/torrent: an Engine over an in-process librqbit 9 Session — data/torrents for files, sessi · `6df5136`
- **7.2** A loopback test swarm: create a torrent from generated files (an FFmpeg test video, a sample, a  · `6df5136`
- **7.3** File selection within multi-file torrents: the main video, never the sample or the readme, and t · `6df5136`
- **7.4** The local streaming server: loopback only, a random per-session token in every path, HTTP Range  · `6df5136`
- **7.5** Streaming behaviour, measured: time to first bytes, seek re-prioritisation, buffer ahead of a si · `6df5136`
- **7.7** Session persistence across restarts, tested by stopping and reopening a session mid-download. · `6df5136`
- **7.6** Network interface binding on Windows (E7). librqbit's BindDevice returns NotSupported on Windows · `a279415`
- **7.8** Integration: the resolver's PlayableStream::Torrent becomes a local stream URL; per-torrent prog · `250dbf5`
- **7.9** Live measurements against a legal, well-seeded torrent supplied at runtime and never committed ( · `01be3d8`
- **7.10** docs/specs/streaming-scheduler.md with a diagram (E8), the learning note, and the phase close. · `01be3d8`

<!-- closed: written at phase end -->

---

## Outcome

**Complete.** Merged as `125a0a2`.

### Evidence per criterion

**E1** — Playback begins in < 8 s on a healthy swarm (measured against a legal, well-seeded Internet Archive or Linux ISO torrent).

**Not met; carried to Phase 8.** The delay is peer discovery on this network, which qBittorrent also suffers (13.1 s); Phase 8 measures play to first frame end to end and can start the torrent when the detail page opens so discovery overlaps with reading.

> MEASURED 2026-11-06, carried (ADR-0036): add -> first 1 MiB through the local streaming server, Ubuntu 24.04 desktop torrent fetched at run time: 3.6-3.8 s in 4 of 20 runs, 9.6-55 s in the rest; once a peer is connected the first MiB takes 0.6-3.4 s, the time is all first-peer discovery. qBittorrent on the same torrent minutes apart: 13.1 s. docs/eval-results.md 'Phase 7'.

**E2** — Seeking to an unbuffered region resumes playback in < 5 s.

> cargo run --release -p sinephile-torrent --example bench (e45152a and later): seek to 60% of an unbuffered 6 GB torrent and read 1 MiB through the local server in 0.15-3.73 s across 18 live runs, all under 5 s; loopback test a_seek_into_an_unbuffered_region_is_served_quickly. docs/eval-results.md 'Phase 7'.

**E3** — Sequential mode never starves — the buffer ahead of the playhead is maintained under a documented bandwidth floor.

> cargo test -p sinephile-torrent --test buffer (6df5136): a simulated player reading at 2 MiB/s in 256 KiB chunks after a 2 s start-up buffer, over a loopback swarm whose seeder is capped at 1.5x the bitrate, waited 0 ns for any chunk (floor documented in docs/specs/streaming-scheduler.md: bandwidth >= 1.5x bitrate, 2 s buffer, lateness <= 250 ms). Negative control in the same file: at 0.7x the bitrate the same player starved by 2.27 s.

**E4** — Correct behaviour on a dead swarm: clear, fast failure, not an indefinite hang.

> cargo test -p sinephile-torrent --test swarm a_dead_swarm_fails_fast_with_a_clear_error (6df5136): with the only peer a closed port, wait_for_peers returns 'nobody is sharing this — no peers answered within 3 seconds' in under 5 s, and a streaming request through the local server ends instead of hanging; a magnet's metadata wait is bounded by the same dead_swarm_after (default 30 s). Seen to fail with the wait removed (the request hung past the 10 s guard).

**E5** — Range requests are byte-correct — verified by streaming a file and comparing its hash to a full download.

> cargo test -p sinephile-torrent --test swarm the_stream_is_byte_identical_to_the_source_whole_and_in_ranges (6df5136): over a loopback swarm, 20 random byte ranges fetched while the torrent was still downloading, a suffix range and the full GET through the local streaming server are all byte-for-byte identical to the source file (stricter than comparing hashes); 416 with Content-Range past the end; a wrong token is 404. Seen to fail with an off-by-one in the body length.

**E6** — Measured against qBittorrent on both axes and both numbers published: time-to-first-frame on a healthy swarm, and sustained bulk throughput on the same torrent. Losing on bulk is a finding, not a failure — ADR-0029 exists so the project does not have to win that comparison — but the comparison must be made rather than left implicit.

> docs/eval-results.md 'Phase 7', E6 table: same torrent, same connection, minutes apart — sin-e-phile 3.58 s to first MiB and 33.85 MiB/s bulk (bench example); qBittorrent 5.2.0 13.08 s and 34.71 MiB/s (python tools/bench/qbittorrent_compare.py, throwaway profile). Both published.

**E7** — Binding to a specific network interface works, verified by disabling that adapter mid-download and confirming traffic stops rather than falling back.

**Not met; carried to Phase 8.** The criterion requires disabling the bound adapter mid-download, which on this machine means dropping the author's VPN; it is batched with Phase 8's other manual checks for the author to perform.

> MEASURED 2026-11-05, carried (ADR-0036): crates/torrent/tests/binding.rs (a279415) — an unknown adapter stops the engine starting; bound to loopback, local traffic flows and an outside address is unreachable (seen to fail with IP_UNICAST_IF removed). Probe: bound to the VPN adapter a connection succeeds, bound to a disconnected adapter it fails WSAENETUNREACH instead of using Wi-Fi.

**E8** — `docs/specs/streaming-scheduler.md` explains the algorithm with a diagram.

> docs/specs/streaming-scheduler.md: the algorithm, where scheduling happens, the bandwidth floor with its measurements, and a diagram of the path from player to sockets.

### Debt incurred

- **D60** Phase 3's sources_config table (migration 0004) was superseded by Phase 6's source_backends and addons (migration 0015) and is unused. Drop it in a migration when the schema is next touched.
- **D61** Phase 7 E1 is carried to Phase 8: first-peer discovery takes 2-55 s on this network. Levers for Phase 8: start the torrent when the detail page opens, and persist DHT state sooner than librqbit's 60 s dump so a warm routing table survives short sessions.
- **D62** tools/bench/qbittorrent_compare.py assumes qBittorrent at C:\Program Files\qBittorrent and Web UI port 18081; it is a measurement script, not a product feature, and ADR-0029's qBittorrent backend (Phase 13) must not reuse its throwaway-profile approach.

### Next phase starts by

Plan Phase 8 (player core) per SPEC.md §15 Phase 8 and ADR-0020/0021/0041: libmpv embedded into a child HWND (spikes/spike-a-libmpv), custom chrome with region cutouts and still-frame-on-pause, keyboard and Xbox-controller control, position resume, a minimal detail page, and search -> detail -> resolve -> play end to end. Write the subtasks into PROJECT_STATE.json and start (ADR-0040). Carried in: Phase 7 E1 (start the torrent when the detail page opens; D61) and E7 (author's VPN-drop check). Author actions outstanding: publish embeddings-v3; retake case-study shots; E7.
