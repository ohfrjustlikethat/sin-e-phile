# Phase 10 — Subtitle Pipeline

**Status:** complete · **Depends on:** 8 · **Sessions:** 2

> The single file a session reads to know what it is doing. Generated from
> `SPEC.md` §15 by `tools/phasedoc/generate.py`. Working file, not a document.

## Goal

Subtitles are in sync on the first frame, always, with no user action. Ever.

## Deliverables

The priority chain from Section 14: (1) embedded tracks in the media file — the default and most common path, always perfectly synced; (2) sidecar files found next to the media or inside the torrent; (3) OpenSubtitles hash match, quota-budgeted and cached; (4) other free providers; (5) alignment of whatever was found. **The aligner** (`crates/subtitle-align`, a standalone crate): extract a voice-activity signal from the audio track via FFmpeg + a VAD, build a comparable signal from the subtitle timings, and solve for offset *and* framerate scale by cross-correlation — handling the 23.976↔25 fps drift that a pure offset cannot. Confidence scoring; refuse to apply a low-confidence alignment and fall back rather than making things worse. A live `±` nudge in the player (`G`/`H` keys) that persists per file and per subtitle. Styling controls: font, size, position, background, and proper ASS/SSA rendering preserved when the source uses it. Language preference ordering per profile. `fixtures/subtitles/` corpus and the alignment eval harness. Tier 0: chain steps 1–3 plus the duration heuristic and the manual nudge; alignment is Tier 1+.

## Exit criteria

- [x] **E1** > 90% of the fixture corpus aligns to within 150 ms.
- [x] **E2** A 23.976 vs 25 fps mismatch is detected and corrected, not just offset.
- [x] **E3** Embedded tracks are found and used without any network request.
- [x] **E4** The OpenSubtitles daily quota is respected and never exceeded; quota state is visible in Settings.
- [x] **E5** The manual nudge persists across sessions for the same file.
- [x] **E6** `crates/subtitle-align` is independently testable and documented as a standalone crate.

## Subtasks

