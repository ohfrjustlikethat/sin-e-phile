import { useEffect, useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { StarRating } from "@/design-system";
import { commands, type Dismissal } from "@/lib/ipc";
import { toast } from "@/lib/toast";
import { useUi } from "@/lib/store";
import { useTitleRating } from "@/features/ratings/useRating";
import { TitleCard } from "./TitleCard";

/**
 * A title's card with its menu: X on a pad, the context-menu key or Shift+F10 on a keyboard, a right click with a
 * mouse. Every card's menu rates the title (ADR-0056, the author's answers): a film's stars there and then, a show's
 * seasons on its page. In Continue watching it also removes the title from the row (hidden, its place kept), which an ×
 * on the card does too for the mouse. A recommendation's menu (Phase 18) adds "not for me", "not right now" and
 * "already seen"; the card then stands in its place as what was said, with an Undo, and a toast says what follows.
 */

const KINDS: Dismissal[] = ["not_for_me", "not_right_now", "already_seen"];

export const SAID: Record<Dismissal, { label: string; after: string }> = {
  not_for_me: { label: "Not for me", after: "it won't be recommended again" },
  not_right_now: { label: "Not right now", after: "it's off Home for two weeks" },
  already_seen: { label: "Already seen", after: "marked as seen" },
};

type Props = Parameters<typeof TitleCard>[0] & {
  title: string;
  /** A show: rated season by season, on its page. */
  isShow: boolean;
  /** A recommendation: it can be dismissed. */
  dismissible?: boolean;
  /** In Continue watching: it can be removed from the row. */
  continuing?: boolean;
};

export function MenuCard({ isShow, dismissible = false, continuing = false, ...props }: Props) {
  const { id, title } = props;
  const client = useQueryClient();
  const [menu, setMenu] = useState(false);
  const [gone, setGone] = useState<Dismissal | null>(null);
  const box = useRef<HTMLDivElement>(null);
  const panel = useRef<HTMLDivElement>(null);
  const undo = useRef<HTMLButtonElement>(null);

  // Focus follows what is on screen: into the menu, onto Undo, back to the card.
  useEffect(() => {
    if (menu) panel.current?.querySelector<HTMLElement>("input, button")?.focus();
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

  const remove = async () => {
    setMenu(false);
    const r = await commands.removeFromContinue(id);
    if (r.status === "error") {
      toast(`Not removed: ${r.error}`, "danger");
      return;
    }
    toast(`${title}: removed from Continue watching. Play it again and it comes back.`, "neutral");
    void client.invalidateQueries({ queryKey: ["home"] });
    void client.invalidateQueries({ queryKey: ["continue-watching"] });
    void client.invalidateQueries({ queryKey: ["in-continue", id] });
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

  const item = "px-3 py-2 text-left font-ui text-[13px] text-ink hover:bg-raised focus-visible:bg-raised";
  return (
    <div
      ref={box}
      className="group/card relative"
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
      {continuing && !menu && (
        // The mouse's way out of the row; a pad and a keyboard use the menu, so it takes no focus.
        <button
          type="button"
          tabIndex={-1}
          aria-label={`Remove ${title} from Continue watching`}
          onClick={(e) => {
            e.stopPropagation();
            void remove();
          }}
          className="absolute right-1.5 top-1.5 z-30 grid h-7 w-7 place-items-center rounded-sm bg-void/80 text-ink-muted opacity-0 transition-opacity duration-[var(--dur-standard)] hover:text-ink group-hover/card:opacity-100"
        >
          <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden>
            <path d="M0 0l10 10M10 0L0 10" stroke="currentColor" strokeWidth="1.2" />
          </svg>
        </button>
      )}
      {menu && (
        <div
          ref={panel}
          role="dialog"
          aria-label={`${title}: options`}
          className="absolute left-2 right-2 top-2 z-40 flex flex-col rounded-sm border border-line bg-overlay py-1.5"
        >
          {isShow ? (
            <button
              type="button"
              onClick={() => {
                setMenu(false);
                useUi.getState().openDetail(id);
              }}
              className={item}
            >
              Rate seasons…
            </button>
          ) : (
            <MenuStars id={id} />
          )}
          {continuing && (
            <button type="button" onClick={() => void remove()} className={item}>
              Remove from Continue watching
            </button>
          )}
          {dismissible &&
            KINDS.map((kind) => (
              <button key={kind} type="button" onClick={() => void choose(kind)} className={item}>
                {SAID[kind].label}
              </button>
            ))}
        </div>
      )}
    </div>
  );
}

/** A film's stars in its card's menu. */
function MenuStars({ id }: { id: number }) {
  const { halves, set } = useTitleRating(id);
  return (
    <div className="flex items-center gap-3 px-3 py-2">
      <StarRating size="sm" value={halves} onChange={set} label="Your rating" />
    </div>
  );
}
