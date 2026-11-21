# Phase 19 — learning notes

## 1. What we built

- Before the phase proper (ADR-0057, the author: the app took "a disgusting amount of space"): streaming fetches only the
  watched file; streamed torrents are a 20 GB cache, least recently watched out first; build output kept small; a
  search that is a person's name shows their films.

## 2. Why

- 524 GB of the author's disk was streams the app never removed, mostly season packs fetched whole for one episode. A
  cache needs a size and a rule for what goes; "never" is not a rule.
- 283 GB of `target/` was debug information copied into 129 test binaries and their incremental caches; the build
  profiles decide that, not anyone's habits.
- Text ranking weighs a title far above a cast list, rightly for "Heat" and wrongly for "Zendaya": a name is a
  different question, so it gets its own answer.

## 3. New concepts

- **A cache as a pure decision** (what to evict from sizes, ages, a cap and what plays) — `crates/torrent/src/cache.rs:29`
  (`evictions`).
- **Narrowing a torrent to one file** — `crates/torrent/src/engine.rs:298` (`select_files`), used by `play::prepare`.
- **Orphans**: deleting what nothing owns, only at start, before anything can be mid-write —
  `crates/torrent/src/engine.rs:326` (`trim_streams`).
- **Cargo profiles**: `debug = "line-tables-only"`, per-package overrides, `incremental` per profile — `Cargo.toml`.
- **Folding for comparison** (Unicode NFD, then dropping combining marks) so "timothee" is "Timothée" —
  `crates/persistence/src/repositories/search.rs:185` (`person_named`, through `normalise_query`).
- **A cost on every query**: the person lookup took p95 from 17 to 38 ms until it read less — measure the hot path, not
  only the new feature.

## 4. Five questions

1. Why does narrowing a torrent to one file need the torrent added first, and what would go wrong narrowing a download?
2. Why is the stream opened last protected from eviction, and what would happen if the cap were lowered mid-film
   without that?
3. Why are orphan folders deleted only when the engine starts?
4. Why does a person's name get its own tier instead of a heavier weight on the people column?
5. Why does deleting streamed files alone not free the space for good?
