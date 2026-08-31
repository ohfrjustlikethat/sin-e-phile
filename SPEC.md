# sin-e-phile — Master Build Specification & Claude Code Operating Prompt

> **How to use this document.** Save this file to the repository root as `SPEC.md` before anything else exists. In every Claude Code session, this document is the constitution: it outranks convenience, it outranks "we could just", and it outranks anything you remember from a previous session. Start session one by pasting the "Session Zero Instruction" at the bottom of Section 10.

---

## 1. Mission

Build **sin-e-phile**: a Windows desktop application that unifies three things nobody has successfully unified — streaming, torrenting, and your own local film library — behind a single, beautiful, genuinely intelligent interface built for people who take film seriously.

It is a media *engine*, not a media service. It ships with no catalogue of its own. What it ships with is the machinery: a semantic search engine, a taste model that learns and deliberately pushes back against itself, a torrent engine that streams, a player that rivals commercial ones, and a local-library matcher that identifies your files by looking at them rather than trusting their names.

This is a portfolio project. Its author is a student, new to both Rust and React, who intends to show this repository to employers and be able to explain every part of it in an interview. **That constraint is as binding as any technical requirement in this document.** Code that works but that the author cannot explain is a failure condition.

---

## 2. Non-Negotiable Constraints

These are not preferences. Violating any of them is a defect.

### 2.1 Legal posture — read this before writing a line of code

The application **ships with zero content sources**. No indexer URLs, no scraper implementations for specific torrent sites, no bundled addon list, no seed data pointing at infringing sources — not in the code, not in the config, not in test fixtures, not in documentation examples, and **not in git history**.

What the repository contains instead is:

- A **source addon protocol**: a documented, versioned interface that any source backend can implement.
- A **declarative addon manifest format** (TOML/JSON, no executable code) so a source can be described as data rather than shipped as an implementation.
- A **community addon catalogue browser** that fetches a catalogue from a URL the *user* supplies. No default URL ships.
- Working reference backends against **unambiguously legal sources only**: the local filesystem, the Internet Archive's public-domain collection, and user-supplied M3U/direct HTTP.

This is precisely Stremio's architecture, and it is why Stremio still exists while Popcorn Time's forks were removed from GitHub. It costs the user nothing — they configure their own sources in thirty seconds — and it is the difference between a repository that impresses a hiring manager and one that gets taken down.

**When any phase is tempted to add a "convenient default", the answer is no.** If a feature genuinely cannot be demonstrated without a source, demonstrate it against the local-filesystem backend or the Internet Archive backend.

Documentation must never include a specific indexer name, URL, or scraping example. Write `https://example.com/addon/manifest.json`. Examples and test vectors use RFC 2606 reserved domains — `.invalid`, `.test`, `.example`, `example.com` — which no registry can ever issue (ADR-0009).

**Test fixtures containing real-world filenames** (§12.2, Phase 12) follow a redaction policy (ADR-0011). **Release-group tokens are kept verbatim** — a release group is not an indexer, and group identity is a genuine quality signal the Phase 9 scorer uses. **Site tags inside filenames are replaced at authoring time** with synthetic `[SITE01]`, `[SITE02]` tokens, allocated per distinct *shape* so the corpus keeps its variety of tag position, bracket style and casing. The mapping from synthetic token to real site is never recorded anywhere, so there is nothing to leak. The parser still learns the lesson that matters, which is positional and structural — strip a bracketed distribution tag in leading or trailing position — and is indifferent to the string inside the brackets. `fixtures/filenames/README.md` documents that redaction occurred, so nobody mistakes a redacted corpus for a synthetic one.

### 2.2 Understanding over velocity

The author is new to Rust and to React. Therefore:

- Prefer the clear implementation over the clever one. If an abstraction saves 30 lines but requires understanding four Rust traits to follow, don't build it yet.
- Every phase produces a **learning note** (Section 13). This is a deliverable, not a nicety.
- When introducing an unfamiliar concept — lifetimes, `Arc<Mutex<>>`, async cancellation, React's rendering model, Tauri's IPC boundary — explain it *in the learning note* with reference to the actual code just written, not in the abstract.
- Never write more than roughly 400 lines of new logic without recording what it does and why — **as bullets in the phase learning note, not as a mid-session pause** (ADR-0016). The explanation is still mandatory; interrupting the session for it is not.
- If a phase requires a genuinely advanced technique, say so explicitly and explain the simpler alternative that was rejected and why.

### 2.3 Performance and the low-end machine

The app must be excellent on a weak Windows PC. Not "degraded but functional" — **excellent**. The optional extras (Section 8) may disappear on low-end hardware; the core promise may not.

Hard budgets, enforced by the performance phase and checked in CI where feasible:

| Metric | Budget |
|---|---|
| Cold start to interactive home screen | < 2.0 s (Tier 2), < 4.0 s (Tier 0) |
| Search keystroke → results rendered | < 80 ms at p95, **including query embedding** (§8, ADR-0015) |
| Press play → first frame (cached/local) | < 500 ms |
| Press play → first frame (healthy swarm) | < 8 s |
| Idle RAM, app open, nothing playing | < 250 MB (Tier 0) |
| Home screen scroll | 60 fps sustained, no dropped frames |
| Installed size | < 120 MB excluding optional models and downloaded artefacts (§2.7, ADR-0014) |

**Every budget in this table is a Tier 0 number unless the row says otherwise, and Tier 0 is what governs.** Individual phases may state tighter targets for the dev machine — Phase 1's "< 2 s to interactive" and "idle RAM < 200 MB" are **Tier 2 dev-machine** numbers, not replacements for the Tier 0 budgets here. Where a phase states a performance target, it must name the tier it applies to. Both sets are tracked in `docs/PERFORMANCE.md`.

### 2.4 Windows only

Do not add cross-platform abstractions "for later". Use Windows APIs directly where they give a better result. Keep platform-specific code in clearly named modules so a future port is *possible*, but do not pay for portability now.

### 2.5 Portable by default

All application data — database, profiles, taste models, metadata cache, downloaded media index, logs — lives in a `data/` directory **next to the executable**. The whole app is a folder you can move to a USB stick or another machine and it keeps working. Provide an installed mode that uses `%APPDATA%` as an opt-in, not the default.

### 2.6 Zero-cost operation

The application must be fully functional with no paid services. Every external dependency must have a free tier sufficient for personal use, or a free fallback. Where a paid service (a debrid provider) improves things, it is a strictly optional backend behind the same interface, and its absence is never a degraded experience — it is simply the default experience.

### 2.7 No telemetry

Nothing leaves the machine except explicit user-initiated requests to APIs they configured. No analytics, no crash reporting to a server, no "anonymous usage statistics". Crash logs are written locally. State this in the README as a feature.

**Static asset downloads are permitted and are not a violation** (ADR-0014). Fetching the ONNX model, the IMDb and MovieLens datasets, or a precomputed embedding artefact published as a versioned GitHub Release asset is not "a server component" in the sense §5 forbids: there is nothing operated, nothing paid for, and nothing whose downtime degrades a running installation. Every such download must be **consented to, with its size shown, before it happens** — never fetched in the background, never assumed.

### 2.8 This specification is amendable — deliberately, never casually

A 28-phase project will discover that some decision here was wrong. That is expected and healthy. What is not acceptable is silent drift, where the code and the spec diverge until the spec is fiction.

**To change this document:**

1. Write an ADR explaining what was learned and why the original decision no longer holds.
2. Edit `SPEC.md` directly. Do not leave stale text and patch around it.
3. Bump `spec_version` in `PROJECT_STATE.json` (minor for a clarification, major for a reversed decision).
4. Record the amendment in `SESSION_LOG.md` and in a running `## Amendments` list at the bottom of `SPEC.md` — one line each: date, section, what changed, ADR number.
5. Get the author's explicit approval first. Always. The spec is the author's, not Claude Code's.

**Never** implement something contrary to this document and plan to update the spec afterwards. Amend first, then build.

If Claude Code finds itself thinking "the spec says X but Y is obviously better here" — that is exactly the moment to stop and raise it, not to proceed.

---

## 3. Product Definition

### 3.1 Navigation

Five top-level surfaces, permanently in the left rail:

1. **Home** — the taste engine's canvas. Continue Watching first, then adaptive rails.
2. **Films** — browsable film catalogue, with Continue Watching scoped to films.
3. **TV Shows** — browsable series catalogue, Continue Watching scoped to series, with proper season/episode handling. Anime lives here and in Films depending on form, but is always identifiable and filterable as anime.
4. **Watchlist** — saved items, with sub-organisation (lists, priority, "available now" vs "hard to find").
5. **Live Channels** — added in Phase 24; the tab exists from Phase 18 with an honest empty state.

A universal search field is always reachable (`Ctrl+K` opens a command palette that searches media, actions, and settings).

### 3.2 The core loop

Search or browse → pick something → the app decides **how** to get it → it plays, in seconds, at the best quality available, in the language you want, with subtitles already in sync. The user never sees a torrent, never sees a file list, never sees a resolution debate. They see a play button.

### 3.3 The experience promises

- **Everything is one thing.** A film you have locally, a film available via a configured source, and a film you can stream directly are the same object in the UI with different availability badges. Search "The Batman" and see one card that knows it exists in your `D:\Films` folder in 1080p, is available in 4K from a configured source, and has a Hindi dub available.
- **Quality is never withheld.** If 4K exists and your machine and connection can handle it, you get 4K. Selecting quality is a preference, not a paywall. (This is an explicit rebuke of commercial streamers, and belongs in the README.)
- **Discovery is the product.** An app with everything is worthless without a reason to choose. The recommender is not a feature bolted on at the end — it is the thing that makes the catalogue navigable.
- **Setup is three screens.** A non-technical person can install this and be watching something in under two minutes without reading documentation.

---

## 4. Explicit Non-Goals and Anti-Patterns

Derived directly from what the author dislikes in existing tools. Every phase should be checked against this list.

| Anti-pattern | Where it comes from | The rule |
|---|---|---|
| Bland, undesigned UI | Stremio | Every screen gets deliberate visual design. No default-styled components ship. |
| Search that only matches titles | Stremio | Search is semantic from Phase 5. "Slow films about loneliness" must work. |
| No discovery, no algorithm | Stremio | The recommender is Phases 15–17, not an afterthought. |
| Addon configuration hell | Stremio | Source setup is one paste-a-URL field with plain-language help. Never a raw JSON editor as the primary path. |
| Hostile setup, server concepts | Plex | No servers, no libraries-to-configure-before-use, no ports. It's a desktop app. It opens and works. |
| Content buried more than two levels deep | Netflix | Anything in the catalogue is reachable in ≤ 3 interactions from Home. Rails have "see all" that goes somewhere useful. |
| Quality withheld by platform/device | Netflix, Prime | Max available quality always, subject only to hardware and bandwidth. |
| Choosing a torrent from a list | Stremio | Never shown by default. The app decides. Expert panel is opt-in and hidden. |
| Manual subtitle offset fiddling | Everything | Subtitles are in sync on first frame or the pipeline failed. |
| Forced autoplay of trailers on hover | Netflix | Hover preview is muted, delayed, and disableable. Default to a still frame with motion on sustained hover. |
| Modal dialogs for routine actions | Everything | Prefer inline and non-blocking. |

---

## 5. Locked Technology Decisions

Do not revisit these without writing an ADR that explains what changed. They were chosen deliberately.

| Layer | Choice | Rationale |
|---|---|---|
| Shell | **Tauri v2** | ~10 MB binaries, native webview (no bundled Chromium), Rust backend, good Windows integration story. Meets the low-end-machine constraint in a way Electron cannot. |
| Backend | **Rust** (stable, edition 2021+) | Required for torrent engine performance and safe concurrency; strong CV signal. |
| Frontend | **React 18+ / TypeScript (strict)** | Largest ecosystem, most transferable skill, best fit for a rail-heavy media UI. |
| Build (FE) | **Vite** | Fast HMR, minimal config. |
| Styling | **Tailwind CSS + CSS custom properties for design tokens** | Tokens in CSS variables so theming and the design system stay honest; Tailwind for velocity. |
| State (FE) | **Zustand** for client state, **TanStack Query** for backend-derived state | Avoids Redux ceremony; Query handles caching/invalidation correctly, which matters enormously here. |
| Torrent engine | **librqbit** (in-process Rust) | Sequential download and streaming support built in; no external process; full control over piece prioritisation, which is what makes instant playback possible. |
| Player core | **libmpv** (embedded, custom UI drawn over it) | Plays everything, hardware decode (D3D11VA/NVDEC/QSV), frame-accurate seek, first-class ASS/SSA rendering, per-track audio and subtitle delay control. Used by Jellyfin and Stremio for the same reasons. |
| Database | **SQLite** via `sqlx`, WAL mode | Single file, portable, fast, zero setup — the whole database is one file you can copy, which is what makes §2.4's portability promise true. `sqlx` is a mature async Rust driver with a good migration story. **Not** chosen for its compile-time `query!` macros, which this project does not use (ADR-0026): SQLite gives them too little nullability information, so most columns would need hand-written `as "col!"` assertions, and dynamic SQL cannot use them at all. Schema drift is caught instead by a test that exercises every repository method against a freshly migrated database. |
| Full-text search | **SQLite FTS5** (BM25) | Built in, no extra dependency. |
| Vector search | **HNSW** (`hnsw_rs` or `instant-distance`), persisted to disk | Sub-millisecond ANN over the catalogue. |
| Embeddings | **ONNX Runtime** (`ort` crate) running a quantised sentence-transformer (`bge-small-en-v1.5` or `all-MiniLM-L6-v2`, INT8) | ~90 MB, CPU-viable, no API, no cost, works offline. |
| Audio/video processing | **FFmpeg** (bundled binaries or `ffmpeg-next` bindings) | Needed for VAD extraction, thumbnails, chapter detection, media probing. |
| Metadata | **TMDB** (primary), **AniList GraphQL** (anime), **IMDb datasets** + **MovieLens** (offline ingestion), **Fanart.tv** (optional artwork) | Hybrid offline index + live enrichment. |
| Testing | `cargo test` + `proptest` (Rust), Vitest + React Testing Library (frontend), custom eval harnesses (Section 12) | |
| CI | GitHub Actions on `windows-latest` | fmt, clippy `-D warnings`, test, build, eval harnesses. |
| Licence | **GPL-3.0** | Required by libmpv/FFmpeg linkage anyway; correct posture for this project. Source only — no compiled installers published. |

**Explicit non-choices:** no Electron, no bundled qBittorrent, no cloud LLM in the critical path, no external server component of any kind, no Docker.

---

## 6. Architecture

```
┌──────────────────────────────────────────────────────────────┐
│  React / TypeScript frontend (WebView2)                      │
│  Design system · Home/Films/TV/Watchlist/Live · Player chrome │
│  Zustand (UI state) · TanStack Query (server state)          │
└───────────────────────────┬──────────────────────────────────┘
                            │ Tauri IPC (typed commands + events)
┌───────────────────────────┴──────────────────────────────────┐
│  Rust core                                                    │
│                                                               │
│  ┌────────────┐ ┌─────────────┐ ┌──────────────┐             │
│  │ Catalogue  │ │   Search    │ │  Taste &     │             │
│  │ (metadata, │ │ (FTS5 +     │ │  Discovery   │             │
│  │  ingestion)│ │  HNSW +     │ │ (embeddings, │             │
│  │            │ │  fusion)    │ │  CF, bandit) │             │
│  └────────────┘ └─────────────┘ └──────────────┘             │
│                                                               │
│  ┌──────────────────────────────────────────────┐            │
│  │  Source Resolver (backend-agnostic)          │            │
│  │  ┌────────┐ ┌────────┐ ┌────────┐ ┌───────┐ │            │
│  │  │ Local  │ │ Addon  │ │ Debrid │ │ Direct│ │            │
│  │  │ files  │ │ (P2P)  │ │ (opt.) │ │ HTTP  │ │            │
│  │  └────────┘ └────────┘ └────────┘ └───────┘ │            │
│  └──────────────────────────────────────────────┘            │
│                                                               │
│  ┌────────────┐ ┌─────────────┐ ┌──────────────┐             │
│  │  Torrent   │ │  Playback   │ │  Subtitles   │             │
│  │  engine    │→│  (libmpv +  │←│  (embedded/  │             │
│  │ (librqbit) │ │ local HTTP) │ │  hash/align) │             │
│  └────────────┘ └─────────────┘ └──────────────┘             │
│                                                               │
│  ┌────────────┐ ┌─────────────┐ ┌──────────────┐             │
│  │  Local     │ │  Windows    │ │  Persistence │             │
│  │  library   │ │  platform   │ │  (SQLite,    │             │
│  │  (watch,   │ │ (shell, MTC,│ │   portable)  │             │
│  │   match)   │ │  assoc.)    │ │              │             │
│  └────────────┘ └─────────────┘ └──────────────┘             │
└──────────────────────────────────────────────────────────────┘
```

