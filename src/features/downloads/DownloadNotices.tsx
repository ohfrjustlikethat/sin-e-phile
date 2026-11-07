import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { ToastStack, type ToastMessage } from "@/design-system";

/**
 * Completion notices (SPEC.md Phase 13): "Sunrise has downloaded" wherever the viewer
 * is, from the backend's `download-finished` and `download-failed`. Toasts, not a modal
 * (§4): they say it and go.
 */
export function DownloadNotices() {
  const [toasts, setToasts] = useState<ToastMessage[]>([]);
  const dismiss = useCallback((id: string) => setToasts((t) => t.filter((x) => x.id !== id)), []);

  useEffect(() => {
    let n = 0;
    const push = (text: string, tone: "success" | "danger") =>
      setToasts((t) => [...t, { id: `download-${Date.now()}-${n++}`, text, tone }]);
    const finished = listen<string>("download-finished", (e) =>
      push(`${e.payload} has downloaded, and is in your library.`, "success"),
    );
    const failed = listen<string>("download-failed", (e) =>
      push(`${e.payload} stopped downloading. Downloads says why.`, "danger"),
    );
    return () => {
      void finished.then((f) => f()).catch(() => {});
      void failed.then((f) => f()).catch(() => {});
    };
  }, []);

  return <ToastStack toasts={toasts} onDismiss={dismiss} />;
}
