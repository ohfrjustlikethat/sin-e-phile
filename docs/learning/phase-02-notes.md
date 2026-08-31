# Phase 2 — Learning notes

**Design system and visual language.** Complete, pending the understanding gate.

Lean profile (ADR-0016): four sections — what was new, the decisions, the gotchas,
and the self-check questions. The prose that can be reconstructed from the code
later is not written now.

---

## 1. What was new

### CSS custom properties as the single source of colour

Every colour lives in `src/styles/tokens.css` as a custom property, and
`global.css` maps each one into Tailwind's theme with `@theme inline`. Tailwind's
own default palette is deliberately **not** re-exported, so `bg-slate-800` simply
does not resolve.

That last part is the point. A design system that *asks* you not to hard-code
colours is a convention; one where the wrong colour fails to compile is a
constraint. The same instinct as `#![forbid(unsafe_code)]` in Rust: make the thing
you do not want unrepresentable rather than discouraged.

### Virtualisation

A rail of 500 cards is thousands of DOM nodes and 500 decoded images. Virtualising
means mounting only the visible window plus a small overscan — about 14 elements
instead of 500 — and using a spacer to hold the scroll position where it would
have been.

The maths is deliberately simple because the items are a known width:

```ts
offsetOf(i) = i === 0 ? 0 : lead + (i - 1) * itemWidth
```

A general variable-width virtualiser would be several hundred lines and this
project does not need one. §2.2 says prefer the clear implementation.

### Roving tabindex

New concept, and the more useful half of the phase. A list of 500 items should not
be 500 Tab stops — the standard pattern is that the *whole list* is one Tab stop,
and arrow keys move focus within it. Exactly one descendant has `tabIndex={0}` at a
time; the rest are `tabIndex={-1}`, which means "focusable by script, skipped by
Tab".

For a virtualised list this stops being a nicety and becomes mandatory. See §3.

### Driving a real browser as a test

`tools/uiaudit/run.mjs` starts the dev server, starts headless Chrome, and talks to
it over the DevTools Protocol — the same protocol the DevTools panel uses. It can
dispatch real key events, read computed styles, and emulate `prefers-reduced-motion`.

Node 24 has a built-in WebSocket client, so this is about 300 lines with **no
dependencies** — no Puppeteer to pin, nothing to keep current. ADR-0012's reasoning
about Python tooling, applied to JavaScript.

---

## 2. Decisions

- **ADR-0024**: Take B, with Take A's 74vh hero. The author chose from three real
  mockups with real artwork before any component code was written.
- The **index column** (96px, with a hairline down it) is a layout invariant, not a
  per-screen choice. Breaking it on one screen breaks the design.
- **Two radii only** (`0` and `2px`). A radius scale is one of the generated-look
  tells §9.0 bans.
- `--oxblood` is a **surface** colour and does not pass AA as small text; text that
  must be oxblood uses `--oxblood-text`. The contrast audit enforces the split, so
  it is not left to memory.
- The **artwork-free PosterCard is a designed state**, not a fallback (ADR-0013).

---

## 3. Gotchas — all four were silent

Not one of these produced an error message. Every one was found by measuring
something rather than by looking at the screen, which is the actual lesson of the
phase.

### `min-width: auto` turned virtualisation off entirely

A grid item defaults to `min-width: auto`, which means it **refuses to shrink below
its content's intrinsic width**. The virtualisation track was 97,060px wide, so the
`1fr` column became 97,060px wide. Then:

- the page scrolled sideways;
- the `ResizeObserver` reported a viewport 97,060px wide;
- `visibleCount` came out as 500;
- all 500 cards mounted.

The virtualisation code was correct and ran happily, doing nothing. The fix is
`min-w-0` on the grid child, and it is one of the most common layout traps in CSS —
the same applies to flex items.

**Why it matters beyond CSS:** the component had no way to notice. It asked the
browser how wide it was, got an answer, and believed it. A measurement that can
only be validated from outside the component needs a test from outside the
component.

### Absolutely-positioned children contribute no height

The track was `position: relative` with `height: 100%` and absolutely-positioned
items. `height: 100%` of an auto-height parent resolves to auto, absolute children
add nothing, so the rail rendered at **zero height** — invisible, no error.

Fixed by putting the items back in normal flow (a flex row) with a leading spacer
of `offsetOf(first)`. The cards then give the row its height, and the whole class
of bug disappears rather than being patched.

### Virtualisation silently breaks Tab navigation

This is the one worth remembering. Only the *mounted* window exists in the DOM, so:

- Tab walked the ~13 mounted cards,
- then left the rail entirely,
- and **487 of 500 cards were unreachable by keyboard.**

Native focus-scrolling cannot help, because the next card is not merely off-screen —
it does not exist. Increasing the overscan just moves the wall.

The fix is the roving tabindex: the rail is one Tab stop, arrow keys move an active
index through all 500, and each move scrolls the rail so the target mounts
immediately before it is focused. Measured afterwards: `End` reaches card 499 with
10 cards mounted.

Worth noting that the accessible fix is also the *better* interaction. 500 Tab stops
would have been miserable even if they had worked.

### `prefers-reduced-motion` was adding motion

The global rule set `transition-duration: 100ms !important` on `*`. But CSS
defaults `transition-property` to **`all`** — so that one line made *every*
animatable property on *every* element transition for 100ms. Colours crossfaded and
layout changes animated, under the media query whose entire purpose is to remove
motion.

Fix: pin `transition-property: opacity !important` as well. Measured: 38
non-opacity transitions normally, 0 under reduce.

### And a fifth, from the tooling

The audit harness kept timing out with an empty page and no error. Cause: Chrome's
profile was being written **inside the project**, Vite watches the project tree,
and Chrome keeps `Default/Network/Cookies` locked — the watcher hit `EBUSY` and
Vite exited after serving exactly one request.

The harness now writes the profile to the OS temp dir, logs the dev server's output
instead of discarding it, and captures `Log.entryAdded` as well as
`Runtime.exceptionThrown` — a module that fails to *fetch* never throws, so without
that the page just stays blank. **Discarding a subprocess's stderr cost about an
hour.**

---

## 4. Self-check questions

Per `SPEC.md` §10.10 — answer these out loud, in your own words. If one is hard,
say so and the note gets rewritten rather than the box getting ticked.

1. **The rail mounts ~14 of 500 cards. Explain to someone who has never seen this
   code how the remaining 486 still take up the right amount of scroll space —
   and what the leading spacer is for.**

2. **`min-w-0` is one utility class. Explain what went wrong without it, and why
   the bug made virtualisation stop working rather than just making the layout
   look odd.**

3. **Why does a virtualised list break Tab navigation, and why can't more overscan
   fix it? What does the roving tabindex do instead — what is `tabIndex={-1}` for?**

4. **`transition-duration: 100ms` on `*` under `prefers-reduced-motion` added
   motion instead of removing it. Explain why, and what `transition-property`
   has to do with it.**

5. **All four of this phase's bugs were invisible on screen. What kind of bug does
   a screenshot catch, what kind does it miss, and what does `tools/uiaudit`
   check that your eyes cannot?**