### 6.1 Central architectural principle: the Source Resolver

Everything the user can watch arrives through one interface:

```rust
#[async_trait]
pub trait SourceBackend: Send + Sync {
    fn id(&self) -> &str;
    fn capabilities(&self) -> BackendCapabilities;
    async fn find(&self, query: &MediaQuery) -> Result<Vec<SourceCandidate>>;
    async fn resolve(&self, candidate: &SourceCandidate) -> Result<PlayableStream>;
}
```

A local file, a P2P swarm, a debrid link, and a direct HTTP stream are all `SourceCandidate`s. The auto-selection engine (Phase 9) ranks candidates across *all* backends with one scoring function. This is why "your local copy appears next to the streaming options" works without special-casing, and why debrid can be added later as ~200 lines rather than a rewrite.

**This trait is defined in Phase 6 and must not be violated afterwards.** If something doesn't fit, change the trait deliberately with an ADR.

### 6.2 Media identity

One canonical `MediaItem` type from Phase 3, generic enough to represent a film, a TV episode, an anime season, and — later, without migration — a manga chapter. Identity is a stable internal ID with a set of external ID mappings (`tmdb`, `imdb`, `tvdb`, `anilist`, `mal`). Everything else in the system references the internal ID.

Anime specifically requires: absolute vs seasonal episode numbering reconciliation, romaji/English/native title variants, and sub/dub track awareness. Design for this in Phase 3 rather than patching it in Phase 12.

---

## 7. Repository Layout

```
sin-e-phile/
├─ SPEC.md                       # this document
├─ README.md                     # the portfolio front door
├─ LICENSE                       # GPL-3.0
├─ CHANGELOG.md
├─ CONTRIBUTING.md
├─ PROJECT_STATE.json            # machine-readable resume state
├─ PROGRESS.md                   # human-readable progress
├─ SESSION_LOG.md                # append-only session history
├─ .github/
│  ├─ workflows/ci.yml
│  ├─ ISSUE_TEMPLATE/
│  └─ pull_request_template.md
├─ docs/
│  ├─ HOW_IT_WORKS.md            # plain-English system explanation (for the author)
│  ├─ ARCHITECTURE.md            # diagrams, module boundaries, data flow
│  ├─ GLOSSARY.md
│  ├─ INTERVIEW_PREP.md
│  ├─ SETUP.md                   # prerequisites, API keys, build
│  ├─ RISKS.md                   # the Appendix D risk register, with triggers
│  ├─ DECISIONS_PENDING.md       # deliberately deferred decisions
│  ├─ PERFORMANCE.md             # every budget, current measurement, how achieved
│  ├─ MANUAL_TESTS.md            # the manual test plan (§12.3)
│  ├─ adr/                       # 0001-record-architecture-decisions.md, ...
│  ├─ phases/                    # phase-NN-<slug>.md — spec + retrospective per phase
│  ├─ learning/                  # phase-NN-notes.md — the author's explainers
│  ├─ specs/                     # addon-protocol.md, scoring-model.md, ...
│  ├─ case-studies/              # long-form write-ups for the portfolio
│  └─ schemas/                   # project-state.schema.json, addon-manifest.schema.json
├─ src-tauri/                    # Rust
│  ├─ Cargo.toml
│  └─ src/
│     ├─ main.rs
│     ├─ commands/               # Tauri IPC surface (thin — no logic here)
│     ├─ catalogue/
│     ├─ search/
│     ├─ taste/
│     ├─ discovery/
│     ├─ sources/                # resolver + backends
│     ├─ torrent/
│     ├─ playback/
│     ├─ subtitles/
│     ├─ library/                # local files
│     ├─ platform/               # Windows-specific
│     ├─ persistence/
│     └─ tiers.rs                # hardware capability detection
├─ src/                          # React
│  ├─ design-system/             # tokens, primitives, Storybook-ish gallery
│  ├─ features/                  # home, films, tv, watchlist, live, player, search, settings
│  ├─ lib/                       # ipc client (typed), hooks, utils
│  └─ app/
├─ crates/                       # extracted reusable crates
│  ├─ filename-parser/
│  ├─ subtitle-align/
│  └─ source-protocol/           # published spec + types for addon authors
├─ tools/
│  ├─ ingest/                    # offline dataset ingestion pipeline
│  ├─ eval/                      # evaluation harness runners
│  ├─ guard/                     # posture guard (§12.5) — Python, stdlib only
│  └─ doctor/                    # prerequisite checker (Phase 0) — Python, stdlib only
├─ .githooks/                    # tracked hooks, activated via core.hooksPath
├─ spikes/                       # throwaway de-risking spikes (Phase 1)
└─ fixtures/                     # test corpora (see Section 12)
```

Extracting `filename-parser`, `subtitle-align`, and `source-protocol` into standalone crates is deliberate: they are self-contained, testable, genuinely reusable, and they make the repository read as engineering rather than as one large application blob.

---

## 8. Hardware Capability Tiers

Detect on first run, store in config, allow manual override in settings. Re-detect if hardware changes.

| Tier | Detection | Enabled |
|---|---|---|
| **Tier 0 — Modest** | < 8 GB RAM, or no hardware video decode, or ≤ 2 physical cores | Full core experience. Software decode fallback, capped to 1080p by default (user-overridable). **Catalogue document embeddings** precomputed and downloaded, never computed on device — but queries *are* embedded on device, see below. No face recognition. No local VAD subtitle alignment (hash-match and duration heuristics only). Reduced hover previews, no background blur effects, virtualised rails with smaller windows. |
| **Tier 1 — Standard** | 8–16 GB RAM, hardware decode present, ≥ 4 cores | Everything in Tier 0, plus on-device embedding of new items, VAD-based subtitle alignment, up to 4K playback, full motion design, intro/credit detection. |
| **Tier 2 — Capable** | ≥ 16 GB RAM, discrete GPU or strong iGPU, ≥ 6 cores | Everything, plus face recognition in the pause overlay, optional Whisper-based subtitle generation and alignment, background pre-embedding of the catalogue, higher-quality ANN parameters. |

**Documents versus queries — the Tier 0 embedding rule stated precisely** (ADR-0015). *Tier 0 never embeds catalogue documents on device. Tier 0 does embed one short query per search.* The asymmetry is scale, not kind: embedding the catalogue is hundreds of thousands of forward passes over long composed documents, which is what this rule protects a weak machine from; embedding a query is a single forward pass over roughly 30 tokens. They differ by five or six orders of magnitude. ONNX Runtime is therefore required at runtime on **all** tiers, and `tiers.rs` gates only whether *document* embedding is permitted. Phase 1 Spike C must measure query-embedding latency specifically, the §2.3 80 ms p95 search budget is measured *including* it, and a Spike C p95 above ~30 ms escalates under §10.9 rather than widening the budget.

**Rules:**
- Tier gating is a single module (`tiers.rs`) exposing `Capability::FaceRecognition`, `Capability::LocalEmbedding`, etc. No feature checks hardware directly.
- Every tier-gated feature must degrade to a *good* alternative, never to a broken or empty one. Face recognition off → the overlay shows the full cast list, beautifully. VAD alignment off → hash match plus manual nudge, and the nudge is remembered per file.
- The Settings screen shows the detected tier and exactly which features it enables, in plain language. This is a nice UI moment, not an apology.
- Performance budgets in Section 2.3 are enforced against Tier 0.

---

## 9. Design System — "Charcoal & Oxblood"

Phase 2 builds this properly. **Read §9.0 before anything else in this section:**
the palette and type below serve a direction, and the direction is the part that
must not be compromised.

### 9.0 The direction, and the test it must pass

**References, in order of influence.** MUBI leads.

| | What to take |
|---|---|
| **MUBI** | Near-black ground, full-bleed film stills, enormous breathing room, almost no chrome, magazine-like layout, tiny uppercase metadata labels. **The page should feel edited, not generated.** |
| **Spotify** | Aggressive weight contrast. Very heavy headings against very quiet metadata, tight tracking on large type. Hierarchy readable from across a room. |
| **Criterion Channel** | The feeling of a curated collection — numbered, catalogued, cared for. Editorial serif used sparingly for titles. |
| **Netflix** | **Mechanics only**: rails, hover behaviour, the billboard hero. Take none of its visual language. |

The target is an application that feels like every media need can be met, and met
tastefully. **Premium means considered, not decorated.**

#### It must not look AI-generated

A first-class requirement, not a preference. The design system is **audited against
this checklist**, and a component that fails it is a defect regardless of how clean
it looks.

**Banned, no exceptions:**

- Purple, violet or indigo anywhere in the palette
- Glassmorphism, decorative `backdrop-blur`, glow or bloom
- Gradient text, gradient buttons, gradient anything as decoration
- Emoji as icons or in UI copy
- Bento-box grids
- Drop shadows as the primary depth cue
- Tailwind's default palette values (`slate`, `zinc`, `indigo`, …)
- **Inter — or any neutral UI sans — used as the DISPLAY face.** The single biggest tell.
- **Uniform rounding.** At most **two** radius values in the entire system, and at
  least one must be `0` or `2px`.
- **Uniform sizing.** If every card is the same size and every gap is the same, it
  reads as generated. Build deliberate irregularity.

**Required instead:**

- Depth from **surface value and 1px lines**, never shadows
- A real display typeface, used large, with tight tracking
- Small uppercase metadata labels with **wide** tracking — the MUBI tell
- **Artwork carries all the colour**; chrome stays near-neutral
- Asymmetry and editorial layout — aligned to a grid, not identical within it
- Idiosyncratic details: rule lines, spine numbers, small caps, a deliberate quirk
  a template would not have

#### The test

> **Would this screenshot pass as a MUBI or Criterion screen?**
>
> If it would pass as a generic dark SaaS dashboard, it has failed — regardless of
> how clean it is.

Apply it to every screen before calling it done.

### 9.1 Colour

Near-black and neutral. The greys are **warm** (R ≥ G ≥ B), because the ink is a
warm cream and a cool grey ground fights it — that mismatch is a large part of why
a palette reads as generic.

The steps are **deliberately non-linear**: a large jump from ground to surface, then
small ones. Elevation is therefore rare and means something, rather than being a
gentle ramp every panel sits somewhere on.

```css
:root {
  /* Ground — true black behind artwork, so stills sit on nothing */
  --void:            #000000;  /* player letterbox, behind full-bleed stills */
  --base:            #0A0A0A;  /* app background */
  --surface:         #141312;  /* cards, rails, panels */
  --raised:          #1F1D1B;  /* hover, elevated cards */
  --overlay:         #262421;  /* menus, popovers */
  --scrim:           rgba(0,0,0,0.86);

  /* Line — depth comes from these, not from shadows */
  --line-subtle:     #1C1A18;
  --line:            #2C2926;
  --line-strong:     #423E39;

  /* Type — warm cream, never pure white */
  --ink:             #F5F2ED;
  --ink-muted:       #9A948C;
  --ink-faint:       #6B655D;

  /* Accent — oxblood, kept: see the note below */
  --oxblood:         #8E2B34;
  --oxblood-hover:   #A6333E;
  --oxblood-bright:  #C4434F;  /* focus rings, active state, progress */
  --oxblood-deep:    #4E171C;  /* pressed, deep fills */
  --oxblood-wash:    rgba(142,43,52,0.16);

  /* Semantic — deliberately distinct from oxblood */
  --success:         #4E7A5C;
  --warning:         #B8873F;
  --danger:          #D9503F;   /* vermilion, clearly hotter than oxblood */
  --info:            #5A7A8C;
}
```

**Why oxblood stays.** It is not a technology colour. It reads as cinema — velvet,
Criterion's spine red, a curtain — and it is the one element that stops a near-black
neutral palette from being anonymous. The revision raises its chroma slightly so it
separates from a warmer ground.

**Colour rules:**

- Oxblood is for *intent*: the play button, focus rings, progress, the active nav
  item, continue-watching markers. **Never decorative, never a large fill.**
- **Posters and stills are the colour in this app.** The chrome recedes. If a screen
  looks colourful, the artwork is doing it.
- `--danger` and `--oxblood` must never appear adjacent. Destructive actions use
  `--danger` with a text label, never colour alone.
- Verify every text/background pair at WCAG AA (4.5:1 body, 3:1 large).
  `--ink-faint` on `--surface` is the one to watch — **check it**.
- Poster cards get a 1px `--line-subtle` border so light artwork does not bleed into
  the surface. **No shadow.**

### 9.2 Type

Type is most of this design. MUBI and Spotify both use commercial faces (a Söhne-like
grotesque; Circular), so the equivalents here must be **free and self-hostable**.

- **Display** — a grotesque with real character, used **large with tight tracking**
  (−0.02 to −0.04em). Candidates evaluated in Phase 2: Bricolage Grotesque, Archivo,
  Instrument Sans, Geist, Space Grotesk.
- **Editorial** — a serif for film titles and pull quotes, used **sparingly**:
  Instrument Serif or Newsreader.
- **UI and metadata only** — Inter or Geist. **Never the display face** (§9.0).
- **Mono** — JetBrains Mono, for technical panels, the expert source view and logs.
- Scale: 12 / 13 / 14 / 16 / 20 / 26 / 34 / 46 / 62 / 84. Display sizes take tighter
  tracking; small uppercase labels take **+0.12 to +0.16em**, which is the MUBI tell.
- **Ship font files locally. No network font requests, ever.**

### 9.3 Motion

- Standard transition: 180 ms, `cubic-bezier(0.32, 0.72, 0, 1)`.
- Poster hover: scale 1.045, 220 ms, with a 400 ms dwell before any preview begins.
- Rail scroll: native momentum, snap to card edges, never a jump-by-page carousel.
- Player chrome: fades in 120 ms, out 400 ms after 2.5 s idle.
- **Player chrome during playback is a solid, opaque panel with a hard edge**
  (ADR-0020). No gradient scrim beneath it and no blur-behind. This is a platform
  constraint made into a deliberate design: on Windows a native video surface always
  paints above the webview, so the chrome is *cut out* of the video window
  (`SetWindowRgn`) rather than composited over it — and a window region is binary, so
  there is no per-pixel alpha through the hole. Rounded corners are permitted **only
  with a matching rounded region**: the hole and the opaque chrome must be the same
  shape, because any part of a hole the chrome does not paint shows the desktop
  through the transparent window. **A crisp opaque panel suits this brief anyway.**
  The **pause overlay is exempt** and keeps its dim and blur — it composites over a
  captured still frame, where everything is HTML (ADR-0021).
- `prefers-reduced-motion` disables all scale and parallax, keeps opacity fades at
  100 ms.

### 9.4 Layout

- 8px base grid. **Card sizes are deliberately not uniform** (§9.0): a rail mixes
  sizes by editorial weight rather than repeating one tile.
- Rails: 24px gutters, cards 200×300 (film) / 320×180 (episode, landscape) as the
  *base* sizes, with featured items larger.
- Content max-width 1680px, centred, with rails bleeding to the viewport edge.
- Left nav rail: 72px collapsed (icons), 240px expanded, remembers state.
- **Generous vertical rhythm.** Breathing room is the MUBI signal and the easiest
  thing to lose to density. When in doubt, more space.
- Every interactive element has a visible `--oxblood-bright` focus ring. Full keyboard
  navigation is a Phase 2 requirement, not a Phase 27 retrofit.
- **`PosterCard` has a designed artwork-free state** (ADR-0013): a typographic
  title card, not a grey rectangle. Given this brief it should be genuinely
  beautiful — for a film app a typographic card is arguably the better default.

## 10. Session Protocol and State Management

This section governs how Claude Code operates. It matters more than any individual phase.

### 10.1 The three state files

**`PROJECT_STATE.json`** — the machine-readable truth. Schema in `docs/schemas/project-state.schema.json`.

