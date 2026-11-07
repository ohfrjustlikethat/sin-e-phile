import { useCallback, useEffect, useState } from "react";
import { Masthead } from "@/app/Masthead";
import { PlayerScreen } from "@/features/player/PlayerScreen";
import { DetailScreen } from "@/features/detail/DetailScreen";
import { useDropToPlay } from "@/features/player/useDropToPlay";
import { sendPlayerIntent } from "@/features/player/intentBus";
import { moveFocus, startGamepad } from "@/lib/gamepad";
import { Placeholder } from "@/app/Placeholder";
import { PadKeyboard, closePadKeyboard } from "@/app/PadKeyboard";
import { SettingsScreen } from "@/features/settings/SettingsScreen";
import { ReviewScreen } from "@/features/library/ReviewScreen";
import { DownloadsScreen } from "@/features/downloads/DownloadsScreen";
import { SearchScreen } from "@/features/search/SearchScreen";
import { DesignGallery } from "@/features/design/DesignGallery";
import { CommandPalette, type Command } from "@/design-system";
import { DESTINATIONS, useUi, type Destination } from "@/lib/store";
import { FocusLayer } from "@/design-system/focus/FocusLayer";
import { play, rumble } from "@/design-system/focus/feedback";
import { useControllerSettings } from "@/lib/controllerSettings";

function inTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

/** Full screen for the whole app window (ADR-0042: View, or picking the pad up). */
async function fullscreen(on?: boolean) {
  if (!inTauri()) return;
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  const w = getCurrentWindow();
  await w.setFullscreen(on ?? !(await w.isFullscreen()));
}

/** The focused tile, if focus is on one (ADR-0042's tilt and press apply to tiles only). */
function focusedTile(): HTMLElement | null {
  const a = document.activeElement as HTMLElement | null;
  return a?.closest<HTMLElement>("[data-focus-tile]") ?? null;
}

/**
 * The gallery is a DEV-ONLY route (SPEC.md Phase 2, subtask 2.2). It is reached at
 * `#design`, and `import.meta.env.DEV` keeps it out of a production bundle
 * entirely rather than merely hiding it.
 */
const DESIGN_ROUTE = import.meta.env.DEV && window.location.hash === "#design";

const LABELS: Record<Destination, string> = {
  home: "Home",
  films: "Films",
  tv: "TV Shows",
  watchlist: "Watchlist",
  live: "Live Channels",
};

