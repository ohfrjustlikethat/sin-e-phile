# Phase 8 — Player Core — MILESTONE: FIRST DEMOABLE BUILD

**Status:** complete · **Depends on:** 5, 7 · **Sessions:** 2–3

> The single file a session reads to know what it is doing. Generated from
> `SPEC.md` §15 by `tools/phasedoc/generate.py`. Working file, not a document.

## Goal

Search for something, press play, watch it. The vertical slice that proves the whole thing works.

## Deliverables

libmpv embedded and rendering into the Tauri window, with hardware decoding enabled and a software fallback path. A custom player UI drawn over it — not mpv's OSD: play/pause, a scrub bar with buffered-range and torrent-download indication, volume, time, fullscreen, audio-track and subtitle-track selectors, playback speed. Full keyboard control (space, arrows, `J`/`K`/`L`, `F`, `M`, `,`/`.` frame step, `[`/`]` speed). **Xbox controller control** (ADR-0041): a gamepad layer over the Gamepad API that drives focus navigation on every existing screen and the player's controls, with the ten-foot focus treatment. Chrome auto-hide. Position saving on exit and resume on reopen. A minimal detail page: poster, synopsis, cast, and a play button that goes through the resolver. Wire search → detail → resolve → play end to end.

## Exit criteria

- [ ] **E1** A user can search, click, and watch — from a local file, from the Internet Archive backend, and from a configured P2P source. — *carried to Phase 12*
- [x] **E2** Playback of 4K HEVC HDR uses hardware decode (verified in logs and by CPU usage).
- [x] **E3** Seeking is frame-accurate and responsive.
- [x] **E4** Position resume works across app restarts.
- [ ] **E5** With only an Xbox controller, a user can browse to a film, start it, pause, seek and leave the player — tested with a simulated gamepad in CI and confirmed once by the author with a real controller. — *carried to Phase 9*
- [x] **E6** A demo GIF of this flow exists and is in the README.
- [x] **E7** Tag `phase-08`, and celebrate — this is the first thing worth showing anyone.

## Subtasks

- [x] **8.1** libmpv as a pinned dependency: tools/mpv/fetch.py downloads shinchiro's mpv-dev-x86_64 20261021 build (baseline x86-64, not v3, so Tier 0 CPUs without AVX2 work), verifies its SHA-256, and places libmpv-2.dll beside the app; doctor checks for it; the bundle ships it.
- [x] **8.2** crates/player: libmpv loaded dynamically (no import library, as Spike A), a safe wrapper — options, commands, properties, an event thread — and tests against an FFmpeg-generated clip: load, play, pause, exact seek (frame-accurate), tracks, speed.
- [x] **8.3** Embedding: libmpv renders into a child HWND of the main window (Spike A step 2), resized with it; play/pause/seek/volume/track/speed/stop commands; player state streamed to the frontend.
- [x] **8.4** The player chrome (ADR-0020/0021): scrub bar with buffered ranges and torrent progress, volume, time, fullscreen, audio and subtitle tracks, speed; full keyboard control; auto-hide; still frame on pause.
- [x] **8.5** Xbox controller (ADR-0041): a gamepad layer over the focus system, the player mappings, the ten-foot focus treatment; tested with a simulated gamepad.
- [x] **8.6** Position saving on exit and resume on reopen, per profile, across restarts.
- [x] **8.7** A minimal detail page — title, year, synopsis, cast, a Play button through the resolver — with the torrent started as soon as the page opens (Phase 7 E1, D61).
- [x] **8.8** Search -> detail -> resolve -> play end to end, from a local file, the Internet Archive, and a P2P source.
- [x] **8.9** Evidence: 4K HEVC HDR on hardware decode (log and CPU), frame-accurate seek, resume across restarts, play-to-first-frame timings, the demo GIF in the README; the carried Phase 7 E1; the author's batched manual checks (controller, VPN drop).

## Risks named by this phase

- **R1** — libmpv cannot be cleanly embedded in a Tauri v2 window

## Learning note

How video playback actually works — container, streams, codecs, decode, present; what hardware decode does; why embedding a player beats building one; the mpv property/command API.

---

<!-- subtask log: appended during the phase -->

## Work log
- **8.1** libmpv as a pinned dependency: tools/mpv/fetch.py downloads shinchiro's mpv-dev-x86_64 20261021  · `a3b2079`
- **8.2** crates/player: libmpv loaded dynamically (no import library, as Spike A), a safe wrapper — optio · `54396ea`
- **8.3** Embedding: libmpv renders into a child HWND of the main window (Spike A step 2), resized with it · `e2fd2c8`
- **8.6** Position saving on exit and resume on reopen, per profile, across restarts. · `63ded9f`
- **8.7** A minimal detail page — title, year, synopsis, cast, a Play button through the resolver — with t · `63ded9f`
- **8.5** Xbox controller (ADR-0041): a gamepad layer over the focus system, the player mappings, the ten- · `5506006`
- **8.4** The player chrome (ADR-0020/0021): scrub bar with buffered ranges and torrent progress, volume,  · `02684b6`
- **8.8** Search -> detail -> resolve -> play end to end, from a local file, the Internet Archive, and a P · `973efe0`
- **8.9** Evidence: 4K HEVC HDR on hardware decode (log and CPU), frame-accurate seek, resume across resta · `e0e5872`

