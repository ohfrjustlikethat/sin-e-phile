export interface Channel {
  id: string;
  name: string;
  number?: number;
  category: string;
  logo?: string;
  nowPlaying?: string;
  live?: boolean;
}

/**
 * Live channel card.
 *
 * Square, because a channel logo is square — a third card shape, and that is the
 * point: shape carries meaning here rather than being a grid decision (§9.0).
 */
export function ChannelCard({ channel, onOpen }: { channel: Channel; onOpen?: (id: string) => void }) {
  return (
    <article className="w-[152px] shrink-0">
      <button
        type="button"
        onClick={() => onOpen?.(channel.id)}
        aria-label={channel.name}
        className="relative block aspect-square w-full overflow-hidden rounded-sm border border-line-subtle bg-surface transition-colors hover:border-line-strong"
      >
        {channel.logo ? (
          <img src={channel.logo} alt="" loading="lazy" className="h-full w-full object-contain p-5" />
        ) : (
          <div className="flex h-full items-center justify-center p-4">
            <span className="font-display text-[22px] font-extrabold uppercase leading-none tracking-[-0.04em] text-ink">
              {channel.name.slice(0, 3)}
            </span>
          </div>
        )}
        {channel.live && (
          <span className="absolute right-2 top-2 flex items-center gap-1.5">
            <span className="h-1.5 w-1.5 rounded-sm bg-oxblood-bright" />
            <span className="label text-oxblood-text">Live</span>
          </span>
        )}
      </button>
      <div className="mt-2.5">
        <h3 className="truncate font-display text-[13px] font-semibold tracking-[-0.01em] text-ink">
          {channel.number !== undefined && (
            <span className="mr-1.5 font-mono text-[10px] text-ink-faint">{channel.number}</span>
          )}
          {channel.name}
        </h3>
        {channel.nowPlaying && (
          <p className="mt-0.5 truncate text-[10.5px] text-ink-faint">{channel.nowPlaying}</p>
        )}
      </div>
    </article>
  );
}
