# Phase 6 — Learning notes

**Source resolver and addon protocol.** All eleven subtasks done; all six exit criteria
met. Four sections per ADR-0016 (A4): what we built, why, new concepts as concept +
`file:line`, and the five questions. No code tour.

---

## 1. What we built

- **The protocol** — `crates/source-protocol/` (MIT OR Apache-2.0). The `SourceBackend`
  trait from SPEC.md §6.1, the values that cross it, the addon manifest and catalogue,
  and the wire format of an addon's two endpoints. No HTTP client: it parses, the app
  fetches.
- **The validators** — `manifest.rs`, `catalogue.rs`, `wire.rs`. Everything an addon
  sends is treated as hostile. A 25-file corpus in `fixtures/addon-manifests/`, each
  file refused for its own named reason, plus 10,000 seeded mutations.
- **The schema** — `docs/schemas/addon-manifest.schema.json`, kept in agreement with the
  Rust validator by `tools/protocol/schema_agrees.py` in CI.
- **The resolver** — `crates/sources/src/resolver.rs`. Every backend at once, each in its
  own task under its own timeout, each batch sent the moment it exists.
- **The registry** — migration 0015, `SourceRepository`, `crates/sources/src/registry.rs`.
  On/off, order, and health as counts.
- **Four backends** — `crates/sources/src/backends/`: local files (stub), the Internet
  Archive (real, tested against recorded and live responses), the user's own links and
  M3U playlists (migration 0016), and addons.
- **A safer HTTP client for user addresses** — `HttpTransport::untrusted()`: capped
  bodies, no redirects.
- **The install flow and screen** — `crates/sources/src/addons.rs`,
  `src/features/settings/SourcesSection.tsx`. Paste, read what it claims in sentences,
  confirm.
- **Search backfill** — migration 0017, `BackfillRepository`,
  `crates/sources/src/backfill.rs`. A search miss that a source can answer becomes a
  catalogue entry.
- **The specification** — `docs/specs/addon-protocol.md`, with an example addon in
  Python and a conformance test anyone can point at their own addon.

## 2. Why

- **One interface for everything watchable** (§6.1). A local file, a direct link and a
  swarm are all a `SourceCandidate`, so Phase 9 can rank them with one scoring function
  and "your own copy" appears beside the streams without special cases.
- **Declarative over executable** (ADR-0006, ADR-0038). Parsing hostile data is a bounded
  problem — every limit is a number in a test. Running hostile code is not. So an addon
  is data plus someone else's server, and the manifest format is closed so it cannot
  grow into a scraper written as configuration.
- **A slow source must cost only itself** (E4). If the resolver waited for every
  backend, the slowest would set the pace for all of them; streaming each batch as it
  arrives makes the fastest answer visible at once.
- **Health as counts, not a score.** What a score means is Phase 9's decision; counts can
  become any score later, and a score cannot be turned back into counts.
- **Backfill** (ADR-0030 layer 3) stops an out-of-date catalogue from being a dead end,
  and marks every row it creates so a later refresh can reconcile it.

## 3. New concepts

### Rust

- **Trait objects need boxed futures: `async_trait`** — `crates/source-protocol/src/backend.rs:6`.
  `Vec<Arc<dyn SourceBackend>>` holds different types behind one interface; a native
  `async fn` in a trait returns a different anonymous type per implementation, which
  cannot be `dyn`.
- **`Arc<dyn Trait>` as a heterogeneous list** — `crates/sources/src/backends/mod.rs:55`.
- **`#[serde(other)]` for forward compatibility** — `crates/source-protocol/src/types.rs:59`.
  An unknown enum value becomes `Unknown` instead of failing the whole response. It must
  be the LAST variant, which is why `Resolution` lost `Ord` and gained `rank()`
  (`types.rs:123`).
- **`#[serde(deny_unknown_fields)]` as a security boundary** — `manifest.rs:32`. It
  closes the manifest format: there is no field to put a script in.
- **Checks ordered by cost, before the parser** — `manifest.rs:145`. Size, then UTF-8,
  then nesting depth, then JSON. No input reaches an expensive step having failed a
  cheap one.
