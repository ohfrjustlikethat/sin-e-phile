import { useState } from "react";
import { TitleBar } from "@/app/TitleBar";
import { NavRail } from "@/app/NavRail";
import { Placeholder } from "@/app/Placeholder";
import { SettingsScreen } from "@/features/settings/SettingsScreen";
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
        <NavRail />
        <main className="min-w-0 flex-1 overflow-auto">
          {settingsOpen ? <SettingsScreen /> : <Placeholder destination={destination} />}
        </main>
      </div>
      <CommandPalette commands={commands} open={palette} onOpenChange={setPalette} />
    </div>
  );
}