- [x] **10.1** crates/subtitle-align (MIT/Apache, no FFmpeg): cue timings read and rewritten in place for SRT, WebVTT and ASS/SSA (styling preserved); a pure-Rust band-energy VAD with a syllable-modulation test; offset and framerate scale solved together by FFT cross-correlation over the 23.976/24/25 scale set, the plateau's centre taken; two-part split for inserted or removed intros; a distinctness confidence that refuses rather than guess; Tier 0 duration heuristic. Unit tests on synthetic dialogue.
- [x] **10.2** fixtures/subtitles/ corpus (tools/subcorpus: Windows speech synthesis over room, street and music beds, traces from the app's own VAD, eight damaged copies per film: offsets, 23.976<->25 and 24->25 scales, intros removed and added) plus negative controls (one film's subtitles against another's speech must be refused); cargo run -p eval -- subs reports aligned share (>90% target, a copy aligns when 95% of its cues land within 150 ms), refusals, false accepts and VAD agreement.
- [x] **10.3** The subtitle chain in a testable crate (crates/subtitles): embedded tracks first (from the player's track list, no network), then sidecar files next to the media (same stem, language suffixes) or in the torrent's file list, then OpenSubtitles by movie hash with the user's own key (no key ships), a persisted daily quota budget that is never exceeded and a cache, then other providers behind a trait; language preference ordering per profile. A test proves an embedded track is used with zero network requests.
- [x] **10.4** Wiring in the app: audio extracted with FFmpeg (16 kHz mono, first 20 minutes) for alignment on Tier 1+, the duration heuristic on Tier 0; an aligned or found subtitle loaded into mpv (sub-add); ASS rendered as authored; Settings shows OpenSubtitles key and quota state and subtitle style (font, size, position, background).
- [x] **10.5** The nudge: G and H (and the pad) shift subtitles by 100 ms in the player, persisted per file and per subtitle in the database (migration + repository method + repository_surface test) and reapplied on the next open.
- [ ] **10.6** docs/specs/subtitle-alignment.md, crates/subtitle-align README as a standalone crate, learning note, evidence for E1-E6, regression check, close.

## Risks named by this phase

- **R6** — Subtitle alignment does not reach 90% on the fixture corpus

## Learning note

Voice activity detection; cross-correlation as a way to align two signals; why framerate scaling is a multiplication and offset is an addition, and why you need both; confidence thresholds and knowing when not to act.

---

<!-- subtask log: appended during the phase -->

## Work log
- **10.1** crates/subtitle-align (MIT/Apache, no FFmpeg): cue timings read and rewritten in place for SRT,  · `09cdd8d`
- **10.2** fixtures/subtitles/ corpus (tools/subcorpus: Windows speech synthesis over room, street and musi · `09cdd8d`
- **10.3** The subtitle chain in a testable crate (crates/subtitles): embedded tracks first (from the playe · `ece5de6`
- **10.4** Wiring in the app: audio extracted with FFmpeg (16 kHz mono, first 20 minutes) for alignment on  · `2c331de`
- **10.5** The nudge: G and H (and the pad) shift subtitles by 100 ms in the player, persisted per file and · `2c331de`

<!-- closed: written at phase end -->

---

## Outcome

**Complete.** Merged as `c48c546`.

### Evidence per criterion

**E1** — > 90% of the fixture corpus aligns to within 150 ms.

> 09cdd8d: cargo run --release -p eval -- subs --report over fixtures/subtitles (10 films of synthesised dialogue over room/street/music beds, traces from the app's VAD, 80 damaged copies): 79/80 = 98.8% of copies have >=95% of cues within 150 ms (target >90%); 4 films held out from tuning: 31/32 = 96.9%; false accepts against the wrong film 0/10. Seen to fail: scales reduced to 1.0 gives 48.8%. docs/eval-results.md Phase 10.

**E2** — A 23.976 vs 25 fps mismatch is detected and corrected, not just offset.

> 09cdd8d: crates/subtitle-align/tests/align.rs a_framerate_mismatch_is_a_scale_not_an_offset (23.976->25 found as scale 25/23.976, >97% of cues land; with offsets only, it refuses or lands <50%); eval subs: every scaled copy aligned, 30/30 (23.976->25, 25->23.976, 24->25) and 10/10 intro-added-and-scaled.

**E3** — Embedded tracks are found and used without any network request.

> ece5de6/2c331de: crates/subtitles/tests/chain.rs an_embedded_track_is_used_without_a_single_request (FakeTransport.request_count()==0; seen to fail when pick always fetches); crates/subtitles/tests/plan.rs an_embedded_track_in_your_language_is_shown_as_it_is (embedded beats the file beside it). The app path (src-tauri/src/commands/subtitles.rs open_for) builds the provider only from the profile's own key and calls plan, which calls pick first.

**E4** — The OpenSubtitles daily quota is respected and never exceeded; quota state is visible in Settings.

> ece5de6/2c331de: crates/subtitles/tests/chain.rs the_daily_quota_is_never_exceeded (after FREE_DAILY=5 downloads no further request is made; seen to fail with the budget check off by one), the_providers_own_count_wins_when_it_is_lower, a_failed_attempt_gives_its_reservation_back; persistence repository_surface subtitle_repository_surface (reserve is one conditional UPDATE). Visible in Settings: src/features/settings/SubtitlesSection.test.tsx 'shows how much of today's quota is left' ('3 of 5 downloads left today') and 'believes OpenSubtitles when it says fewer are left'.

**E5** — The manual nudge persists across sessions for the same file.

> 2c331de: crates/persistence/tests/repository_surface.rs subtitle_repository_surface (per file and subtitle; zero clears); crates/subtitles/tests/plan.rs the_nudge_from_last_time_comes_back (keyed by the file's hash); real window 2026-11-06: three subtitle_nudge(-100) on Garden.mkv gave -300, the app was closed and reopened on the same file, and the nudge in force was -300.

**E6** — `crates/subtitle-align` is independently testable and documented as a standalone crate.

> 09cdd8d: crates/subtitle-align is its own MIT/Apache crate with no app or FFmpeg dependency (Cargo.toml: rustfft only); cargo test -p subtitle-align runs 8 unit + 7 integration tests on synthetic dialogue and the README's usage example as a doctest; README documents use, modules, measured numbers and licence; docs/specs/subtitle-alignment.md the method.

### Debt incurred

- **D70** Subtitle files inside a torrent are found (crates/subtitles/src/files.rs in_torrent) but not fetched: plan() returns Off for Origin::Torrent. Fetch the file through the engine (select the file index) and treat it as a sidecar.
- **D71** The OpenSubtitles hash, and so downloads and hash-keyed nudges, need a local file; a stream (Internet Archive, torrent) is keyed by its address and gets no download. Hash a stream from HTTP range requests for the first and last 64 KiB (the streaming server supports ranges).
- **D72** The subtitle corpus is synthetic speech (Windows' synthesiser) over synthetic beds. Add a handful of real public-domain film clips with hand-timed subtitles we write ourselves, to measure the VAD on a real mix (SPEC.md R6).

### Next phase starts by

Phase 11 subtask 11.1 (docs/phases/phase-11-player-experience-layer.md): migration 0019 airing_schedule (series media_item_id, episode number, airs_at UTC); write it from crates/catalogue/src/anime.rs where Media.next_airing is seen; EpisodeRepository::seasons/episodes_of/next_airing with repository_surface lines; a series branch on the detail page (src/features/detail) listing seasons and episodes and the airing line from a pure function airsLine(airsAt, now).
