# Phase 0 — Learning notes

**Bootstrap and project infrastructure.** Sessions 0a and 0b, 2026-08-30/31.

This note is a deliverable, not a nicety (`SPEC.md` §2.2). Its job is to make you
able to explain what was built. The five questions at the end are asked out loud
before Phase 0 can be called done — and if you cannot answer two or more of them,
that means the note failed or we went too fast, and the correct response is to fix
this note or simplify the code (§10.10, risk R9).

---

## 1. What we built

No application. **The machinery that makes building one for months safely
possible.**

Four things, in plain terms:

**A guard that stops the project getting taken down.** This app must ship no
pointers to infringing content. A program now checks that on every commit and every
push, looking at both the current files and every version of every file that has
ever existed in the repository.

**A doctor that tells you what is missing.** Run one command and it says exactly
which prerequisite you lack and how to install it — turning "the build mysteriously
fails" into "you are missing the C++ build tools, here is the link".

**A state system that a future you can trust.** This project runs for months with
gaps. `PROJECT_STATE.json` records where things stand. Critically, it is *generated*
from the specification rather than typed, and *validated* so that you cannot mark
something done without proof.

**The specification itself, corrected.** Reading `SPEC.md` closely turned up twelve
real problems — contradictions, gaps, and one tier that was unreachable as
written. All twelve were ruled on and fixed before any code was written.

---

## 2. Why this approach

### Why build safety rails before there is anything to protect?

Because the alternative is to build them after the first mistake, and by then the
mistake is in the git history — where removing it means rewriting history, and where
anything already pushed to a public repository must be assumed compromised.

`SPEC.md` §12.5 puts it precisely: *a human rule that must hold across 28 phases and
dozens of sessions will eventually be broken by accident.* Not through carelessness.
Through the ordinary process of being tired at session 19 and pasting an example URL
into a doc.

### Why *verify* the guard rather than just write it?

This is the one to remember, because it nearly did not happen and it caught a real
bug.

Phase 0's exit criteria do not say "write a guard". They say the guard must **fail
when fed a deliberately planted test string**. The spec's reasoning: *an unverified
guard is worse than none, because it produces false confidence.*

So we planted a fake API key. The scanner caught the planted GitHub token — and
**silently missed the planted API key**. The cause: its list of placeholder words
(so that `.env.example` stays committable) included the bare word `a`. The planted
key began `a1b2c3`. The rule matched `a` as a placeholder and skipped the line.

That bug would have passed every casual test. Every commit would have shown a green
tick. And it would have protected nothing.

### Why generate the state file instead of writing it?

`PROJECT_STATE.json` must list all 28 phases with their exit criteria — about 154
checkboxes. Typing those is not merely tedious; it creates a *second copy* of the
specification that starts drifting the moment `SPEC.md` is amended. And a state file
that disagrees with the spec is worse than none, because you would trust it.

So `tools/state/build_state.py` parses `SPEC.md` §15 and generates the phase table.
CI checks they still agree. The spec has exactly one copy.

### Why a schema that rejects "looks good"?

§10.8 says an exit criterion is met only with an **artefact** — a passing test name,
a measured number *with the command that produced it*, a file path, a commit SHA.
It calls marking a criterion met without evidence *the single most damaging thing
that can happen to this project*, because it destroys every future session's ability
to trust the file.

A rule in a document is advice. So the JSON schema makes it structural: a criterion
with `met: true` and no evidence **fails validation**, and the banned phrases —
"looks good", "tested manually", "should be fine" — are rejected by pattern.

We tested that too, by planting three violations. All three were caught, including
the spec's own anti-example, `next_action: "continue the torrent engine"`.

---

## 3. New concepts

### Git hooks, and why yours were not running

A **hook** is a script git runs at a point in its lifecycle. We use two:
`pre-commit` (runs the guard and secret scan; a non-zero exit aborts the commit) and
`commit-msg` (checks the message format).

**The catch that matters:** hooks live in `.git/hooks/`, and `.git/` is *not tracked
by git*. So a hook committed to the repository does nothing on a fresh clone. The
fix is `git config core.hooksPath .githooks`, pointing git at a tracked directory —
which `doctor` does for you.

The consequence is real and worth knowing: **a fresh clone is unprotected until
`doctor` runs once.** That is why the guard also runs in CI, and why `SETUP.md`
makes doctor step one. Defence in depth, because the local layer can be absent.

You saw both hooks work: the guard blocked a commit whose README contained a
false-positive example, and `commit-msg` rejected a 77-character subject.

### Hashing, and the difference between hygiene and security

A **hash** turns input into a fixed-size fingerprint, one-way: easy forwards,
infeasible backwards. A **salt** is a fixed extra string mixed in before hashing, so
identical inputs elsewhere produce different fingerprints.

The denylist stores `SHA-256(salt + name)` rather than names, so a forbidden name
never appears in plaintext — because writing it down would itself violate §2.1.

**The honest framing matters more than the mechanism.** The salt is committed, so
anyone determined can hash a list of candidate names and compare. That is
brute-forcing, and it works. This defends against *accidental plaintext and casual
discovery* — a grep, a code search, someone scrolling the tree — and nothing more.
ADR-0009 says so explicitly, and if you ever describe it as security in an
interview, you will be wrong.

Its real cost is **exact matching only**: no substring or fuzzy matching, because
those need the plaintext the file deliberately does not hold. That is why the
*structural* patterns do the heavy lifting.

### RFC 2606, and proving a thing without doing the thing

A puzzle: how do you prove the guard catches forbidden strings without putting a
forbidden string in the repository?

