import { Slider, Toggle } from "@/design-system";
import { play } from "@/design-system/focus/feedback";
import { useControllerSettings } from "@/lib/controllerSettings";

/**
 * Settings → Controller — ADR-0042.
 *
 * The TV mode touches the author chose to have on by default, each with its own switch:
 * navigation sounds (and how loud), rumble on select, and full screen when the pad is
 * picked up. The View button toggles full screen whatever this says.
 */
export function ControllerSection() {
  const s = useControllerSettings();
  return (
    <section className="mb-9" aria-label="Controller">
      <h2 className="mb-3.5 text-[11px] font-semibold uppercase tracking-[0.12em] text-ink-faint">
        Controller
      </h2>
      <div className="flex flex-col gap-4">
        <Toggle
          checked={s.sounds}
          onChange={(sounds) => {
            s.set({ sounds });
            if (sounds) play("select");
          }}
          label="Navigation sounds"
          description="A quiet tick as focus moves, and a soft tone when you select."
        />
        {s.sounds && (
          <Slider
            value={Math.round(s.volume * 100)}
            onChange={(v) => {
              s.set({ volume: v / 100 });
              play("move");
            }}
            label="Sound volume"
            display={`${Math.round(s.volume * 100)}%`}
          />
        )}
        <Toggle
          checked={s.rumble}
          onChange={(rumble) => s.set({ rumble })}
          label="Rumble"
          description="A light, short rumble when you select something."
        />
        <Toggle
          checked={s.fullscreenOnPickup}
          onChange={(fullscreenOnPickup) => s.set({ fullscreenOnPickup })}
          label="Full screen when the controller is picked up"
          description="The View button switches full screen on and off at any time."
        />
      </div>
    </section>
  );
}
