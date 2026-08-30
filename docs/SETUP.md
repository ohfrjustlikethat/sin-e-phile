# Setup

Getting from a clean Windows machine to a running development build.

**The application needs no API key of any kind to work** (ADR-0013). Everything in
§"Optional services" below adds enrichment — artwork, subtitles, sync — on top of a
catalogue that already works offline.

---

## 1. Run doctor first

```bash
git clone https://github.com/ohfrjustlikethat/sin-e-phile.git
cd sin-e-phile
python tools/doctor/doctor.py
```

**This is step one for a reason.** `doctor` checks every prerequisite, prints
exactly what is missing and how to install it, and **bootstraps the git hooks** by
setting `core.hooksPath`. `.git/hooks/` is not tracked by git, so until doctor runs
once, the posture guard and secret scan do not run on your commits (ADR-0012).

If doctor says a tool is *"installed but not on THIS shell's PATH"*, it is telling
you the truth: Windows updated the stored `PATH` when you installed it, but this
terminal still has the environment it inherited at launch. **Restart your terminal.**
Do not reinstall.

---

## 2. Prerequisites

All free. Verified **2026-08-30**.

| Prerequisite | Version | Why | Get it |
|---|---|---|---|
| **Windows** | 10 or 11 | `SPEC.md` §2.4 — Windows only, deliberately | — |
| **Git** | 2.40+ | Also supplies the shell that runs the hooks | [git-scm.com](https://git-scm.com/download/win) |
| **Python** | **3.12+** | `tools/guard/`, `tools/doctor/`, `tools/state/` — stdlib only, no `pip install` (ADR-0012). A *development* prerequisite; users of a built app never need it. | [python.org](https://www.python.org/downloads/) |
| **Rust** | stable, **MSVC** toolchain | The backend | [rustup.rs](https://rustup.rs) |
| **MSVC C++ build tools** | 2019+ | Rust's MSVC toolchain cannot link without it | [Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) — select **"Desktop development with C++"** |
| **Windows 10/11 SDK** | 10.0.19041+ | Included with the C++ workload above | — |
| **Node.js** | 20 LTS+ | The frontend | [nodejs.org](https://nodejs.org) |
| **WebView2 Runtime** | Evergreen | Tauri renders the entire frontend in it | [WebView2](https://developer.microsoft.com/microsoft-edge/webview2/) — preinstalled on current Windows 11 |
| **FFmpeg** | 6+ | Media probing, VAD extraction, thumbnails | [ffmpeg.org](https://ffmpeg.org/download.html), or `winget install Gyan.FFmpeg` |

Optional: **GitHub CLI** (`gh`) and **gitleaks**. Without gitleaks the pre-commit
hook uses `tools/guard/secretscan.py`; CI runs a pinned gitleaks either way.

> **`vswhere` gotcha.** If you have an Insiders or Preview Visual Studio, a plain
> `vswhere -latest` reports nothing and MSVC looks absent when it is not. `doctor`
> passes `-prerelease -all` for exactly this reason.

**Verified working on this machine:** Rust 1.98.0 · MSVC 14.50.35717 ·
Windows SDK 10.0.26100 · Node 24.20.0 / npm 11.19.0 · Python 3.12.10 ·
WebView2 151.0.4129.107 · FFmpeg 9.0 — confirmed by a real `cargo new` /
`cargo build` / run round trip, not by `--version` alone.

---

## 3. Build and run

```bash
npm install
npm run tauri dev      # development build with hot reload
npm run tauri build    # production build
```

*(From Phase 1. There is no application to build yet — Phase 0 is infrastructure.)*

Checks, all of which CI also runs:

```bash
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
npm run lint && npm run test

python tools/guard/guard.py --selftest    # prove the guard works
python tools/guard/guard.py --tree        # posture check
python tools/state/build_state.py --check # state matches SPEC.md §15
python tools/state/validate_state.py --check
```

---

## 4. Optional services

**None of these are required.** The offline catalogue — IMDb datasets plus
MovieLens, both free downloads with no account — supplies titles, years, runtimes,
genres, cast, crew and ratings, and semantic search, the taste model, the
recommender and the discovery engine all run on it unchanged (ADR-0013).

Copy `.env.example` to `.env` and fill in only what you want. `.env` is git-ignored
and **must never be committed**; if a key is ever committed, **rotate it first**,
then clean history (§12.6).

Terms below were verified on **2026-08-30**. They change — re-verify before any
phase that depends on one (risk **R11**).

### TMDB — artwork and rich detail · *Phase 4*

Free API key, instant, at themoviedb.org → Settings → API. Free for
**non-commercial use only**; commercial use needs a separate written agreement.

Three obligations that affect design, all confirmed in the current terms:

- **Attribution is mandatory.** The TMDB logo, less prominent than your own
  branding, plus: *"This product uses TMDB and the TMDB APIs but is not endorsed,
  certified, or otherwise approved by TMDB."*
- **Non-commercial users may not cache data for longer than 6 months.** This is a
  hard constraint on Phase 4's cache TTL design — a naive "cache forever" violates
  the terms.
- **Using the data for AI/ML training is prohibited.** Computing embeddings is
  inference rather than training, but see **P2** in `DECISIONS_PENDING.md`; this
  needs a deliberate ruling before Phase 5, not an assumption.

No explicit numeric rate limit is published; the terms prohibit "an excessive
amount of bandwidth" and leave abuse to TMDB's discretion. Phase 4's shared rate
limiter should therefore be conservative and configurable.

### AniList — anime metadata · *Phase 4*

Free public GraphQL. **No key required** for public read queries.
**90 requests per minute**, plus a burst limiter. Exceeding it earns a one-minute
timeout; responses carry `X-RateLimit-Limit`, `X-RateLimit-Remaining`, and
`Retry-After` on a 429 — Phase 4's backoff should read those headers rather than
guessing.

### Jikan — MyAnimeList mirror, optional fallback · *Phase 4*

Free, **no key**, read-only. **3 requests per second and 60 per minute**, no daily
cap. 429 on breach.

### IMDb datasets — the offline catalogue · *Phase 4*

Free download from `datasets.imdbws.com`, no account. Seven gzipped TSVs:
`title.basics`, `title.akas`, `title.crew`, `title.episode`, `title.principals`,
`title.ratings`, `name.basics`. **Refreshed daily.** Licensed for **personal and
non-commercial use**; local copies are explicitly permitted.

### MovieLens — collaborative-filtering signal · *Phase 16*

Free from GroupLens, no account, but **a usage form must be filled in** and the
licence lives in each dataset's README. **GroupLens states it does not generally
permit public redistribution** — which bears directly on whether a *derived*
item-item similarity matrix can ship as a release asset. Logged as **P3** in
`DECISIONS_PENDING.md`; must be resolved before Phase 16, not during it.

MovieLens 32M (239 MB, 32M ratings, 87,585 movies) is the recommended research set.

### OpenSubtitles — hash-matched subtitles · *Phase 10*

Free account required. **The free tier is small: 5 downloads/day with no account,
20/day with a free account**, rising with upload contributions (50 / 100 / 200 at
bronze / silver / gold).

**This constraint produces a better design than an unlimited API would have.**
Phase 10's chain puts embedded tracks first — they are already perfectly synced and
cost nothing — then sidecar files, and only then spends quota here. Treat the daily
allowance as a budgeted, cached, scarce resource; quota state is visible in
Settings.

### Fanart.tv — higher-quality artwork · *Phase 4, optional*

Free personal key. **Not re-verified in this session** — check current terms before
Phase 4 uses it.

### Trakt — watch history sync and scrobbling · *Phase 19*

Free API key; OAuth, including a device flow. Documented limits have been around
**1,000 GET calls per 5 minutes** for authenticated apps, with writes far tighter.

**Trakt is actively revising its Free and VIP limits for 2026** and has tightened
the free tier, including restricting how many applications a free user may connect.
**Re-verify before Phase 19 and design for a smaller allowance than you expect.**

---

## 5. Troubleshooting

**`link.exe not found` / Rust fails to link.** MSVC C++ build tools missing. Install
the "Desktop development with C++" workload.

**A tool is installed but doctor says it's missing.** Restart your terminal —
Windows updates `PATH` in the registry, but open shells keep the environment they
started with. Doctor distinguishes these two cases explicitly.

**The pre-commit hook does not run.** Run `python tools/doctor/doctor.py` once to
set `core.hooksPath`. Verify with `git config --get core.hooksPath` → `.githooks`.

**The posture guard fires on something.** Read `tools/guard/README.md`. The fix is
to **remove the content** — if it is already committed, rewrite history before
pushing. Never suppress the guard (§12.5).

**`build_state.py --check` fails.** `SPEC.md` §15 was amended without regenerating
the state file. Run `python tools/state/build_state.py --write`.
