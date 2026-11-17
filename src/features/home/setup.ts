/**
 * What the "Finish setting up" card says (SPEC.md Appendix F, I38). Pure, so the rules
 * are tested without a page: which lines there are, which are done, and when the card
 * shows at all.
 *
 * The author found after Phase 15 that with no source nothing could play and nothing said
 * so. The film page now says it where Play would be; this says it on Home, once, with the
 * way to fix it.
 */

export interface SetupFacts {
  /** Library folders added. */
  folders: number;
  /** Add-ons installed (sources the viewer chose; the app ships none, §2.1). */
  addons: number;
  /** This profile has its own OpenSubtitles key. */
  subtitlesKey: boolean;
  /** The download engine is bound to a network adapter (a VPN's). */
  adapterBound: boolean;
}

export interface SetupLine {
  key: "library" | "source" | "subtitles" | "downloads";
  title: string;
  detail: string;
  done: boolean;
  /** Optional lines never keep the card up by themselves being undone and dismissed. */
  optional: boolean;
  /** The `aria-label` of the Settings section this line opens. */
  section: string;
}

const plural = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`;

export function setupLines(f: SetupFacts): SetupLine[] {
  return [
    {
      key: "library",
      title: "Your films on this PC",
      detail:
        f.folders > 0
          ? `${plural(f.folders, "folder", "folders")} added`
          : "Add a folder, and every film and episode in it is identified.",
      done: f.folders > 0,
      optional: false,
      section: "Library",
    },
    {
      key: "source",
      title: "Somewhere to watch from",
      detail:
        f.addons > 0
          ? `${plural(f.addons, "add-on", "add-ons")} installed`
          : "sin·e·phile comes with no sources of its own. Add one you have chosen, by its address.",
      done: f.addons > 0,
      optional: false,
      section: "Add a source",
    },
    {
      key: "subtitles",
      title: "Subtitles",
      detail: f.subtitlesKey
        ? "Your OpenSubtitles key is in"
        : "Optional. Your own OpenSubtitles key finds subtitles for almost anything.",
      done: f.subtitlesKey,
      optional: true,
      section: "Subtitles",
    },
    {
      key: "downloads",
      title: "Downloads",
      detail: f.adapterBound
        ? "Downloads stop if your VPN drops"
        : "Ready, with the built-in engine. Optional: choose your VPN's adapter so downloads stop if it drops.",
      done: f.adapterBound,
      optional: true,
      section: "Network",
    },
  ];
}

/** The card shows while anything is left to do, unless the viewer hid it. */
export function showSetup(lines: SetupLine[], hidden: boolean): boolean {
  return !hidden && lines.some((l) => !l.done);
}

/** "2 of 4 done". */
export function setupProgress(lines: SetupLine[]): string {
  return `${lines.filter((l) => l.done).length} of ${lines.length} done`;
}
