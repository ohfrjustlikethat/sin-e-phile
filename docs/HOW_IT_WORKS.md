# How sin-e-phile works

A plain-English explanation of the whole system, assuming no prior knowledge.
`SPEC.md` §13.1 makes this a per-phase deliverable: it is updated every phase, and
by the end it should be readable start to finish by someone who has never seen the
project.

**Status: Phase 0.** Only the infrastructure exists. The sections below describe
what each subsystem *will* do and are marked `⬜ not built yet` until the phase that
builds them lands. That marking is deliberate — this file is not allowed to describe
things that do not exist as though they do.

Terms in **bold** are defined in [`GLOSSARY.md`](GLOSSARY.md).

---

## What the app does, in one paragraph

You search for a film, or browse recommendations. You press play. It plays — within
seconds, at the best quality your machine and connection can manage, in the language
you chose, with subtitles already in sync. You are never shown a list of torrents, a
file picker, or a resolution debate. The interesting part is everything that happens
between "press play" and "picture appears", and almost all of it is invisible.

---

## The shape of the whole thing

Two processes, talking over a typed boundary.

```
   ┌──────────────────────────────────────────────┐
   │  What you see: React + TypeScript            │
   │  running in WebView2 (the system browser)    │
   └───────────────────┬──────────────────────────┘
                       │  Tauri IPC
                       │  typed commands + events
   ┌───────────────────┴──────────────────────────┐
   │  What does the work: a Rust program          │
   │                                              │
   │  catalogue · search · taste · discovery      │
   │  source resolver · torrent · playback        │
   │  subtitles · local library · persistence     │
   └──────────────────────────────────────────────┘
```

The **frontend** draws things. The **backend** does everything else: it holds the
database, runs the search index, talks to peers, drives the video player, and
decides what to recommend.

They communicate over **IPC** — the frontend calls named commands and receives
events back. Phase 1 *generates* the TypeScript types from the Rust definitions, so
if a Rust function's signature changes, the frontend fails to compile rather than
failing at runtime in front of a user.

**Why this split?** Rust is fast, safe with threads, and the right language for a
torrent engine and a search index. React is the right tool for an animated,
poster-heavy interface. Tauri lets each do what it is good at, using the operating
system's existing browser rather than bundling a second one — which is what keeps
the whole app around 10 MB instead of 150 MB (ADR-0002).

---

## What happens when you press play

This is the path worth understanding, because nearly every subsystem is on it.
`⬜ not built yet` — this is Phase 8's vertical slice.

**1. You click a film.** The frontend asks the backend to resolve it, passing the
internal media ID plus your language and quality preferences.

**2. The Source Resolver asks every backend at once.** *(Phase 6)* A backend is
anything that can supply a playable stream: your local filesystem, the Internet
Archive, a source you configured, a direct HTTP URL. They are queried concurrently,
each with its own timeout, so one slow backend never delays the others. Each returns
zero or more **candidates**.

The important design idea: a local file, a torrent, and an HTTP stream are all just
candidates. Nothing downstream knows the difference. That is why your own copy of a
film can appear beside streaming options with no special-casing anywhere
(`SPEC.md` §6.1).

**3. Everything is scored, once, by one function.** *(Phase 9)* Swarm health,
plausibility of size against claimed quality (a 700 MB "4K" file is a lie), codec
compatibility with your actual hardware, which audio languages are present, whether
subtitles come with it, and whether you already have it locally — a local file
always wins. The best candidate is chosen. You see none of this, unless you open the
expert panel, which shows the full ranked list and every score component.

**4. If it is a torrent, the streaming scheduler takes over.** *(Phase 7)* This is
the hardest part of the project and it gets its own section below.

**5. The stream is served locally over HTTP.** *(Phase 7)* Whatever the source, the
backend exposes it at a `localhost` URL supporting **range requests**, so the player
can seek. The player therefore does not know or care whether it is playing a local
file or a torrent that is still downloading.

