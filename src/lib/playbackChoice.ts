import { create } from "zustand";
import { persist } from "zustand/middleware";

/**
 * The viewer's audio language and quality (SPEC.md Phase 9). Remembered across films,
 * the way a TV app remembers the language you watch in: choose Hindi once and every film
 * that has Hindi plays in it. `null` means no preference: the app picks.
 */
interface PlaybackChoice {
  /** BCP 47 primary subtag, e.g. `hi`. */
  audio: string | null;
  /** A wire name: `uhd`, `full_hd`, `hd`, `sd`. */
  quality: string | null;
  /** Show every candidate's score on film pages (the expert panel, Phase 9). */
  details: boolean;
  /** Play the next episode when the countdown ends (Phase 11). */
  autoplay: boolean;
  setAutoplay: (autoplay: boolean) => void;
  setAudio: (audio: string | null) => void;
  setQuality: (quality: string | null) => void;
  setDetails: (details: boolean) => void;
}

export const usePlaybackChoice = create<PlaybackChoice>()(
  persist(
    (set) => ({
      audio: null,
      quality: null,
      details: false,
      autoplay: true,
      setAutoplay: (autoplay) => set({ autoplay }),
      setAudio: (audio) => set({ audio }),
      setQuality: (quality) => set({ quality }),
      setDetails: (details) => set({ details }),
    }),
    { name: "sinephile.choice" },
  ),
);

const QUALITY: Record<string, string> = { uhd: "4K", full_hd: "1080p", hd: "720p", sd: "SD" };

export function qualityLabel(code: string): string {
  return QUALITY[code] ?? code;
}

/** "Hindi" for `hi`, from the platform's own list; the code itself if it has none. */
export function languageName(code: string): string {
  try {
    return new Intl.DisplayNames(["en"], { type: "language" }).of(code) ?? code;
  } catch {
    return code;
  }
}