export function App() {
  const { destination, settingsOpen, setDestination, setSettingsOpen } =
    useUi();
  const playing = useUi((s) => s.playing);
  const reviewOpen = useUi((s) => s.reviewOpen);
  const downloadsOpen = useUi((s) => s.downloadsOpen);
  const mini = useUi((s) => s.mini);
  const detail = useUi((s) => s.detail);
  useDropToPlay();
  const [palette, setPalette] = useState(false);
  const [searching, setSearching] = useState(false);
  // Search opens from anywhere: over Settings and over a film's page, which would
  // otherwise take precedence and leave search switched on but invisible.
  const openSearch = useCallback(() => {
    const ui = useUi.getState();
    ui.setSettingsOpen(false);
    ui.setDownloadsOpen(false);
    ui.closeDetail();
    setSearching(true);
  }, []);

  // TV mode's scale must not apply while the player positions its native window.
  useEffect(() => {
    // The mini player leaves the app as it is, TV scale included.
    if (playing && !mini) document.documentElement.dataset["playing"] = "";
    else delete document.documentElement.dataset["playing"];
  }, [playing, mini]);

  // ADR-0041: an Xbox controller as a remote. The player takes its intents while it is
  // on screen; everywhere else the pad moves focus and acts on it.
  useEffect(
    () =>
      startGamepad({
        player: sendPlayerIntent,
        nav: (action) => {
          switch (action.kind) {
            case "move":
              play(moveFocus(action.direction) ? "move" : "edge");
              break;
            case "activate":
              play("select");
              rumble();
              (document.activeElement as HTMLElement | null)?.click();
              break;
            case "back":
              if (closePadKeyboard()) break;
              window.dispatchEvent(
                new KeyboardEvent("keydown", { key: "Escape" }),
              );
              break;
            case "tab": {
              const ui = useUi.getState();
              const i = DESTINATIONS.indexOf(ui.destination);
              const next =
                DESTINATIONS[
                  (i + (action.next ? 1 : DESTINATIONS.length - 1)) %
                    DESTINATIONS.length
                ];
              if (next) ui.setDestination(next);
              setSearching(false);
              break;
            }
            case "search":
              openSearch();
              break;
            case "settings":
              useUi.getState().setSettingsOpen(true);
              break;
          }
        },
        tilt: (rx, ry) => {
          const tile = focusedTile();
          tile?.style.setProperty("--tilt-x", `${rx}deg`);
          tile?.style.setProperty("--tilt-y", `${ry}deg`);
          tile?.style.setProperty("--sheen-shift", `${ry * 3}%`);
        },
        press: (down) => {
          const tile = focusedTile();
          if (!tile) return;
          if (down) tile.dataset["pressed"] = "";
          else delete tile.dataset["pressed"];
        },
        view: () => void fullscreen(),
        pickup: () => {
          if (useControllerSettings.getState().fullscreenOnPickup)
            void fullscreen(true);
        },
      }),
    [],
  );

  // SEARCH IS A MODE, NOT A DESTINATION. §3.1 names five top-level surfaces and the rail
  // comment is explicit that the navigation shape must never change under the user, so
  // search does not join them — it is reached the way it is reached everywhere else:
  // "/" or Ctrl+F from anywhere, Escape to leave.
  //
  // The guard matters. Someone typing "/" into the search box means a slash, not a
  // shortcut, and a shortcut that fires inside a text field is the kind of bug that
  // makes an app feel hostile.
  useEffect(() => {
    function onKey(event: KeyboardEvent) {
      const target = event.target as HTMLElement | null;
      const typing =
        target?.tagName === "INPUT" ||
        target?.tagName === "TEXTAREA" ||
        target?.isContentEditable;

      if (event.key === "Escape" && searching) {
        setSearching(false);
        return;
      }
      if (typing) return;
      if (
        event.key === "/" ||
        (event.key.toLowerCase() === "f" && event.ctrlKey)
      ) {
        event.preventDefault();
        openSearch();
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [searching, openSearch]);

  // §3.1: the palette searches "media, actions, and settings". Phase 5 supplies
  // media; these are the actions and settings that exist today.
  const commands: Command[] = [
    ...DESTINATIONS.map((d) => ({
      id: `go-${d}`,
      label: `Go to ${LABELS[d]}`,
      group: "Navigate",
      run: () => {
        setSettingsOpen(false);
        setDestination(d);
      },
    })),
    {
      id: "downloads",
      label: "Open downloads",
      group: "Navigate",
      run: () => {
        setSearching(false);
        useUi.getState().setDownloadsOpen(true);
      },
    },
    {
      id: "settings",
      label: "Open settings",
      group: "Settings",
      run: () => setSettingsOpen(true),
    },
    {
      id: "search",
      label: "Search",
      group: "Navigate",
      hint: "/",
      run: openSearch,
    },
  ];

  if (DESIGN_ROUTE) {
    return (
      <div className="flex h-full flex-col bg-base text-ink">
        <Masthead searching={false} onSearchChange={() => {}} />
        <DesignGallery />
      </div>
    );
  }

  // The player is immersive: no masthead, its own top bar (with a drag region). Its
  // chrome sits under a native video window, so it keeps per-element outlines rather
  // than the drawn ring, and TV mode's scale is off while it is open (focus.css).
  //
  // The mini player (ADR-0043) keeps the SAME PlayerScreen mounted in a corner over the
  // app: the slot below never moves, so shrinking or expanding does not reopen the film.
  const full = playing !== null && !mini;
  return (
    <>
      {!full && (
        <div className="flex h-full flex-col bg-base text-ink">
          <Masthead searching={searching} onSearchChange={setSearching} />
          <main className="min-h-0 min-w-0 flex-1 overflow-auto">
            {settingsOpen && reviewOpen ? (
              <ReviewScreen />
            ) : settingsOpen ? (
              <SettingsScreen />
            ) : downloadsOpen ? (
              <DownloadsScreen />
            ) : detail !== null ? (
              <DetailScreen />
            ) : searching ? (
              <SearchScreen />
            ) : (
              <Placeholder destination={destination} />
            )}
          </main>
          <CommandPalette
            commands={commands}
            open={palette}
            onOpenChange={setPalette}
          />
          <PadKeyboard />
          <FocusLayer />
        </div>
      )}
      {playing && (
        <div
          className={
            full
              ? "fixed inset-0 bg-black text-ink"
              : "fixed right-4 bottom-4 z-40 h-[262px] w-[400px] shadow-none"
          }
          data-no-focus-ring
        >
          <PlayerScreen mini={mini} />
          {full && <FocusLayer />}
        </div>
      )}
    </>
  );
}
