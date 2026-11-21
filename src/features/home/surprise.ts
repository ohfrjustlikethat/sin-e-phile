import { commands } from "@/lib/ipc";
import { toast } from "@/lib/toast";
import { useUi } from "@/lib/store";

/**
 * Surprise me (SPEC.md Phase 18, ADR-0043, ADR-0056).
 *
 * A text button in the masthead, just left of Search, and RT on a pad from anywhere while browsing: one film, opened on
 * its page ready to play. (A length choice, I42, was built and then removed by the author, ADR-0056.)
 */
export async function surpriseMe(): Promise<void> {
  const r = await commands.surpriseMe(new Date().getHours());
  if (r.status === "error") {
    toast(`No surprise this time: ${r.error}`, "danger");
    return;
  }
  if (!r.data) {
    toast("Nothing to choose from yet: rate a few films first.", "neutral");
    return;
  }
  useUi.getState().openDetail(r.data.id);
  toast(r.data.note ? `Surprise: ${r.data.note}` : "Surprise: one for you", "neutral");
}