**RFC 2606** reserves `.invalid`, `.test` and `.example` so that no registry can
ever issue them. `notarealindexer.invalid` is *provably* not a real site — not by
policy, by construction. So every test vector uses one, and the guard is proven to
work with nothing real ever committed.

### False positives are a security failure

The instinct is that a check erring toward catching too much is safe. It is not.

The guard's first real run produced **118 findings, every one wrong**. A tool like
that gets `--no-verify`d within a week, and then it protects nothing at all — the
same outcome as never writing it.

So the self-test runs 16 vectors that must *not* fire alongside the 12 that must,
and getting to zero took three rounds of genuine design fixes:

1. A bare domain counts only if its last part is a plausible top-level domain.
   `main.rs`, `manifest.json` and `Movie.2019.BluRay.x264-GROUP.mkv` all look
   *exactly* like domains otherwise. Listing what *is* a TLD is finite; listing what
   is not is endless.
2. URLs and bare domains are found separately, so `/manifest.json` in a path stops
   reading as a domain.
3. Inside source code, only real URLs are checked, because `os.name` and
   `result.name` are shaped identically to domains and no rule reliably separates
   them.
4. Tree scans look only at git-tracked files. **CI caught this one**: the scan read a
   downloaded tool's own README and failed the build on someone else's example key.

### Regular expressions, and how they lie

The guard and scanner are built on **regex** — patterns describing text shapes.

Both bugs this session were regex bugs, and both were the *same kind*: a pattern
that looked right and quietly matched more than intended. `a` inside a list of
placeholder words. `[0-9a-f]{40}` matching part of a longer hash without boundary
anchors.

The lesson is not "regexes are bad". It is that a regex you have not tested against
things that must *not* match is a guess. That is why `vectors.py` has two lists.

### Schemas, and making bad states unrepresentable

A **JSON Schema** describes a document's valid shape. Ours goes beyond types: it
uses conditional rules — *if* `met` is true, *then* `evidence` must be a string of
at least 12 characters and must not match the banned phrases.

The idea generalises well beyond JSON, and you will meet it constantly in Rust:
**make illegal states unrepresentable**. Rather than remembering a rule, arrange
things so breaking it fails automatically.

### Why Python here, when the project is Rust and React

`doctor` exists to diagnose a *broken* toolchain. Writing it in Rust would mean it
cannot run in the exact situation it was built for — a circular dependency. And the
hooks must work on a fresh clone before anything is installed.

So the tools are Python, standard library only, no `pip install`. **Permanently**,
not temporarily (ADR-0012): they are development tools, never shipped, and never
touching the performance budgets. Rewriting them in Rust later would be churn with
no benefit.

Knowing *why* a project uses a third language is a better answer than apologising
for it.

---

## 4. Code tour: what happens when you type `git commit`

Trace it end to end.

**1. Git looks for hooks.** It reads `core.hooksPath` — set to `.githooks` by
`doctor` — and finds `.githooks/pre-commit`.

**2. `pre-commit` runs the guard** (`.githooks/pre-commit:19`):
`python tools/guard/guard.py --staged`.

**3. The guard collects staged content** (`guard.py`, `scan_staged`). Note it uses
`git show :file` rather than reading from disk — it must check what you are
*committing*, which is not necessarily what is in your working directory.

**4. Every line goes through three checks** (`guard.py`, `scan_text`):

- **Structural patterns** — magnet URIs, bare infohashes, tracker paths,
  non-empty default-source config keys. Skipped for the four files that must contain
  these patterns to define them; that exemption list is pinned by the self-test so it
  cannot quietly grow.
- **The hashed denylist** — every word-like token is normalised (lowercased, scheme
  and `www.` stripped, punctuation trimmed), hashed with the salt, and compared.
  `normalise_token` must behave identically here and in `add_token.py`, or nothing
  ever matches. That coupling is the fragile part of the design.
- **Domains** — URL hosts always, bare domains only outside source files. Each must
  be RFC 2606 reserved, recognised infrastructure, or on `allowlist.txt`. Anything
  else is a finding.

**5. Findings abort the commit.** Exit 1 stops git, and you see the file, line, rule
and a fix.

**6. The secret scan runs** (`.githooks/pre-commit:23`) — gitleaks if installed,
otherwise `secretscan.py`. Note it never prints a suspected secret in full: `redact`
shows four characters and a length, because printing a secret copies it into your
terminal history.

**7. `commit-msg` checks the message** against the Conventional Commits pattern and
a 72-character subject limit.

**8. CI does it again, harder.** On push, `.github/workflows/ci.yml` runs the
self-test, a tree scan, and a **full history** scan — every blob ever committed —
plus a pinned gitleaks. The `posture` job has no `needs:`, so it reports even when
everything else is broken.

**Worth noticing:** the same guard runs at three depths — staged locally, tree in
CI, full history in CI. The local check stays fast so it does not get bypassed; the
expensive check runs where slowness does not matter.

---

## 5. Questions to check yourself

Answer these out loud, in your own words. Not "yeah I get it" — explain it back.

**1.** The guard's denylist stores hashes rather than names. Why can it not just
store the names? And why is calling this "security" wrong?

**2.** Your friend clones the repo and commits a file with a tracker URL in it. The
commit succeeds and nothing complains. What went wrong, and what still catches it?

**3.** The first real run of the guard produced 118 findings, all false. Why is that
a *security* problem and not merely an annoying one?

**4.** `PROJECT_STATE.json` says an exit criterion is met and the evidence field
reads `"tested manually"`. Two separate mechanisms should stop this from being
committed. What are they?

**5.** `SPEC.md` §12.5 said to keep a denylist of site names, and §2.1 said site
names must never appear in the repository. Both are rules in the same document.
What did we do about that, and why was writing an ADR the necessary first step
rather than just fixing the code?
