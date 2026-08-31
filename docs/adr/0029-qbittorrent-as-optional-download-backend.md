# 0029 — qBittorrent as an optional download backend

- **Status:** Accepted · **Date:** 2026-09-01 · **Phase:** 4 (decided), built in 13
- **Amends:** `SPEC.md` §2 (locked technology), Phase 7, Phase 13 (spec_version 1.7.0)
- **Relates to:** R2, ADR-0022

## Context

The author's concern, stated directly: *"I need the streaming to work perfectly, up
to 4K"* and *"the downloading experience to rival qBittorrent"*, plus a question —
if writing a BitTorrent client is a safety risk, could qBittorrent be integrated
instead?

Three things need separating, because they have different answers.

**Writing a BitTorrent client is not a legal risk.** BitTorrent is a protocol.
qBittorrent, Transmission, libtorrent and rqbit are all lawful open-source projects
with substantial non-infringing uses. The legal exposure in this project's domain
comes from *pointing users at infringing content* — indexers, scrapers, bundled
source lists — which §2.1 already forbids absolutely and `tools/guard` enforces
against the full git history.

**There is a security dimension, and it points the other way.** A torrent client
parses hostile input from untrusted peers. Memory-safety bugs in that position are
remotely exploitable. Rust is a genuine argument *for* librqbit and *against*
embedding a C++ client, not the reverse.

**Rivalling qBittorrent is two different bars.** For *streaming*, a purpose-built
deadline scheduler should beat it — qBittorrent is not optimised for sequential
playback ahead of a playhead. For *bulk download throughput*, matching a decade of
tuned peer selection, disk I/O and protocol extensions is a hard, long fight that
`SPEC.md` never actually committed to.

## Decision

**Three backends, with one required and one optional.**

1. **librqbit, in-process — required, and the only streaming path.** Unchanged from
   §2. qBittorrent's sequential mode is not precise enough for the Phase 7 scheduler,
   so streaming cannot be delegated regardless of what else is available.
2. **librqbit for downloads — the default.** Works with no external dependency, keeps
   the §2.4 promise that the app is one folder you can copy.
3. **A user's own qBittorrent, over its Web API — optional.** Detected, never
   bundled, never required. When configured, bulk downloads can be handed to it.

**qBittorrent is never bundled.** §2's explicit non-choice stands. This is an HTTP
client talking to software the user already installed and runs, which is what Sonarr
and Radarr do.

## Why this rather than committing to beat qBittorrent

**It converts a fight into a choice.** The project no longer has to *win* the
throughput comparison to give the author qBittorrent-quality downloads; it only has
to not lose the ones it keeps. Someone who already runs qBittorrent gets its
throughput immediately. Someone who does not gets a working app with no extra
install.

**No bundling, no licence mixing, no packaging burden.** Talking to a REST API
creates no derived work. There is nothing to ship, nothing to update, and no C++
build.

**It keeps the memory-safety argument intact.** The parsing of untrusted peer traffic
stays in Rust for everything the app does itself.

**It is honest about the comparison.** Phase 7 should measure against qBittorrent on
*both* axes — time-to-first-frame and sustained bulk throughput — and publish both
numbers. If librqbit loses on bulk, that is a finding and the backend exists; if it
wins, that is a far better claim for having been measured.

## Consequences

- `SPEC.md` §2's locked technology row for Torrent becomes "librqbit in-process
  (required, and the only streaming path); a user's own qBittorrent optionally
  available as a bulk-download backend, never bundled."
- Phase 13 gains the backend abstraction and the qBittorrent client. Phase 7 is
  unchanged in scope but gains a measurement obligation.
- The download queue must be backend-agnostic, so a download can be handed to either
  and still file into the library identically on completion.
- **This must not become a way to smuggle sources in.** qBittorrent is a *transport*.
  Nothing about this ADR permits reading qBittorrent's search plugins, its RSS feeds,
  or any configured tracker list — that would be exactly the §2.1 violation the whole
  posture exists to prevent, arriving by the back door. The guard should be extended
  to check for it when this is built.
- If qBittorrent is not running when a handed-off download is queued, the app falls
  back to librqbit rather than failing. A backend that vanishes must degrade, not
  break.
