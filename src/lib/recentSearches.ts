import { create } from "zustand";
import { persist } from "zustand/middleware";
import { profileStorage } from "@/lib/profileStorage";
import { remember } from "@/app/keyboardText";

/** The TV keyboard's recent searches (ADR-0043): the last eight, on this machine only, each
 * profile its own (Phase 14). */
interface RecentSearches {
  recent: string[];
  remember: (query: string) => void;
}

export const useRecentSearches = create<RecentSearches>()(
  persist(
    (set) => ({
      recent: [],
      remember: (query) => set((s) => ({ recent: remember(s.recent, query) })),
    }),
    { name: "sinephile.recent-searches", storage: profileStorage },
  ),
);
