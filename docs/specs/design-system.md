# Design system

Implements `SPEC.md` §9. Direction fixed by **ADR-0023** (visual direction) and
**ADR-0024** (Take B, with Take A's 74vh hero).

This describes what exists and, where it matters, why it is the way it is. It is
not a style guide to admire — every claim in it is checked by something that runs.

---

## 1. What enforces this document

| Check | Command | Enforces |
|---|---|---|
| Contrast audit | `python tools/contrast/audit.py` | §9.1, WCAG AA on every enforced token pair |
| UI audit | `node tools/uiaudit/run.mjs` | 60fps rail, focus rings, keyboard reach, reduced motion |
| Lint / types | `npm run lint` · `npx tsc --noEmit` | TS strict, no unused, no stray console |
| Gallery | `npm run tauri dev` → `#design` | Every primitive in every state, by eye |

Both audits run in CI on every push. Both were verified to **fail** on a
deliberately reintroduced regression before being trusted — see §7.

---

## 2. Tokens

All tokens live in `src/styles/tokens.css` and are exposed to Tailwind through
`@theme inline` in `src/styles/global.css`. **Nothing hard-codes a colour.**
Tailwind's own default palette is deliberately not re-exported, so `bg-slate-800`
does not resolve and cannot creep in.

### Surfaces

Warm, not neutral: every grey satisfies R ≥ G ≥ B. The steps are non-linear —
`--base` → `--surface` is a small lift, `--surface` → `--raised` a larger one —
because evenly-spaced greys are one of the tells §9.0 bans.

```
--void  #000000   --base    #0a0a0a   --surface #141312
--raised #1f1d1b  --overlay #262421
```

### Lines

```
--line-subtle #1c1a18   structure you should not notice
--line        #2c2926   ordinary separation
--line-strong #423e39   hover, emphasis
--line-interactive #6a635c   3.15:1 — the boundary of an interactive control
```

`--line-interactive` exists because WCAG 1.4.11 requires **3:1 for non-text
elements**. A border that only signals "this is a text field" still has to be
visible. The subtler line values are decorative and are recorded by the audit but
not enforced against that threshold.

### Ink

```
--ink       #f5f2ed   13.9:1 on --base
--ink-muted #b9b5b0    7.6:1
--ink-faint #958d84    4.7:1   still AA for body text
```

Three levels, all of which pass. An earlier palette had `--ink-faint` and
`--ink-muted` collapse toward each other, and moving only one of them would have
lost a level of hierarchy — both moved, so there are still three distinct steps
and every one clears 4.5:1.

### Accent

```
--oxblood #8e2b34   --oxblood-hover #a6333e   --oxblood-bright #c4434f
--oxblood-text #cb5863   accent carrying SMALL TEXT
```

`--oxblood` is a *surface* colour. It does not pass AA as small text on a dark
ground, so text that must be oxblood uses `--oxblood-text`. The audit enforces
this distinction; it is not left to memory.

Semantic colours (`--success --warning --danger --info`) are separate, and §9.1
forbids `--danger` and oxblood from appearing adjacent — a destructive confirm
pairs danger with a *secondary* button, never with primary.

### Radius

Exactly two values: `--radius-none` and `--radius-sm: 2px`. §9.0 bans a radius
scale, because uniformly rounded everything is the strongest generated-look tell.

### Type

| Token | Face | Used for |
|---|---|---|
| `--font-display` | Bricolage Grotesque | headings, section labels |
| `--font-serif` | Instrument Serif | **film titles only** |
| `--font-ui` | Inter | body, controls, metadata |
| `--font-mono` | JetBrains Mono | spine numbers, timecodes, measurements |

§9.2 forbids Inter as a display face. Film titles are set in the serif because
that is the single strongest signal that this is a film application and not a
dashboard.

All four are bundled in `public/fonts/` as woff2 with `font-display: block`.
Nothing is fetched from a font CDN — §2.4 forbids anything leaving the machine.

---

## 3. Layout invariants

**The index column.** Every surface is a `96px | 1fr` grid, with a hairline down
the division. That vertical rule is the spine of the design (ADR-0024); breaking
it on one screen breaks the whole thing. `--index-col` is the token.

**The hero is 74vh** (`--hero-height`), Take A's proportion on Take B's system —
the author's choice when picking between the mockups.

**Cards are not uniform.** §9.4 requires it and §9.0 explains why: a grid of
identical tiles is the generated look. Three card shapes exist on purpose —
`PosterCard` 3:4, `EpisodeCard` 16:9, `ChannelCard` 1:1 — because a poster, a
frame and a logo are different objects. Rails lead with a larger card
(`size="lead"`), which the virtualiser accounts for explicitly (§5).

---

## 4. Components

`src/design-system/`

**Primitives** — Button, IconButton, Input, Select, Toggle, Slider, Tabs, Tooltip,
Popover, Dialog, Toast, Skeleton, Badge, ProgressBar, Rating.

**Media** — PosterCard, EpisodeCard, ChannelCard, Rail, HeroBanner, EmptyState.

Plus `CommandPalette`. Everything is exported from the `index.ts` barrel; features
import from `@/design-system`, never from a file path inside it.

### PosterCard has two states, and the second is not a fallback

ADR-0013 made TMDB optional, so a large fraction of a real library will have no
artwork at all. The artwork-free card is therefore *designed*: spine number, title
in the editorial serif, credits in wide-tracked caps, an oxblood rule down the
left edge. It reads as **catalogued**, not as missing something. For a film
application it is arguably the better card, because it selects on knowledge rather
than on poster recognition.

The caption below the frame repeats nothing. The typographic card already carries
the title and credits inside it, so printing them again underneath said the same
thing twice — obvious the moment the gallery was rendered with and without
artwork side by side, and invisible before that.

Artwork cards get a top legibility scrim. The spine number sits over an arbitrary
frame, and film stills are frequently brightest exactly where the number goes.
§9.0 bans shadows used for *depth*; a scrim for *legibility* is what the hero
already does.

---

## 5. Rail — virtualisation and keyboard

`src/design-system/media/Rail.tsx`. The most load-bearing component in the system
and the one with the most non-obvious constraints.

**Virtualisation.** Items are a known width, so the visible range is arithmetic
rather than measurement. `offsetOf(i) = i === 0 ? 0 : lead + (i - 1) * itemWidth`
— the wider lead card is the one exception the maths accounts for, rather than
becoming a general variable-width virtualiser this project does not need. Scroll
is rAF-throttled: a handler that sets state on every scroll event causes exactly
the dropped frames the component exists to avoid.

**Two layout constraints that are load-bearing, and both failed silently:**

- `min-w-0` on the grid content column. A grid item defaults to
  `min-width: auto` and refuses to shrink below its content, so the 97,060px
  virtualisation track expanded its `1fr` column to 97,060px. The page scrolled
  sideways, the ResizeObserver reported a viewport wider than the content, and
  **all 500 cards mounted** — virtualisation did nothing at all, and nothing
  looked wrong.
- The track is a flex row in **normal flow** with a leading spacer of
  `offsetOf(first)`, not absolutely-positioned items. Absolute children
  contribute no height, so the rail rendered at zero height and disappeared.

**Keyboard: one tab stop, not five hundred.** Virtualisation breaks plain Tab
navigation and does so invisibly — only the mounted window exists in the DOM, so
Tab walked the ~13 mounted cards and then left the rail, leaving 487 unreachable.
The rail is therefore a single tab stop with a roving tabindex: arrow keys move
focus through all 500, scrolling the rail so each card mounts just before it is
focused; Home and End jump to the ends. This is the standard pattern for list-like
widgets and is better for keyboard users than 500 stops would have been even if
virtualisation had not forced it.

The roving tabindex is applied imperatively rather than threaded through a prop,
because `render` returns arbitrary caller-owned markup — the rail should not
require every card component in the application to know it is being virtualised.

---

## 6. Motion and accessibility

Durations are tokens (`--dur-fast`, `--dur-standard`, `--dur-slow`) with
`--ease-standard`. Hover previews wait `--dwell-preview` (400ms) before beginning,
so moving the pointer across a rail does not trigger a cascade.

**`prefers-reduced-motion`** is handled globally in `tokens.css` rather than
per-component, so it cannot be forgotten by a component author:

```css
@media (prefers-reduced-motion: reduce) {
  *, *::before, *::after {
    animation-duration: 0.01ms !important;
    animation-iteration-count: 1 !important;
    transition-property: opacity !important;
    transition-duration: 100ms !important;
    scroll-behavior: auto !important;
  }
  [data-motion="scale"], [data-motion="parallax"] { transform: none !important; }
}
```

`transition-property: opacity` is the load-bearing line and it was missing. CSS
defaults `transition-property` to `all`, so an earlier version that set only a
duration on `*` made **every** animatable property on **every** element transition
— reduced motion was *adding* motion that had not been there. Measured: 38
non-opacity transitions normally, 0 under reduce.

**Focus** is a global `:focus-visible` ring, verified rather than assumed: a
scripted Tab walk across the gallery records the computed outline at every stop
and fails if any stop has no visible ring or lands off-screen.

---

## 7. Measured, on this machine

`node tools/uiaudit/run.mjs`, design gallery, headless Chrome:

```
rail       14/500 mounted · median 16.7ms · p95 16.7ms · worst 16.8ms · 0 dropped
keyboard   rail is 1 tab stop; End reaches card 499 of 500
focus      45 stops, 18 distinct, 0 without a ring
motion     38 non-opacity transitions normally → 0 under reduce
contrast   29 enforced pairs pass WCAG AA (3 decorative recorded, not enforced)
```

16.7ms is the 60fps vsync cadence; the budget fails at a worst frame over 34ms
(one doubled frame) or any frame over 33.3ms.

**Both audits were verified to fail before being trusted.** Reverting the `min-w-0`
fix produces `rail is not scrollable — its track is not width-constrained`, exit 1.
Removing `transition-property: opacity` produces `180 non-opacity transitions
survive prefers-reduced-motion`, exit 1. A check that has never been seen to fail
is not evidence of anything.

---

## 8. Rules for adding to this

1. New colour → a token, and a row in `tools/contrast/pairs.txt` with its class.
   No exceptions; the audit reads the real token values out of the CSS.
2. New primitive → into the design gallery **in every state**, including the
   states nobody wants to look at: loading, disabled, error, empty, too-long text.
   A component that only looks right in its happy state is not finished.
3. Never re-introduce a third radius, a shadow used for depth, a uniform card
   grid, or Inter as a display face. §9.0 lists the tells and why they are banned.
4. Anything that animates must be checked under `prefers-reduced-motion`.
5. Anything focusable must show a ring — and the UI audit must still pass, which
   means it must be reachable by keyboard, not merely styled.
