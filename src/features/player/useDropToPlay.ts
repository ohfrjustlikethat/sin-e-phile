import { sendPlayerIntent } from "./intentBus";
import type { Intent } from "./intents";
import { useEffect } from "react";
import { commands } from "@/lib/ipc";
import { useUi } from "@/lib/store";

const VIDEO = /\.(mkv|mp4|m4v|avi|mov|webm|ts|m2ts|mpg|mpeg|wmv|ogv)$/i;

function inTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

/** The file name without its folder or extension, as a title until Phase 12 matches it. */
export function titleFromPath(path: string): string {
  const name = path.split(/[\\/]/).pop() ?? path;
  return name.replace(/\.[^.]+$/, "").replace(/[._]+/g, " ").trim();
}

export function isVideoPath(path: string): boolean {
  return VIDEO.test(path);
}

/**
 * Drop a video file on the window and it plays — the direct way to watch a local file
 * until Phase 12's library makes local files a source. Also, in debug builds only,
 * `SINEPHILE_AUTOPLAY` plays something on launch (see `dev_autoplay`).
 */
export function useDropToPlay() {
  const play = useUi((s) => s.play);
  useEffect(() => {
    if (!inTauri()) return;
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    void import("@tauri-apps/api/webview").then(({ getCurrentWebview }) =>
      getCurrentWebview()
        .onDragDropEvent((e) => {
          if (e.payload.type !== "drop") return;
          const file = e.payload.paths.find(isVideoPath);
          if (file) play({ target: file, title: titleFromPath(file), mediaItemId: null, start: null });
        })
        .then((u) => {
          if (cancelled) u();
          else unlisten = u;
        }),
    );
    void commands.devAutoplay().then((r) => {
      if (r.status === "ok" && r.data) {
        play({ target: r.data, title: titleFromPath(r.data), mediaItemId: null, start: null });
        probe();
      }
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [play]);
}

/**
 * Debug builds, under `SINEPHILE_AUTOPLAY` only: every half second, log whether the page
 * thinks it is visible, how many animation frames ran, and whether a pad is seen. Phase 16
 * found the pad dead while video plays; this is how the real window says why.
 */
function probe() {
  (window as unknown as { __probe?: boolean }).__probe = true;
  window.addEventListener(
    "keydown",
    (e) => void commands.devLog(`probe key=${e.key} at=${performance.now().toFixed(0)}`),
    true,
  );
  // A timed script of player intents, sent as the pad sends them.
  void commands.devProbeScript().then((script) => {
    for (const step of (script ?? "").split(",").filter(Boolean)) {
      const [at, kind] = step.split(":");
      window.setTimeout(() => {
        if (kind === "style") {
          // The subtitle style, set as the panel sets it, then read back from mpv.
          void (async () => {
            const r = await commands.subtitleSettings();
            if (r.status !== "ok") return;
            await commands.setSubtitleSettings({ ...r.data.settings, colour: "#FFD400", outline: 5 });
            const colour = await commands.devPlayerProp("sub-color");
            const border = await commands.devPlayerProp("sub-border-size");
            void commands.devLog(`probe style sub-color=${colour} sub-border-size=${border}`);
            await commands.setSubtitleSettings(r.data.settings);
          })();
          return;
        }
        const taken = sendPlayerIntent({ kind } as Intent);
        void commands.devLog(`probe intent=${kind} taken=${taken} at=${performance.now().toFixed(0)}`);
      }, Number(at));
    }
  });
  let frames = 0;
  const count = () => {
    frames++;
    requestAnimationFrame(count);
  };
  requestAnimationFrame(count);
  setInterval(() => {
    const pads = navigator.getGamepads?.().filter((p) => p && p.connected).length ?? 0;
    void commands.devLog(
      `probe visibility=${document.visibilityState} focus=${document.hasFocus()} frames=${frames} pads=${pads}`,
    );
    frames = 0;
  }, 500);
}
