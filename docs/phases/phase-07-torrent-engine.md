# Phase 7 — Torrent Engine and Streaming Server

**Status:** in_progress · **Depends on:** 6 · **Sessions:** 2–3

> The single file a session reads to know what it is doing. Generated from
> `SPEC.md` §15 by `tools/phasedoc/generate.py`. Working file, not a document.

## Goal

Torrents that stream, not torrents that download. This is the hardest engineering in the project.

## Deliverables

`librqbit` integrated in-process. Magnet and `.torrent` parsing, DHT, peer discovery, trackers. **The streaming scheduler** — the heart of it: a deadline-driven piece prioritisation window that keeps the pieces just ahead of the playhead at highest priority, maintains a rarest-first background fetch for the rest, and re-prioritises within 2 s when the user seeks to an unbuffered region. A local HTTP server exposing an in-progress torrent as a seekable stream with correct HTTP range support, so libmpv can treat it as an ordinary file. File selection within multi-file torrents (pick the episode, ignore the sample and the readme). Swarm health probing: measure achievable throughput on the first pieces and expose it. Bandwidth limits, connection limits, and seeding policy, all user-configurable with sane defaults — **seeding stops at completion by default**, because seeding is uploading and uploading is distribution, and that is the side enforcement actually targets. **Network interface binding**: the engine binds to a chosen adapter so that if a VPN drops, torrent traffic stops rather than silently falling back to the real connection. This is the single highest-value user-safety feature in the phase and qBittorrent has it for the same reason. Session persistence across restarts. Per-torrent state and progress events streamed to the frontend.

## Exit criteria

- [ ] **E1** Playback begins in < 8 s on a healthy swarm (measured against a legal, well-seeded Internet Archive or Linux ISO torrent).
- [ ] **E2** Seeking to an unbuffered region resumes playback in < 5 s.
- [x] **E3** Sequential mode never starves — the buffer ahead of the playhead is maintained under a documented bandwidth floor.
- [x] **E4** Correct behaviour on a dead swarm: clear, fast failure, not an indefinite hang.
- [x] **E5** Range requests are byte-correct — verified by streaming a file and comparing its hash to a full download.
- [ ] **E6** Measured against qBittorrent on both axes and both numbers published: time-to-first-frame on a healthy swarm, and sustained bulk throughput on the same torrent. Losing on bulk is a finding, not a failure — ADR-0029 exists so the project does not have to win that comparison — but the comparison must be made rather than left implicit.
- [ ] **E7** Binding to a specific network interface works, verified by disabling that adapter mid-download and confirming traffic stops rather than falling back.
- [ ] **E8** `docs/specs/streaming-scheduler.md` explains the algorithm with a diagram.

## Subtasks

- [x] **7.1** crates/torrent: an Engine over an in-process librqbit 9 Session — data/torrents for files, session persistence and fastresume under data/, DHT on, magnet and .torrent input, add/list/pause/remove, a stats snapshot type. Config with sane defaults: bandwidth and connection limits, seeding stops at completion.
- [x] **7.2** A loopback test swarm: create a torrent from generated files (an FFmpeg test video, a sample, a readme), seed it from one session and leech from another via initial_peers. No network, no URLs, runs in CI. Every later test uses it.
- [x] **7.3** File selection within multi-file torrents: the main video, never the sample or the readme, and the requested episode when S/E is given. A pure function with a table of real-world-shaped layouts.
- [x] **7.4** The local streaming server: loopback only, a random per-session token in every path, HTTP Range (206, Content-Range, 416) over librqbit's FileStream, so libmpv treats a torrent as a file. E5: a full stream and random ranges hash-identical to the source.
- [x] **7.5** Streaming behaviour, measured: time to first bytes, seek re-prioritisation, buffer ahead of a simulated playhead under a rate-limited seeder (E3's documented floor), and a dead swarm failing fast and clearly (E4).
- [x] **7.6** Network interface binding on Windows (E7). librqbit's BindDevice returns NotSupported on Windows; vendor librqbit-dualstack-sockets 0.7.0 via [patch.crates-io] and implement it with GetAdaptersAddresses plus IP_UNICASTIF / IPV6_UNICAST_IF, so traffic stops rather than falling back when the adapter goes. Adapter listing for the UI.
- [x] **7.7** Session persistence across restarts, tested by stopping and reopening a session mid-download.
- [x] **7.8** Integration: the resolver's PlayableStream::Torrent becomes a local stream URL; per-torrent progress events to the frontend; Settings gains the network section (limits, seeding, adapter).
- [ ] **7.9** Live measurements against a legal, well-seeded torrent supplied at runtime and never committed (E1, E2), and the qBittorrent comparison on time-to-first-bytes and bulk throughput (E6), recorded in docs/eval-results.md.
- [ ] **7.10** docs/specs/streaming-scheduler.md with a diagram (E8), the learning note, and the phase close.

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
