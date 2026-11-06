# Phase 10 — Subtitle Pipeline

**Status:** in_progress · **Depends on:** 8 · **Sessions:** 2

> The single file a session reads to know what it is doing. Generated from
> `SPEC.md` §15 by `tools/phasedoc/generate.py`. Working file, not a document.

## Goal

Subtitles are in sync on the first frame, always, with no user action. Ever.

## Deliverables

The priority chain from Section 14: (1) embedded tracks in the media file — the default and most common path, always perfectly synced; (2) sidecar files found next to the media or inside the torrent; (3) OpenSubtitles hash match, quota-budgeted and cached; (4) other free providers; (5) alignment of whatever was found. **The aligner** (`crates/subtitle-align`, a standalone crate): extract a voice-activity signal from the audio track via FFmpeg + a VAD, build a comparable signal from the subtitle timings, and solve for offset *and* framerate scale by cross-correlation — handling the 23.976↔25 fps drift that a pure offset cannot. Confidence scoring; refuse to apply a low-confidence alignment and fall back rather than making things worse. A live `±` nudge in the player (`G`/`H` keys) that persists per file and per subtitle. Styling controls: font, size, position, background, and proper ASS/SSA rendering preserved when the source uses it. Language preference ordering per profile. `fixtures/subtitles/` corpus and the alignment eval harness. Tier 0: chain steps 1–3 plus the duration heuristic and the manual nudge; alignment is Tier 1+.

## Exit criteria

- [ ] **E1** > 90% of the fixture corpus aligns to within 150 ms.
- [ ] **E2** A 23.976 vs 25 fps mismatch is detected and corrected, not just offset.
- [ ] **E3** Embedded tracks are found and used without any network request.
- [ ] **E4** The OpenSubtitles daily quota is respected and never exceeded; quota state is visible in Settings.
- [ ] **E5** The manual nudge persists across sessions for the same file.
- [ ] **E6** `crates/subtitle-align` is independently testable and documented as a standalone crate.

## Subtasks

- [ ] **10.1** crates/subtitle-align (MIT/Apache, no FFmpeg): cue timings read and rewritten in place for SRT, WebVTT and ASS/SSA (styling preserved); a pure-Rust band-energy VAD with a syllable-modulation test; offset and framerate scale solved together by FFT cross-correlation over the 23.976/24/25 scale set, the plateau's centre taken; two-part split for inserted or removed intros; a distinctness confidence that refuses rather than guess; Tier 0 duration heuristic. Unit tests on synthetic dialogue.
- [ ] **10.2** fixtures/subtitles/ corpus (tools/subcorpus: Windows speech synthesis over room, street and music beds, traces from the app's own VAD, eight damaged copies per film: offsets, 23.976<->25 and 24->25 scales, intros removed and added) plus negative controls (one film's subtitles against another's speech must be refused); cargo run -p eval -- subs reports aligned share (>90% target, a copy aligns when 95% of its cues land within 150 ms), refusals, false accepts and VAD agreement.
- [ ] **10.3** The subtitle chain in a testable crate (crates/subtitles): embedded tracks first (from the player's track list, no network), then sidecar files next to the media (same stem, language suffixes) or in the torrent's file list, then OpenSubtitles by movie hash with the user's own key (no key ships), a persisted daily quota budget that is never exceeded and a cache, then other providers behind a trait; language preference ordering per profile. A test proves an embedded track is used with zero network requests.
- [ ] **10.4** Wiring in the app: audio extracted with FFmpeg (16 kHz mono, first 20 minutes) for alignment on Tier 1+, the duration heuristic on Tier 0; an aligned or found subtitle loaded into mpv (sub-add); ASS rendered as authored; Settings shows OpenSubtitles key and quota state and subtitle style (font, size, position, background).
- [ ] **10.5** The nudge: G and H (and the pad) shift subtitles by 100 ms in the player, persisted per file and per subtitle in the database (migration + repository method + repository_surface test) and reapplied on the next open.
- [ ] **10.6** docs/specs/subtitle-alignment.md, crates/subtitle-align README as a standalone crate, learning note, evidence for E1-E6, regression check, close.

## Risks named by this phase

- **R6** — Subtitle alignment does not reach 90% on the fixture corpus

## Learning note

Voice activity detection; cross-correlation as a way to align two signals; why framerate scaling is a multiplication and offset is an addition, and why you need both; confidence thresholds and knowing when not to act.

---

<!-- subtask log: appended during the phase -->

## Work log

