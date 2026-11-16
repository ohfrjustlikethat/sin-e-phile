# 0047 — Stremio add-ons speak to the app too

- **Status:** Accepted 2026-11-17 by the author ("use option 2, make it so whatever structures
  makes stremio and its streaming work really well, also work in sin-e-phile"), a decision ADR-0040
  reserves to them: it widens what the app can be pointed at.
- **Date:** 2026-11-17
- **Phase:** 16
- **Amends:** [ADR-0038](0038-addons-are-remote-services-described-by-a-manifest.md) (one protocol);
  `SPEC.md` §2.1; spec_version 4.2.0
- **Specification:** [`docs/specs/addon-protocol.md`](../specs/addon-protocol.md) §10

## Context

The app's add-on protocol (version 1: a manifest, `find` and `resolve`) is its own, so in practice no
add-on existed for it but the reference one. Stremio's protocol is the one add-ons are actually
written for: a `manifest.json`, then `GET <base>/stream/<movie|series>/<id>.json` by IMDb id, answered
with streams (a torrent's info hash with the file inside it and trackers, or a web address). The
app's catalogue is keyed by IMDb ids and its torrent engine already plays a hash and a file index, so
the two meet without strain.

## Decision

**The app speaks Stremio's add-on protocol beside its own.** Which one a manifest speaks is decided by
its shape (`resources` and no `protocol`: Stremio's). Pasting a `stremio://` install link stands for
the add-on's `https` address.

- **Asked by IMDb id** (`tt…`; an episode as the series' id with `:season:episode`). An add-on that
  only knows other ids (`kitsu:`) is refused at install with that reason; one that needs setting up
  first (`configurationRequired`) is refused with "set it up on its own page, then paste the address
  it gives you". A configured add-on's settings live in its path, which is kept whole.
- **Streams it cannot play in this player are left out**, not offered and failed: a video site's id,
  a page to open elsewhere, a stream that needs request headers.
- **The same trust as any add-on**: the untrusted transport (no redirects, capped bodies), the
  manifest and every answer size- and depth-capped, every stream checked by `wire::check_stream`, and
  shown text stripped of control and invisible characters. Torrents go to the built-in engine (the
  only streaming path, ADR-0029); web addresses play directly.
- **Stremio has no second step**, so a candidate's key holds its stream, checked again on resolve.
- **Quality, size and seeders** are read from the stream's own text, where Stremio add-ons put them,
  so the app's ranking (the app decides; a list is never the default) works on them.

## What does not change

The app ships no add-on, suggests none and names none (§2.1); the user brings their own address, sees
what it declares, and confirms. The posture is Stremio's own, which §2.1 already chose to copy.

## Consequences

- A Stremio add-on that answers with torrents can be **downloaded** as well as streamed: a download is
  a torrent the plan chose (`download_title`). One that answers only with web addresses streams but
  cannot be downloaded until direct downloads exist.
- Not yet used: `bingeGroup` (keeping the same release for the next episode), `catalog` and `meta`
  resources (the app has its own catalogue), subtitle add-ons (the app has OpenSubtitles).
