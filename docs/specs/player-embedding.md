# Player embedding

How libmpv is embedded in the Tauri window, and the rules that keep it working.
ADR-0020 (holes) and ADR-0021 (still frame on pause) are the decisions; this is the
build input for anything that touches the player (subtitles in Phase 10, the
Phase 18 overhaul).

## 1. The pieces

| Piece | Where | Owns |
|---|---|---|
| `Player` | `crates/player/src/player.rs` | one libmpv instance, its options, its event thread |
| `VideoHost` | `crates/player/src/host.rs` | the native child window mpv renders into (`wid`) |
| `PlayerRuntime` | `src-tauri/src/commands/player.rs` | one `Player` + one `VideoHost`, created on first play |
| `PlayerScreen` | `src/features/player/PlayerScreen.tsx` | the chrome, the pause screen, the opening state |
| `plan` | `crates/playback/src/lib.rs` | catalogue row → URL or path + start position |

## 2. Windows and threads

1. **The video window is created on the UI thread.** A window belongs to the thread
   that creates it, and Windows sends a child synchronous messages when its parent
   moves, paints or is printed. Created on a Tokio worker (which never pumps
   messages), it froze the whole app window. `VideoHost::new` returns an error on any
   other thread; `src-tauri` creates it with `run_on_main_thread` and waits.
2. `SetWindowPos`, `SetWindowRgn` and `ShowWindow` may be called from any thread; they
   are forwarded to the UI thread, which answers.
3. mpv creates its own window inside the host, on its own thread, and sizes it to the
   host.

## 3. Placement and holes

- The page sends `player_layout(x, y, w, h, devicePixelRatio, cutouts)` in CSS pixels on
  mount, on resize, and whenever the chrome shows or hides.
- `Placement::physical` converts to physical pixels relative to the host's corner.
- **The last placement is kept** and applied when the host is created. The page lays
  out before `player_open` creates the host; dropping that request left the video at
  1×1 — black under the chrome, and a 1×1 pause still.
- The window region is the host's rectangle minus each panel's silhouette, rounded to
  the panel's own radius (`CreateRoundRectRgn`). A square hole under a rounded panel
  shows the desktop at the corners.

## 4. The lifecycle of one play

1. `player_open(target, start)`: create the runtime if needed, **hide** the host, load.
2. The page shows the opening state (title, "Opening" or "Resuming from m:ss") until
   `player-state.started` is true.
3. A blocking task waits for mpv's first `playback-restart` after the load, then shows
   the host (unless the user paused first) and logs `first frame` with its timing.
4. `player-state` is emitted every 250 ms: position, duration, buffer, volume, tracks,
   `hwdec`, `started`.
5. Pause: `player_pause_still` pauses, captures `screenshot-to-file … window` (video mode
   when headless), waits for a complete JPEG, hides the host, returns a `data:` URI.
   Resume shows the host and unpauses.
6. Leaving saves the position (also every 15 s) and calls `player_close`.

## 5. Seeking

`hr-seek=yes`. mpv reports the *target* as the position as soon as a seek starts, so
"done" is the `playback-restart` that follows, counted by the event thread
(`seek_to_and_wait`). Measured: paused exact seeks land on the frame in under 5 ms.

## 6. Diagnosis

- `SINEPHILE_MPV_LOG=v` raises mpv's log level into the app log.
- `SINEPHILE_AUTOPLAY=<path or URL>` (debug builds only) plays on launch.
- First frame slow? Look for `ao/wasapi … AUDCLNT_E_DEVICE_IN_USE`: another program holds
  the audio device exclusively, and mpv retries for ~1.3 s before playing silently.
- To check the real window without touching the desktop: `PrintWindow` with
  `PW_RENDERFULLCONTENT` renders it, WebView2 and video included. Desktop capture needs
  the window on top and moves the user's windows; do not use it.
