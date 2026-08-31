import { useState } from "react";
import {
  Badge, Button, ChannelCard, CommandPalette, Dialog, EmptyState, EpisodeCard,
  HeroBanner, IconButton, Input, PosterCard, ProgressBar, Rail, Rating, Select,
  Skeleton, Slider, Spinner, Tabs, ToastStack, Toggle, Tooltip,
  type Command, type PosterFilm, type ToastMessage,
} from "@/design-system";

/**
 * The component gallery — SPEC.md Phase 2, subtask 2.2. Dev-only route `/design`.
 *
 * The exit criterion is "every primitive renders correctly in the gallery, in all
 * states", and "the entire gallery is navigable by keyboard alone with visible
 * focus at every step". So this is a test surface, not a showcase: every variant,
 * size, and state appears, including the ones nobody wants to look at — disabled,
 * loading, error, empty, and the artwork-free card.
 *
 * It also holds the 500-card rail that the 60fps criterion is measured against.
 */

/** Real public-domain films, so cards are exercised against real title lengths.
 *
 * The artwork is real frames from the films themselves, in `public/stills/`, for
 * the reason the author gave when commissioning the mockups: a design reviewed
 * against grey placeholder boxes is not reviewed at all. Every one of these is a
 * public-domain film, so the stills carry no licensing question. */
const FILMS: PosterFilm[] = [
  { id: "notld", title: "Night of the Living Dead", year: 1968, director: "George A. Romero", spine: 41, availability: "local", progress: 0.38, artwork: "/stills/night-of-the-living-dead-0.jpg" },
  { id: "potemkin", title: "Battleship Potemkin", year: 1925, director: "Sergei Eisenstein", spine: 7, availability: "stream", artwork: "/stills/battleship-potemkin-1.jpg" },
  { id: "scarlet", title: "Scarlet Street", year: 1945, director: "Fritz Lang", spine: 112, availability: "local", artwork: "/stills/scarlet-street-0.jpg" },
  { id: "chien", title: "Un Chien Andalou", year: 1929, director: "Luis Buñuel", spine: 3, availability: "download", artwork: "/stills/un-chien-andalou-1.jpg" },
  { id: "detour", title: "Detour", year: 1945, director: "Edgar G. Ulmer", spine: 88, availability: "local", progress: 0.72, artwork: "/stills/detour-0.jpg" },
  // ADR-0013: no artwork. This must be the nicest card in the row, not the saddest.
  { id: "joan", title: "The Passion of Joan of Arc", year: 1928, director: "Carl Th. Dreyer", spine: 62 },
  { id: "general", title: "The General", year: 1926, director: "Buster Keaton", spine: 19 },
];

/** 500 cards, for the 60fps virtualisation criterion. Mixed states on purpose:
 *  the measurement should include image decoding, not only text layout. */
const MANY: PosterFilm[] = Array.from({ length: 500 }, (_, i) => ({
  ...FILMS[i % FILMS.length]!,
  id: `many-${i}`,
  spine: i + 1,
}));

