import { useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Button, Input, Toggle } from "@/design-system";
import { commands, type ProfileView } from "@/lib/ipc";
import { AVATAR_COLOURS, Avatar, colourOf, type AvatarColour } from "@/features/profiles/Avatar";
import { PROFILES, useProfiles } from "@/features/profiles/useProfiles";
import { ImportHistory } from "@/features/profiles/ImportHistory";

/**
 * Settings → Profiles — SPEC.md Phase 14. Add a profile (a name, a colour, kids or not,
 * a PIN if wanted), change or delete one, and set the watching profile's own PIN.
 * Deleting takes the profile's history, ratings, watchlist and settings, and says so
 * first. A kids profile can see this list but not change it.
 */

async function ok<T>(p: Promise<{ status: "ok"; data: T } | { status: "error"; error: string }>) {
  const r = await p;
  if (r.status === "error") throw new Error(r.error);
  return r.data;
}

export function ProfilesSection() {
  const { profiles, active } = useProfiles();
  const [adding, setAdding] = useState(false);
  const locked = active?.is_kids ?? false;

  return (
    <section className="mb-9" aria-label="Profiles">
      <h2 className="mb-3.5 text-[11px] font-semibold uppercase tracking-[0.12em] text-ink-faint">Profiles</h2>
      <ul className="flex flex-col">
        {profiles.map((p) => (
          <ProfileRow key={p.id} profile={p} locked={locked} last={profiles.length === 1} />
        ))}
      </ul>
      {!locked &&
        (adding ? (
          <ProfileForm onDone={() => setAdding(false)} />
        ) : (
          <div className="mt-3">
            <Button variant="secondary" onClick={() => setAdding(true)}>
              Add a profile
            </Button>
          </div>
        ))}
      {active && <OwnPin profile={active} />}
      {active && (
        <div className="mt-8">
          <h3 className="mb-2 text-[13px] text-ink">Import {active.name}'s history</h3>
          <ImportHistory />
        </div>
      )}
    </section>
  );
}

function ProfileRow({ profile, locked, last }: { profile: ProfileView; locked: boolean; last: boolean }) {
  const client = useQueryClient();
  const [editing, setEditing] = useState(false);
  const [confirm, setConfirm] = useState(false);
  const remove = useMutation({
    mutationFn: () => ok(commands.deleteProfile(profile.id)),
    onSettled: () => void client.invalidateQueries({ queryKey: PROFILES }),
  });

  if (editing) return <ProfileForm profile={profile} onDone={() => setEditing(false)} />;
  return (
    <li className="flex items-center gap-3 border-b border-line-subtle py-2.5">
      <Avatar name={profile.name} colour={profile.avatar_colour} />
      <span className="flex-1 text-sm text-ink">
        {profile.name}
        <span className="ml-2 text-xs text-ink-faint">
          {[profile.active ? "watching now" : null, profile.is_kids ? "kids" : null, profile.has_pin ? "PIN" : null]
            .filter(Boolean)
            .join(" · ")}
        </span>
      </span>
      {!locked &&
        (confirm ? (
          <>
            <span className="text-xs text-ink-muted">Delete {profile.name}, with its history and watchlist?</span>
            <Button size="sm" variant="danger" onClick={() => remove.mutate()}>
              Delete
            </Button>
            <Button size="sm" variant="ghost" onClick={() => setConfirm(false)}>
              Keep
            </Button>
          </>
        ) : (
          <>
            <Button size="sm" variant="ghost" aria-label={`Edit ${profile.name}`} onClick={() => setEditing(true)}>
              Edit
            </Button>
            {!last && (
              <Button size="sm" variant="ghost" aria-label={`Delete ${profile.name}`} onClick={() => setConfirm(true)}>
                Delete…
              </Button>
            )}
          </>
        ))}
      {remove.isError && <span className="text-xs text-danger">{remove.error.message}</span>}
    </li>
  );
}

function ProfileForm({ profile, onDone }: { profile?: ProfileView; onDone: () => void }) {
  const client = useQueryClient();
  const [name, setName] = useState(profile?.name ?? "");
  const [colour, setColour] = useState<AvatarColour>(colourOf(profile?.avatar_colour ?? "sage"));
  const [kids, setKids] = useState(profile?.is_kids ?? false);
  const [pin, setPin] = useState("");
  const save = useMutation({
    mutationFn: async () => {
      if (profile) await ok(commands.updateProfile(profile.id, name, colour, kids));
      else await ok(commands.createProfile(name, colour, kids, pin || null));
    },
    onSuccess: () => {
      void client.invalidateQueries({ queryKey: PROFILES });
      onDone();
    },
  });

  return (
    <li className="mt-3 flex list-none flex-col gap-3 border border-line p-4">
      <Input label="Name" value={name} onChange={(e) => setName(e.target.value)} maxLength={40} />
      <div role="radiogroup" aria-label="Colour" className="flex gap-2">
        {AVATAR_COLOURS.map((c) => (
          <button
            key={c}
            type="button"
            role="radio"
            aria-checked={colour === c}
            aria-label={c}
            onClick={() => setColour(c)}
            className={`rounded-sm p-0.5 ${colour === c ? "outline outline-1 outline-ink" : ""}`}
          >
            <Avatar name={name || "?"} colour={c} />
          </button>
        ))}
      </div>
      <Toggle
        checked={kids}
        onChange={setKids}
        label="Kids profile"
        description="Shows only films filed as for children or families, and cannot change profiles."
      />
      {!profile && (
        <Input
          label="PIN (optional, four digits)"
          inputMode="numeric"
          maxLength={4}
          value={pin}
          onChange={(e) => setPin(e.target.value.replace(/\D/g, ""))}
        />
      )}
      <div className="flex items-center gap-2">
        <Button onClick={() => save.mutate()} disabled={save.isPending || !name.trim()}>
          {profile ? "Save" : "Add"}
        </Button>
        <Button variant="ghost" onClick={onDone}>
          Cancel
        </Button>
        {save.isError && <span className="text-xs text-danger">{save.error.message}</span>}
      </div>
    </li>
  );
}

function OwnPin({ profile }: { profile: ProfileView }) {
  const client = useQueryClient();
  const [pin, setPin] = useState("");
  const [note, setNote] = useState<string | null>(null);
  const set = useMutation({
    mutationFn: (value: string | null) => ok(commands.setProfilePin(value)),
    onSuccess: (_, value) => {
      setPin("");
      setNote(value ? "PIN set." : "PIN removed.");
      void client.invalidateQueries({ queryKey: PROFILES });
    },
    onError: (e) => setNote(e.message),
  });
  return (
    <div className="mt-6 flex items-end gap-2">
      <div className="w-56">
        <Input
          label={`${profile.name}'s PIN`}
          inputMode="numeric"
          maxLength={4}
          placeholder={profile.has_pin ? "Set; type to change" : "None"}
          value={pin}
          onChange={(e) => setPin(e.target.value.replace(/\D/g, ""))}
        />
      </div>
      <Button size="sm" onClick={() => set.mutate(pin)} disabled={pin.length !== 4}>
        Set PIN
      </Button>
      {profile.has_pin && (
        <Button size="sm" variant="ghost" onClick={() => set.mutate(null)}>
          Remove PIN
        </Button>
      )}
      {note && (
        <span className="text-xs text-ink-muted" role="status">
          {note}
        </span>
      )}
    </div>
  );
}