**6. Subtitles are found and aligned.** *(Phase 10)* In strict priority order:
tracks embedded in the file (perfectly synced already, and the most common case),
then sidecar files, then a hash match against OpenSubtitles, then other providers.
Whatever is found gets checked for sync, and corrected if needed.

**7. libmpv plays it, with our UI drawn over the top.** *(Phase 8)* mpv handles
decoding, hardware acceleration and rendering. Every control you see is ours; mpv's
own on-screen display never appears.

---

## The subsystems

### The catalogue — knowing what films exist `⬜ Phase 4`

The app ships knowing about hundreds of thousands of films and shows, and works with
no API key and no network (ADR-0013). This comes from free bulk datasets — IMDb for
titles, years, cast, crew and ratings; MovieLens for the ratings patterns the
recommender needs; AniList for anime, which has its own numbering problems.

TMDB is layered on top for artwork and richer detail, and is **optional**. Without a
key you get a fully working app with typographic cards instead of posters — which,
for a film-focused app, is arguably a better look anyway.

**The analogy:** a library that owns a complete card catalogue but not the book
covers. It can still tell you everything about every book.

### Search — finding things by name *and* by meaning `⬜ Phase 5`

Two searches run at once over the same query.

The **keyword** search (**FTS5**, **BM25**) is good at exactness. Type `Heat` and it
finds *Heat*.

The **meaning** search turns your query into an **embedding** — a point in a space
where similar meanings sit close together — and finds catalogue entries whose
embeddings are nearby. This is what makes *"slow films about loneliness"* work, since
no film's description necessarily contains those words.

The two ranked lists are merged by **reciprocal rank fusion**, with a short-circuit
guaranteeing an exact title match always comes first. Both run locally. The whole
path, embedding included, must finish in under 80 ms at the 95th percentile — on a
weak machine.

**The analogy:** one librarian who remembers exact titles, and one who understands
what you are in the mood for. You ask both, then combine their answers.

### The torrent engine — why streaming is hard `⬜ Phase 7`

BitTorrent splits a file into **pieces** and, by default, fetches the **rarest**
piece first. That is the right strategy for keeping a swarm healthy, and it is
exactly wrong for playback, which needs the *next* piece, now.

Naively switching to sequential order damages the swarm and still stalls, because
one slow peer holding the next piece blocks everything.

The scheduler is deadline-driven: a **priority window** of pieces just ahead of the
playhead is fetched urgently, everything else is fetched rarest-first in the
background, and when you seek somewhere unbuffered the window moves and
re-prioritises within two seconds. Target: **playing within eight seconds** on a
healthy swarm.

**The analogy:** a restaurant kitchen. Rarest-first is cooking whatever ingredient is
scarcest. Sequential is cooking strictly in order of tickets. What actually works is
cooking the dish being served *right now* first, while prepping the rest in whatever
order keeps the kitchen efficient.

### Subtitles — in sync on the first frame `⬜ Phase 10`

Most tools give you an offset slider. That fails on the common case, because a
subtitle file made for a 25 fps master drifts **progressively** against a 23.976 fps
release. The error grows over the runtime, and no constant offset can fix a drift.

So the aligner solves for two things: a constant **offset** (an addition) and a
**framerate scale** (a multiplication). It extracts a speech/silence pattern from the
audio using **VAD**, builds the same shape from the subtitle timings, and slides one
against the other to find the best fit — **cross-correlation**.

It also scores its own confidence and **refuses to apply a low-confidence result**.
Making subtitles worse is worse than leaving them alone.

**The analogy:** two people clapping along to the same song, recorded separately. You
can work out both how late one started *and* whether they were clapping slightly
fast, by sliding the recordings against each other until the claps line up.

### The local library — identifying your files by looking at them `⬜ Phase 12`

Point it at a folder and it works out what everything is. Filenames alone are not
enough — they are inconsistent, and ambiguous cases exist (`The.Batman` could be 1989
or 2022).