export function DesignGallery() {
  const [tab, setTab] = useState("primitives");
  const [toggle, setToggle] = useState(true);
  const [slider, setSlider] = useState(38);
  const [dialog, setDialog] = useState(false);
  const [palette, setPalette] = useState(false);
  const [toasts, setToasts] = useState<ToastMessage[]>([]);

  const commands: Command[] = [
    { id: "play", label: "Play something I'll like", group: "Action", hint: "Enter", run: () => {} },
    { id: "search", label: "Search the catalogue", group: "Action", run: () => {} },
    { id: "settings", label: "Open settings", group: "Settings", hint: "Ctrl+,", run: () => {} },
    { id: "tier", label: "Change hardware tier", group: "Settings", run: () => {} },
    { id: "f1", label: "Night of the Living Dead", group: "Film", hint: "1968", run: () => {} },
    { id: "f2", label: "Battleship Potemkin", group: "Film", hint: "1925", run: () => {} },
  ];

  // `exactOptionalPropertyTypes` is on, so an optional field may be ABSENT but
  // not explicitly `undefined`. Spreading conditionally is the honest fix; casting
  // would just hide the distinction the flag exists to enforce.
  const pushToast = (text: string, tone?: ToastMessage["tone"]) =>
    setToasts((t) => [
      ...t,
      { id: String(Date.now()), text, ...(tone ? { tone } : {}) },
    ]);

  return (
    <div className="h-full overflow-y-auto bg-base">
      <Section title="Design system" lead="SPEC.md §9 · ADR-0023 · ADR-0024 (Take B, 74vh hero)">
        <p className="max-w-2xl text-[13px] leading-relaxed text-ink-muted">
          Every primitive in every state, including the ones nobody wants to look
          at. If a component only looks right here in its happy state, it is not
          finished.
        </p>
      </Section>

      <div className="px-10">
        <Tabs
          tabs={[
            { id: "primitives", label: "Primitives" },
            { id: "media", label: "Media" },
            { id: "type", label: "Type & colour" },
            { id: "perf", label: "500-card rail" },
          ]}
          active={tab}
          onChange={setTab}
        />
      </div>

      {tab === "primitives" && (
        <>
          <Section title="Button" lead="Four variants × three sizes, plus loading and disabled">
            <Row>
              <Button variant="primary">Play</Button>
              <Button variant="secondary">Watchlist</Button>
              <Button variant="ghost">Dismiss</Button>
              <Button variant="danger">Delete download</Button>
            </Row>
            <Row>
              <Button variant="primary" size="sm">Small</Button>
              <Button variant="primary" size="md">Medium</Button>
              <Button variant="primary" size="lg">Large</Button>
            </Row>
            <Row>
              <Button variant="primary" loading>Resolving</Button>
              <Button variant="secondary" disabled>Unavailable</Button>
              <Button variant="ghost" disabled>Disabled</Button>
            </Row>
            <Note>
              Only <b>one</b> primary per screen — oxblood marks the single action a
              screen is asking for (§9.1). A destructive confirm pairs
              <code> danger </code> with <code> secondary </code>, never with
              <code> primary </code>, because §9.1 forbids danger and oxblood adjacent.
            </Note>
          </Section>

          <Section title="IconButton" lead="Icon-only, with a required accessible name">
            <Row>
              <IconButton label="Play"><PlayGlyph /></IconButton>
              <IconButton label="Mute"><MuteGlyph /></IconButton>
              <IconButton label="Fullscreen" active><FullscreenGlyph /></IconButton>
              <IconButton label="Disabled" disabled><PlayGlyph /></IconButton>
            </Row>
          </Section>

          <Section title="Input, Select" lead="Default, hint, error, disabled">
            <div className="grid max-w-3xl grid-cols-2 gap-6">
              <Input label="Source manifest URL" placeholder="https://example.com/manifest.json" />
              <Input label="Folder" defaultValue="D:\\Films" hint="Scanned continuously" />
              <Input label="API key" defaultValue="nope" error="That key was rejected" />
              <Input label="Disabled" placeholder="Not available" disabled />
              <Select
                label="Preferred audio"
                options={[
                  { value: "en", label: "English" },
                  { value: "ja", label: "Japanese" },
                  { value: "hi", label: "Hindi" },
                ]}
              />
              <Select label="Disabled" disabled options={[{ value: "a", label: "Nothing" }]} />
            </div>
          </Section>

          <Section title="Toggle, Slider">
            <div className="max-w-xl">
              <Toggle checked={toggle} onChange={setToggle} label="Hover previews"
                description="Muted, and only after a 400 ms dwell." />
              <Toggle checked={false} onChange={() => {}} label="Start with Windows" />
              <Toggle checked disabled onChange={() => {}} label="Face recognition"
                description="Requires Tier 2 hardware. Your machine is Tier 1." />
              <Slider label="Subtitle delay" value={slider} onChange={setSlider}
                display={`${(slider - 50) * 10} ms`} />
            </div>
          </Section>

          <Section title="Badge, Rating, ProgressBar">
            <Row>
              <Badge>1080p</Badge>
              <Badge tone="accent">Continue</Badge>
              <Badge tone="success">On disk</Badge>
              <Badge tone="warning">Quota low</Badge>
              <Badge tone="danger">Dead swarm</Badge>
              <Badge tone="info">Stream</Badge>
            </Row>
            <Row>
              <Rating value={7.8} source="IMDb" />
              <Rating value={8.1} source="TMDB" />
              <Rating value={6.2} source="AniList" />
            </Row>
            <div className="max-w-md space-y-4">
              <ProgressBar value={0.38} buffered={0.59} label="Playback" />
              <ProgressBar value={0.72} buffered={0.72} label="Download" size="thick" />
            </div>
            <Note>
              Ratings are numeric, not stars: stars quantise a 7.8 into "four stars"
              and lose the distinction a cinephile cares about — and a row of gold
              stars would put decorative colour on the chrome, which §9.1 reserves
              for the artwork.
            </Note>
          </Section>

          <Section title="Skeleton, Spinner">
            <Row>
              <Skeleton className="h-10 w-40" />
              <Skeleton className="h-10 w-24" />
              <Spinner />
            </Row>
          </Section>

          <Section title="Overlays" lead="Dialog, Tooltip, Toast, command palette">
            <Row>
              <Button onClick={() => setDialog(true)}>Open dialog</Button>
              <Tooltip content="Opens on hover and on focus"><Button>Hover me</Button></Tooltip>
              <Button onClick={() => pushToast("Added to watchlist", "success")}>Toast</Button>
              <Button onClick={() => pushToast("Source did not respond", "danger")}>Error toast</Button>
              <Button variant="primary" onClick={() => setPalette(true)}>Command palette (Ctrl+K)</Button>
            </Row>
          </Section>
        </>
      )}

      {tab === "media" && (
        <>
          <Section title="PosterCard" lead="Three sizes, and the artwork-free state (ADR-0013)">
            <Row>
              <PosterCard film={FILMS[0]!} size="lead" />
              <PosterCard film={FILMS[1]!} />
              <PosterCard film={FILMS[2]!} size="sm" />
              <PosterCard film={FILMS[5]!} />
              <PosterCard film={FILMS[5]!} size="lead" />
            </Row>
            <Note>
              The last two have <b>no artwork</b>. ADR-0013 made TMDB optional, so a
              real library will be full of these — they are a designed state, not a
              fallback. For a film app a typographic card is arguably the better one:
              it selects on knowledge rather than poster recognition.
            </Note>
          </Section>

          <Section title="EpisodeCard, ChannelCard" lead="Different shapes on purpose">
            <Row>
              <EpisodeCard ep={{ id: "e1", series: "Twin Peaks", season: 2, episode: 7, title: "Lonely Souls", runtime: 47, progress: 0.4, still: "/stills/his-girl-friday-0.jpg" }} />
              <EpisodeCard ep={{ id: "e2", series: "Twin Peaks", season: 2, episode: 8, title: "Drive with a Dead Girl", runtime: 45 }} />
              <ChannelCard channel={{ id: "c1", name: "Arte", number: 12, category: "Culture", nowPlaying: "Le Mépris (1963)", live: true }} />
              <ChannelCard channel={{ id: "c2", name: "Talking Pictures", number: 81, category: "Film" }} />
            </Row>
            <Note>
              Three card shapes — 3:4, 16:9, 1:1 — because a poster, a frame and a
              logo are different objects. Uniform card shapes across content types is
              one of the tells §9.0 bans.
            </Note>
          </Section>

          <Section title="EmptyState">
            <div className="h-[300px] rounded-sm border border-line-subtle">
              <EmptyState
                phase="Phase 24"
                title="No channels yet"
                body="Paste an M3U playlist URL and the guide fills itself in."
                whatInstead="Until then, everything else works — this tab is honest about being empty rather than pretending otherwise."
                action={<Button variant="primary">Add a playlist</Button>}
              />
            </div>
          </Section>

          <Section title="HeroBanner" lead="74vh — Take A's hero on Take B's system">
            <div className="-mx-10 border-y border-line">
              <HeroBanner
                film={{
                  title: "Battleship Potemkin", altTitle: "Bronenosets Potyomkin",
                  year: 1925, runtime: 75, director: "Sergei Eisenstein",
                  country: "Soviet Union", spine: 7,
                  reason: "Your blind spot — Soviet montage",
                  artwork: "/stills/battleship-potemkin-2.jpg",
                }}
                actions={<><Button variant="primary">Play</Button><Button>Watchlist</Button></>}
              />
            </div>
          </Section>
        </>
      )}

      {tab === "type" && (
        <>
          <Section title="Type" lead="Bricolage Grotesque · Instrument Serif · Inter · JetBrains Mono">
            <div className="space-y-6">
              <div>
                <div className="label mb-2">Display — Bricolage Grotesque 800</div>
                <div className="font-display text-[62px] font-extrabold uppercase leading-[0.92] tracking-[-0.045em]">
                  Battleship Potemkin
                </div>
              </div>
              <div>
                <div className="label mb-2">Editorial serif — film titles only</div>
                <div className="font-serif text-[46px] leading-tight">The Passion of Joan of Arc</div>
              </div>
              <div>
                <div className="label mb-2">UI — Inter, never at display sizes</div>
                <p className="max-w-xl text-[14px] leading-relaxed text-ink-muted">
                  Seven strangers barricade themselves in a Pennsylvania farmhouse as
                  the recently dead rise and attack the living.
                </p>
              </div>
              <div>
                <div className="label mb-2">Mono — technical panels</div>
                <div className="font-mono text-[13px] text-ink-muted">
                  00:38:12 / 01:15:00 · 14 peers · d3d11va
                </div>
              </div>
            </div>
            <Note>
              Inter appears only as UI and metadata. Using a neutral UI sans as the
              display face is, per §9.0, the single biggest tell that a design was
              generated.
            </Note>
          </Section>

          <Section title="Colour" lead="Every token, and what it is for">
            <div className="grid max-w-4xl grid-cols-4 gap-3">
              {[
                ["--void", "Player letterbox"], ["--base", "App ground"],
                ["--surface", "Cards, panels"], ["--raised", "Hover"],
                ["--overlay", "Menus"], ["--line-subtle", "Hairlines"],
                ["--line", "Panel edges"], ["--line-strong", "Card outlines"],
                ["--line-interactive", "Control borders"], ["--ink", "Primary text"],
                ["--ink-muted", "Body copy"], ["--ink-faint", "Metadata"],
                ["--oxblood", "Play, intent"], ["--oxblood-bright", "Focus, progress"],
                ["--oxblood-text", "Spine numbers"], ["--oxblood-deep", "Pressed"],
                ["--success", "Available"], ["--warning", "Quota"],
                ["--danger", "Destructive"], ["--info", "Informational"],
              ].map(([token, use]) => (
                <div key={token} className="rounded-sm border border-line-subtle">
                  <div className="h-12 rounded-t-sm" style={{ background: `var(${token})` }} />
                  <div className="p-2.5">
                    <div className="font-mono text-[10px] text-ink">{token}</div>
                    <div className="mt-0.5 text-[10px] text-ink-faint">{use}</div>
                  </div>
                </div>
              ))}
            </div>
            <Note>
              Checked by <code>tools/contrast/audit.py</code>, which fails CI. It found
              that <code>--ink-faint</code> was at 2.7:1 — exactly the pair §9.1 warned
              about — and that fixing it alone would have collapsed it into
              <code> --ink-muted</code>, so both moved.
            </Note>
          </Section>
        </>
      )}

      {tab === "perf" && (
        <Section title="500 cards" lead="Phase 2 exit criterion: 60fps, no dropped frames">
          <Note>
            The rail is virtualised: only the visible window plus overscan is mounted,
            so this is roughly a dozen cards in the DOM rather than 500. Tab into it
            once, then use the arrow keys: it is one tab stop, not five hundred.
          </Note>
          <div className="-mx-10">
            <Rail
              label="Five hundred films"
              why="Virtualisation test"
              items={MANY}
              keyOf={(f) => f.id}
              itemWidth={194}
              leadWidth={254}
              render={(f, i) => <PosterCard film={f} size={i === 0 ? "lead" : "md"} />}
            />
          </div>
        </Section>
      )}

      <Dialog
        open={dialog}
        onClose={() => setDialog(false)}
        title="Delete download"
        footer={
          <>
            <Button variant="secondary" onClick={() => setDialog(false)}>Keep</Button>
            <Button variant="danger" onClick={() => setDialog(false)}>Delete 4.2 GB</Button>
          </>
        }
      >
        This removes the downloaded file from disk. The film stays in your watchlist
        and can be downloaded again.
      </Dialog>

      <CommandPalette commands={commands} open={palette} onOpenChange={setPalette} />
      <ToastStack toasts={toasts} onDismiss={(id) => setToasts((t) => t.filter((x) => x.id !== id))} />
    </div>
  );
}

