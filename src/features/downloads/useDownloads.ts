import { useEffect } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { listen } from "@tauri-apps/api/event";
import { commands } from "@/lib/ipc";

export const DOWNLOADS = ["downloads"];
export const STORAGE = ["downloads-storage"];

async function ok<T>(p: Promise<{ status: "ok"; data: T } | { status: "error"; error: string }>) {
  const r = await p;
  if (r.status === "error") throw new Error(r.error);
  return r.data;
}

/**
 * The download list, kept current by the backend's `downloads-changed` (once a second
 * while anything is queued or running). Shared by the masthead count and the page.
 */
export function useDownloads() {
  const client = useQueryClient();
  useEffect(() => {
    const off = listen("downloads-changed", () => {
      void client.invalidateQueries({ queryKey: DOWNLOADS });
    });
    return () => {
      void off.then((f) => f()).catch(() => {});
    };
  }, [client]);
  return useQuery({ queryKey: DOWNLOADS, queryFn: () => ok(commands.downloads()) });
}