So it combines several weak signals: what the filename parser extracted, the actual
duration from the file, its resolution, which audio languages are present, embedded
container metadata, and the folder's context. Several weak signals agreeing beats one
strong signal guessing.

Only genuinely ambiguous cases reach a **review queue**, resolved with one click on a
side-by-side poster comparison. Being **wrong while confident** is the worst failure
here, so it is measured separately and held under 1%.

### Taste and discovery — the reason to use this at all `⬜ Phases 15–17`

The taste model does not average you into one vector, because someone who loves both
Tarkovsky and slasher films is not described by their midpoint. Positive signals are
clustered into 3–8 taste **modes**, each with its own vector, weight and recency. It
also learns *negative* preferences from what you abandon, and where you abandoned it:
bailing at 5 minutes and at 80 minutes mean different things.

Recommendations combine content similarity against each mode with **collaborative
filtering** — "people who loved this also loved" — precomputed offline, so there is
no server and no other users involved.

On top sits a **contextual bandit** choosing between *strategies*: safe-familiar,
adjacent-stretch, cross-mode bridge, blind-spot, canon-gap, deep-cut. It learns which
kind of suggestion you actually act on. There is a hard **exploration floor**, so
even a maximally conservative user still gets genuine discovery — a filter bubble is
an optimisation failure, not an inevitability.

**The analogy:** a friend who knows your taste well enough to predict what you will
like, but who is also willing to say "I know this is not your usual thing, but trust
me." And who notices when you did not like it.

---

## The parts that exist today

### The posture guard ✅ `Phase 0`

The app ships **no content sources** — that is architectural, not a disclaimer
(ADR-0006). But a rule that must hold across 28 phases will eventually be broken by
accident, so it is enforced by a program rather than by discipline.

`tools/guard/` runs before every commit and on every push, checking the working tree
*and every version of every file that has ever existed in the history*. It looks for
structural shapes (magnet links, bare infohashes, tracker URLs), for known-bad names
via a **hashed** list — so that forbidden names never appear in plaintext even in
the list itself — and for any source domain not explicitly declared and justified.

It is **verified rather than trusted**: 30 self-tests covering both what must be
caught and what must not, using domains that RFC 2606 guarantees can never be real,
so the guard can be proven to work without a real forbidden string ever entering the
repository.

Full detail: [`tools/guard/README.md`](../tools/guard/README.md).

### `doctor` ✅ `Phase 0`

Checks every prerequisite and says precisely what is missing and how to install it.
It also distinguishes "not installed" from "installed, but this terminal has a stale
`PATH`" — a genuine Windows behaviour that otherwise sends you reinstalling something
you already have. It also wires up the git hooks, which do nothing until it runs.

### The state files ✅ `Phase 0`

`PROJECT_STATE.json` is the machine-readable resume point. Its phase table is
**generated from `SPEC.md`** rather than typed, so it cannot drift from the
specification, and it is validated against a schema that **refuses to let a criterion
be marked done without evidence**. `PROGRESS.md` is generated from it. `SESSION_LOG.md`
is the append-only history.

The reason for all this machinery: this project runs for months across many sessions
with long gaps. Whether the state file can be trusted is the difference between
resuming and re-deriving.

---

## Where to look in the code

| Want to understand | Look at |
|---|---|
| How the legal posture is enforced | `tools/guard/` + `docs/adr/0009`, `0010`, `0011` |
| Whether your machine can build it | `tools/doctor/doctor.py` |
| Why the state file cannot drift from the spec | `tools/state/build_state.py` |
| Why evidence-free claims are impossible | `docs/schemas/project-state.schema.json` |
| Why any technology was chosen | `docs/adr/0002`–`0008` |
| What could go wrong, and the pre-decided response | `docs/RISKS.md` |
| What is deliberately undecided | `docs/DECISIONS_PENDING.md` |
