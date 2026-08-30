# 0009 — Posture guard denylist design

- **Status:** Accepted
- **Date:** 2026-08-30
- **Phase:** 0
- **Amends:** `SPEC.md` §12.5
- **Risk:** R10

## Context

`SPEC.md` §2.1 states that indexer and tracker site names, and their common
abbreviations, must never appear in this repository — not in code, config, test
fixtures, documentation, or git history. §12.5 then specifies that the posture
guard enforces this using "a denylist of site names and their common
abbreviations, maintained in `tools/guard/denylist.txt`".

These two requirements contradict each other. To detect a forbidden token by
literal comparison, the literal token must be written down in the repository. A
bare site name with no scheme and no path is still a site name; committing one to
`denylist.txt` is committing the exact class of string §2.1 forbids, and git
history makes it permanent.

§12.5's parenthetical — "the denylist file itself contains patterns, not working
addresses" — identifies the tension without resolving it. A pattern that matches a
specific site necessarily encodes that site.

A second problem surfaced while designing the guard's own tests. Phase 0's exit
criteria require proving that the guard fires on a deliberately planted string. If
that string were a real forbidden token, then verifying the guard would require
committing the very thing the guard exists to prevent — even briefly, and even
reverted, it lands in history.

## Decision

**Two matchers, with different visibility properties.**

### 1. A hashed exact-token matcher

`tools/guard/denylist.txt` stores `SHA-256(salt || normalised_token)` in hex, one
per line, each with a short non-identifying comment (`# tracker index, added
ADR-00NN`). The salt lives in `tools/guard/denylist.salt`, committed alongside. The
guard normalises each candidate token from the scanned text — lowercase, strip
scheme, strip `www.`, strip punctuation — hashes it identically, and compares.

This is **hygiene, not security**, and the distinction is load-bearing. A committed
salt is brute-forceable by anyone determined; the candidate space of plausible site
names is small enough to enumerate offline. The threat model being addressed is
accidental plaintext and casual discovery — a grep of the working tree, a GitHub
code search, a hiring manager scrolling the file list — and against that threat
model it is completely effective. It is not, and must never be described as,
protection against a motivated adversary. Anyone who documents it otherwise has
misunderstood it.

### 2. A plaintext structural matcher

Patterns that describe the *shape* of forbidden content without naming any instance
of it, and which are therefore safe to commit in the clear:

- `magnet:?xt=urn:btih:` URIs
- bare 40-character hex and 32-character base32 infohashes
- `/announce` and `/scrape` URL path shapes
- HTTP(S) URLs ending in `.torrent`
- configuration keys shaped like `default_source_url`, `default_indexer`,
  `default_catalogue_url` carrying any non-empty value

The structural matcher does the majority of the real work. It generalises to sites
nobody has thought of, which the hashed matcher by construction cannot.

### 3. RFC 2606 reserved domains for every test vector, fixture and example

Every guard self-test, every denylist test case, and every documentation example
uses `.invalid`, `.test`, `.example`, or `example.com`. A string such as
`notarealindexer.invalid` is *provably* not a real site — RFC 2606 permanently
reserves these TLDs so that no registry can ever issue them — which means the guard
can be proven to work without a real forbidden string ever entering the repository
or its history.

This also makes documentation examples mechanically checkable: any example URL that
is neither on a reserved TLD nor on the allowlist (ADR-0010) is itself a finding.

## Consequences

**Easier.** §2.1 and §12.5 stop contradicting each other. The guard becomes
testable, and Phase 0's "prove it fires" exit criterion is satisfiable without
self-inflicted history damage.

**Harder — and this is the real cost.** A hashed denylist can only do **exact token
matching**. It cannot do substring, fuzzy, leetspeak, or homoglyph matching, because
all of those require plaintext to compare against. A site name embedded inside a
longer identifier, or misspelled by one character, passes the hashed matcher
silently. This is accepted deliberately: the structural matcher covers the shapes
that actually carry risk, and the residual gap — a bare name in prose — is both the
least likely erosion path and the least damaging.

**Harder.** Nobody can read `denylist.txt` and tell what is on it. Adding an entry
requires `tools/guard/add_token.py`, which reads from stdin without echoing and
appends only the hash. Reviewing a denylist change means trusting the accompanying
ADR. Acceptable trade for the file being safe to commit.

**Operational.** The salt must never be rotated casually. Rotating it invalidates
every existing hash and requires re-adding every token from plaintext that, by
design, no longer exists anywhere in the repository. Rotation is a deliberate,
ADR-gated act.

## Alternatives Considered

**Plaintext denylist, accepting the §2.1 violation.** Rejected: it inverts the
project's central legal posture in the one file most likely to be read by someone
evaluating that posture, and makes the repository trivially greppable for exactly
the list it must not contain.

**No denylist at all — structural patterns only.** Genuinely tempting, and close to
sufficient. Rejected because it cannot catch a bare site name in a comment, fixture,
or doc string, which is precisely the erosion path R10 names ("a source URL appears
in a fixture, a doc example, or a temporary default"). Structural matching is
retained as the primary matcher; the hashed list is the backstop.

**Denylist held outside the repository** — an environment variable, a private gist,
a CI secret. Rejected: the pre-commit hook must work on a fresh clone with no
configuration, or it will be bypassed. A guard that runs only in CI catches the
mistake after it is already in history, which is the expensive moment.

**Encrypted denylist with the key in CI secrets.** Rejected as strictly worse than
hashing: identical brute-force exposure once the key leaks, plus the pre-commit hook
can no longer run locally without the key.

**Bloom filter over tokens.** Rejected as needless complexity. No meaningful
obscurity advantage over salted hashes at this list size, and it introduces false
positives into a check that blocks commits — a false positive here is a developer
who cannot commit and cannot see why.
