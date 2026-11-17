import { useQuery } from "@tanstack/react-query";
import { commands } from "@/lib/ipc";
import { Avatar } from "./Avatar";

/** The query key for a profile's picture: changed, it is asked again everywhere it shows. */
export const PICTURE = (profile: number) => ["profile-picture", profile] as const;

/**
 * A profile's avatar: its picture when it has one (Phase 16: next to Downloads in the masthead, in the profile picker,
 * in Settings), its coloured initial otherwise.
 */
export function ProfileAvatar({
  profile,
  size,
}: {
  profile: { id: number; name: string; avatar_colour: string | null };
  size?: "sm" | "lg";
}) {
  const picture = useQuery({
    queryKey: PICTURE(profile.id),
    queryFn: async () => {
      const r = await commands.profilePicture(profile.id);
      return r.status === "ok" ? r.data : null;
    },
    staleTime: Infinity,
  });
  return <Avatar name={profile.name} colour={profile.avatar_colour} {...(size ? { size } : {})} picture={picture.data ?? null} />;
}
