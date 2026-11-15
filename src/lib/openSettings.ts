import { useUi } from "@/lib/store";

/**
 * Open Settings at one section, with focus on its first control, so a keyboard or a pad
 * can act at once (ADR-0041). `section` is the section's `aria-label`.
 */
export function openSettingsAt(section: string): void {
  useUi.getState().setSettingsOpen(true);
  // Two frames: one for Settings to mount, one for its sections to lay out.
  requestAnimationFrame(() =>
    requestAnimationFrame(() => {
      const at = document.querySelector<HTMLElement>(`[aria-label="${section}"]`);
      at?.scrollIntoView({ block: "start" });
      at?.querySelector<HTMLElement>("input, button, select, [tabindex='0']")?.focus();
    }),
  );
}
