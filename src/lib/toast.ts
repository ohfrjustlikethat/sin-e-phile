import { create } from "zustand";
import type { ToastMessage } from "@/design-system";

/**
 * The app's toasts (Phase 16, the author: "after adding an addon or a key or connecting local library, the app should
 * tell that something happened"). Every add, save and connect says so twice: inline, where it happened ("Added ✓"),
 * and here, a small note that fades after five seconds, so it is seen wherever focus went. Errors say what to do.
 */
interface Toasts {
  toasts: ToastMessage[];
  dismiss: (id: string) => void;
}

let next = 0;

export const useToasts = create<Toasts>()((set) => ({
  toasts: [],
  dismiss: (id) => set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) })),
}));

/** Say that something happened. At most three show at once; the oldest goes first. */
export function toast(text: string, tone: ToastMessage["tone"] = "success"): void {
  const id = `t${++next}`;
  useToasts.setState((s) => ({ toasts: [...s.toasts, { id, text, tone }].slice(-3) }));
}
