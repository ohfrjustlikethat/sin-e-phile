# Phase 4 — Learning notes

**Metadata backbone.** In progress — 6 of 13 subtasks. This note grows as the phase
does; the five questions are asked when the phase closes.

Four sections per ADR-0016 (A4): what we built, why, new concepts as concept +
`file:line`, and the five questions. No prose tour.

---

## 1. What we built

- **`tools/ingest`** — the offline pipeline. A resumable job runner (`job.rs`), the
  IMDb loaders (`load.rs`, `credits.rs`, `akas.rs`), the normalised-title backfill
  (`normalise.rs`), and the AniList matcher and ingester (`matching.rs`, `anime.rs`).
- **`crates/metadata-api`** — the HTTP boundary. Token-bucket rate limiting, backoff
  with full jitter, three-state cache freshness, and a `Transport` trait with a fake
  so every client is testable without a network.
- **Nine migrations**, the last of which (`0009`) adds `titles.normalised` — the
  column that makes matching possible at all.
- **6,184,278 titles normalised**; 854,752 core-tier titles with credits and akas.

## 2. Why

- **A wrong match is worse than no match.** A missed anime shows an IMDb entry with no
  Japanese title. A wrong one attaches *Fullmetal Alchemist*'s episodes to
  *Brotherhood*, and nothing downstream ever notices. So the matcher **refuses ties**
  rather than breaking them, and every refusal is counted by reason.
- **`titles.normalised` is a column, not a function.** SQLite cannot compute our
  normalisation, and duplicating the rule in SQL would create two definitions of "the
  same title" that drift apart invisibly. See the header of `0009_normalised_titles`.
- **Checkpoints commit in the same transaction as the work they describe** — the one
  rule the whole resumability story rests on (`job.rs`).

## 3. New concepts

### Rust

- **`Arc<T>` vs `&T`, and why the borrow checker forced the change** —
  `crates/metadata-api/src/anilist.rs:134`. `Job::run_step` takes a closure under a
  *higher-ranked trait bound*: `for<'t> FnMut(&'t mut SqliteTx, ...) -> BatchFuture<'t>`.
  `for<'t>` means "for **every** lifetime `'t`", so a future returned from that closure
  may not borrow anything with a *particular* lifetime — including `&AniList`. The fix
  is not a cleverer annotation; it is to stop borrowing. `AniList::owned` takes an
  `Arc<dyn Transport>` and produces an `AniList<'static>`, which can be cloned into the
  closure. **Phase 11 needs the same thing** — Tauri's managed state cannot hold a
  borrow either — so the constraint pointed at the design the app wanted anyway.
- **`Pin<Box<dyn Future + Send + 'a>>`** — `job.rs:43`. An `async` block produces an
  anonymous type; to store one in a struct field or return it from a trait method it
  must be boxed. `Pin` is there because a future may hold pointers into itself, so
  moving it after polling would invalidate them.
- **Deferred transactions** — `anime.rs:16`. SQLite's `BEGIN` takes no lock until the
  transaction's first statement. That is what makes an HTTP fetch *inside* a
  transaction safe: no lock is held across the network call, and the page that comes
  back is covered by the same checkpoint as the writes it produces.

### The matching problem

- **Normalisation** — `matching.rs:80`. Lowercase, drop punctuation, collapse spaces.
  Deliberately does *not* transliterate: `Kimi no Na wa` and `君の名は` are matched as
  separate title forms, not folded into one.
- **Refusing rather than guessing** — `matching.rs:216`. Two equally good candidates
  return `NoMatch::Ambiguous`, never a coin flip.
- **Narrowing by strength of evidence** — `matching.rs`, the `_ =>` arm. When several
  candidates tie, prefer those with a *known and agreeing* year over those with no year
  at all, then prefer the shape AniList states. Each stage may discard a
  worse-evidenced rival; none may create a match or discard the last one standing.

### The three bugs, and what each one teaches

1. **Counting rows instead of items.** The matcher treated `Death Note`, `DEATH NOTE`
   and `Death note` — three title rows of *one* catalogue item — as three rival
   candidates, and refused the four most famous anime in the sample as ambiguous
   against themselves. 97 of 250 outcomes. **Unit tests could not have caught this**: a
   fixture never has three spellings of one title. It took running against six million
   real rows. *Fixtures test the logic you thought of; real data tests the logic you
   didn't.*

2. **A stale comment outranking the schema.** The upsert in `anime.rs` used
   `ON CONFLICT (media_item_id, variant, COALESCE(language, ''), ...)`, taken from the
   comment in migration `0001`. Migration `0007` had since redefined that index to
   `(media_item_id, variant, title)`. SQLite rejected it outright — the good case. *An
   applied migration is history, not documentation; ask the database what its indexes
   are.*

3. **`is_resuming` reporting "fresh run" on every real resume.** It counted only steps
   with status `complete`. A crash lands *mid*-step, leaving a cursor on a step that
   never completed — the common case, and the one a caller most wants to be told about.
   *A predicate that is true in the rare case and false in the common one is worse than
   no predicate.*

## 4. Self-check questions

Asked when the phase closes, not before.

1. Why does `titles.normalised` exist as a stored column when the same string could be
   computed in Rust at match time? What breaks if it is computed instead?
2. `match_title` refuses when two candidates are equally good. Describe a concrete case
   where that refusal costs us a correct match — and argue why we accept that cost.
3. The narrowing stages prefer a dated candidate over an undated one. Why is that not
   the same kind of decision as preferring the candidate with more votes?
4. `Job::run_step` guarantees that a crash never duplicates or loses work. What
   specific property of the code provides that guarantee, and what would break it?
5. `AniList::owned` exists because of a compiler error. Explain what `for<'t>` means in
   `run_step`'s bound, and why no lifetime annotation could have fixed the call site.
