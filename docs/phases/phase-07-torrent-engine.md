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
- [ ] **E3** Sequential mode never starves — the buffer ahead of the playhead is maintained under a documented bandwidth floor.
- [ ] **E4** Correct behaviour on a dead swarm: clear, fast failure, not an indefinite hang.
- [ ] **E5** Range requests are byte-correct — verified by streaming a file and comparing its hash to a full download.
- [ ] **E6** Measured against qBittorrent on both axes and both numbers published: time-to-first-frame on a healthy swarm, and sustained bulk throughput on the same torrent. Losing on bulk is a finding, not a failure — ADR-0029 exists so the project does not have to win that comparison — but the comparison must be made rather than left implicit.
- [ ] **E7** Binding to a specific network interface works, verified by disabling that adapter mid-download and confirming traffic stops rather than falling back.
- [ ] **E8** `docs/specs/streaming-scheduler.md` explains the algorithm with a diagram.

## Subtasks

_none authored yet — write them when the phase is planned._

## Risks named by this phase

- **R2** — librqbit's streaming control is insufficient for the Phase 7 scheduler

## Learning note

How BitTorrent works — pieces, swarms, DHT, rarest-first; why sequential download conflicts with swarm health and how the scheduler balances them; HTTP range requests; async Rust and `tokio`; backpressure.

---

<!-- subtask log: appended during the phase -->

## Work log

