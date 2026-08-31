import { TitleBar } from "@/app/TitleBar";
import { NavRail } from "@/app/NavRail";
import { Placeholder } from "@/app/Placeholder";
import { SettingsScreen } from "@/features/settings/SettingsScreen";
import { useUi } from "@/lib/store";

export function App() {
  const { destination, settingsOpen } = useUi();

  return (
    <div className="flex h-full flex-col bg-base text-ink">
      <TitleBar />
      <div className="flex min-h-0 flex-1">
        <NavRail />
        <main className="min-w-0 flex-1 overflow-auto">
          {settingsOpen ? <SettingsScreen /> : <Placeholder destination={destination} />}
        </main>
      </div>
    </div>
  );
}
