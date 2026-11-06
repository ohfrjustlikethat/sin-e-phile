import { useEffect, useState } from "react";
import { Masthead } from "@/app/Masthead";
import { PlayerScreen } from "@/features/player/PlayerScreen";
import { DetailScreen } from "@/features/detail/DetailScreen";
import { useDropToPlay } from "@/features/player/useDropToPlay";
import { sendPlayerIntent } from "@/features/player/intentBus";
import { moveFocus, startGamepad } from "@/lib/gamepad";
import { Placeholder } from "@/app/Placeholder";
import { PadKeyboard, closePadKeyboard } from "@/app/PadKeyboard";
import { SettingsScreen } from "@/features/settings/SettingsScreen";
import { SearchScreen } from "@/features/search/SearchScreen";
import { DesignGallery } from "@/features/design/DesignGallery";
import { CommandPalette, type Command } from "@/design-system";
import { DESTINATIONS, useUi, type Destination } from "@/lib/store";

/**
 * The gallery is a DEV-ONLY route (SPEC.md Phase 2, subtask 2.2). It is reached at
 * `#design`, and `import.meta.env.DEV` keeps it out of a production bundle
 * entirely rather than merely hiding it.
 */
const DESIGN_ROUTE = import.meta.env.DEV && window.location.hash === "#design";

const LABELS: Record<Destination, string> = {
  home: "Home", films: "Films", tv: "TV Shows", watchlist: "Watchlist", live: "Live Channels",
};

export function App() {
  const { destination, settingsOpen, setDestination, setSettingsOpen } = useUi();
  const playing = useUi((s) => s.playing);
  const detail = useUi((s) => s.detail);
  useDropToPlay();
  const [palette, setPalette] = useState(false);
  const [searching, setSearching] = useState(false);

  // ADR-0041: an Xbox controller as a remote. The player takes its intents while it is
  // on screen; everywhere else the pad moves focus and acts on it.
  useEffect(
    () =>
      startGamepad({
        player: sendPlayerIntent,
        nav: (action) => {
          switch (action.kind) {
            case "move":
              moveFocus(action.direction);
              break;
            case "activate":
              (document.activeElement as HTMLElement | null)?.click();
              break;
            case "back":
              if (closePadKeyboard()) break;
              window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }));
              break;
            case "tab": {
              const ui = useUi.getState();
              const i = DESTINATIONS.indexOf(ui.destination);
              const next = DESTINATIONS[(i + (action.next ? 1 : DESTINATIONS.length - 1)) % DESTINATIONS.length];
              if (next) ui.setDestination(next);
              setSearching(false);
              break;
            }
            case "search":
              setSearching(true);
              break;
            case "settings":
              useUi.getState().setSettingsOpen(true);
              break;
          }
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
      if (event.key === "/" || (event.key.toLowerCase() === "f" && event.ctrlKey)) {
        event.preventDefault();
        setSettingsOpen(false);
        setSearching(true);
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [searching, setSettingsOpen]);

  // §3.1: the palette searches "media, actions, and settings". Phase 5 supplies
  // media; these are the actions and settings that exist today.
  const commands: Command[] = [
    ...DESTINATIONS.map((d) => ({
      id: `go-${d}`,
      label: `Go to ${LABELS[d]}`,
      group: "Navigate",
      run: () => { setSettingsOpen(false); setDestination(d); },
    })),
    { id: "settings", label: "Open settings", group: "Settings", run: () => setSettingsOpen(true) },
    { id: "search", label: "Search", group: "Navigate", hint: "/", run: () => { setSettingsOpen(false); setSearching(true); } },
  ];

  if (DESIGN_ROUTE) {
    return (
      <div className="flex h-full flex-col bg-base text-ink">
        <Masthead searching={false} onSearchChange={() => {}} />
        <DesignGallery />
      </div>
    );
  }

  // The player is immersive: no masthead, its own top bar (with a drag region).
  if (playing) {
    return (
      <div className="h-full bg-black text-ink">
        <PlayerScreen />
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col bg-base text-ink">
      <Masthead searching={searching} onSearchChange={setSearching} />
      <main className="min-h-0 min-w-0 flex-1 overflow-auto">
        {settingsOpen ? (
          <SettingsScreen />
        ) : detail !== null ? (
          <DetailScreen />
        ) : searching ? (
          <SearchScreen />
        ) : (
          <Placeholder destination={destination} />
        )}
      </main>
      <CommandPalette commands={commands} open={palette} onOpenChange={setPalette} />
      <PadKeyboard />
    </div>
  );
}
