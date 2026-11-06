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
      if (r.status === "ok" && r.data) play({ target: r.data, title: titleFromPath(r.data), mediaItemId: null, start: null });
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [play]);
}