<!-- closed: written at phase end -->

---

## Outcome

**Complete.** Merged as `5e5558f`.

### Evidence per criterion

**E1** — A user can search, click, and watch — from a local file, from the Internet Archive backend, and from a configured P2P source.

**Not met; carried to Phase 12.** Searching for and clicking a local file needs the scanned library index that Phase 12 builds; until then LocalFileBackend has nothing to find. The IA and P2P halves are met.

> MEASURED 2026-11-06, carried (ADR-0036): the Internet Archive and P2P paths are tested from catalogue row to first frame - crates/playback/tests/end_to_end.rs::search_to_first_frame_over_a_swarm (offline swarm) and ::live_archive_search_to_first_frame (live), and in the real window search -> detail -> Play -> picture in 5.5-7.9 s from the Internet Archive (docs/eval-results.md, docs/media/phase-08-demo.gif). A local file plays by drag-and-drop (src/features/player/useDropToPlay.ts) but cannot be searched for: LocalFileBackend::find is a stub until the library scanner.

**E2** — Playback of 4K HEVC HDR uses hardware decode (verified in logs and by CPU usage).

> cargo run --release -p sinephile-player --example hwdec -- hdr4k.mp4 8 (3840x2160 HEVC Main10 PQ/BT.2020): hwdec-current=d3d11va-copy at 18.2% of one core vs forced software 35.9%, both real-time; RTX 5070 Ti Laptop, 2026-11-06, docs/eval-results.md Phase 8 E2

**E3** — Seeking is frame-accurate and responsive.

> crates/player/tests/playback.rs::an_exact_seek_lands_on_the_frame_asked_for: paused exact seeks to 3.52 / 1.08 / 5.2 s (3.52 between keyframes) land at 3.520 / 1.080 / 5.200 in 4.5 / 2.3 / 2.8 ms (cargo test --release -p sinephile-player --test playback an_exact_seek -- --nocapture, 2026-11-06), then a frame step to 5.24; seek_to_and_wait waits on playback-restart so the landing is real; passed 25/25 runs

**E4** — Position resume works across app restarts.

> Three links, each tested: the player saves on the way out (src/features/player/PlayerScreen.test.tsx 'saves the position it reached on the way out', seen to fail with the unmount save removed); the position survives closing and reopening the database file (crates/playback/tests/plan.rs::the_position_survives_a_restart, seen to fail against a fresh file); the player starts there (crates/player/tests/playback.rs::resuming_starts_where_it_left_off)

**E5** — With only an Xbox controller, a user can browse to a film, start it, pause, seek and leave the player — tested with a simulated gamepad in CI and confirmed once by the author with a real controller.

**Not met; carried to Phase 9.** The simulated half is met and runs in CI; the criterion also needs the author's one-time check with a real Xbox controller, which only the author can do. Asked for in the batched author checks.

> Simulated half met: tools/uiaudit/controller.mjs (npm run audit:controller, CI step 'Controller flow (simulated Xbox pad)') drives the built app in headless Chrome with only pad buttons - Y, D-pad+A on the new on-screen keyboard (src/app/PadKeyboard.tsx), A result, A Play, A pause, RT seek, B leave - 8/8 checks pass 2026-11-06; seen to fail with Play's focus effect removed. Outstanding: the author's one-time check with a real controller (one is paired over Bluetooth on the dev machine).

**E6** — A demo GIF of this flow exists and is in the README.

> docs/media/phase-08-demo.gif (960x600, 88 frames, 1.2 MB), recorded 2026-11-06 from the real window with PrintWindow: search 'nosferatu', open the 1922 film, Resume from 0:52 from the Internet Archive, picture; embedded at the top of README.md

**E7** — Tag `phase-08`, and celebrate — this is the first thing worth showing anyone.

> Tag phase-08 (annotated, 1da3d88) on merge commit 5e5558f, pushed 2026-11-06 after CI run 42157416324 passed on main; the demo GIF docs/media/phase-08-demo.gif is the thing to show.

### Debt incurred

- **D63** Pressing Play the instant a film page opens still waits on torrent peer discovery (2-55 s on this network). Persist DHT state sooner than librqbit's 60 s dump, and consider prestarting the top P2P candidate from search results.
- **D64** When another application holds the default audio device exclusively, WASAPI retries for ~1.3 s before mpv plays silently; first frame waits for it. Consider audio init that does not block video, and telling the user the device is busy.
- **D65** libmpv-2.dll is fetched by tools/mpv/fetch.py for development and CI; the release bundle must ship it (and its licence notice) - Phase 27's packaging.
- **D66** ADR-0036 says PROGRESS.md lists carried criteria in their own section; tools/state/validate_state.py do_progress does not. Add the section (carried, and discharged) so a carry cannot hide among met criteria.

### Next phase starts by

Phase 9 (docs/phases/phase-09-intelligent-source-selection.md), subtask 9.1: create crates/selection with score(candidate, &Context) -> Score { total, terms: Vec<Term{name, value, weight, why}> } as a pure function over source_protocol::SourceCandidate; start with local availability, swarm health and size-versus-quality plausibility, fixture lists in crates/selection/tests/fixtures.rs, then wire it into crates/playback::rank in place of the fixed transport order.
