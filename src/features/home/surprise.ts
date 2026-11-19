import { create } from "zustand";
import { commands } from "@/lib/ipc";
import { toast } from "@/lib/toast";
import { useUi } from "@/lib/store";

/**
 * Surprise me (SPEC.md Phase 18, ADR-0043) and "I have 90 minutes" (Appendix F, I42).
 *
 * The length holds for this session: it narrows Home's recommendation rails and Surprise me alike. Surprise me is the
 * button on Home and RT on a pad from anywhere while browsing: one film, opened on its page ready to play.
 */

export const LENGTHS: { label: string; minutes: number | null }[] = [
  { label: "Any length", minutes: null },
  { label: "Under 90 minutes", minutes: 90 },
  { label: "Under 2 hours", minutes: 120 },
];

export const useLength = create<{ maxMinutes: number | null; setMaxMinutes: (m: number | null) => void }>()(
  (set) => ({
    maxMinutes: null,
    setMaxMinutes: (maxMinutes) => set({ maxMinutes }),
  }),
);

export async function surpriseMe(): Promise<void> {
  const r = await commands.surpriseMe(new Date().getHours(), useLength.getState().maxMinutes);
  if (r.status === "error") {
    toast(`No surprise this time: ${r.error}`, "danger");
    return;
  }
  if (!r.data) {
    toast("Nothing to choose from yet: rate a few films, or allow a longer length.", "neutral");
    return;
  }
  useUi.getState().openDetail(r.data.id);
  toast(r.data.note ? `Surprise: ${r.data.note}` : "Surprise: one for you", "neutral");
}
