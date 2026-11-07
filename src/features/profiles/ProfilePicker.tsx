import { useEffect, useRef, useState } from "react";
import { Button } from "@/design-system";
import type { ProfileView } from "@/lib/ipc";
import { useUi } from "@/lib/store";
import { Avatar } from "./Avatar";
import { useProfiles } from "./useProfiles";

/**
 * Who's watching? — SPEC.md Phase 14, ADR-0043: shown on launch only when there are two
 * or more profiles, and from the masthead's avatar. One tile per profile, most recently
 * used first; a profile with a PIN asks for it on a pad the D-pad, the mouse and the
 * number keys all work.
 *
 * Escape (B) keeps whoever is watching.
 */
export function ProfilePicker({ onDone }: { onDone: () => void }) {
  const { profiles, switchTo } = useProfiles();
  const openSettings = useUi((s) => s.setSettingsOpen);
  const setTasteOpen = useUi((s) => s.setTasteOpen);
  const [asking, setAsking] = useState<ProfileView | null>(null);
  const first = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    if (!asking) first.current?.focus();
  }, [asking, profiles.length]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" && !asking) onDone();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [asking, onDone]);

  const choose = async (p: ProfileView) => {
    if (p.has_pin) {
      setAsking(p);
      return;
    }
    await switchTo(p.id, null);
    onDone();
  };

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label="Who's watching?"
      className="fixed inset-0 z-50 flex flex-col items-center justify-center bg-base text-ink"
    >
      {asking ? (
        <PinPad
          profile={asking}
          onCancel={() => setAsking(null)}
          onEnter={async (pin) => {
            await switchTo(asking.id, pin);
            onDone();
          }}
        />
      ) : (
        <>
          <h1 className="font-display text-[44px] font-extrabold tracking-[-0.03em]">Who's watching?</h1>
          <ul className="mt-14 flex flex-wrap justify-center gap-8">
            {profiles.map((p, i) => (
              <li key={p.id}>
                <button
                  ref={i === 0 ? first : undefined}
                  type="button"
                  onClick={() => void choose(p)}
                  aria-label={`${p.name}${p.is_kids ? ", kids" : ""}${p.has_pin ? ", PIN" : ""}`}
                  className="group flex w-36 flex-col items-center gap-4 rounded-sm p-3"
                >
                  <Avatar name={p.name} colour={p.avatar_colour} size="lg" />
                  <span className="text-[15px] text-ink-muted group-hover:text-ink">{p.name}</span>
                  <span className="label h-4">
                    {[p.is_kids ? "Kids" : null, p.has_pin ? "PIN" : null].filter(Boolean).join(" · ")}
                  </span>
                </button>
              </li>
            ))}
          </ul>
          <div className="mt-14 flex gap-4">
            <Button
              variant="ghost"
              onClick={() => {
                onDone();
                openSettings(true);
              }}
            >
              Manage profiles
            </Button>
            <Button
              variant="ghost"
              onClick={() => {
                onDone();
                setTasteOpen(true);
              }}
            >
              Your taste
            </Button>
          </div>
        </>
      )}
    </div>
  );
}

const KEYS = ["1", "2", "3", "4", "5", "6", "7", "8", "9", "", "0", "⌫"];

export function PinPad({
  profile,
  onEnter,
  onCancel,
}: {
  profile: ProfileView;
  onEnter: (pin: string) => Promise<void>;
  onCancel: () => void;
}) {
  const [pin, setPin] = useState("");
  const [wrong, setWrong] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const firstKey = useRef<HTMLButtonElement>(null);

  useEffect(() => firstKey.current?.focus(), []);

  const press = (k: string) => {
    if (busy) return;
    setWrong(null);
    if (k === "⌫") setPin((p) => p.slice(0, -1));
    else if (/^\d$/.test(k)) setPin((p) => (p.length < 4 ? p + k : p));
  };

  // Four digits: try them.
  useEffect(() => {
    if (pin.length !== 4) return;
    setBusy(true);
    onEnter(pin)
      .catch((e: Error) => {
        setWrong(e.message);
        setPin("");
      })
      .finally(() => setBusy(false));
  }, [pin]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (/^\d$/.test(e.key)) press(e.key);
      else if (e.key === "Backspace") press("⌫");
      else if (e.key === "Escape") onCancel();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  return (
    <div className="flex flex-col items-center">
      <Avatar name={profile.name} colour={profile.avatar_colour} size="lg" />
      <h1 className="mt-6 font-display text-[28px] font-extrabold tracking-[-0.02em]">{profile.name}'s PIN</h1>
      <div className="mt-6 flex gap-3" aria-label={`${pin.length} of 4 digits entered`} role="status">
        {[0, 1, 2, 3].map((i) => (
          <span
            key={i}
            className={`h-3 w-3 rounded-sm border border-line-interactive ${i < pin.length ? "bg-ink" : ""}`}
          />
        ))}
      </div>
      <p className="mt-4 h-5 text-sm text-danger" role="alert">
        {wrong}
      </p>
      <div className="mt-4 grid grid-cols-3 gap-2">
        {KEYS.map((k, i) =>
          k === "" ? (
            <span key={i} />
          ) : (
            <button
              key={i}
              ref={k === "1" ? firstKey : undefined}
              type="button"
              onClick={() => press(k)}
              aria-label={k === "⌫" ? "Delete" : k}
              className="grid h-14 w-14 place-items-center rounded-sm border border-line text-[20px] text-ink hover:bg-raised"
            >
              {k}
            </button>
          ),
        )}
      </div>
      <div className="mt-8">
        <Button variant="ghost" onClick={onCancel}>
          Back
        </Button>
      </div>
    </div>
  );
}
