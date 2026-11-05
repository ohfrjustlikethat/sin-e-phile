# Phase 8 — Player Core — MILESTONE: FIRST DEMOABLE BUILD

**Status:** in_progress · **Depends on:** 5, 7 · **Sessions:** 2–3

> The single file a session reads to know what it is doing. Generated from
> `SPEC.md` §15 by `tools/phasedoc/generate.py`. Working file, not a document.

## Goal

Search for something, press play, watch it. The vertical slice that proves the whole thing works.

## Deliverables

libmpv embedded and rendering into the Tauri window, with hardware decoding enabled and a software fallback path. A custom player UI drawn over it — not mpv's OSD: play/pause, a scrub bar with buffered-range and torrent-download indication, volume, time, fullscreen, audio-track and subtitle-track selectors, playback speed. Full keyboard control (space, arrows, `J`/`K`/`L`, `F`, `M`, `,`/`.` frame step, `[`/`]` speed). **Xbox controller control** (ADR-0041): a gamepad layer over the Gamepad API that drives focus navigation on every existing screen and the player's controls, with the ten-foot focus treatment. Chrome auto-hide. Position saving on exit and resume on reopen. A minimal detail page: poster, synopsis, cast, and a play button that goes through the resolver. Wire search → detail → resolve → play end to end.

## Exit criteria

- [ ] **E1** A user can search, click, and watch — from a local file, from the Internet Archive backend, and from a configured P2P source.
- [ ] **E2** Playback of 4K HEVC HDR uses hardware decode (verified in logs and by CPU usage).
- [ ] **E3** Seeking is frame-accurate and responsive.
- [ ] **E4** Position resume works across app restarts.
- [ ] **E5** With only an Xbox controller, a user can browse to a film, start it, pause, seek and leave the player — tested with a simulated gamepad in CI and confirmed once by the author with a real controller.
- [ ] **E6** A demo GIF of this flow exists and is in the README.
- [ ] **E7** Tag `phase-08`, and celebrate — this is the first thing worth showing anyone.

## Subtasks

- [x] **8.1** libmpv as a pinned dependency: tools/mpv/fetch.py downloads shinchiro's mpv-dev-x86_64 20261021 build (baseline x86-64, not v3, so Tier 0 CPUs without AVX2 work), verifies its SHA-256, and places libmpv-2.dll beside the app; doctor checks for it; the bundle ships it.
- [ ] **8.2** crates/player: libmpv loaded dynamically (no import library, as Spike A), a safe wrapper — options, commands, properties, an event thread — and tests against an FFmpeg-generated clip: load, play, pause, exact seek (frame-accurate), tracks, speed.
- [ ] **8.3** Embedding: libmpv renders into a child HWND of the main window (Spike A step 2), resized with it; play/pause/seek/volume/track/speed/stop commands; player state streamed to the frontend.
- [ ] **8.4** The player chrome (ADR-0020/0021): scrub bar with buffered ranges and torrent progress, volume, time, fullscreen, audio and subtitle tracks, speed; full keyboard control; auto-hide; still frame on pause.
- [ ] **8.5** Xbox controller (ADR-0041): a gamepad layer over the focus system, the player mappings, the ten-foot focus treatment; tested with a simulated gamepad.
- [ ] **8.6** Position saving on exit and resume on reopen, per profile, across restarts.
- [ ] **8.7** A minimal detail page — title, year, synopsis, cast, a Play button through the resolver — with the torrent started as soon as the page opens (Phase 7 E1, D61).
- [ ] **8.8** Search -> detail -> resolve -> play end to end, from a local file, the Internet Archive, and a P2P source.
- [ ] **8.9** Evidence: 4K HEVC HDR on hardware decode (log and CPU), frame-accurate seek, resume across restarts, play-to-first-frame timings, the demo GIF in the README; the carried Phase 7 E1; the author's batched manual checks (controller, VPN drop).

## Risks named by this phase

- **R1** — libmpv cannot be cleanly embedded in a Tauri v2 window

## Learning note

How video playback actually works — container, streams, codecs, decode, present; what hardware decode does; why embedding a player beats building one; the mpv property/command API.

---

<!-- subtask log: appended during the phase -->

## Work log
- **8.1** libmpv as a pinned dependency: tools/mpv/fetch.py downloads shinchiro's mpv-dev-x86_64 20261021  · `a3b2079`
