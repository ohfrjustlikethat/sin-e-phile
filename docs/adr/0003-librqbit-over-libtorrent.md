# 0003 — librqbit over libtorrent-rasterbar

- **Status:** Accepted
- **Date:** 2026-08-30
- **Phase:** 0 (recording a decision locked in `SPEC.md` §5)
- **Risk:** R2

## Context

Phase 7 needs a BitTorrent engine that can **stream**, not merely download: a
deadline-driven scheduler keeping pieces just ahead of the playhead at highest
priority, a rarest-first background fetch for the rest, and re-prioritisation within
2 s when the user seeks into an unbuffered region.

That requires runtime control over per-piece priority. Almost everything an engine
normally optimises for — completion time, swarm health — is in tension with what
playback needs, so the engine must expose enough control to take that trade
deliberately rather than hide it.

`SPEC.md` §5 also rules out running an external process, so it must be a library.

## Decision

**librqbit, in-process.** Pure Rust, no C++ toolchain, no FFI boundary, sequential
download and streaming support already present, and a real HTTP-streaming story
rather than one bolted on afterwards.

## Consequences

**Easier.** No FFI: no `unsafe` boundary to maintain, no build-time C++ dependency,
no marshalling between an async Rust runtime and a C++ callback model. It composes
naturally with `tokio`, which the rest of the backend already uses. For a learner,
the whole engine is readable Rust.

**Easier.** Because it is pure Rust, the piece scheduler can be reasoned about — and
if librqbit's own prioritisation proves insufficient, the source is approachable
enough to extend rather than fight.

**Harder, and this is R2.** librqbit is far younger and less battle-tested than
libtorrent-rasterbar, which is two decades old and used by essentially every serious
client. Real-world swarm behaviour — bad peers, NAT traversal, tracker quirks,
edge-case metadata — is exactly where maturity matters most and where a young
library is most likely to disappoint.

**Mitigation.** Spike B in Phase 1 measures time-to-first-usable-bytes against a
legal, well-seeded torrent *and* audits whether the API exposes runtime per-piece
priority control at all. Trigger and fallback are pre-decided in `docs/RISKS.md`.

## Alternatives Considered

**libtorrent-rasterbar via FFI.** The proven path, and the recorded fallback if R2
fires. Rejected as the default because it costs a C++ toolchain on every developer
machine and in CI, plus binding and lifetime work across an FFI boundary — roughly a
week — buying maturity that may never be needed. Deferring that cost is correct;
paying it upfront is not.

**Bundling qBittorrent or similar as a subprocess.** Explicitly rejected by §5. It
would mean shipping a second application, driving it through an API not designed for
streaming, and giving up the piece-level control that makes instant playback
possible — which is the entire point of Phase 7.

**Writing a BitTorrent client from scratch.** Rejected. Genuinely interesting, and it
would consume the whole project. The interesting problem here is the *streaming
scheduler*, not the wire protocol.