- **A non-recursive depth scan protecting a recursive parser** — `manifest.rs:159`.
- **`tokio::spawn` as panic isolation** — `crates/sources/src/resolver.rs:209`. A panic in
  a spawned task comes back as a `JoinError` at the join; it does not unwind into the
  caller.
- **A dropped `JoinHandle` does not cancel its task** — `resolver.rs:218`. On timeout the
  task is `abort()`ed explicitly; without that, a hung backend would keep running after
  everyone stopped waiting (seen to fail in `a_timed_out_backend_is_stopped_not_abandoned`).
- **`tokio::time::timeout`** — `resolver.rs:205`, the per-backend budget.
- **`mpsc` channels to stream partial results** — `resolver.rs:112`. One message per
  backend in the order they finish; `find_all` (`resolver.rs:128`) collects the same stream.
- **Tokio's paused clock** — `crates/sources/tests/resolver.rs:117`. `start_paused = true`
  makes sleeps complete instantly while advancing virtual time exactly, so timing tests
  are deterministic on a loaded CI machine.
- **`JoinSet` for concurrent lookups, re-sorted afterwards** —
  `crates/sources/src/backends/archive.rs:104`. Results arrive in completion order; the
  search's ranking is restored by index.
- **`serde_json::Value` for inconsistent APIs** — `archive.rs:238`. The Archive returns a
  field as a string, a number or a list depending on the item.
- **Reading a body in chunks to enforce a cap** — `crates/metadata-api/src/reqwest_transport.rs:119`.
  `Content-Length` is checked first but not trusted; the running count is the real limit.
- **Redirect policy as a security setting** — `reqwest_transport.rs:37`.
- **A raw TCP server in a test** — `crates/metadata-api/tests/untrusted_http.rs:13`. The
  only way to send a lying `Content-Length` or a redirect on demand.
- **A seeded xorshift for reproducible fuzzing without a dependency** —
  `crates/source-protocol/tests/hostile_manifests.rs:188`.

### SQL

- **`ON CONFLICT … DO NOTHING` to register without overwriting the user's choices** —
  `crates/persistence/src/repositories/sources.rs:62`.
- **An exponentially weighted average in one `UPDATE`** — `sources.rs:140`.
  `latency + 0.2 × (sample − latency)`: recent answers dominate.
- **A partial index over a rare predicate** — `crates/persistence/migrations/0017_backfilled.up.sql:15`.
- **One transaction across a table and two FTS indexes** —
  `crates/persistence/src/repositories/backfill.rs:44`, so a half-indexed row cannot exist.

### Security, in general

- **Same-host rule for endpoints**, and why a redirect is a way round it.
- **Bidirectional-override characters** (U+202E and friends) — `manifest.rs:232`. They
  make one name render as another.
- **Rebuild URLs from a checked key, never trust a candidate's locator** —
  `crates/sources/src/backends/archive.rs:221`.
- **What the user confirmed is what gets installed** — `src-tauri/src/state.rs:112`. The
  preview holds the manifest; install does not fetch it again.

### React and TypeScript

- **`exactOptionalPropertyTypes`** — `src/features/settings/SourcesSection.tsx`. An
  optional prop may be absent but not `undefined`, so a conditional error is spread in
  (`{...(isError ? { error } : {})}`) rather than passed as `error={x ?? undefined}`.
- **Turning a tagged result into a throw for TanStack Query** — `SourcesSection.tsx:19`.

## 4. The questions

Written, not asked — Phase 6 is not a tier boundary (§10.10). They accumulate until the
end of Phase 8.

1. Why can't the resolver hold `Vec<Box<dyn SourceBackend>>` if `find` is a plain
   `async fn` in the trait, and what exactly does `#[async_trait]` change to make it
   possible?
2. A backend times out after 300 ms. Without `task.abort()`, what is still happening,
   and why does dropping the `JoinHandle` not stop it?
3. The manifest refuses unknown fields, but a find response ignores them. Why is the
   stricter rule right for the manifest and the looser one right for the answers?
4. The endpoints must be on the manifest's own host. Describe how an addon could get
   round that rule if the HTTP client followed redirects.
5. Backfill refuses to create a row when the catalogue already has the title in that
   year, even though the search that triggered it found nothing. How can a search miss a
   film the catalogue contains, and what would go wrong if backfill trusted the miss?
