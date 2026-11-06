import { useControllerSettings } from "@/lib/controllerSettings";

/**
 * Navigation sounds and rumble — ADR-0042, `docs/specs/tv-focus.md` §4.
 *
 * The sounds are synthesised with Web Audio, so there are no audio files to license or
 * ship. All three are short and quiet; the volume setting scales them. Rumble uses the
 * Gamepad API's `vibrationActuator` where the WebView exposes it and is silently absent
 * where it does not.
 */

export type Cue = "move" | "select" | "edge";

let ctx: AudioContext | null = null;

function audio(): AudioContext | null {
  try {
    ctx ??= new AudioContext();
    if (ctx.state === "suspended") void ctx.resume();
    return ctx;
  } catch {
    return null;
  }
}

/** One short enveloped tone. */
function tone(ac: AudioContext, freq: number, at: number, length: number, gain: number) {
  const osc = ac.createOscillator();
  const amp = ac.createGain();
  osc.type = "sine";
  osc.frequency.value = freq;
  amp.gain.setValueAtTime(0, at);
  amp.gain.linearRampToValueAtTime(gain, at + 0.002);
  amp.gain.exponentialRampToValueAtTime(0.0001, at + length);
  osc.connect(amp).connect(ac.destination);
  osc.start(at);
  osc.stop(at + length + 0.01);
}

/** What each cue sounds like: a high tick to move, a soft rising pair to select, a low tick at an edge. */
export const VOICES: Record<Cue, { freq: number; length: number; at: number }[]> = {
  move: [{ freq: 2200, length: 0.012, at: 0 }],
  select: [
    { freq: 660, length: 0.05, at: 0 },
    { freq: 990, length: 0.06, at: 0.035 },
  ],
  edge: [{ freq: 220, length: 0.03, at: 0 }],
};

export function play(cue: Cue): void {
  const { sounds, volume } = useControllerSettings.getState();
  if (!sounds || volume <= 0) return;
  const ac = audio();
  if (!ac) return;
  const now = ac.currentTime;
  for (const v of VOICES[cue]) tone(ac, v.freq, now + v.at, v.length, 0.5 * volume);
}

interface Actuator {
  playEffect?: (type: string, params: Record<string, number>) => Promise<unknown>;
}

/** About 20 ms of light rumble, on select only. */
export function rumble(): void {
  if (!useControllerSettings.getState().rumble) return;
  try {
    const pad = navigator.getGamepads?.().find((p) => !!p && p.connected) as
      | (Gamepad & { vibrationActuator?: Actuator })
      | undefined;
    void pad?.vibrationActuator?.playEffect?.("dual-rumble", {
      duration: 20,
      startDelay: 0,
      weakMagnitude: 0.25,
      strongMagnitude: 0,
    });
  } catch {
    /* no actuator in this WebView */
  }
}
