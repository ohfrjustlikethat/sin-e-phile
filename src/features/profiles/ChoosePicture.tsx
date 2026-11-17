import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Button } from "@/design-system";
import { commands, type CropView, type PicturePreview } from "@/lib/ipc";
import { toast } from "@/lib/toast";
import { PictureEditor } from "./PictureEditor";
import { PICTURE } from "./ProfileAvatar";

async function ok<T>(p: Promise<{ status: "ok"; data: T } | { status: "error"; error: string }>) {
  const r = await p;
  if (r.status === "error") throw new Error(r.error);
  return r.data;
}

/**
 * "Choose a picture…" for a profile (Phase 16, brief item 11): any picture file, squared automatically, adjusted in
 * the editor, kept; and "Remove picture" once there is one. Said on the spot and in a toast.
 */
export function ChoosePicture({ profile }: { profile: number }) {
  const client = useQueryClient();
  const [chosen, setChosen] = useState<{ path: string; preview: PicturePreview } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const has = useQuery({
    queryKey: PICTURE(profile),
    queryFn: async () => {
      const r = await commands.profilePicture(profile);
      return r.status === "ok" ? r.data : null;
    },
    staleTime: Infinity,
  });

  const choose = async () => {
    setError(null);
    try {
      const path = await ok(commands.pickPicture());
      if (!path) return;
      setChosen({ path, preview: await ok(commands.picturePreview(path)) });
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };
  const save = useMutation({
    mutationFn: (crop: CropView) => ok(commands.setProfilePicture(profile, chosen!.path, crop)),
    onSuccess: () => {
      setChosen(null);
      void client.invalidateQueries({ queryKey: PICTURE(profile) });
      toast("Picture saved: it shows next to Downloads and when choosing who is watching.");
    },
    onError: (e) => setError(e.message),
  });
  const remove = useMutation({
    mutationFn: () => ok(commands.clearProfilePicture(profile)),
    onSuccess: () => {
      void client.invalidateQueries({ queryKey: PICTURE(profile) });
      toast("Picture removed.", "neutral");
    },
  });

  return (
    <div className="flex flex-wrap items-center gap-2">
      <Button size="sm" variant="secondary" onClick={() => void choose()}>
        {has.data ? "Change picture…" : "Choose a picture…"}
      </Button>
      {has.data && (
        <Button size="sm" variant="ghost" onClick={() => remove.mutate()} disabled={remove.isPending}>
          Remove picture
        </Button>
      )}
      {error && (
        <p className="basis-full text-sm text-danger" role="alert">
          {error}
        </p>
      )}
      {chosen && (
        <PictureEditor
          preview={chosen.preview}
          busy={save.isPending}
          onSave={(crop) => save.mutate(crop)}
          onCancel={() => setChosen(null)}
        />
      )}
    </div>
  );
}
