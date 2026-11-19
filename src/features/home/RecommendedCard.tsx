import { useEffect, useRef, useState } from "react";
import { commands, type Dismissal } from "@/lib/ipc";
import { toast } from "@/lib/toast";
import { TitleCard } from "./TitleCard";

/**
 * A recommended title's card (Phase 18): Home's taste rail and the discovery rails. Besides
 * opening, it can be told "not for me", "not right now" or "already seen": X on a pad, the
 * context-menu key or Shift+F10 on a keyboard, a right click with a mouse. The card then
 * stands in its place as what was said with an Undo, and a toast says what follows.
 */

const KINDS: Dismissal[] = ["not_for_me", "not_right_now", "already_seen"];

export const SAID: Record<Dismissal, { label: string; after: string }> = {
  not_for_me: { label: "Not for me", after: "it won't be recommended again" },
  not_right_now: { label: "Not right now", after: "it's off Home for two weeks" },
  already_seen: { label: "Already seen", after: "marked as seen" },
};

type Props = Parameters<typeof TitleCard>[0] & { title: string };

export function RecommendedCard(props: Props) {
  const { id, title } = props;
  const [menu, setMenu] = useState(false);
  const [gone, setGone] = useState<Dismissal | null>(null);
  const box = useRef<HTMLDivElement>(null);
  const first = useRef<HTMLButtonElement>(null);
  const undo = useRef<HTMLButtonElement>(null);

  // Focus follows what is on screen: into the menu, onto Undo, back to the card.
  useEffect(() => {
    if (menu) first.current?.focus();
  }, [menu]);
  useEffect(() => {
    if (gone) undo.current?.focus();
  }, [gone]);
  const refocus = () =>
    requestAnimationFrame(() => box.current?.querySelector<HTMLElement>("[data-focus-tile]")?.focus());

  // B on a pad arrives at the window, not at the menu: close the menu, and keep Back from also
  // leaving the page beneath it.
  useEffect(() => {
    if (!menu) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      e.preventDefault();
      setMenu(false);
      refocus();
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [menu]);

  const choose = async (kind: Dismissal) => {
    setMenu(false);
    const r = await commands.dismiss(id, kind, new Date().getHours());
    if (r.status === "error") {
      toast(`Not changed: ${r.error}`, "danger");
      refocus();
      return;
    }
    setGone(kind);
    toast(`${title}: ${SAID[kind].after}.`);
  };

  const restore = async () => {
    const r = await commands.undismiss(id);
    if (r.status === "error") {
      toast(`Not changed: ${r.error}`, "danger");
      return;
    }
    setGone(null);
    toast(`${title} is back.`, "neutral");
    refocus();
  };

  if (gone) {
    return (
      <div
        ref={box}
        data-focus-key={`title-${id}`}
        className={[
          "flex h-full min-h-[120px] shrink-0 flex-col justify-between rounded-sm border border-line-subtle px-3 py-3",
          { lead: "w-[236px]", sm: "w-[152px]", md: "w-[176px]", fill: "w-full" }[props.size ?? "md"],
        ].join(" ")}
      >
        <p className="font-ui text-[12.5px] text-ink-muted">
          <span className="text-ink">{title}</span>
          <br />
          {SAID[gone].label}
        </p>
        <button
          ref={undo}
          type="button"
          onClick={() => void restore()}
          className="self-start font-ui text-[12.5px] text-ink underline underline-offset-4"
        >
          Undo
        </button>
      </div>
    );
  }

  return (
    <div
      ref={box}
      className="relative"
      onKeyDown={(e) => {
        if (menu && e.key === "Escape") {
          e.preventDefault();
          setMenu(false);
          refocus();
        } else if (!menu && (e.key === "ContextMenu" || (e.shiftKey && e.key === "F10"))) {
          e.preventDefault();
          setMenu(true);
        }
      }}
      onContextMenu={(e) => {
        e.preventDefault();
        setMenu(true);
      }}
    >
      <TitleCard {...props} />
      {menu && (
        <div
          role="dialog"
          aria-label={`${title}: not interested?`}
          className="absolute left-2 right-2 top-2 z-40 flex flex-col rounded-sm border border-line bg-overlay py-1.5"
        >
          {KINDS.map((kind, i) => (
            <button
              key={kind}
              ref={i === 0 ? first : undefined}
              type="button"
              onClick={() => void choose(kind)}
              className="px-3 py-2 text-left font-ui text-[13px] text-ink hover:bg-raised focus-visible:bg-raised"
            >
              {SAID[kind].label}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
