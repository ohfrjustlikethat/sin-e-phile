# 0026 — Runtime-checked SQL, and the test that compensates for it

- **Status:** Accepted · **Date:** 2026-09-01 · **Phase:** 3 (decided at close)
- **Amends:** `SPEC.md` §2 technology table (spec_version 1.5.0)
- **Closes:** P9 · **Relates to:** ADR-0022

## Context

`SPEC.md` §2's technology table gave the reason for choosing `sqlx` as
*"compile-time checked queries… `sqlx` catches SQL errors at compile time — valuable
for a learner."*

Phase 3 built the whole data layer with runtime-checked `sqlx::query()` rather than
the compile-time-checked `query!` / `query_as!` macros. That was surfaced at the end
of the phase as a deviation rather than kept quietly, and the author ruled on it.

## Decision

**Runtime-checked queries everywhere. No case-by-case split.**

**`SPEC.md` §2's rationale for `sqlx` is amended** to what is actually true: single-
file portability, WAL, and a mature async Rust driver with a good migration story.
Compile-time macros are explicitly not the reason.

**A compensating control is mandatory** (below), and is a standing requirement, not a
one-off.

## Why the macros lost

The author's reasoning, and it is the right reasoning:

**1. "Compile-time checked" would have meant "compile-time asserted by me."**
SQLite gives sqlx very weak nullability information — it cannot tell whether a column
in a `LEFT JOIN` is nullable — so most columns need an `as "column!"` annotation. That
annotation is an *unchecked assertion by the author*. Getting one wrong converts a
compile-time guarantee into a runtime panic, which is worse than the runtime error it
replaced, because it arrives dressed as a guarantee.

**2. Dynamic SQL cannot use them at all.** `query!` requires a literal string. The
archive's source-preference query is built with `format!` because the same ordering
expression is needed under two table aliases. So the rule would have carried an
exception from its first day, and a rule with case-by-case exceptions erodes.

**3. The ritual compounds over 24 more phases.** `cargo sqlx prepare` after every
schema change, `sqlx-cli` as a build prerequisite (so `tools/doctor` grows a check),
and a `.sqlx/` cache that goes stale silently and fails CI later, in a different
place, for a reason that looks unrelated.

## The compensating control

**Every repository method is exercised against a freshly migrated database in
`cargo test`.** This catches exactly what the macros would have caught — a typo'd
column, a renamed table, a query the schema no longer supports — at test time rather
than build time. Slightly later, no ritual, and it is automatic.

`crates/persistence/tests/repository_surface.rs` calls every public method on every
repository against a real migrated database and asserts each one executes.

**Standing requirement: a new repository method without a schema-exercising test does
not pass review.** The test is not optional documentation of the methods that
happened to be convenient — it is the thing that replaces the compile-time check, and
a method missing from it is a method with no protection at all.

The test is also strictly better than the macros in one respect: it runs the query
against the *actual migrated schema*, so it catches a migration that forgot to add a
column the code expects. `query!` checks against whatever database `prepare` was last
pointed at, which may be neither.

## Consequences

- SQL errors surface at `cargo test`, not `cargo build`. CI runs both, so nothing
  reaches `main` either way.
- No `sqlx-cli`, no `.sqlx/` directory, no prepare step, no doctor check.
- `Cargo.toml` keeps the `macros` feature: `query_scalar` and `query_as` (the
  function forms) come from it, and it costs nothing.
- If a repository method is added without a test, the gap is invisible. That is the
  residual risk this ADR accepts, and the standing requirement above is how it is
  managed. It is a review rule, not a mechanical one — stated plainly rather than
  pretended otherwise.
