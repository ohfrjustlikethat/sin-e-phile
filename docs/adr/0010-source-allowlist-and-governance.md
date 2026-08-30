# 0010 — Source allowlist and its governance

- **Status:** Accepted
- **Date:** 2026-08-30
- **Phase:** 0
- **Amends:** `SPEC.md` §12.5
- **Risk:** R10

## Context

`SPEC.md` §12.5 specifies only denial: the guard fails the build when it finds
forbidden patterns. But §2.1 explicitly requires the repository to ship working
reference backends against "unambiguously legal sources only: the local filesystem,
the Internet Archive's public-domain collection, and user-supplied M3U/direct
HTTP", and §14 requires live API clients for TMDB, AniList, Jikan and Fanart.tv.

So the repository must contain content and metadata source domains in shipped code,
by design. A guard that only denies has no way to express "this domain is
permitted", and will either fire on the Internet Archive backend — training
everyone to ignore it, which is how guards die — or be written so loosely that it
catches nothing.

The spec has no concept of an allowlist. It needs one. And the allowlist is the
*exhaustive* half of the mechanism: the denylist is best-effort by construction
(ADR-0009), but the allowlist is complete. Anything source-shaped that is not on it
is a finding.

## Decision

`tools/guard/allowlist.txt` enumerates, in plaintext, every content or metadata
source domain permitted to appear in shipped code, config, or documentation.

### Scope — deliberately narrow

The allowlist covers **content and metadata source domains only**. It does not
cover, and the guard does not interrogate, general development infrastructure:
`crates.io`, `npmjs.com`, `github.com`, `docs.rs`, `rust-lang.org` and similar are
out of scope entirely. Scanning those would produce constant noise for zero posture
benefit.

The guard's structural and hashed matchers (ADR-0009) apply everywhere. The
allowlist question is asked only of domains that look like a content or metadata
source.

### Initial contents

Each line carries an inline justification comment:

| Domain | Justification |
|---|---|
| `archive.org` | Internet Archive — public-domain collection; §2.1 named reference backend |
| `themoviedb.org`, `tmdb.org` | TMDB — primary metadata; §14, free non-commercial API |
| `anilist.co` | AniList — anime metadata; §14, public GraphQL, no key |
| `jikan.moe` | Jikan — MyAnimeList mirror; §14, public REST, no key |
| `opensubtitles.com` | OpenSubtitles — hash-matched subtitles; §14, free account |
| `fanart.tv` | Fanart.tv — artwork; §14, free personal key |
| `trakt.tv` | Trakt — watch history sync; §14, Phase 19 |
| `grouplens.org`, `movielens.org` | MovieLens — collaborative-filtering dataset; §14, free download |
| `imdbws.com`, `imdb.com` | IMDb datasets — offline title index; §14, non-commercial use |

### Governance

Adding a line requires an ADR. That ADR must state what the source is, why it is
unambiguously legal, and whether it ships enabled or requires the user to opt in.

This is deliberately heavy. The allowlist is the single point at which "just this
once" would silently reverse §2.1, so the cost of adding to it *is* the mechanism
that protects it.

Per ruling ⑫, **Phase 24's FAST services** — Pluto TV, Samsung TV Plus, Plex FAST —
are permitted allowlist additions when Phase 24 is built, as legal free ad-supported
streaming services, each with its own justification line and ADR. The **`iptv-org`
index is not bundled** and does not go on the allowlist: it is a URL the user pastes
like any other, which is both cleaner and consistent with §2.1's rule that no
default URL ships.

## Consequences

**Easier.** The guard can be strict without being noisy, because permitted sources
are declared rather than inferred. `allowlist.txt` doubles as a single honest answer
to "what does this application actually talk to?" — a question both a hiring manager
and a privacy-minded user will ask, and one §2.7 already promises to answer
truthfully.

**Harder.** Every new integration now carries ADR overhead. Intended.

**Risk, and it is the real one.** The allowlist is a far more attractive erosion
path than the denylist, because adding a line is *easy* and always feels justified
in the moment. R10's mitigation must therefore explicitly include reviewing
`allowlist.txt` diffs: a one-line change here can undo the entire legal posture, and
it will look innocuous. Phase 27's exit criterion that re-verifies §2.1 must read
this file line by line, not merely confirm the guard passes.

## Alternatives Considered

**No allowlist; tune the denylist to avoid false positives.** Rejected: tuning a
denylist against known-good domains produces an allowlist anyway — built
accidentally, scattered across regexes, and undocumented.

**Allowlist inferred from code** — permit any domain referenced from
`src-tauri/src/sources/`. Rejected: this makes the permitted set implicit and
mutable by ordinary code changes, which is exactly the silent drift §2.8 exists to
prevent.

**Allowlist covering all URLs including development infrastructure.** Rejected:
high noise, no benefit. Every `crates.io` link in a doc comment would need an entry,
and a guard that fires constantly is a guard that gets `--no-verify`d.
