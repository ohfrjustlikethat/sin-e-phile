# 0004 — libmpv over libVLC

- **Status:** Accepted
- **Date:** 2026-08-30
- **Phase:** 0 (recording a decision locked in `SPEC.md` §5)
- **Risk:** R1

## Context

Phase 8 needs a playback core that handles whatever a user actually has: arbitrary
containers and codecs, 4K HEVC with HDR, hardware decode with a software fallback,
frame-accurate seeking, multiple audio tracks, and correct ASS/SSA subtitle
rendering — anime subtitles use positioning and styling that a naive renderer
destroys.

Phase 10 additionally needs per-track audio and subtitle **delay** control at
runtime, because the aligner computes an offset that must then be applied live.

Crucially, `SPEC.md` §4 forbids shipping the player's own on-screen display: the UI
is drawn by us, over the video. So the library must be embeddable and controllable,
not a self-contained player.

## Decision

**libmpv**, embedded, with a custom UI drawn over it.

## Consequences

**Easier.** mpv's property/command API is exactly the right shape: set a property to
change a track, observe a property to drive the scrub bar, issue a command to seek.
Hardware decode via D3D11VA, NVDEC and QSV becomes a configuration concern rather
than an implementation one. ASS/SSA rendering is first-class, which matters more for
this audience than for a general player. Jellyfin and Stremio embed it for the same
reasons, so the path is trodden.

**Easier.** Per-track audio and subtitle delay are properties, so Phase 10's
alignment output and Phase 11's manual nudge are each a property write.

**Harder, and this is R1 — the project's most severe technical risk.** Compositing a
native video surface with a webview UI on top is the fiddliest integration here. Two
approaches exist (render-API-into-a-texture, and a child window clipped under the
webview), both with real costs. Spike A in Phase 1 tries both before anything
depends on the answer.

**Constraint accepted.** libmpv is LGPL/GPL which, combined with FFmpeg, is one
reason the project is GPL-3.0 (ADR-0007). Not a cost — it is the correct posture
anyway.

## Alternatives Considered

**libVLC.** Friendlier bindings and a gentler embedding story, and it is R1's first
library-level fallback. Rejected as the default on subtitle rendering and control
granularity: mpv's ASS/SSA fidelity and property API suit a custom UI better. If
Spike A shows mpv cannot be embedded acceptably, this is where we go, with an ADR.

**HTML5 `<video>` in the webview.** By far the simplest — the UI compositing problem
disappears entirely, since the video is just another DOM element. Rejected because
WebView2 supports a narrow codec set, so anything outside it needs transcoding or
remuxing on the fly, and "plays everything" is a core promise. Retained as R1's
last-resort fallback, with the codec limitation documented honestly.

**GStreamer.** Powerful and genuinely embeddable. Rejected on Windows deployment
complexity and a much steeper learning curve for a first-time Rust developer, for
capability mpv already provides.

**FFmpeg directly, with our own rendering and A/V sync.** Rejected firmly.
Audio/video synchronisation, frame pacing and hardware decode negotiation are years
of work, and mpv is the accumulated answer to them. The interesting problems in this
project are elsewhere.
