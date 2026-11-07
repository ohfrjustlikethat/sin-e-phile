import { useCallback, useEffect } from "react";
import { useQuery, useQueryClient, type QueryClient } from "@tanstack/react-query";
import { onEvent } from "@/lib/events";
import { commands, type ProfileView } from "@/lib/ipc";
import { usePlaybackChoice } from "@/lib/playbackChoice";
import { switchStores } from "@/lib/profileStorage";
import { useRecentSearches } from "@/lib/recentSearches";

export const PROFILES = ["profiles"];

async function ok<T>(p: Promise<{ status: "ok"; data: T } | { status: "error"; error: string }>) {
  const r = await p;
  if (r.status === "error") throw new Error(r.error);
  return r.data;
}

/**
 * Everything that follows a switch: the personal browser stores read the new profile's
 * own values, and every query is fetched again (history, watchlist, ratings, the
 * download advice: all of it is someone else's now).
 */
export async function adopt(client: QueryClient, id: number): Promise<void> {
  await switchStores(id, [usePlaybackChoice, useRecentSearches]);
  await client.invalidateQueries();
}

/**
 * Once, in App: follow a switch the backend made (the profile watching was deleted), and
 * the first time the active profile is known, make its stores its own.
 */
export function useProfileSync() {
  const client = useQueryClient();
  const { active } = useProfiles();
  useEffect(() => onEvent<number>("profile-changed", (id) => void adopt(client, id)), [client]);
  const id = active?.id;
  useEffect(() => {
    if (id !== undefined) void switchStores(id, [usePlaybackChoice, useRecentSearches]);
  }, [id]);
}

export function useProfiles() {
  const client = useQueryClient();
  const query = useQuery({ queryKey: PROFILES, queryFn: () => ok(commands.profiles()) });
  const active: ProfileView | undefined = query.data?.find((p) => p.active);

  /** Switch, PIN first if it has one. Throws "that PIN is not right" and changes nothing. */
  const switchTo = useCallback(
    async (id: number, pin: string | null): Promise<ProfileView> => {
      const p = await ok(commands.switchProfile(id, pin));
      await adopt(client, id);
      return p;
    },
    [client],
  );

  return { profiles: query.data ?? [], active, loading: query.isPending, error: query.error, switchTo };
}