```jsonc
{
  "spec_version": "1.0.0",
  "project": "sin-e-phile",
  "last_updated": "2026-08-29T14:22:10Z",
  "sessions_completed": 12,
  "current_phase": {
    "number": 7,
    "slug": "torrent-engine",
    "status": "in_progress",          // not_started | in_progress | blocked | awaiting_review | complete
    "branch": "phase/07-torrent-engine",
    "started_at": "2026-08-29T09:00:00Z",
    "subtasks": [
      { "id": "7.1", "text": "librqbit integration + magnet parsing", "status": "complete", "commit": "a1b2c3d" },
      { "id": "7.2", "text": "Sequential piece prioritisation", "status": "in_progress", "commit": null },
      { "id": "7.3", "text": "Local HTTP range server", "status": "not_started", "commit": null }
    ],
    "exit_criteria": [
      { "id": "E1", "text": "Playback starts < 8s on a healthy swarm", "met": false, "evidence": null },
      { "id": "E2", "text": "Seeking to an undownloaded region re-prioritises within 2s", "met": false, "evidence": null }
    ]
  },
  "phases": [ /* one summary object per phase, 0..27, with status + completion commit */ ],
  "next_action": "Implement piece prioritisation window in src-tauri/src/torrent/scheduler.rs; the deadline-based algorithm is described in docs/specs/streaming-scheduler.md",
  "blockers": [
    { "id": "B3", "raised": "2026-08-29T13:00:00Z", "needs_user": true,
      "description": "Need a TMDB API key in .env before Phase 4 integration tests can run." }
  ],
  "decisions_pending": [],
  "adrs": [ { "id": "0007", "title": "Use librqbit over libtorrent FFI", "date": "2026-08-20" } ],
  "environment": {
    "rust_version": "1.8x.x",
    "node_version": "2x.x.x",
    "verified_at": "2026-08-29T09:02:00Z",
    "configured_services": ["tmdb"]
  },
  "known_debt": [
    { "id": "D2", "text": "Metadata cache eviction is naive LRU; revisit in Phase 21", "phase_raised": 4 }
  ]
}
```

**`PROGRESS.md`** — the human view. A phase checklist with checkboxes, a "where we are right now" paragraph, and a "what's next" line. Regenerated from `PROJECT_STATE.json` at the end of every session so the two can never disagree.

**`SESSION_LOG.md`** — append-only. One entry per session: date, phase, what was attempted, what was completed, what was learned, what broke, what the next session should do first. Never edit past entries.

### 10.2 Session start ritual — mandatory, every session

1. **Read only these** (ADR-0016): `PROJECT_STATE.json`, the **last** `SESSION_LOG.md` entry, and §15's entry for the current phase. `CLAUDE.md` loads automatically and carries the standing rules, so **do not re-read `SPEC.md` end to end.** Read another section only when the work actually touches it — or on a cold resume (§10.11), which keeps its fuller reading list.
2. **Verify state against reality**: run `git status`, `git log --oneline -10`, `cargo test`, `npm test`. If the repository disagrees with `PROJECT_STATE.json`, **the repository is right**. Correct the state file, and record the discrepancy in `SESSION_LOG.md`.
3. Report to the author in **five lines or fewer**: current phase, what's done, what's next, blockers, anything needing a decision. Do not restate the spec back. Findings are bullets, not tables. Prose is for when something genuinely needs arguing (ADR-0016).
4. If there are blockers with `needs_user: true`, raise them *before* planning.
5. Produce a plan for this session. Wait for approval before implementing.

### 10.3 Session end ritual — mandatory, every session

Never end a session without all of these:

1. All tests pass, or failing tests are explicitly recorded as a blocker with an explanation.
2. `cargo fmt`, `cargo clippy -- -D warnings`, and frontend lint are clean.
3. `PROJECT_STATE.json` updated: subtask statuses, exit criteria evidence, `next_action` written as an unambiguous instruction to the next session (not "continue the torrent engine" — "implement `SubtitleAligner::estimate_framerate_scale` in `crates/subtitle-align/src/lib.rs`; the correlation approach is described in `docs/specs/subtitle-alignment.md` §3").
4. `PROGRESS.md` regenerated.
5. `SESSION_LOG.md` entry appended.
6. Docs for anything built this session written (Section 11).
7. Learning note updated (Section 13).
8. Everything committed and pushed.

### 10.4 Mid-session state discipline

State is updated **after each completed subtask**, not only at session end. A session can be interrupted at any moment — a crash, a closed laptop, an exhausted context window. The state file must never be more than one subtask stale.

### 10.5 Branching and commits

- One branch per phase: `phase/NN-slug`.
- Conventional commits: `feat(torrent): implement sequential piece scheduler`, `docs(adr): record librqbit decision`, `test(subtitles): add misalignment fixture corpus`.
- Merge to `main` only when the phase's exit criteria are all met and evidenced. Squash-merge with a summary body listing what landed.
- Tag each phase completion: `phase-07`.
- `main` must always build and run. If it doesn't, that is the highest-priority bug.

### 10.6 Scope discipline

Each phase has a defined scope. When something desirable but out-of-scope surfaces:

- If it's a bug in previously completed work → fix it now, note it in the session log.
- If it's a new feature idea → add it to `known_debt` or open a GitHub issue, and move on.
- If it's a genuine blocker requiring an architectural change → stop, write an ADR, get the author's approval, then proceed.

Do not silently expand a phase. A phase that grows by 60% is a phase whose exit criteria stop meaning anything.

### 10.7 Session Zero Instruction

> Paste this to start the very first Claude Code session:
>
> "Read `SPEC.md` in full before doing anything. It is the complete specification for this project and it governs every session. Then execute Phase 0 exactly as specified in Section 15, including creating `PROJECT_STATE.json`, `PROGRESS.md`, and `SESSION_LOG.md`. Before you write any code, confirm back to me: the locked technology decisions, the legal posture in Section 2.1, and your understanding of the session protocol in Section 10. Then plan Phase 0 and wait for my approval."
>
> Every subsequent session: **"Read `SPEC.md`, then run the session start ritual in Section 10.2."**

### 10.8 The evidence standard

An exit criterion is met when there is an **artefact**, not an opinion. `PROJECT_STATE.json`'s `evidence` field must contain one of:

- a test name that passes (`torrent::scheduler::tests::maintains_buffer_under_bandwidth_floor`)
- a measured number with the command that produced it (`p95 = 61ms — cargo run -p eval -- search --report`)
- a file path to a produced artefact (`docs/specs/streaming-scheduler.md`, `fixtures/filenames/` — 512 cases)
- a commit SHA implementing a specific, verifiable thing
- an explicit `manual: <what the author did and observed>` for criteria only a human can judge

**Never** prose like "implemented and working", "tested manually", "looks good", or "should be fine". If evidence cannot be produced, the criterion is **not met** — say so and explain what is blocking it.

Marking a criterion met without evidence is the single most damaging thing that can happen to this project, because it destroys the ability of any future session to trust the state file. Treat it as a correctness bug.

### 10.9 The stuck rule and escalation

Claude Code will get stuck. libmpv will not embed. A crate will not build. A test will fail for reasons that make no sense. The failure mode to avoid is burning an entire session on it.

**After three genuinely distinct attempts at the same problem, stop.** Not three variations of the same idea — three different approaches. Then:

1. Write the problem into `PROJECT_STATE.json` as a blocker with `needs_user: true`, including what was tried and what happened each time.
2. Present the author with the actual options, honestly costed: a different library, a different approach, deferring this to a later phase, or cutting the requirement.
3. Recommend one and say why.
4. **Stop and wait.** Do not keep going.

If a blocker will take more than a session to resolve, propose reordering — work a later independent phase while the blocker is thought about. Sitting stuck is worse than working out of order.

Equally: if something turns out to be much easier than expected, say so. Do not pad a phase to feel proportionate.

### 10.10 The understanding gate is active, not passive

**The gate fires at tier boundaries, not every phase** (ADR-0016): the end of Phase 8, the end of Phase 18, the end of whatever tier the author stops at, and whenever the author asks for it. At those points Claude Code **asks the five self-check questions** from the relevant learning notes, in the chat, and waits for answers.

Every phase still *writes* its five questions (§13.2). They accumulate unasked until a gate, which is what makes a gate meaningful rather than a formality.

- If the author answers them well → the phase is done.
- If they struggle on one → re-explain that concept, in a different way, and update the learning note. That note failed; fix it.
- If they struggle on most → the code is too complex or too fast. Say so directly, and propose either simplifying the implementation or splitting the phase.

The author explicitly asked to keep up with everything. Honouring that means being willing to say "I think we went too fast here, let's slow down" — which is more useful than a green checkmark.

Do not accept "yeah I get it" as an answer. Ask them to explain it back.

### 10.11 Cold resume — returning after a long gap

If `PROJECT_STATE.json`'s `last_updated` is more than **14 days** old, run the extended ritual instead of the normal one:

1. The full session start ritual (§10.2).
2. Read the last **five** `SESSION_LOG.md` entries, not two.
3. Read every ADR written since the last completed phase.
4. Run the full test suite *and* every eval harness, and report the numbers against the last recorded ones — dependency drift and rot are real.
5. `cargo update --dry-run` / `npm outdated` and report anything that has moved significantly.
6. Give the author a **re-orientation briefing**: where the project stands, what the current phase is trying to achieve, what the last few sessions did, and what they were about to do next. Assume they have forgotten everything, because they have.
7. Only then plan.

### 10.12 Regression policy

The suite only grows. Every phase's tests keep running in every later phase.

Before merging any phase branch: the **full** test suite and **every** eval harness must run, and the results are compared against the previous phase's recorded numbers in `docs/PERFORMANCE.md` and the eval reports. A regression in a metric from an earlier phase blocks the merge, exactly as a failing test does.

This matters most for the eval harnesses, which measure quality rather than correctness. A refactor in Phase 18 that quietly drops search nDCG from 0.78 to 0.61 would otherwise never be noticed.

---

## 11. Documentation Requirements

Documentation is written *as the code is written*, never retrofitted.

### 11.1 Per-phase, always

Lean by default (ADR-0016). Per phase, only:

- **The phase retrospective folds into the `SESSION_LOG.md` entry** — what was built, what deviated and why, what was harder than expected, what debt was incurred. **No separate `docs/phases/` file.**
- `docs/learning/phase-NN-notes.md` — the lean four-section note (§13.2).
- **`docs/eval-results.md` — every eval-harness metric and performance measurement, appended the moment it is produced.** Not deferrable; see below.
- A 2–3 sentence stub appended to `docs/HOW_IT_WORKS.md`, and raw one-line entries appended to `docs/GLOSSARY.md`. Prose write-up is deferred.
- `docs/specs/*.md` algorithm specifications where the phase produces one. These are **build inputs**, not learning material, and are written in full.
- ADRs for every non-obvious decision (§11.2), terse.

**Deferred to Phase 27 or a tier boundary:** all five case studies, `INTERVIEW_PREP.md`, and the prose expansion of `HOW_IT_WORKS.md` and `GLOSSARY.md`.

**The exception that is never deferred — numbers.** Every eval-harness run and every performance measurement is recorded in `docs/eval-results.md` **at the moment it is produced**: metric, value, date, commit, and the command that produced it. Numbers cannot be reconstructed after the fact, and the regression policy (§10.12) is worthless without them. A measurement not written down is a measurement that did not happen.

### 11.2 ADRs

Every non-obvious decision gets an ADR in `docs/adr/NNNN-title.md`: Context / Decision / Consequences / Alternatives Considered. Write it *at the moment of deciding*, not later — a reconstructed ADR records the justification rather than the reasoning, which is the half worth having.

**Terse: bullets, roughly 15 lines** (ADR-0016). ADRs are mandatory and are not what gets cut; length is. The Alternatives section is the one that must never be empty — if nothing was genuinely rejected, there was no decision.

An ADR is required for: choosing between libraries, designing a public interface, choosing an algorithm where alternatives exist, any performance trade-off, and any deviation from this spec.

Seed ADRs to write in Phase 0, documenting decisions already made here: Tauri over Electron; librqbit over libtorrent; libmpv over libVLC; SQLite + FTS5 + HNSW over a vector database; ships-empty source posture; GPL-3.0 and the dependency licence audit; portable-by-default storage.

### 11.3 The README

The README is the single most-read file in the repository and the first thing an employer sees. It must contain, in this order:

1. A one-line description and a hero screenshot.
2. A demo GIF showing search → play → subtitle sync, under 15 seconds.
3. **What makes it interesting** — three or four paragraphs on the genuinely hard problems solved: the discovery engine, hybrid semantic search, the streaming torrent scheduler, subtitle auto-alignment, multi-signal local file identification. This section is the pitch. Write it well.
4. Architecture diagram.
5. Feature list, honestly scoped to what actually works today.
6. Screenshots of Home, a detail page, the player with the pause overlay, and the local library review queue.
7. Build instructions that actually work on a clean machine.
8. A clear statement of the legal posture: this ships no content sources; it is a media engine.
9. Tech stack, licence, acknowledgements.

The README is updated at the end of every phase that changes what the app can do. It is never allowed to over-claim.

### 11.4 Case studies

From Phase 16 onward, each major subsystem gets a long-form write-up in `docs/case-studies/`: the problem, the approaches considered, the approach taken, the maths or algorithm explained accessibly, the evaluation results with numbers, and what would be done differently. Target: 1,200–2,000 words each, with diagrams.

Priority case studies: the discovery engine; hybrid semantic search; the streaming torrent scheduler; subtitle auto-alignment; source scoring and selection.

These are the artefacts to link directly in job applications.

---

## 12. Testing and Evaluation

### 12.1 Unit and integration tests

Required for all pure logic: filename parsing, source scoring, subtitle alignment maths, recommender ranking, episode-number reconciliation, tier detection, bandit arm selection. Use `proptest` for parsers and scorers — property tests catch the cases hand-written tests miss.

Integration tests for: the torrent → HTTP server → player pipeline, library scanning end-to-end, database migrations forward and backward, IPC command surface.

Frontend: component tests for the design system primitives, hooks tests for anything with real logic. Do not chase coverage on presentational components.

### 12.2 Evaluation harnesses — the distinguishing feature

Four harnesses live in `tools/eval/`, run in CI, and report metrics that get tracked over time in `docs/case-studies/`. **These are what make this project read as engineering rather than assembly.**

| Harness | Corpus | Metric | Target |
|---|---|---|---|
| **Filename identification** | `fixtures/filenames/` — 500+ real-world messy filenames with hand-labelled correct answers, including deliberate ambiguities (`The.Batman` 1989 vs 2022, anime absolute numbering, scene-release garbage, multi-episode files, foreign titles) | Top-1 accuracy; false-confident rate (wrong answer above threshold) | > 95% top-1; < 1% false-confident |
| **Subtitle alignment** | `fixtures/subtitles/` — subtitle files deliberately offset by known amounts, framerate-scaled (23.976↔25), and with inserted/removed intro segments, paired with audio VAD traces | % of files aligned to within 150 ms | > 90% |
| **Search relevance** | `fixtures/search/` — 100+ queries with hand-graded relevant results, including semantic queries ("slow films about grief that aren't depressing"), lookalike queries ("like Wong Kar-wai but Korean"), and exact-title queries | nDCG@10; exact-title top-1 rate | nDCG@10 > 0.75; exact-title top-1 = 100% |
| **Recommender quality** | Held-out split of a public ratings dataset, plus a synthetic cinephile profile | Recall@20, catalogue coverage, intra-list diversity, novelty (mean inverse popularity) | Recall@20 beats a popularity baseline by > 40% **relative** (ratio, not percentage points); coverage > 15% of catalogue |

Building these corpora is real work and it is worth it. Phase 12, 10, 5, and 16 each include their corpus as a deliverable.

### 12.3 Manual test plan

`docs/MANUAL_TESTS.md`: a checklist for the things automation can't cover — playback quality, subtitle rendering fidelity, UI responsiveness under load, the feel of the discovery rails. Run it before merging any phase that touches playback or UI.

### 12.4 CI

GitHub Actions on `windows-latest`, on every push and PR:
`cargo fmt --check` → `cargo clippy -- -D warnings` → `cargo test` → `npm run lint` → `npm run test` → `npm run build` → `cargo tauri build` (compile only) → **posture guard** (§12.5) → **secret scan** (§12.6) → eval harnesses (from the phase they exist) → publish eval metrics as a job summary.

### 12.5 The posture guard — automated enforcement of §2.1

A human rule that must hold across 28 phases and dozens of sessions will eventually be broken by accident. So automate it.

`tools/guard/` contains a Python 3.12 script (ADR-0012), run in CI on every push and as a pre-commit hook, that fails the build on the findings below. It uses **two matchers with deliberately different visibility properties** (ADR-0009), plus an **allowlist** (ADR-0010).

**Matcher 1 — structural patterns, in plaintext.** These describe the *shape* of forbidden content without naming any instance of it, so they are safe to commit in the clear, and they generalise to sites nobody has thought of:

- `magnet:?xt=urn:btih:` URIs
- bare 40-character hex and 32-character base32 infohashes, outside clearly-marked legal test fixtures
- `/announce` and `/scrape` URL path shapes
- HTTP(S) URLs ending in `.torrent`
- configuration keys shaped like `default_source_url`, `default_indexer`, or `default_catalogue_url` carrying any non-empty value

This matcher does the majority of the real work.

**Matcher 2 — a hashed exact-token denylist.** `tools/guard/denylist.txt` stores `SHA-256(salt || normalised_token)` in hex, one per line, with the salt committed alongside in `tools/guard/denylist.salt`. Plaintext site names are **never** written to this repository, because doing so would itself violate §2.1. Entries are added via `tools/guard/add_token.py`, which reads from stdin without echoing.

This is **hygiene, not security.** A committed salt is brute-forceable by anyone determined. The threat model is accidental plaintext and casual discovery — a grep of the tree, a code search, a reader scrolling the file list — and against that it is fully effective. It is not protection against a motivated adversary and must never be described as such. Its accepted cost is that it matches **exact tokens only**: no substring, fuzzy, or homoglyph matching, because those require the plaintext it deliberately does not hold.

**The allowlist.** `tools/guard/allowlist.txt` enumerates every content or metadata source domain permitted to appear in shipped code, config, or documentation — the Internet Archive, TMDB, AniList, Jikan, OpenSubtitles, Fanart.tv, Trakt, MovieLens/GroupLens, and the IMDb datasets — each with an inline justification. It covers *content and metadata sources only*; general development infrastructure (`crates.io`, `npmjs.com`, `github.com` and similar) is out of scope. **Adding a line requires an ADR**, stating what the source is, why it is unambiguously legal, and whether it ships enabled or opt-in. The allowlist is the easiest place for the §2.1 posture to erode quietly, so its diffs get reviewed specifically.

**Test vectors and examples use RFC 2606 reserved domains.** Every guard self-test, denylist test case, and documentation example uses `.invalid`, `.test`, `.example`, or `example.com`. A string like `notarealindexer.invalid` is *provably* not a real site, so the guard can be proven to fire without a real forbidden string ever entering the repository or its history. Any example URL that is neither on a reserved TLD nor on the allowlist is itself a finding.

The guard also scans **commit history**, not just the working tree, so a mistake that was committed and then removed is still caught. History scanning runs in CI (which checks out with `fetch-depth: 0`); the pre-commit hook scans staged content only, because a hook whose cost grows with repository age is a hook that eventually gets bypassed. Wire this in Phase 0, before there is anything to catch.

When the guard fires, the fix is to remove the content and — if it was committed — rewrite history before pushing. Never suppress the guard to make CI pass.

### 12.6 Secret scanning and `.gitignore` discipline

This is a public repository and it will hold API keys during development. Phase 0 establishes:

- `.gitignore` covering `.env`, `.env.*` (except `.env.example`), `data/`, `*.db`, `*.onnx`, `models/`, `fixtures/media/`, and all build output.
- A secret-scan step that fails on anything resembling an API key, token, or credential (ADR-0012). CI runs a **version-pinned, checksum-verified gitleaks** binary. The pre-commit hook runs `tools/guard/secretscan.py`, a standard-library regex fallback covering the key shapes relevant here, so the hook works whether or not gitleaks is installed locally. `doctor` reports which is active. Defence in depth: the local check is fast and approximate, CI is thorough and authoritative, and GitHub push protection is the backstop if both are bypassed.
- **Hooks are activated via `core.hooksPath`.** They live in a tracked `.githooks/` directory and are wired up by `tools/doctor/` as a bootstrap step, because `.git/hooks/` is not tracked and a hook that does not activate on clone protects nothing. A fresh clone is unprotected until `doctor` runs once — which is why the guard runs in CI as well, and why `docs/SETUP.md` makes `doctor` step one.
- GitHub's own push-protection and secret-scanning enabled on the repository.
- `.env.example` documents every variable with a placeholder value and never a real one.

If a key is ever committed: **rotate it immediately**, then clean the history. Rotating first matters more than cleaning — assume anything pushed to a public repository is compromised the moment it lands.

---

## 13. Learning Requirements

The author is new to Rust and React and must be able to explain this project in an interview. These are hard deliverables.

### 13.1 `docs/HOW_IT_WORKS.md`

A plain-English explanation of the entire system, assuming no prior knowledge. Updated every phase. Structure: what the app does → what happens when you press play, traced end to end → each subsystem in a section of 300–600 words with an analogy and a diagram → where to look in the code. No jargon without a definition. If a section can't be explained without three other concepts, explain those first.

### 13.2 `docs/learning/phase-NN-notes.md`

Written at the end of every phase, **lean** (ADR-0016). Four sections:

1. **What we built** — 3–5 bullets.
2. **Why this approach** — 2–3 bullets, naming the alternatives rejected.
3. **New concepts** — a **list** of concept plus `file:line` pointer. No prose explanation. `Arc<Mutex<T>> — torrent/scheduler.rs:42` is the whole entry.
4. **Five self-check questions** — written now, **asked at the next tier boundary** (§10.10).

**The code tour section is dropped.** A tour traced from the real code on demand is better than one written from memory and immediately stale.

The principle: **record what cannot be reconstructed** — the decision, the gotcha, the pointer to where a concept lives. Explanatory prose can be regenerated from the code later, so it is not written now.

### 13.3 `docs/GLOSSARY.md`

Every domain term (piece, swarm, seeder, DHT, magnet, VAD, HNSW, ANN, BM25, nDCG, contextual bandit, embedding, collaborative filtering, muxing, hardware decode, ASS/SSA) and every stack term the author might be asked about. One clear paragraph each, with a link to where it's used in the codebase.

### 13.4 `docs/INTERVIEW_PREP.md`

Built up from Phase 8 onward. Likely interview questions about this project, with real answers grounded in the actual code:

- "Walk me through the architecture."
- "What was the hardest problem?"
- "How does the recommender avoid a filter bubble?"
- "Why Rust for the backend?"
- "How do you get a torrent to start playing in eight seconds?"
- "How do you know your subtitle alignment works?"
- "What would you do differently?"
- "What's the biggest weakness in this codebase?"

Answers must be honest, including about weaknesses. A candidate who can name their project's flaws is more convincing than one who can't.

---

## 14. Prerequisites and External Services

All free. Phase 0 verifies current terms for each and records them in `docs/SETUP.md` — terms change, so check rather than trusting this table.

**Development prerequisites** (checked by `tools/doctor/`): Rust stable with the MSVC toolchain, Node, MSVC C++ build tools, the Windows SDK, the WebView2 runtime, git, FFmpeg, and **Python 3.12+** (ADR-0012 — required by `tools/guard/` and `tools/doctor/`, which use the standard library only and need no `pip install`). Python is a *development* prerequisite; a user running a built application never needs it.

**No API key is required to run the application** (ADR-0013). The offline catalogue — IMDb datasets plus MovieLens, both free downloads with no account — supplies titles, years, runtimes, genres, cast, crew and ratings, and semantic search, the taste model, the recommender and the discovery engine all run on it unchanged. **Every service below is optional enrichment**, and none of their keys ever ship in the build.

| Service | Needed for | Cost | Notes |
|---|---|---|---|
| **TMDB** | Artwork, rich detail, cast headshots | **The user's own free key**, instant, non-commercial | Register at themoviedb.org → Settings → API. **Optional** — offered in onboarding as ~30 seconds of work, with a "later" that is not a dead end (ADR-0013). **No key ever ships with the application** (ADR-0027): each user supplies their own, per profile, in settings, under their own acceptance of TMDB's terms, and may remove it at any time. Every TMDB-dependent surface degrades to the §9.4 typographic treatment rather than breaking. |
| **AniList** | Anime metadata, relations, absolute/seasonal numbering | Free, public GraphQL, no key | Required for anime. |
| **Jikan** | MyAnimeList mirror, supplementary anime data | Free, no key, rate-limited | Optional fallback. |
| **IMDb datasets** | Offline title/rating/crew index | Free download, no account | `datasets.imdbws.com`. Non-commercial use. |
| **MovieLens** | Item-item collaborative filtering signal | Free download, no account | GroupLens; use the largest set the machine can ingest. |
| **OpenSubtitles.com** | Hash-matched subtitles | Free account, **heavily rate-limited** (single-digit downloads/day on the free tier) | **Design around this.** See below. |
| **Fanart.tv** | High-quality logos, backgrounds, clearart | Free personal key | Optional; noticeably improves the UI. |
| **Trakt** | Watch history sync, scrobbling | Free API key | Phase 19. |
| **Debrid** | Optional fast direct streaming | ~€3–4/month | Explicitly optional. Never required. |

**Important consequence of the OpenSubtitles rate limit:** the free tier is too small to be the primary subtitle source. The pipeline in Phase 10 must therefore prioritise, in order:

1. **Subtitle tracks embedded in the media file itself** — MKV releases usually carry them, they are by definition perfectly synced, and they cost nothing. *This should be the default path and will handle the majority of cases.*
2. Subtitles present as sidecar files alongside the media (local library, or in the torrent's file list).
3. OpenSubtitles hash match — spend the daily quota only where the first two failed.
4. Other free providers (Podnapisi and similar public APIs), then alignment.
5. Tier 2 only: locally generated via Whisper.

Treat the quota as a scarce resource with a budget and a cache. This constraint produces a better design than an unlimited API would have.

---

## 15. The Phases

28 phases, `0`–`27`. Each is scoped to one Claude Code session where possible; larger ones state their expected session count.

**Session counts are estimates, not commitments.** They are the honest expectation at the time of writing and they will be wrong in both directions. In particular, §2.2's rule that no more than ~400 lines of new logic ships without stopping to explain it, combined with the active understanding gate in §10.10, means **Phases 4, 7 and 16 should be expected to run 4+ sessions each** rather than the 2–3 stated. That is the specification working as intended, not falling behind. Written down here so it does not feel like failure when it happens.

**Phase 8 is the first demoable milestone** — a working vertical slice — and it exists that early on purpose.

Each phase spec below is copied into `docs/phases/phase-NN-<slug>.md` at the start of the phase and gains a retrospective at the end.

---

### Phase 0 — Bootstrap and Project Infrastructure
**Depends on:** nothing. **Sessions:** 1.

**Goal.** Create a repository that already looks like a serious project before it does anything.

**Deliverables.** Git repo initialised and pushed. Full directory skeleton from Section 7. `LICENSE` (GPL-3.0), `README.md` (skeleton with the pitch section drafted), `CONTRIBUTING.md`, `CHANGELOG.md`. `PROJECT_STATE.json` with all 28 phases enumerated and their exit criteria populated from this document, plus its JSON schema. `PROGRESS.md`, `SESSION_LOG.md` with entry one. `docs/SETUP.md` listing every prerequisite with verified current terms and step-by-step key acquisition. `docs/GLOSSARY.md` seeded with 30 terms. `docs/HOW_IT_WORKS.md` skeleton. ADRs 0001–0008 recording the decisions already locked in Section 5. `.github/` with CI workflow (initially: fmt, clippy, test on a hello-world), issue templates, PR template. `.gitignore` per §12.6. `.env.example`. Conventional-commit enforcement via a commit-msg hook.

**Also, and importantly — the safety rails:**
- `tools/guard/` — the posture guard (§12.5), wired into CI and a pre-commit hook, with its denylist. Build this *before* there is anything to catch.
- Secret scanning in CI and pre-commit (§12.6); GitHub push protection and secret scanning enabled on the repo.
- `tools/doctor/` — a `doctor` script that checks every prerequisite (Rust toolchain and version, Node, MSVC build tools, Windows SDK, WebView2 runtime, git, FFmpeg, required env vars) and prints exactly what is missing and how to install it. Run it at the top of every session's start ritual. This turns "the build mysteriously fails" into "you're missing the C++ build tools, here's the link".
- `docs/RISKS.md` — the risk register from Appendix D, with owner, likelihood, impact, mitigation, and trigger for each. Reviewed at the start of every phase whose risks it names.
- `docs/DECISIONS_PENDING.md` — a running list of things deliberately deferred, so "we'll decide later" never becomes "we forgot".

**Exit criteria.**
- [ ] `git clone` on a clean machine + following `docs/SETUP.md` produces a working dev environment.
- [ ] CI passes on `phase/00-bootstrap`, and again on `main` after the merge. (§10.5 puts all phase work on a branch, so "green on `main`" can only be verified post-merge; both are required.)
- [ ] `PROJECT_STATE.json` validates against its schema and enumerates all 28 phases with their exit criteria.
- [ ] Eight seed ADRs exist and are non-trivial.
- [ ] **The posture guard fails CI when fed a deliberately-planted test string, and passes on the clean tree.** Verify it actually works — an unverified guard is worse than none, because it produces false confidence.
- [ ] Secret scanning fails CI on a deliberately-planted fake key.
- [ ] `doctor` correctly reports a missing prerequisite when one is removed from PATH.
- [ ] `docs/RISKS.md` exists with all Appendix D risks and at least one concrete trigger condition each.

**Learning note.** What each file in the repo root is for; what conventional commits and ADRs are and why professionals use them; how CI works.

---

### Phase 1 — Application Shell and Capability Tiers
**Depends on:** 0. **Sessions:** 1–2.

**Goal.** A Tauri app that opens, has the five-tab navigation, detects hardware, and has a clean typed IPC boundary.

**Deliverables.** Tauri v2 + React + TypeScript (strict) + Vite + Tailwind wired and building. Window: custom title bar, remembered size/position, minimum 1024×640. Left navigation rail with the five destinations (all placeholder screens) and collapse/expand. Typed IPC layer — Rust command definitions with a codegen or `specta`-style step producing TypeScript types, so the boundary can never drift. `tiers.rs`: detect RAM, physical cores, GPU and hardware-decode capability; classify into Tier 0/1/2; persist with manual override. A Settings screen showing detected hardware and which features the tier enables, in plain language. Structured logging (`tracing`) to a rotating file in `data/logs/`. Global error boundary and a Rust panic handler that writes a crash report locally.

**Technical de-risking spikes — do these BEFORE the rest of the phase.** Three decisions in Section 5 are bets, and each is depended on by a much later phase. Discovering in Phase 8 that libmpv cannot be embedded would be catastrophic; discovering it now costs a day. Each spike is throwaway code in `spikes/`, timeboxed to roughly two hours, with the finding written into `docs/RISKS.md` and an ADR if the answer changes a locked decision.

- **Spike A — libmpv in a Tauri v2 window** (risk R1). Prove a video can render, controlled from Rust, with UI drawn over it. Try the render-API-into-a-texture approach and the child-window-overlay approach. Note which works and what it costs.
- **Spike B — librqbit sequential streaming** (risk R2). Against a legal, well-seeded torrent (a Linux ISO or an Internet Archive item), measure time-to-first-usable-bytes with sequential priority, and confirm the API exposes enough control to build the Phase 7 scheduler.
- **Spike C — ONNX Runtime in Rust on Windows** (risk R3). Get `ort` building and running a quantised sentence-transformer, and measure latency and memory. **Measure query-embedding latency specifically** — model already loaded, a single ~30-token query, wall clock to a returned vector, p50 and p95, on the dev machine and on a constrained VM approximating Tier 0 — not just document-embedding throughput. Confirm the build does not require an unreasonable toolchain. **A p95 above ~30 ms escalates under §10.9**; do not widen the §2.3 80 ms search budget to accommodate it (ADR-0015). Because ONNX Runtime is now required on all tiers, this spike gates Phase 5 — run it early in the phase, not last.

**If a spike fails, stop and escalate under §10.9.** The fallback for each is recorded in Appendix D. Do not proceed to the rest of Phase 1 hoping it will work out later.

**Exit criteria.**
- [ ] All three spikes completed, with findings and measurements recorded in `docs/RISKS.md`.
- [ ] Any spike that failed has an ADR recording the fallback decision and the author's approval.
- [ ] App launches to interactive in < 2 s on the dev machine (**Tier 2 target**; the governing budget is §2.3's < 4 s on Tier 0).
- [ ] IPC types are generated, not hand-written; changing a Rust command signature breaks the TypeScript build.
- [ ] Tier detection is correct on the dev machine and on a deliberately-constrained run (simulate Tier 0 via override).
- [ ] Idle RAM < 200 MB on the dev machine (**Tier 2 target**; the governing budget is §2.3's < 250 MB on Tier 0).
- [ ] A deliberately-triggered panic writes a crash log and shows a graceful error screen.

**Learning note.** What Tauri actually is (webview + Rust process, not a browser); how IPC works and why the type generation matters; Rust's `Result` and error handling; what React strict mode does.

---

### Phase 2 — Design System and Visual Language
**Depends on:** 1. **Sessions:** 1–2.

**Goal.** The complete visual language, built and documented, before any product UI exists.

**Deliverables.** All Section 9 tokens as CSS custom properties, consumed by a Tailwind theme extension. Fonts bundled locally. A component gallery route (`/design`, dev-only) rendering every primitive in every state. Primitives: `Button` (primary/secondary/ghost/danger × sizes × loading/disabled), `IconButton`, `Input`, `Select`, `Toggle`, `Slider`, `Tabs`, `Tooltip`, `Popover`, `Dialog`, `Toast`, `Skeleton`, `Spinner`, `Badge`, `ProgressBar`, `Rating`. Media primitives: `PosterCard`, `EpisodeCard`, `ChannelCard`, `Rail` (virtualised, momentum scroll, edge-bleed, keyboard-navigable), `HeroBanner`, `EmptyState`. A focus-management system with visible rings and correct tab order. Full keyboard navigation infrastructure including a `Ctrl+K` command palette shell. `prefers-reduced-motion` support throughout. A contrast audit script that fails CI if any token pair used for text drops below AA.

**Exit criteria.**
- [ ] Every primitive renders correctly in the gallery, in all states.
- [ ] The entire gallery is navigable by keyboard alone with visible focus at every step.
- [ ] Contrast audit passes.
- [ ] A rail of 500 poster cards scrolls at 60 fps with no dropped frames.
- [ ] `docs/specs/design-system.md` documents every token and component with usage rules.

**Learning note.** Why a design system before features; what design tokens are; how CSS custom properties enable theming; virtualisation and why rendering 500 DOM nodes is a mistake.

---

### Phase 3 — Data Layer and Portable Storage
**Depends on:** 1. **Sessions:** 1–2.

**Goal.** The database schema everything else depends on, designed once, correctly.

**Deliverables.** SQLite via `sqlx` with WAL mode, in portable `./data/` by default with an installed-mode option. A migration system with forward and backward migrations, tested. Core schema: `media_items` (the generic type from §6.2), `external_ids`, `titles` (multi-language, romaji/native/english variants), `people`, `credits`, `genres`, `keywords`, `series`, `seasons`, `episodes` (with both seasonal and absolute numbering columns and a reconciliation table), `collections`, `profiles`, `watch_events`, `playback_positions`, `watchlist_items`, `local_files`, `local_file_matches`, `sources_config`, `settings`. Repository-pattern access layer — no raw SQL outside `persistence/`. A generic `media_kind` discriminator with `film | episode | series | anime_film | anime_series | live_channel | manga_chapter | comic_issue` so Phases 24–25 need no migration. Backup-on-migrate, and an export/import of the whole profile as a portable archive.

**Exit criteria.**
- [ ] All migrations run forward and backward cleanly against a populated database.
- [ ] Schema documented with an entity-relationship diagram in `docs/ARCHITECTURE.md`.
- [ ] A database populated with 500,000 synthetic media items answers **indexed lookups in under 100 ms**. (The 100 ms budget is the lookup alone; bulk insertion of the 500,000 rows has no time budget and is measured and recorded separately in `docs/PERFORMANCE.md`.)
- [ ] Copying the app folder to another location and launching it preserves all data.
- [ ] An ADR records why the schema is generic over media kind.

**Learning note.** Relational schema design; why `sqlx`'s compile-time checking matters; what WAL mode is; migrations; the repository pattern; why designing for manga now costs nothing and retrofitting later costs weeks.

---

### Phase 4 — Metadata Backbone
**Depends on:** 3. **Sessions:** 2–3.

**Goal.** A local catalogue of hundreds of thousands of titles that works offline, enriched live on demand.

**Deliverables.** `tools/ingest/`: a pipeline that downloads IMDb datasets, normalises them, joins TMDB data, ingests AniList's anime catalogue, and populates the database. Resumable — it must survive interruption. Progress reporting. A first-run flow that either ships a prebuilt index or builds it in the background with a good progress UI (the app is usable during the build, searching what's ingested so far). Live API clients for TMDB, AniList, Jikan, Fanart.tv with: a shared rate limiter, exponential backoff, a persistent response cache with sensible TTLs per resource type, and graceful offline behaviour. Image handling: lazy fetch, disk cache with a size budget, WebP re-encoding, blurhash placeholders. External-ID cross-mapping (TMDB ↔ IMDb ↔ AniList ↔ MAL) with conflict resolution rules.

**Exit criteria.**
- [ ] Full ingestion completes on the dev machine and the resulting database is under a documented size budget.
- [ ] Ingestion killed mid-run resumes correctly.
- [ ] Catalogue lookups work with the network disconnected.
- [ ] Rate limits are never exceeded under a stress test of 1,000 rapid lookups.
- [ ] Anime titles resolve across AniList and TMDB with correct ID mapping for a hand-checked set of 50 titles including tricky cases (long-running shonen, split-cour seasons, films tied to series).
- [ ] **The catalogue is fully usable with no TMDB key** (ADR-0013): titles, years, runtimes, genres, cast, crew and ratings all present from IMDb + MovieLens alone. TMDB enrichment adds artwork and rich detail and is verified to be additive, never load-bearing.
- [ ] **The embedding artefact is produced and published** (ADR-0014) by a reproducible script in `tools/ingest/`, run on the author's machine. It is deterministic, checksummed, resumable, and records model identity, quantisation, embedding dimension, document-builder version and catalogue snapshot date. The application refuses to load an artefact whose model identity does not match its own, and degrades to FTS5-only search when the artefact is absent.

**Learning note.** ETL pipelines; why offline-first beats API-first here; rate limiting and backoff; caching strategy and TTL choice; the anime metadata problem specifically.

---

### Phase 5 — Semantic Search Engine
**Depends on:** 4. **Sessions:** 2.

**Goal.** Search that understands meaning, is instant, works offline, and never gets an exact title wrong.

**Deliverables.** FTS5 index over titles, alternative titles, people, and keywords, with BM25 ranking and trigram fuzzy matching for typos. `ort` (ONNX Runtime) running a quantised sentence-transformer; a document text builder that composes each item's embedding input from synopsis, genres, keywords, director, mood descriptors, and era. HNSW index over the embeddings, persisted to disk, memory-mapped. **Reciprocal rank fusion** combining BM25 and vector results, with an exact-title short-circuit that guarantees a literal title match always ranks first. Query understanding: detect and extract structured filters from natural language (year ranges, "in Japanese", "under 100 minutes", "directed by") and apply them as constraints rather than as embedding input. Tier 0 path: **catalogue document** embeddings precomputed and downloaded, never generated on device — but the user's query *is* embedded on device on every tier, since that is one forward pass over ~30 tokens (§8, ADR-0015). The 80 ms p95 budget below includes it. Search UI: instant results as you type, grouped by kind, with keyboard navigation and a "why this matched" hint on semantic results. `fixtures/search/` corpus and the relevance eval harness.

**Exit criteria.**
- [ ] p95 keystroke-to-results < 80 ms over the full catalogue.
- [ ] Exact-title top-1 rate is 100% on the fixture corpus.
- [ ] nDCG@10 > 0.75 on the semantic query set.
- [ ] "films about grief that aren't depressing" and "like Wong Kar-wai but Korean" both return defensible results — documented with screenshots in the case study.
- [ ] Works fully offline.

**Learning note.** What an embedding is, geometrically; BM25 in one paragraph; why hybrid search beats either alone; approximate nearest neighbour and the HNSW trade-off; reciprocal rank fusion; nDCG.

---

### Phase 6 — Source Resolver and Addon Protocol
**Depends on:** 3. **Sessions:** 1–2.

**Goal.** The interface every source in the system will implement, forever. Get this right.

**Deliverables.** The `SourceBackend` trait (§6.1) and its types: `MediaQuery`, `SourceCandidate` (with quality, language, codec, size, health, provenance), `PlayableStream`, `BackendCapabilities`. A resolver that queries backends concurrently with per-backend timeouts, deduplicates, and merges results. Backend registry with enable/disable, ordering, and per-backend health tracking. **The addon manifest format**: a declarative TOML/JSON schema describing a source, published as `crates/source-protocol` with a JSON Schema and a specification document in `docs/specs/addon-protocol.md`. Three reference backends: `LocalFileBackend` (stub until Phase 12), `InternetArchiveBackend` (real, working, legal), `DirectHttpBackend` (user-supplied URLs). An addon installation UI: paste a manifest URL, see what it declares, confirm, enable — with plain-language explanations and no raw JSON in the primary path. A catalogue browser that fetches a user-supplied catalogue URL; **no default URL ships**.

**Exit criteria.**
- [ ] Adding a new backend requires implementing one trait and registering it — no changes elsewhere.
- [ ] The Internet Archive backend returns real, playable results.
- [ ] A malformed or hostile addon manifest is rejected with a clear error and cannot crash or hang the app.
- [ ] Backend timeout: one slow backend never delays results from the others.
- [ ] `docs/specs/addon-protocol.md` is complete enough that a third party could implement a backend from it alone.
- [ ] Repository-wide grep confirms zero indexer names or URLs anywhere, including tests and docs.

**Learning note.** Trait objects and `async_trait` in Rust; designing an interface you can't change later; concurrent queries with timeouts; why declarative-over-executable matters for untrusted input.

---

### Phase 7 — Torrent Engine and Streaming Server
**Depends on:** 6. **Sessions:** 2–3.

**Goal.** Torrents that stream, not torrents that download. This is the hardest engineering in the project.

**Deliverables.** `librqbit` integrated in-process. Magnet and `.torrent` parsing, DHT, peer discovery, trackers. **The streaming scheduler** — the heart of it: a deadline-driven piece prioritisation window that keeps the pieces just ahead of the playhead at highest priority, maintains a rarest-first background fetch for the rest, and re-prioritises within 2 s when the user seeks to an unbuffered region. A local HTTP server exposing an in-progress torrent as a seekable stream with correct HTTP range support, so libmpv can treat it as an ordinary file. File selection within multi-file torrents (pick the episode, ignore the sample and the readme). Swarm health probing: measure achievable throughput on the first pieces and expose it. Bandwidth limits, connection limits, and seeding policy, all user-configurable with sane defaults. Session persistence across restarts. Per-torrent state and progress events streamed to the frontend.

**Exit criteria.**
- [ ] Playback begins in < 8 s on a healthy swarm (measured against a legal, well-seeded Internet Archive or Linux ISO torrent).
- [ ] Seeking to an unbuffered region resumes playback in < 5 s.
- [ ] Sequential mode never starves — the buffer ahead of the playhead is maintained under a documented bandwidth floor.
- [ ] Correct behaviour on a dead swarm: clear, fast failure, not an indefinite hang.
- [ ] Range requests are byte-correct — verified by streaming a file and comparing its hash to a full download.
- [ ] `docs/specs/streaming-scheduler.md` explains the algorithm with a diagram.

**Learning note.** How BitTorrent works — pieces, swarms, DHT, rarest-first; why sequential download conflicts with swarm health and how the scheduler balances them; HTTP range requests; async Rust and `tokio`; backpressure.

---

### Phase 8 — Player Core — **MILESTONE: FIRST DEMOABLE BUILD**
**Depends on:** 5, 7. **Sessions:** 2–3.

**Goal.** Search for something, press play, watch it. The vertical slice that proves the whole thing works.

**Deliverables.** libmpv embedded and rendering into the Tauri window, with hardware decoding enabled and a software fallback path. A custom player UI drawn over it — not mpv's OSD: play/pause, a scrub bar with buffered-range and torrent-download indication, volume, time, fullscreen, audio-track and subtitle-track selectors, playback speed. Full keyboard control (space, arrows, `J`/`K`/`L`, `F`, `M`, `,`/`.` frame step, `[`/`]` speed). Chrome auto-hide. Position saving on exit and resume on reopen. A minimal detail page: poster, synopsis, cast, and a play button that goes through the resolver. Wire search → detail → resolve → play end to end.

**Exit criteria.**
- [ ] A user can search, click, and watch — from a local file, from the Internet Archive backend, and from a configured P2P source.
- [ ] Playback of 4K HEVC HDR uses hardware decode (verified in logs and by CPU usage).
- [ ] Seeking is frame-accurate and responsive.
- [ ] Position resume works across app restarts.
- [ ] **A demo GIF of this flow exists and is in the README.**
- [ ] Tag `phase-08`, and celebrate — this is the first thing worth showing anyone.

**Learning note.** How video playback actually works — container, streams, codecs, decode, present; what hardware decode does; why embedding a player beats building one; the mpv property/command API.

---

### Phase 9 — Intelligent Source Selection
**Depends on:** 8. **Sessions:** 1–2.

**Goal.** The user picks a language and a quality. The app does everything else, invisibly and well.

**Deliverables.** A scoring function over `SourceCandidate`s combining: swarm health (seeders, peers, measured throughput), size-versus-quality plausibility (catching fakes and mislabels), release-group and encoding reputation heuristics, codec and HDR compatibility with the user's hardware, audio-track languages present, subtitle presence, and local availability (a local file always wins). Fake and mislabel detection: a 700 MB "4K" file is a lie; a 60-minute file for a 140-minute film is a lie. Language selection as a **first-class UI concept** — a Netflix-style language dropdown listing available audio languages, which resolves to whichever candidate actually carries that audio track, with dub/sub distinction for anime. Quality selection the same way, defaulting to the best the hardware and connection support. Automatic fallback: if the top candidate fails to start within a timeout, silently try the next, and log why. The hidden expert panel (`Ctrl+Shift+S` and a Settings toggle) showing the full ranked candidate list with every score component — this is both a debugging tool and a compelling demo. A "why this source" one-liner under the play button.

**Exit criteria.**
- [ ] Scoring is a pure function with unit and property tests over a fixture set of candidate lists.
- [ ] Selecting "Hindi" plays a source with a Hindi audio track, or clearly reports that none is available.
- [ ] Selecting 4K plays genuine 4K, or explains why it can't.
- [ ] A deliberately-failing top candidate falls back within 10 s with no user action.
- [ ] The expert panel's score breakdown is legible and matches the code.
- [ ] `docs/specs/source-scoring.md` documents every term in the scoring function and its weight, with justification.

**Learning note.** Designing a scoring function; why weights need justification and testing; the fake-detection heuristics; how to build an abstraction that hides complexity without hiding failure.

---

### Phase 10 — Subtitle Pipeline
**Depends on:** 8. **Sessions:** 2.

**Goal.** Subtitles are in sync on the first frame, always, with no user action. Ever.

**Deliverables.** The priority chain from Section 14: (1) embedded tracks in the media file — the default and most common path, always perfectly synced; (2) sidecar files found next to the media or inside the torrent; (3) OpenSubtitles hash match, quota-budgeted and cached; (4) other free providers; (5) alignment of whatever was found. **The aligner** (`crates/subtitle-align`, a standalone crate): extract a voice-activity signal from the audio track via FFmpeg + a VAD, build a comparable signal from the subtitle timings, and solve for offset *and* framerate scale by cross-correlation — handling the 23.976↔25 fps drift that a pure offset cannot. Confidence scoring; refuse to apply a low-confidence alignment and fall back rather than making things worse. A live `±` nudge in the player (`G`/`H` keys) that persists per file and per subtitle. Styling controls: font, size, position, background, and proper ASS/SSA rendering preserved when the source uses it. Language preference ordering per profile. `fixtures/subtitles/` corpus and the alignment eval harness. Tier 0: chain steps 1–3 plus the duration heuristic and the manual nudge; alignment is Tier 1+.

**Exit criteria.**
- [ ] > 90% of the fixture corpus aligns to within 150 ms.
- [ ] A 23.976 vs 25 fps mismatch is detected and corrected, not just offset.
- [ ] Embedded tracks are found and used without any network request.
- [ ] The OpenSubtitles daily quota is respected and never exceeded; quota state is visible in Settings.
- [ ] The manual nudge persists across sessions for the same file.
- [ ] `crates/subtitle-align` is independently testable and documented as a standalone crate.

**Learning note.** Voice activity detection; cross-correlation as a way to align two signals; why framerate scaling is a multiplication and offset is an addition, and why you need both; confidence thresholds and knowing when not to act.

---

### Phase 11 — Player Experience Layer
**Depends on:** 8, 9, 10. **Sessions:** 2.

**Goal.** The player becomes better than the commercial ones.

**Deliverables.** **The pause overlay** — the signature feature. On pause, a Prime-X-Ray-inspired panel slides in over a dimmed, blurred frame: full cast with headshots, director and key crew, ratings (IMDb, TMDB, and AniList for anime — **not** Rotten Tomatoes, which has no free API and whose scraping would violate its terms), runtime remaining, the current chapter, trivia and goofs, soundtrack, and "more like this". Clicking a cast member opens their filmography. It must be beautiful — this is the screenshot people will remember. (Face recognition arrives in Phase 22; until then it shows the full cast list, well-designed.) Next-episode handling with a countdown and a preview card. Episode navigation within the player. Chapter markers on the scrub bar. Thumbnail previews on scrub-bar hover, generated in the background via FFmpeg and cached. Picture-in-picture. Audio delay control. Video filter controls (brightness, contrast, saturation) for badly-encoded sources. Statistics-for-nerds overlay showing decode path, dropped frames, bitrate, and swarm state.

**Exit criteria.**
- [ ] The pause overlay is visually excellent and appears in < 200 ms.
- [ ] Scrub thumbnails generate without stuttering playback.
- [ ] Next-episode autoplay works and is disableable.
- [ ] All player features are keyboard-accessible and documented in a shortcuts overlay (`?`).
- [ ] Manual test plan for playback passes on all three tiers.

**Learning note.** Why the overlay data is available but scene-level data isn't; background work without blocking playback; how thumbnail sprites work.

---

### Phase 12 — Local Library Engine
**Depends on:** 4. **Sessions:** 2–3.

**Goal.** Point it at a folder. It figures out what everything is, correctly.

**Deliverables.** Folder registration with recursive scanning and live filesystem monitoring (`ReadDirectoryChangesW` via `notify`), handling moves, renames, and deletions without a rescan. `crates/filename-parser`: a standalone, well-tested parser extracting title, year, season, episode, absolute episode number, resolution, source (BluRay/WEB/etc.), codec, audio, language, and release group from real-world filenames — including scene releases, anime fansub conventions, and multi-episode files. **Multi-signal matching**: combine parsed filename data with actual file evidence — duration from FFprobe, resolution, audio track languages, embedded container title metadata, folder context, sibling files — and score candidates from the catalogue. A tunable confidence threshold. **The review queue**: only genuinely ambiguous items surface, presented as a side-by-side poster comparison resolved with one click, with the option to search manually. Decisions are remembered and used to improve future matching of similar names. Batch resolution for series where one decision settles many files. `fixtures/filenames/` corpus (500+ hand-labelled cases) and the identification eval harness.

**Exit criteria.**
- [ ] > 95% top-1 accuracy on the fixture corpus.
- [ ] < 1% false-confident rate — being wrong while confident is the worst failure mode here.
- [ ] Scanning 10,000 files completes in under 3 minutes and does not block the UI.
- [ ] Renaming a file outside the app is detected and does not create a duplicate.
- [ ] The review queue is genuinely pleasant to use — resolving 20 ambiguous items takes under a minute.
- [ ] `crates/filename-parser` is documented and independently useful.

**Learning note.** Parsing messy real-world input; why multiple weak signals beat one strong one; confidence thresholds and the asymmetry between being wrong and being unsure; filesystem watching on Windows.

---

### Phase 13 — Download Manager and the Stream-vs-Download Advisor
**Depends on:** 7, 9, 12. **Sessions:** 1–2.

**Goal.** Downloading is first-class, and the app tells you honestly which is the better choice.

**Deliverables.** A download manager: queue with priorities, pause/resume/cancel, per-download and global bandwidth limits, scheduling (start after midnight), completion notifications, and automatic filing into the local library so a completed download is indistinguishable from a file you already had. **The advisor**, combining all four signals the author asked for: (1) measured swarm health versus the bitrate the file requires, producing a stall-probability estimate; (2) free disk space and existing local availability; (3) learned network conditions by time of day, suggesting overnight downloads for large files; (4) taste-model input — something predicted to be a favourite and a likely rewatch is worth the disk, something being sampled is not. Presented as a single clear recommendation with a one-line reason, not a wall of statistics, with the other option always one click away. Storage management: a library size budget, a "cleanup" view showing watched downloads by size and age, and per-item keep/delete decisions.

**Exit criteria.**
- [ ] The advisor's recommendation is correct (matches what actually happens) in at least 8 of 10 manual trials across varied swarm conditions.
- [ ] Downloads survive an app restart and resume.
- [ ] A completed download is automatically matched and appears in the local library with no user action.
- [ ] Bandwidth limits are actually respected, verified by measurement.
- [ ] The recommendation reason is a single readable sentence.

**Learning note.** Estimating whether a stream will stall; bitrate versus throughput; why the taste model belongs in this decision.

---

### Phase 14 — Profiles and First-Run Onboarding
**Depends on:** 3, 5. **Sessions:** 2.

**Goal.** A three-screen setup that a non-technical person completes in under two minutes, and that gives the recommender everything it needs.

**Deliverables.** Profile system: creation, avatar, optional PIN, Netflix-style picker on launch, fast switching. Full isolation of taste model, history, watchlist, continue-watching, and playback preferences; shared downloaded files and metadata cache. A kids profile with content-rating limits. **The onboarding wizard**, three steps: (1) create profile; (2) taste — **defaulting to the three methods that need no artwork** (ADR-0013), since the app is fully functional with no TMDB key and a poster grid needs posters: import from Letterboxd CSV, Trakt, IMDb ratings export, or MAL/AniList XML; taste statements about pace, ambiguity, era, subtitles, runtime, and formal experimentation — *not* genre checkboxes; and a free-text "describe what you love" box embedded directly into the taste vector. The **adaptive poster grid** (3–4 rounds, each adapted to previous picks, choices spread to maximise information gain) appears as an additional round **only once artwork is available**. Where artwork is absent the equivalent surface is a **typographic title-card grid** — title, year, director, set in the §9.2 display serif — which for a cinephile app is arguably the better design anyway: it selects on knowledge and taste rather than poster recognition. Offering the optional ~30-second TMDB step sits here, framed as *unlock artwork and rich detail*, with a "later" that is not a dead end; (3) sources — optional, one paste-a-URL field with plain-language help and a "later" option that is not a dead end. **The app must be fully usable if step 3 is skipped**, via local files and the Internet Archive backend. Add-a-local-folder is offered inline in step 3 as the zero-configuration path.

**Exit criteria.**
- [ ] A first-time user with no technical knowledge reaches a populated Home screen in under two minutes. Test this on an actual person.
- [ ] A Letterboxd export of 500+ films imports correctly and is reflected in the taste model.
- [ ] Skipping the sources step leaves a functional, non-empty app.
- [ ] **Completing onboarding with no TMDB key produces a working, populated, good-looking app** (ADR-0013). Every artwork-bearing surface has a designed artwork-free state — a typographic fallback, never a grey rectangle.
- [ ] Profile switching is instant and leaks no state between profiles.
- [ ] Every onboarding screen is beautiful — this is the first impression.

**Learning note.** Cold-start in recommender systems; why an import beats a questionnaire; information gain in adaptive questioning; why taste statements beat genre checkboxes.

---

### Phase 15 — Taste Model
**Depends on:** 5, 14. **Sessions:** 2.

**Goal.** A rich, honest representation of what a person likes, built from what they actually do.

**Deliverables.** Signal collection with weights: completed (strong positive), abandoned early (strong negative, with the abandon point recorded), rewatched (very strong), watchlisted, explicitly rated, searched for, viewed detail page without playing (weak negative), time-of-day and session-length context. **Multi-vector taste representation** — not one average vector, because a person who loves both Tarkovsky and slasher films is not well described by their midpoint. Cluster the user's positive signals into taste *modes* (typically 3–8), each with its own vector, weight, and recency. Explicit axis extraction alongside the vectors: preferred eras, runtimes, countries, languages, subtitle tolerance, pacing, directors, and — importantly — *negative* preferences learned from abandons. Temporal decay so the model tracks a changing person, with a longer half-life for strongly-held preferences. A **taste profile screen** where the user can see their own model in plain language ("You gravitate toward slow, formally rigorous films from East Asia, and separately toward 1980s American genre cinema") and correct it — this is both a trust feature and a great demo.

**Exit criteria.**
- [ ] The model produces distinct, recognisable clusters for a synthetic profile with two deliberately unrelated tastes.
- [ ] Abandonment is weighted correctly — abandoning at 5 minutes and at 80 minutes mean different things.
- [ ] The taste profile screen describes a test profile in a way the author agrees is accurate.
- [ ] Temporal decay is tested with a synthetic profile whose taste shifts over simulated time.
- [ ] The model updates within 2 s of a watch event, without blocking the UI.

**Learning note.** User modelling; why a single average vector fails; clustering; implicit versus explicit signals; temporal decay and half-lives; the value of letting a user see and edit their own model.

---

### Phase 16 — Recommendation Engine
**Depends on:** 15. **Sessions:** 2–3.

**Goal.** Recommendations that are right, that go beyond the obvious, and that can be explained.

**Deliverables.** **Layer 1 — content-based**: similarity in embedding space against each taste mode, filtered and boosted by the explicit axes from Phase 15. **Layer 2 — item-item collaborative filtering**: precompute an item-item similarity matrix offline from MovieLens (and AniList/IMDb rating signals), shipped or built during ingestion, giving genuine "people who loved this also loved" signal with no server and no other users. Handle the popularity bias that raw co-occurrence produces — normalise so that a beloved obscure film can surface above a mediocre blockbuster. **Layer 3 — the hybrid ranker**: blend both signals with candidate generation (retrieve ~1,000 candidates cheaply, rank precisely), apply diversity constraints (maximal marginal relevance so a rail isn't eight films by one director), availability weighting (something you can actually watch right now ranks higher), and freshness. **Explanations**: every recommendation carries a human-readable reason grounded in the model ("Because you finished three Béla Tarr films", "Loved by people who share your taste in 1970s political thrillers"). Cold-start handling for items with no ratings. `fixtures/recommender/` and the recommender eval harness.

**Exit criteria.**
- [ ] Recall@20 beats a popularity baseline by > 40% **relative** on a held-out split — i.e. `(recall_model / recall_baseline) - 1 > 0.40`, not 40 percentage points.
- [ ] Catalogue coverage > 15% — the recommender does not only ever surface the same 2,000 films.
- [ ] Intra-list diversity meets the documented target; no rail is dominated by one director, franchise, or year.
- [ ] Every recommendation has a truthful explanation.
- [ ] Ranking 1,000 candidates completes in < 150 ms on Tier 0.
- [ ] Case study written with real numbers.

**Learning note.** Collaborative versus content-based filtering; the item-item matrix and why it works without other users; popularity bias and how to correct it; candidate generation versus ranking; maximal marginal relevance; Recall@K and coverage.

---

### Phase 17 — Discovery Engine
**Depends on:** 16. **Sessions:** 2.

**Goal.** The thing that makes this app worth using: recommendations that are *surprising and right*.

**Deliverables.** A **contextual bandit** over recommendation strategies. Arms include: safe-familiar, adjacent-stretch (one axis away from a taste mode), cross-mode bridge (something sitting between two of the user's clusters), blind-spot (a well-regarded region of the catalogue the user has never touched — a country, a decade, a movement), canon-gap (acclaimed work the user's taste predicts they'd like but hasn't seen), and deep-cut (highly-rated obscurity matching a taste mode). Reward signal from actual outcomes: played, completed, abandoned-early, watchlisted, dismissed. **The exploration budget**: a user-facing "Comfort ↔ Adventure" slider setting the baseline, adapted by the bandit based on how the user actually responds — finish the difficult ones and it pushes further; bail repeatedly and it eases off. Rails are labelled with honest intent, and the labels are part of the product: *"Because you loved Stalker"*, *"A stretch — but we think you're ready"*, *"Your blind spot: Iranian New Wave"*, *"Beloved by people like you, seen by almost no one"*, *"You've never watched anything from 1974"*. A dismissal mechanism ("not for me", "not right now", "already seen") that feeds back properly. Guard against degenerate behaviour: never recommend the same item in two rails; never let one strategy dominate; enforce a minimum exploration floor so the model can't collapse into a bubble even if the user always picks the safe option.

**Exit criteria.**
- [ ] The bandit demonstrably shifts its strategy distribution in simulation when reward patterns change.
- [ ] The exploration floor holds: even a maximally-conservative simulated user still receives genuine discovery.
- [ ] Rail labels accurately describe why each item is there.
- [ ] Novelty metric (mean inverse popularity of recommendations) meets its target while relevance is maintained.
- [ ] The author tests it on their own taste and finds at least three films they hadn't heard of and want to watch. **This is the real acceptance test.**
- [ ] Case study written — this is the flagship one.

**Learning note.** Multi-armed and contextual bandits; the exploration/exploitation trade-off; Thompson sampling or UCB, whichever is used, explained properly; why filter bubbles are an optimisation failure, not an inevitability; how to evaluate serendipity.

---

### Phase 18 — Browsing Surfaces
**Depends on:** 17. **Sessions:** 2–3.

**Goal.** Home, Films, and TV Shows become the beautiful, navigable front of the whole system.

**Deliverables.** **Home**: Continue Watching first (with progress, next episode, and a remove option), then adaptive rails generated by the discovery engine, ordered by predicted engagement, with an optional hero for a single strong recommendation. **Films** and **TV Shows**: Continue Watching scoped to that kind, then browsable structure — by genre, decade, country, language, director, movement, collection, awards, and curated lists — with real filtering and sorting. **Every rail has a "see all"** that opens a proper browsable grid with filters, honouring the anti-pattern rule that nothing is more than three interactions from Home. Detail pages: hero artwork, synopsis, cast and crew with links, ratings, availability badges (local / streamable / downloadable, with quality and language), episode lists with progress for series, related and "more like this", and a prominent play button with the language and quality selectors from Phase 9. Hover previews: muted, 400 ms dwell, still-frame-then-motion, disableable. A Live Channels tab with an honest, well-designed empty state pointing to Phase 24. Everything virtualised, everything keyboard-navigable, everything at 60 fps.

**Exit criteria.**
- [ ] Home renders in < 800 ms on Tier 0 with 20 rails.
- [ ] Anything in the catalogue is reachable in ≤ 3 interactions from Home.
- [ ] Every rail's "see all" goes somewhere genuinely useful.
- [ ] Full keyboard navigation across all browsing surfaces.
- [ ] Screenshots of Home, Films, and a detail page are good enough for the README.

**Learning note.** Virtualised rendering; how rail ordering is itself a ranking problem; why hover previews are a performance trap; information architecture.

---

### Phase 19 — Watchlist and External Sync
**Depends on:** 18. **Sessions:** 1–2.

**Goal.** The watchlist becomes a tool a cinephile actually uses, connected to the services they already use.

**Deliverables.** Watchlist with multiple named lists, drag-and-drop ordering, priority, notes, and tags. Smart views: "available now", "hard to find", "short enough for tonight", "leaving soon" where determinable, "matches your current mood". Availability tracking — the watchlist knows what has become newly available and surfaces it. **Two-way sync**: Trakt (watch history, ratings, collection, and live scrobbling during playback), Letterboxd (CSV import and export — no public write API), IMDb ratings export import, MAL/AniList (import and export, with anime list status mapping). Conflict resolution rules for two-way sync, documented and tested. Sync status UI showing what synced, when, and any conflicts.

**Exit criteria.**
- [ ] Trakt scrobbling correctly marks items watched at the right threshold.
- [ ] A round-trip Letterboxd export → import → export preserves all data.
- [ ] Sync conflicts are detected and resolved per the documented rules, never silently losing data.
- [ ] Smart views return sensible results on a real watchlist.
- [ ] Sync failures degrade gracefully and never block the app.

**Learning note.** OAuth device flow; two-way sync and conflict resolution; why sync is harder than it looks; idempotency.

---

### Phase 20 — Windows Platform Integration
**Depends on:** 12, 18. **Sessions:** 2–3.

**Goal.** It stops feeling like a web app in a window and starts feeling like a Windows application.

**Deliverables.** **Explorer context menu**: right-click any video file → "Play in sin-e-phile" / "Add to library" (registered properly, and cleanly removable). **File associations**: optional handler registration for `.mkv`, `.mp4`, `.avi`, `.mov`, `.webm`, and — importantly — the `magnet:` protocol, so a magnet link clicked anywhere opens sin-e-phile. **Watched folders** with live `ReadDirectoryChangesW` monitoring (from Phase 12), surfaced properly in Settings. **Taskbar and shell**: jump list with Continue Watching entries and quick actions; taskbar progress during downloads; thumbnail toolbar with playback controls; **System Media Transport Controls** so keyboard media keys, the Windows volume flyout, and Bluetooth remotes all control playback with correct now-playing metadata and artwork. **Deep shell integration** (the ambitious part): an Explorer thumbnail/preview handler for library files, rich property columns (title, year, watched status, rating), and watched/unwatched icon overlays on library folders. Single-instance enforcement with argument forwarding. Optional start-with-Windows. Proper high-DPI and multi-monitor handling. Windows notifications for completed downloads and newly-available watchlist items.

**Exit criteria.**
- [ ] Every registration is cleanly reversible; uninstalling leaves no orphaned registry entries. Verify this.
- [ ] Media keys control playback with correct metadata in the Windows volume flyout.
- [ ] Clicking a magnet link in a browser opens sin-e-phile and starts resolving.
- [ ] Icon overlays appear in Explorer for library files. (If the overlay-handler slot limit blocks this, document the limitation honestly and ship the rest.)
- [ ] Correct rendering at 100%, 150%, and 200% DPI, and across two monitors with different scaling.

**Learning note.** Windows shell extensions and COM; the registry and why cleanup matters; SMTC; DPI awareness; the shell icon overlay limit and why so many apps fight over it.

---

### Phase 21 — Performance Engineering and the Low-End Path
**Depends on:** 18 (20 optional). **Sessions:** 2.

> Phase 20 is a *soft* dependency: with no shell integration built, there is simply no shell-integration cost to profile. This keeps Tier B (Appendix E) reachable without Tier C.

**Goal.** Meet every budget in Section 2.3 on a genuinely weak machine.

**Deliverables.** Profiling of every hot path — startup, search, home render, playback start, library scan, recommender ranking — with before/after numbers recorded. Startup optimisation: lazy module initialisation, deferred index loading, a fast path to first paint. Memory: audit every cache, add explicit budgets and eviction, verify no leaks over a long session. Database: query plan review, index audit, batching, prepared-statement reuse. Frontend: bundle analysis and code splitting, render profiling, memoisation where measured (not speculatively), image decode off the main thread. Tier 0 path fully implemented and *tested on real constrained hardware or a hard-constrained VM* — 4 GB RAM, 2 cores, no hardware decode. Benchmarks added to CI with regression thresholds so performance can't silently rot. A `docs/PERFORMANCE.md` recording every budget, current measurement, and how it was achieved.

**Exit criteria.**
- [ ] Every budget in Section 2.3 met on the constrained test configuration.
- [ ] No memory growth over a 4-hour session with continuous playback.
- [ ] CI fails on a > 10% regression in any tracked benchmark.
- [ ] The Tier 0 experience is genuinely good, not merely functional — verified by the manual test plan.

**Learning note.** How to profile rather than guess; where time actually goes in a desktop app; caching and eviction; why premature memoisation in React makes things slower; regression testing performance.

---

### Phase 22 — Vision Layer (Tier 2)
**Depends on:** 11. **Sessions:** 1–2.

**Goal.** The pause overlay knows who is on screen.

**Deliverables.** On pause (Tier 2 only): capture the current frame, run face detection, generate embeddings for detected faces, and match against embeddings precomputed from the title's cast headshots (fetched from TMDB, cached per title, computed lazily on first play). Show the matched actors prominently in the overlay with character names, with the full cast still available below. All models via ONNX Runtime; a small face detector plus a face-recognition embedding model, both quantised. Strict budget: the whole pass must complete in under 400 ms or it is abandoned and the overlay shows the full cast list. Confidence threshold — showing the wrong actor is worse than showing none. Models are an optional download, not bundled, with a clear size and purpose shown before downloading. Graceful and complete absence on Tier 0 and 1.

**Exit criteria.**
- [ ] Correct identification on a hand-checked set of 50 paused frames across varied films, lighting, and eras.
- [ ] The pass never blocks the UI and never exceeds 400 ms.
- [ ] Wrong identifications are rarer than 2% — below-threshold matches show nothing rather than guessing.
- [ ] Tier 0 and Tier 1 users see a well-designed cast panel with no hint anything is missing.
- [ ] Models are optional and the app is fully functional without them.

**Learning note.** Face detection versus face recognition; embeddings again, this time for images; why the confidence threshold matters more than the accuracy number; running ONNX models in Rust.

---

### Phase 23 — Binge Intelligence
**Depends on:** 11, 12. **Sessions:** 1–2.

**Goal.** Watching a series feels as smooth as it does on a commercial streamer.

**Deliverables.** **Intro detection** by audio fingerprinting across episodes of a season — find the common segment, and offer a "Skip Intro" button positioned exactly right. **Recap detection** using the same approach plus subtitle heuristics. **Credit detection** via scene-change and audio analysis, driving a next-episode prompt that appears at the right moment rather than a fixed offset from the end. Chapter extraction from container metadata where present, generated where not. Seamless next-episode transition: prefetch and pre-buffer the next episode during the current one's credits so it starts instantly. Binge state: remembering where you are in a series across sessions and profiles. "Skip intro" preference learned per user — someone who never skips stops being asked.

**Exit criteria.**
- [ ] Intro detection is accurate to within 2 s on a test set of 10 series across different runtimes and structures.
- [ ] Next episode starts in < 1 s when prefetched.
- [ ] Detection runs in the background without affecting playback.
- [ ] Failures are silent — no intro detected means no button, never a wrong button.

**Learning note.** Audio fingerprinting; finding a common subsequence across files; scene-change detection; prefetching and its memory cost.

---

### Phase 24 — Live Channels
**Depends on:** 18. **Sessions:** 2.

**Goal.** The fifth tab becomes real.

**Deliverables.** M3U and M3U8 playlist import from a user-supplied URL or file; Xtream Codes credential support. XMLTV EPG parsing with a full programme guide UI — a proper timeline grid, not a list. Channel logos, categories, country grouping, and search. Free legal FAST services as built-in options the user can enable: Pluto TV, Samsung TV Plus and Plex FAST — offered as *choices*, never defaults, and each added to `tools/guard/allowlist.txt` with its own justification and ADR (ADR-0010). **The `iptv-org` index is not bundled**: it is a URL the user pastes like any other, which is cleaner and consistent with §2.1's rule that no default URL ships. HLS playback through libmpv with adaptive bitrate. Channel favourites and recents. Recording to disk with scheduling from the EPG. Time-shift and pause-live where the stream supports it. Integration with the taste model so live content participates in recommendations where metadata allows.

**Exit criteria.**
- [ ] A user-supplied M3U with 5,000 channels imports and browses smoothly.
- [ ] EPG parses and displays correctly with a usable timeline.
- [ ] Channel switching takes < 3 s.
- [ ] Recording works and produces a file the local library correctly identifies.
- [ ] Dead channels are detected and marked rather than hanging.

**Learning note.** HLS and adaptive bitrate; XMLTV; why live streams break differently from files.

---

### Phase 25 — Manga and Comics
**Depends on:** 3, 5, 16. **Sessions:** 2–3.

**Goal.** The unified taste model extends to reading, using the generic data model built in Phase 3 — with no schema migration.

**Deliverables.** A sixth navigation destination, **Reading**. Format support: CBZ, CBR, CB7, PDF, and folders of images. A reader UI with single page, double page, and webtoon continuous-scroll modes; right-to-left support for manga; zoom and pan; page-fit modes; and a genuinely good keyboard and mouse experience. Progress tracking per chapter and per volume. Local library scanning and matching for comics and manga files, reusing the Phase 12 machinery with a reading-specific parser. Metadata from AniList and MangaDex-style schemas; MAL/AniList reading-list sync via the Phase 19 infrastructure. Online sources via the same `SourceBackend` protocol — the reader is just another consumer of the resolver. The taste model, recommender, and semantic search extended to reading with no architectural change, delivering genuine cross-media recommendations ("you loved this manga; here's the anime, and here's a live-action film with the same preoccupations").

**Exit criteria.**
- [ ] All listed formats open correctly, including malformed archives, without crashing.
- [ ] Webtoon continuous scroll is smooth at 60 fps on Tier 0.
- [ ] **No database migration was required** — proving the Phase 3 design was right.
- [ ] Cross-media recommendations work and are demonstrably sensible.
- [ ] Reading progress syncs to AniList.

**Learning note.** Why the generic data model in Phase 3 paid off here; archive formats; image decoding and memory in a long scroll; cross-domain recommendation.

---

### Phase 26 — Connected Playback
**Depends on:** 11. **Sessions:** 2.

**Goal.** Playback leaves the desktop.

**Deliverables.** **DLNA/UPnP renderer discovery** and casting, with the app as a full remote (transport controls, seek, track selection). **Chromecast** support via the Cast protocol, with transcoding through FFmpeg where the receiver doesn't support the source codec. A local HTTP serving layer so a cast device can pull from the torrent stream. **Watch Together**: synchronised playback with a friend, via a minimal relay (WebRTC data channel with a public signalling fallback, or a tiny optional self-hosted relay — no mandatory server), with play/pause/seek sync, drift correction, and text chat. A join-by-link or join-by-code flow.

**Exit criteria.**
- [ ] Casting works to at least one real DLNA renderer and one Chromecast.
- [ ] Transcoding engages only when required, and is documented.
- [ ] Watch Together keeps two clients within 500 ms of each other over a 90-minute film.
- [ ] Watch Together degrades gracefully when a peer disconnects.
- [ ] No mandatory server component was introduced.

**Learning note.** DLNA/UPnP discovery; the Cast protocol; transcoding versus remuxing; WebRTC data channels; clock synchronisation and drift correction.

---

### Phase 27 — Hardening, Packaging, and Portfolio Finalisation
**Depends on:** everything. **Sessions:** 2–3.

**Goal.** Turn a working app into a project that gets someone hired.

**Deliverables.** A full error-handling audit — every failure path produces a clear, actionable message, never a raw error or a silent failure. Offline behaviour audit across every screen. A first-run experience audit with fresh eyes. Accessibility pass: keyboard completeness, focus order, screen-reader labels on the browsing surfaces, contrast re-verification. A security review: addon manifest validation hardening, path traversal in library scanning, the local HTTP server bound to loopback only with a per-session token, safe handling of all untrusted input. A dependency licence audit with the results documented and the GPL obligations satisfied. `cargo audit` and `npm audit` clean, wired into CI.

**Then the portfolio work, which is the point:**
- README finalised: hero screenshot, demo GIF, the "what makes this interesting" section rewritten to be genuinely good, accurate feature list, honest limitations section.
- A 2–3 minute demo video: search → discovery rail → play → pause overlay → subtitle sync → local library match → download advisor.
- All five case studies complete, with real numbers from the eval harnesses.
- `docs/HOW_IT_WORKS.md` complete and readable end to end by someone who has never seen the project.
- `docs/INTERVIEW_PREP.md` complete, including honest weaknesses.
- Architecture diagrams regenerated and accurate.
- A `docs/ROADMAP.md` of what would come next — signals judgement, not just execution.
- Repository presentation: description, topics, pinned, a clean commit history, tagged phase releases.

**Exit criteria.**
- [ ] A stranger can read the README and understand what this is and why it's hard, in under three minutes.
- [ ] Every eval harness has published, current numbers.
- [ ] The author can answer every question in `INTERVIEW_PREP.md` without notes. **This is the real exit criterion for the entire project.**
- [ ] Fresh-clone build succeeds on a clean Windows machine following only `docs/SETUP.md`.
- [ ] Section 2.1 verified once more: no source URLs, indexer names, or catalogues anywhere in the repository or its history.

**Learning note.** How to present technical work; what employers actually read; why naming your own project's weaknesses is a strength.

---

## 16. Appendices

### A. Phase Dependency Graph

Regenerated from the `Depends on:` line of each phase, which is authoritative. Where this diagram and a phase header disagree, **the header wins and the diagram is wrong** — fix the diagram.

```
0 ──→ 1 ──┬──────────────────→ 2 ─────────────────────────┐
          │                                               │
          └→ 3 ──┬──→ 4 ──→ 5 ────────────┬───────────────┤
                 │                        │               │
                 └→ 6 ──→ 7 ──────────────┴→ 8 ◄──────────┘
                                             │   MILESTONE
                     ┌───────────┬───────────┼───────────┐
                     ↓           ↓           ↓           ↓
                     9          10          11          12
                     │           │           ↑           │
                     │           └───────────┤           │
                     └───────────────────────┘           │
                     │        (11 needs 8, 9, 10)        │
                     └────────────┬──────────────────────┘
                                  ↓
                                 13   (needs 7, 9, 12)


  3 ─┬─→ 14 ──→ 15 ──→ 16 ──→ 17 ──→ 18 ─┬──→ 19
  5 ─┘                                   ├──→ 20 ──┐
                                         ├──→ 21 ◄─┘  (20 optional)
                                         ├──→ 23      (also needs 11, 12)
                                         ├──→ 24
                                         └──→ 25      (also needs 3, 5, 16)

  11 ─┬──→ 22
      └──→ 26

  27 ← everything (run it whenever you stop — see Appendix E)
```

**Corrections made in this regeneration.** Phase 8 depends on **5 and 7**; the previous diagram implied a hard `2 → 8` edge that no phase header states (Phase 2 is a practical prerequisite for a good-looking Phase 8, not a declared one). Phase 14 depends on **3 and 5**; the previous diagram drew `14 → 15 → …` as a disconnected root. Phase 21 depends on **18, with 20 optional** (see its header). Phases 22 and 26 depend on **11**; 23 on **11 and 12**; 25 on **3, 5 and 16**.

Phases 9–13 can be reordered if a session's energy suits one better. Phases 22–26 are genuinely independent of one another and can be tackled in any order, or deferred indefinitely without harming the project — 27 is reachable from 21.

### B. What "done" means

A phase is complete when: every exit criterion is met with evidence recorded in `PROJECT_STATE.json`; all tests and lints pass; the phase retrospective is written; the learning note is written and the author has confirmed they understand it; `HOW_IT_WORKS.md` and the README reflect the new capability; and the branch is merged and tagged.

**The author confirming they understand the learning note is a hard gate.** If they don't, the phase is not done — rewrite the note or simplify the code.

### C. Standing rules for every session

1. Read `SPEC.md` first. It outranks memory.
2. Verify state against the repository, not against the state file.
3. Plan before implementing. Wait for approval.
4. Small commits, conventional messages, always buildable `main`.
5. No source URLs, indexer names, or catalogues. Ever. Anywhere.
6. Explain unfamiliar concepts as they're introduced, in the learning note.
7. Never expand a phase's scope silently.
8. Update state after every subtask, not just at the end.
9. When choosing between clever and clear, choose clear.
10. When something is genuinely uncertain, say so and ask, rather than guessing confidently.
11. Measure before optimising; record the numbers.
12. If a phase's exit criteria can't be met, say so and explain why rather than redefining them.
13. Evidence, not opinion (§10.8). Three attempts then escalate (§10.9). Amend the spec before deviating from it (§2.8).

### D. Risk register

Maintained in `docs/RISKS.md` from Phase 0. Reviewed at the start of any phase whose risks it names. Each has a **trigger** — the observable condition that means the fallback should be taken — decided in advance, so the decision isn't made while frustrated at 2am.

| # | Risk | Likelihood | Impact | Mitigation | Fallback if triggered |
|---|---|---|---|---|---|
| **R1** | **libmpv cannot be cleanly embedded in a Tauri v2 window.** Rendering a native video surface with a webview UI over it is the single fiddliest integration in this project. | Medium | Severe — Phase 8 and everything downstream | Spike A in Phase 1, before anything depends on it. Two approaches tried. | In order: (a) child window positioned and clipped under the webview; (b) libVLC, which has friendlier bindings; (c) HTML5 `<video>` with FFmpeg remuxing, accepting the codec limitations. Each is an ADR. |
| **R2** | **librqbit's streaming control is insufficient** to build the Phase 7 deadline scheduler. | Medium | Severe — the "8 seconds to first frame" promise | Spike B in Phase 1 measures real numbers and audits the API surface. | libtorrent-rasterbar via FFI. Costs a C++ toolchain and binding work, roughly a week, but it is the proven path and every serious client uses it. |
| **R3** | **ONNX Runtime is painful to build on Windows, or too slow on Tier 0.** | Low–Medium | **Moderate/Severe** — semantic search and the taste model. Raised from Moderate by ADR-0015: query embedding now runs on *all* tiers, so an unusable `ort` breaks semantic search everywhere, not only on Tier 0. Spike C gates Phase 5. | Spike C in Phase 1 measures latency and memory on the dev machine, then on a constrained VM. | Ship precomputed embeddings for the whole catalogue and never embed on device (already the Tier 0 design). New items get embeddings on next catalogue refresh. Semantic search survives; on-the-fly embedding of free-text queries would need a smaller model or a hash-based fallback. |
| **R4** | **Catalogue ingestion is far larger or slower than expected** — IMDb plus TMDB plus MovieLens is a lot of data. | Medium | Moderate — first-run experience | Measure in Phase 4 before committing to a shape. Scope by a vote/popularity threshold rather than ingesting everything. | Tiered catalogue: ship a core index of ~200k well-known titles, fetch the long tail live on demand and cache. The "vastest library" promise is met by the source layer, not by the local index. |
| **R5** | **Windows shell integration hits platform limits.** Windows 11 context-menu entries require a packaged (sparse MSIX) app; icon overlay handlers are limited to roughly 15 slots system-wide and cloud-storage apps have already taken them. | **High** | Low — these are enhancements | Research the constraints in Phase 20 *before* implementing, not after. | Ship the legacy context menu (still reachable via "Show more options"), or produce a sparse package purely for shell registration. Drop icon overlays and document why — knowing *why* a platform limit exists is itself a good interview answer. |
| **R6** | **Subtitle alignment doesn't reach 90%** on the fixture corpus. | Medium | Low–Moderate | Build the corpus first, in Phase 10, so the target is measured rather than assumed. | Lower the target *with evidence*, and lean harder on the embedded-track path (which is already the primary route and needs no alignment at all). Report the honest number in the case study — a measured 76% with an explanation beats an unverified claim of 95%. |
| **R7** | **The project is abandoned partway.** 28 phases is months of solo work. This is the most likely failure mode of all, and it is not technical. | **High** | Severe | The Tier structure in Appendix E: every tier is a legitimate stopping point that still produces a portfolio project. Phase 8 is demoable. Phase 18 is a complete product. | Stop at a tier boundary, run Phase 27's portfolio work against what exists, and ship it. **A finished Tier B project beats an abandoned Tier D one, always.** |
| **R8** | **Dependency drift over a long project** — a crate is abandoned, a breaking release lands, an API's free tier changes. | Medium | Low–Moderate | Lockfiles committed. `cargo audit`/`npm audit` in CI. The cold-resume ritual (§10.11) checks for drift after a gap. | Pin the working version and defer upgrading. Never upgrade a dependency mid-phase for its own sake. |
| **R9** | **The author falls behind and stops understanding the code** — at which point the project has failed at its actual purpose even if the app works. | Medium | **Severe** | The active understanding gate (§10.10). Learning notes as a hard deliverable. The 400-line explanation rule. | Stop feature work entirely. Spend a session or more on a code tour and a rewrite of the weak learning notes. Simplify implementations that can't be explained. This is always worth doing over adding a feature. |
| **R10** | **The §2.1 posture erodes** — a source URL appears in a fixture, a doc example, or a "temporary" default across dozens of sessions. | Medium | **Severe** — takedown, and the portfolio piece disappears | The automated posture guard (§12.5), scanning working tree *and* history, in CI and pre-commit, verified in Phase 0. | Remove, rewrite history before pushing, and strengthen the denylist. Never suppress the guard. |

### E. Tiers — where you can legitimately stop

Not all 28 phases are equal, and pretending otherwise is how projects die at phase 19. Each tier boundary is a real stopping point.

**Tier A — Vertical slice (Phases 0–8).**
Search for a film, press play, watch it, from a local file or a configured source. You have something to show a person. Not yet a portfolio project, but proof the hard parts work.

**Tier B — The product (Phases 9–18, then 21, then 27).**
Intelligent source selection, subtitles that sync themselves, the full player with the pause overlay, the local library matcher, the download advisor, profiles and onboarding, the taste model, the recommender, the discovery engine, all the browsing surfaces, performance work, and the portfolio finalisation.

**This is the project.** If you complete Tier B and stop, you have a genuinely impressive, complete, coherent piece of work with measured results and five case studies. Everything after this is breadth, not depth. **Treat Tier B as the definition of done and everything beyond it as a bonus.**

**Tier C — Depth (Phases 19, 20, 23).**
External sync, Windows platform integration, binge intelligence. Each meaningfully strengthens the project. Phase 20 in particular is a strong differentiator, since almost nobody does real shell integration.

**Tier D — Breadth (Phases 22, 24, 25, 26).**
Face recognition, live TV, manga and comics, casting and watch-together. Fully independent of one another; do any, all, or none, in any order. Each is a good self-contained project in its own right.

**Rule:** Phase 27 (portfolio finalisation) is run **whenever you decide to stop**, against whatever exists at that point. It is not the last phase — it is the phase you run when you're done. Run it at the end of Tier B even if you intend to continue; a project that is always presentable is a project you can send to an employer the day an opportunity appears.

---

## Amendments

Every change to this document is recorded here: date, section, what changed, ADR.

### spec_version 1.5.0 — 2026-09-01 (Phase 3 close)

| # | Section | What changed | ADR |
|---|---|---|---|
| A12 | §2 (technology table) | **The rationale for `sqlx` is corrected.** It was "compile-time checked queries… valuable for a learner"; the data layer does not use the `query!` macros and will not. SQLite gives them too little nullability information, so most columns would need hand-written `as "col!"` assertions — which are unchecked assertions by the author, not guarantees — and dynamic SQL cannot use them at all. The real reasons are single-file portability, WAL, and a mature async driver. A test exercising **every repository method against a freshly migrated database** is the compensating control, and is a standing requirement. | [0026](docs/adr/0026-runtime-checked-sql.md) |
| A13 | §14 | **No TMDB key ever ships.** Each user supplies their own, per profile, in settings, under their own acceptance of TMDB's terms, removable at any time. Every TMDB-dependent surface degrades to the §9.4 typographic treatment. Closes P2; the drafted enquiry to TMDB is not sent and no longer tracked. | [0027](docs/adr/0027-tmdb-per-user-key.md) |

---

### spec_version 1.4.0 — 2026-09-01 (Phase 2, design brief)

`SPEC.md` §9 rewritten to carry the author's design brief, which specifies a visual
direction the original section did not. [ADR-0023](docs/adr/0023-visual-direction.md).

| # | Section | What changed |
|---|---|---|
| A8 | **§9.0 (new)** | The direction: MUBI leads, then Spotify, Criterion, and Netflix for mechanics only. A **banned list** (purple, glassmorphism, gradients, emoji, bento grids, shadows-as-depth, Tailwind defaults, Inter as display, uniform rounding, uniform sizing) and a **required list** (depth from value and 1px lines, a real display face, wide-tracked uppercase metadata, artwork carries colour, asymmetry, idiosyncratic details). Plus **the test**: would this pass as a MUBI or Criterion screen? If it would pass as a generic dark SaaS dashboard, it has failed. |
| A9 | §9.1 | Palette revised. Greys are now **warm** (R ≥ G ≥ B) to match the warm cream ink, and the value steps are **non-linear** so elevation is rare and meaningful. Ground goes to true black behind artwork. Oxblood **stays**, with slightly raised chroma — argued in the ADR. |
| A10 | §9.2 | Display face must be a **grotesque with character**, never Inter or any neutral UI sans. Editorial serif for film titles. Small uppercase labels take **+0.12–0.16em** tracking. Scale extended to 84. |
| A11 | §9.4 | **Card sizes deliberately non-uniform**; generous vertical rhythm; `PosterCard`'s artwork-free state must be beautiful rather than a fallback. |

Carried into §9.3 from Phase 1: ADR-0020's solid opaque player panel, which this
brief treats as suiting the direction rather than as a limitation.

---

### spec_version 1.3.0 — 2026-08-31 (Phase 1, Spike A)

One amendment, from a measured platform constraint rather than a preference.

| # | Section | What changed | ADR |
|---|---|---|---|
| A7 | §9.3 | **The gradient scrim and blur-behind under player chrome are removed**, replaced by a solid opaque panel with a hard edge. Rounded corners permitted only with a matching rounded region. The **pause overlay is exempt** and keeps dim and blur. | [0020](docs/adr/0020-solid-player-chrome-region-cutouts.md) |

**Why.** Spike A established that on Windows a native child HWND always paints above
a sibling WebView2, so HTML cannot be composited over video — confirmed
independently by `ventic/ventic`, which ships the same architecture. The working
approach inverts it: cut the chrome silhouette *out* of the video window with
`SetWindowRgn` and let the page show through, which carries painting and hit-testing
both. A window region is binary, so a gradient through the hole is not expressible.

Amended deliberately rather than carrying a requirement the platform cannot meet.
[ADR-0021](docs/adr/0021-player-composition-architecture.md) records the full player
composition architecture; if wry PR #1762 (DirectComposition hosting) ever merges,
§9.3 can be restored by a superseding ADR.

---

### spec_version 1.2.0 — 2026-08-31 (Phase 0, session 0c)

Six amendments shifting the project to a **lean documentation profile**. The author
reweighted the goals: shipping the app is now the priority and the learning happens
later. Single ADR by request — [0016](docs/adr/0016-lean-documentation-and-session-profile.md).

**Governing principle: record what cannot be reconstructed, defer what can.**
Measured numbers, decisions and gotchas are recorded live because they are otherwise
gone forever. Explanatory prose can be regenerated from the code, so it is not
written now.

| # | Section(s) | What changed |
|---|---|---|
| A1 | §10.2 | Session reading reduced to `PROJECT_STATE.json`, the last `SESSION_LOG.md` entry, and §15's current-phase entry. **`SPEC.md` is no longer re-read end to end**; `CLAUDE.md` carries the standing rules. Cold resume (§10.11) keeps its fuller list. |
| A2 | §10.10 | Understanding gate fires at **tier boundaries only** — end of Phase 8, end of Phase 18, end of whatever tier the author stops at, and on request. Questions are still written every phase; they accumulate unasked. |
| A3 | §2.2 | The ~400-line rule records its explanation as **bullets in the learning note** rather than a mid-session pause. The explanation stays mandatory; the interruption does not. |
| A4 | §13.2 | Learning notes cut to four sections: what we built (3–5 bullets), why (2–3 bullets), new concepts (**a list of concept + `file:line`, no prose**), five questions. **Code tour dropped** — one traced from real code on demand beats one written from memory. |
| A5 | §11.1, §11.4, §13 | Case studies, `INTERVIEW_PREP.md` and prose expansion of `HOW_IT_WORKS.md`/`GLOSSARY.md` deferred to Phase 27 or a tier boundary; per phase only a 2–3 sentence stub and one-line terms. **Phase retrospectives fold into the `SESSION_LOG.md` entry — no separate `docs/phases/` file.** `docs/specs/*.md` stay in full, being build inputs. |
| A6 | §10.2 | Terse reporting: five-line session start, bullets not tables, no restating the spec. |

**Never deferred, and stated explicitly in §11.1:** every eval-harness metric and
performance measurement goes into **`docs/eval-results.md` the moment it is
produced** — metric, value, date, commit, command. The §10.12 regression policy
depends on those numbers and they cannot be reconstructed later.

**Explicitly not cut**, being the resume and safety mechanisms: the posture guard,
§10.8 evidence strings, §10.9 three-attempts-then-stop, per-subtask
`PROJECT_STATE.json` updates, `SESSION_LOG.md` entries, ADRs, and the eval numbers.

---

### spec_version 1.1.0 — 2026-08-30 (Phase 0, session 0a)

Seventeen amendments arising from the Phase 0 specification audit. Author approved
each ruling explicitly before any edit was made (§2.8: amend first, then build).
The seven that introduce new design carry ADRs; the ten that fix internal
contradictions or imprecision do not.

| # | Section(s) | What changed | ADR |
|---|---|---|---|
| 1 | §12.5 | Posture guard redesigned around two matchers: plaintext **structural patterns** and a **salted-SHA-256 hashed denylist**, resolving the contradiction whereby §12.5's plaintext site-name denylist violated §2.1. Documented explicitly as hygiene, not security. | [0009](docs/adr/0009-posture-guard-denylist-design.md) |
| 2 | §2.1, §12.5 | All test vectors, fixtures and documentation examples must use **RFC 2606 reserved domains**, so the guard can be proven to fire without a real forbidden string entering history. | [0009](docs/adr/0009-posture-guard-denylist-design.md) |
| 3 | §12.5 | Added the **allowlist** (`tools/guard/allowlist.txt`), which §12.5 lacked entirely despite §2.1 requiring shipped legal sources. Scoped to content/metadata source domains only; adding a line requires an ADR. | [0010](docs/adr/0010-source-allowlist-and-governance.md) |
| 4 | §2.1, §12.2 | **Fixture site-tag redaction policy**: release-group tokens kept verbatim, site tags replaced with synthetic `[SITE01]` tokens at authoring time, mapping never recorded. Resolves the collision between §12.2's "real-world filenames" and §2.1. | [0011](docs/adr/0011-fixture-site-tag-redaction.md) |
| 5 | §12.5, §12.6, §14 | Dev tooling (`guard`, `doctor`) fixed as **Python 3.12, stdlib only, permanently**; hooks activated via **`core.hooksPath`**; gitleaks **pinned in CI** with a regex fallback locally. All three were unspecified. Python added to §14. | [0012](docs/adr/0012-dev-tooling-language-and-hooks.md) |
| 6 | §14, Phase 4, Phase 14 | **TMDB is now optional, not required.** The app is fully functional on the offline IMDb + MovieLens catalogue with no key. Resolves the contradiction between §14 and the §3.3 two-minute onboarding promise. Cold-start taste defaults to the three artwork-free methods; the poster grid appears only once artwork exists. | [0013](docs/adr/0013-tmdb-optional-offline-first-catalogue.md) |
| 7 | §2.7, Phase 4 | Embedding artefacts ship as **versioned GitHub Release assets**, explicitly permitted by an amended §2.7 and requiring consent with size shown. Added the Phase 4 subtask that **produces** the artefact — previously unowned work. | [0014](docs/adr/0014-embedding-artefact-distribution.md) |
| 8 | §8, Phase 1, Phase 5 | **Tier 0 embeds queries but never documents.** Resolves the three-way disagreement between §8, Phase 5 and R3. Spike C must measure query latency specifically; >~30 ms p95 escalates under §10.9 rather than widening the budget. | [0015](docs/adr/0015-tier-0-query-embedding.md) |
| 9 | Appendix D (R3) | R3 impact raised **Moderate → Moderate/Severe**, since ONNX Runtime is now required on all tiers. Spike C gates Phase 5. | — |
| 10 | Phase 0 | Exit criterion "CI passes on `main`" was circular against §10.5's branch-per-phase rule. Now: green on `phase/00-bootstrap`, **and** on `main` after merge. | — |
| 11 | Phase 21 | Dependency changed from "18, 20" to **"18 (20 optional)"**. Tier B contained Phase 21 but not Phase 20, making Tier B — the designated definition of done — unreachable as specified. | — |
| 12 | Appendix A | Dependency graph **regenerated from the phase headers**, which are now stated to be authoritative. Corrected phantom `2 → 8` edge, disconnected Phase 14 root, and missing edges for 21, 22, 23, 25, 26. | — |
| 13 | §7 | Repository layout was missing `tools/guard/`, `tools/doctor/`, `.githooks/`, `spikes/`, `docs/RISKS.md`, `docs/DECISIONS_PENDING.md`, `docs/PERFORMANCE.md` and `docs/MANUAL_TESTS.md`, all of which Phase 0 or Phase 1 requires. Added. | — |
| 14 | §2.3, Phase 1 | Budgets **labelled by tier**. §2.3 is Tier 0 and governs; Phase 1's <2 s and <200 MB are Tier 2 dev-machine targets. Neither number changed. Search budget now explicitly includes query embedding. | — |
| 15 | Phase 3 | Exit criterion split so the **100 ms applies unambiguously to indexed lookup**, not to bulk insertion of 500,000 rows. | — |
| 16 | Phase 16, §12.2 | "Beats a popularity baseline by > 40%" clarified as **relative** (a ratio), not percentage points. | — |
| 17 | Phase 11, Phase 24 | **Rotten Tomatoes cut** (no free API; scraping violates its terms) — use IMDb, TMDB, AniList. Phase 24's FAST services go on the allowlist with justification; **`iptv-org` is not bundled** and becomes a user-pasted URL. | — |

Deferred rather than resolved, and tracked in `docs/DECISIONS_PENDING.md`: the
GPL-3.0 "source only, no compiled installers" posture versus Phase 27's packaging
deliverables and the §2.3 installed-size budget.
