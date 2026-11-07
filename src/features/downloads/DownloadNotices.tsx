import { useCallback, useEffect, useState } from "react";
import { ToastStack, type ToastMessage } from "@/design-system";
import { onEvent } from "@/lib/events";

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
    const finished = onEvent<string>("download-finished", (title) =>
      push(`${title} has downloaded, and is in your library.`, "success"),
    );
    const failed = onEvent<string>("download-failed", (title) =>
      push(`${title} stopped downloading. Downloads says why.`, "danger"),
    );
    return () => {
      finished();
      failed();
    };
  }, []);

  return <ToastStack toasts={toasts} onDismiss={dismiss} />;
}
