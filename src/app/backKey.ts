import { useUi } from "@/lib/store";

/**
 * The app's one way back from the keyboard (Phase 16): Escape, Backspace outside a field, and the pad's B (which the
 * gamepad layer sends as Escape). It takes one step of `back` in src/lib/store.ts: an overlay first, then the tab's
 * previous page, then search to the destination.
 *
 * It acts only when nothing else used the key: menus, dialogs, the full player and the review queue mark Escape as
 * handled, and the first run, the profile picker and the palette (`blocked`) have their own. Escape inside a field
 * leaves the field first, except the search box, whose Escape leaves search.
 *
 * Returns whether the key was a back key at all (so the caller stops looking at it).
 */
export function handleBack(event: KeyboardEvent, blocked: boolean): boolean {
  const target = event.target as HTMLElement | null;
  const typing =
    target?.tagName === "INPUT" ||
    target?.tagName === "TEXTAREA" ||
    target?.tagName === "SELECT" ||
    Boolean(target?.isContentEditable);
  if (event.key !== "Escape" && !(event.key === "Backspace" && !typing)) return false;
  const ui = useUi.getState();
  if (event.defaultPrevented || blocked || ui.pickerOpen || (ui.playing && !ui.mini)) return true;
  if (typing && ui.tab !== "search") {
    target?.blur();
    return true;
  }
  if (ui.back()) event.preventDefault();
  return true;
}
