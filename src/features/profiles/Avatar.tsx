/**
 * A profile's avatar: its initial in the display face on one of six muted grounds
 * (tokens.css, Phase 14). Typographic, like everything else in an artwork-free install.
 */

export const AVATAR_COLOURS = ["slate", "sage", "ochre", "plum", "clay", "teal"] as const;
export type AvatarColour = (typeof AVATAR_COLOURS)[number];

const GROUND: Record<AvatarColour, string> = {
  slate: "bg-avatar-slate",
  sage: "bg-avatar-sage",
  ochre: "bg-avatar-ochre",
  plum: "bg-avatar-plum",
  clay: "bg-avatar-clay",
  teal: "bg-avatar-teal",
};

/** A stored colour name, or the first one for a profile made before there were any. */
export function colourOf(name: string | null): AvatarColour {
  return (AVATAR_COLOURS as readonly string[]).includes(name ?? "")
    ? (name as AvatarColour)
    : "slate";
}

/** "Mina" -> "M"; a name starting with a symbol or digit keeps it. */
export function initialOf(name: string): string {
  const first = [...name.trim()][0];
  return first ? first.toLocaleUpperCase() : "?";
}

export function Avatar({
  name,
  colour,
  size = "sm",
}: {
  name: string;
  colour: string | null;
  size?: "sm" | "lg";
}) {
  const box = size === "lg" ? "h-28 w-28 text-[52px]" : "h-7 w-7 text-[13px]";
  return (
    <span
      aria-hidden
      className={`grid shrink-0 place-items-center rounded-sm font-display font-extrabold leading-none text-ink ${box} ${GROUND[colourOf(colour)]}`}
    >
      {initialOf(name)}
    </span>
  );
}