function Section({ title, lead, children }: { title: string; lead?: string; children: React.ReactNode }) {
  return (
    <section className="border-b border-line-subtle px-10 py-9">
      <h2 className="font-display text-[20px] font-extrabold uppercase tracking-[-0.035em] text-ink">
        {title}
      </h2>
      {lead && <p className="label mt-1.5 mb-6">{lead}</p>}
      <div className="space-y-5">{children}</div>
    </section>
  );
}

const Row = ({ children }: { children: React.ReactNode }) => (
  <div className="flex flex-wrap items-end gap-3">{children}</div>
);

const Note = ({ children }: { children: React.ReactNode }) => (
  <p className="max-w-2xl border-l border-line pl-4 text-[12px] leading-relaxed text-ink-faint">
    {children}
  </p>
);

const PlayGlyph = () => (
  <svg width="18" height="18" viewBox="0 0 20 20" fill="currentColor"><path d="M6 4l10 6-10 6V4z" /></svg>
);
const MuteGlyph = () => (
  <svg width="18" height="18" viewBox="0 0 20 20" fill="none" stroke="currentColor" strokeWidth="1.4">
    <path d="M3 10h4l4-4v8l-4-4H3z" /><path d="M14 7l4 6M18 7l-4 6" />
  </svg>
);
const FullscreenGlyph = () => (
  <svg width="18" height="18" viewBox="0 0 20 20" fill="none" stroke="currentColor" strokeWidth="1.4">
    <path d="M3 7V3h4M17 7V3h-4M3 13v4h4M17 13v4h-4" />
  </svg>
);
