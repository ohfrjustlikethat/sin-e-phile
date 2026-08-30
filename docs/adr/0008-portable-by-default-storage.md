# 0008 — Portable-by-default storage

- **Status:** Accepted
- **Date:** 2026-08-31
- **Phase:** 0 (recording a decision locked in `SPEC.md` §2.5)

## Context

A desktop application has to decide where its data lives. The Windows convention is
`%APPDATA%` and `%LOCALAPPDATA%`, which is what an installer-based application does
and what most users never think about.

But this application has properties that make the convention a poor default. It has
**no accounts and no server** (§5), so everything a user has — watch history, the
taste model, the local library index, playback positions — exists only on their
machine and nowhere else. It is a single-user tool whose value accumulates entirely
in local state.

The consequence is that "where is my data" and "how do I move it" are questions with
real weight, and the convention answers them badly: data scattered under a hidden
per-user path, coupled to one machine and one Windows account.

## Decision

**All application data lives in `./data/`, next to the executable.** Database,
profiles, taste models, metadata cache, downloaded-media index, logs.

**The whole application is a folder you can move.** Copy it to a USB stick or
another machine and it keeps working, with all state intact.

An **installed mode** using `%APPDATA%` exists as an explicit opt-in, not the
default.

## Consequences

**Easier — and this is the point.** Backup is "copy the folder". Migration to a new
machine is "copy the folder". Running from external storage works. Trying the
application leaves nothing behind, which matters for a tool someone is evaluating.

**Easier for development and for the portfolio.** The entire application state is
visible in one directory during development, so inspecting the database or clearing
state is trivial. It also makes §2.7's no-telemetry claim concretely demonstrable:
you can point at exactly where everything is.

**Easier for testing.** Phase 3's exit criterion — copy the folder elsewhere, launch,
all data preserved — is a real, mechanically checkable test of the whole storage
layer, rather than an aspiration.

**Harder.** Every path must be resolved relative to the executable, and this must be
right from the first line of Phase 3. Retrofitting it later means finding every
hardcoded path, and any one that is missed produces state split across two
locations — a bug that is invisible until a user moves the folder and silently loses
half their history.

**Harder.** `Program Files` is not writable by a standard user, so a portable
install placed there breaks. This must be detected at startup and reported clearly
("this folder is not writable — move the application, or switch to installed mode")
rather than failing obscurely on first write. Belongs in Phase 3, not Phase 27.

**Harder.** Portable data on removable media means the database can vanish
mid-operation when a stick is pulled. SQLite in WAL mode is resilient, but Phase 3's
error handling must treat "the data directory disappeared" as a real case.

**Consequence to be aware of.** Data next to the executable is unencrypted and
inherits only the filesystem's permissions. For watch history and a taste model that
is proportionate — and Phase 14's optional profile PIN is explicitly a
convenience-level control, not security. This should be stated honestly rather than
implied otherwise.

## Alternatives Considered

**`%APPDATA%` by default, portable as an option.** The conventional inversion of
this decision, and the stronger of the alternatives. Rejected because the default is
what almost every user gets, and the portable property only holds if it is the
default — an application that is *occasionally* portable has to be written as if it
always is, so making it the exception buys nothing while losing the benefit.

**A user-chosen directory on first run.** Rejected on §3.3: setup is three screens
and a non-technical person is watching something within two minutes. A
"where should I put my data" prompt is exactly the kind of question that stops
someone before they start, and it has no good answer for a person who does not
already know.

**Follow the XDG-style split — config, cache and data in separate locations.**
Correct and idiomatic on Linux, and a poor fit for a Windows-only application whose
entire premise is that its state is one movable folder. Rejected, though the
*internal* structure of `./data/` should still separate cache from durable state,
so that clearing the cache is a safe operation.

**A single-file bundle with everything inside one archive.** Genuinely portable, and
rejected because SQLite needs a real file for WAL mode to work, and losing WAL would
cost concurrency and crash resilience — a bad trade for tidiness.
