import { useEffect } from "react";

export interface ToastMessage {
  id: string;
  text: string;
  tone?: "neutral" | "success" | "danger";
}

/**
 * Toast.
 *
 * Non-blocking, which is what §4 asks for in place of a modal for routine
 * feedback. `role="status"` and `aria-live="polite"` so a screen reader announces
 * it without interrupting.
 */
export function ToastStack({
  toasts, onDismiss,
}: {
  toasts: ToastMessage[];
  onDismiss: (id: string) => void;
}) {
  return (
    <div
      role="status"
      aria-live="polite"
      className="pointer-events-none fixed bottom-6 right-6 z-50 flex flex-col gap-2"
    >
      {toasts.map((t) => (
        <Toast key={t.id} toast={t} onDismiss={onDismiss} />
      ))}
    </div>
  );
}

function Toast({ toast, onDismiss }: { toast: ToastMessage; onDismiss: (id: string) => void }) {
  useEffect(() => {
    const t = window.setTimeout(() => onDismiss(toast.id), 5000);
    return () => window.clearTimeout(t);
  }, [toast.id, onDismiss]);

  const accent =
    toast.tone === "success" ? "border-l-success"
    : toast.tone === "danger" ? "border-l-danger"
    : "border-l-line-strong";

  return (
    <div
      className={[
        "pointer-events-auto min-w-[260px] rounded-sm border border-line border-l-2 bg-overlay",
        "px-4 py-3 font-ui text-[12.5px] text-ink",
        accent,
      ].join(" ")}
    >
      {toast.text}
    </div>
  );
}
