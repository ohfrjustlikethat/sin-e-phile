import { useEffect, useState } from "react";
import { TitleBar } from "@/app/TitleBar";
import { NavRail } from "@/app/NavRail";
import { Placeholder } from "@/app/Placeholder";
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
  const [palette, setPalette] = useState(false);
  const [searching, setSearching] = useState(false);

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
        <TitleBar />
        <DesignGallery />
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col bg-base text-ink">
      <TitleBar />
      <div className="flex min-h-0 flex-1">
        <NavRail suppressActive={searching || settingsOpen} />
        <main className="min-w-0 flex-1 overflow-auto">
          {settingsOpen ? (
            <SettingsScreen />
          ) : searching ? (
            <SearchScreen />
          ) : (
            <Placeholder destination={destination} />
          )}
        </main>
      </div>
      <CommandPalette commands={commands} open={palette} onOpenChange={setPalette} />
    </div>
  );
}
