# Phase 8 — Learning notes

**Player core: libmpv embedded, the chrome over it, and an Xbox controller as a remote.**
Four sections per ADR-0016 (A4): what we built, why, new concepts as concept +
`file:line`, and the five questions.

---

## 1. What we built

- **libmpv, pinned and loaded at run time** — `tools/mpv/fetch.py` fetches one exact
  build and checks its SHA-256; `crates/player/src/ffi.rs` loads `libmpv-2.dll` without
  an import library.
- **A safe player** — `crates/player/src/player.rs`. Options, commands, properties, and
  one thread that reads mpv's events and records load, first frame, seek completion and
  end of file.
- **The video window** — `crates/player/src/host.rs`. A native child window above the
  WebView; the chrome shows through holes cut in it (ADR-0020); on pause it hides and a
  still frame takes its place (ADR-0021).
- **The plan** — `crates/playback/src/lib.rs`. From a catalogue row to "open this, from
  here": ask the sources, rank them (local, then HTTP, then P2P), skip the ones that will
  not open, and resume from the saved position.
- **Screens** — the player (`src/features/player/PlayerScreen.tsx`) and a minimal detail
  page (`src/features/detail/DetailScreen.tsx`); drop a file on the window to play it.
- **The controller** — `src/lib/gamepad.ts` turns pad presses into player intents or
  focus moves; `src/app/PadKeyboard.tsx` lets a pad type.
- **Proof** — headless player tests, end-to-end tests over a loopback swarm and live
  against the Internet Archive, and `tools/uiaudit/controller.mjs`, which drives the
  built app in Chrome with nothing but simulated pad buttons.

## 2. Why

- **Embed a player, never build one.** Decoding, hardware acceleration, subtitles,
  audio sync and HDR are decades of work in mpv. The app's job is the chrome around it.
- **A native window, not a `<video>` tag.** WebView2's video element cannot hardware-
  decode HEVC or play MKV, and cannot show HDR. Cost: HTML cannot be drawn over a native
  window, hence the holes.
- **The page and the player share one set of intents** (`intents.ts`), so the keyboard
  and the controller can never offer different things.
- **Check the real window, not only the tests.** Two of the phase's worst bugs (the hung
  window and the 1×1 video) passed every headless test; only a capture of the real
  window found them.

## 3. New concepts

### Video

- **Container, stream, codec** — an `.mkv` is a box of streams; each stream is coded
  (HEVC, AAC, …); mpv demuxes, decodes, then presents. `player.rs:157` (`load`).
- **Hardware decode** — the GPU's fixed-function decoder does the work; `hwdec=auto-safe`
  asks for it and `hwdec-current` says what was chosen (`d3d11va`, or `d3d11va-copy`
  when the frame must come back to the CPU, as headless). Measured: half the CPU.
- **Keyframes and exact seeks** — only keyframes decode alone; an exact seek decodes
  forward from the one before. `hr-seek` plus waiting for `playback-restart`
  (`player.rs:236`) is what makes "landed on 3.520" true rather than reported early.
- **A still frame for pause** — `screenshot-to-file … window` (`player.rs:292`), waited
  for until it is a complete JPEG (`still.rs:10`).

### Windows

- **Dynamic loading** — `LoadLibrary` + `GetProcAddress` per function (`ffi.rs:101`), so
  the build needs no `.lib` and a missing DLL is an error, not a crash at start.
- **Child windows and threads** — a window belongs to the thread that created it, and
  that thread must pump its messages. Created on a worker, the video window froze the
  whole app; `host.rs:76` now refuses any other thread, and src-tauri creates it with
  `run_on_main_thread` (`src-tauri/src/commands/player.rs:33`).
- **Window regions** — `SetWindowRgn` makes a window's visible shape any region:
  rectangle minus rounded rectangles (`host.rs:228`).
- **CSS pixels to physical pixels** — multiply by `devicePixelRatio`, relative to the
  host's own corner (`host.rs:199`).
- **`PrintWindow(PW_RENDERFULLCONTENT)`** — how the real window was checked without
  touching the desktop (the method is in `SESSION_LOG.md`).

### Rust

- **A `Condvar` beside a `Mutex`** — `player.rs:375`. The event thread updates what it
  has seen and wakes whoever is waiting for "first frame" or "seek done".
- **A lazily created runtime, applied state on creation** — the last layout is kept in a
  `static Mutex` and applied the moment the window exists (`src-tauri/src/commands/player.rs`).

### React and the browser

- **The Gamepad API** — polled every animation frame, not evented; "pressed this frame"
  is `newlyPressed` (`gamepad.ts:54`); held directions repeat after a delay
  (`gamepad.ts:168`).
- **Spatial navigation** — `nearest` (`gamepad.ts:122`): the closest focusable thing in
  the direction pressed, with sideways drift counted double.
- **Typing into a React-controlled input from outside** — set the value through the
  native setter and dispatch `input`, or React never sees it (`PadKeyboard.tsx:28`).
- **A fake backend at the IPC seam** — `window.__TAURI_INTERNALS__.invoke` is what the
  generated bindings call, so a test can answer as Rust would (`tools/uiaudit/controller.mjs`).

## 4. The questions

Phase 8 ends Tier A, so these and the ones accumulated since Phase 5 are asked at the
understanding gate (§10.10).

1. The video window is a native child of the app window. Why can HTML not be drawn over
   it, and how do the holes and the pause still each work around that?
2. Why did creating the video window on a Tokio worker thread freeze the *whole* app
   window, not only the video? What does "a window belongs to a thread" mean?
3. An exact seek to 3.52 s in a file with a keyframe every second: what does mpv decode,
   and why does the test wait for `playback-restart` instead of reading the position?
4. The first frame took 1.5 s on this machine, but the video path was ~200 ms. Where did
   the rest go, and why is that not the player's fault?
5. On a pad, B closes the on-screen keyboard if it is open, and leaves the screen if it
   is not. Why does that order matter, and what would a user experience without it?
