import { useUi, type SettingsSection } from "@/lib/store";

/** Which side-list section holds each place a link opens (the places' `aria-label`s). */
const SECTION_OF: Record<string, SettingsSection> = {
  Library: "library",
  "Add a source": "addons",
  Subtitles: "subtitles",
  Network: "network",
};

/**
 * Open Settings at one section, with focus on its first control, so a keyboard or a pad
 * can act at once (ADR-0041). `section` is the section's `aria-label`.
 */
export function openSettingsAt(section: string): void {
  const ui = useUi.getState();
  const holder = SECTION_OF[section];
  if (holder) ui.setSettingsSection(holder);
  ui.setSettingsOpen(true);
  // Two frames: one for Settings to mount, one for its sections to lay out.
  requestAnimationFrame(() =>
    requestAnimationFrame(() => {
      const at = document.querySelector<HTMLElement>(`[aria-label="${section}"]`);
      at?.scrollIntoView({ block: "start" });
      at?.querySelector<HTMLElement>("input, button, select, [tabindex='0']")?.focus();
    }),
  );
}
