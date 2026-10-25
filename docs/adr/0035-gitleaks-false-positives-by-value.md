# 0035 — gitleaks false positives are allowlisted by exact value, and only with an ADR

- **Status:** Accepted
- **Date:** 2026-10-23
- **Phase:** 5
- **Resolves:** blocker B6
- **Amends:** ADR-0012 ("the escape hatch is local and temporary, never permanent")

## Context

CI's full-history gitleaks scan (`.github/workflows/ci.yml:75`) failed every run from
2026-09-04 to 2026-10-16. The step printed only a count, so nobody could act on it.
On 2026-10-23 the author installed gitleaks 8.30.1 (the version CI pins) and ran the CI
command with `-v`. It found exactly the five findings CI counts, all from the
`generic-api-key` rule:

| Commit | File:line | Text |
|---|---|---|
| `396f256` | `crates/embedder/src/lib.rs:118` | doc comment: the word "tokens", a comma, then the next step's name |
| `554c50d` | `tools/ingest/src/embed.rs:351` | the same doc comment |
| `490fe94` | `tools/ingest/src/embed.rs:300` | the same doc comment, earlier commit |
| `2358993` | `crates/persistence/src/secrets.rs:134` | DPAPI test vector `abcdef0123456789` ×2 |
| `2358993` | `crates/persistence/src/secrets.rs:143` | the same test vector |

None of them was ever a credential, so there is nothing to rotate. The hypothesis B6
recorded (the 64-hex checksum constants) was wrong, which is why B6 refused to act on it.

ADR-0012 gives no permanent way to clear a confirmed false positive. Without one, the
scan can only stay red, and a red scan stops being read.

## Decision

`.gitleaks.toml` at the repository root keeps gitleaks' default rules unchanged and adds
one allowlist. Each entry is a confirmed false-positive **value**, anchored `^…$` and
scoped to the one rule that flagged it.

- **One ADR per addition**, the same rule `tools/guard/allowlist.txt` has (ADR-0010).
  This ADR covers the two values above: the word after "tokens" in that doc comment,
  and the test vector.
- **Whole values, never rules or paths.** A real key cannot equal either value, and any
  near-miss is still a finding. This was checked: a probe file holding a realistic key,
  the test vector plus one character, and `L2-normalise` plus a suffix produced three
  findings.
- **Never for a real secret.** A real secret pushed to the repository is rotated first
  and then removed from history (CLAUDE.md rule 14). It does not go in this file.
- CI runs gitleaks with `-v`, so a finding is named in the log instead of only counted.
- If the text can be changed in the current tree, change it as well. The test vector in
  `secrets.rs` is now `"test-only-not-a-secret"`.

## Alternatives Considered

**`.gitleaksignore` with commit-pinned fingerprints.** This was the first choice, and
the author approved it as option A. It failed at commit time: a fingerprint has to carry
a full 40-hex commit SHA, and the posture guard's `infohash-hex` rule correctly reads
that as infohash-shaped (`SPEC.md` §2.1). Exempting the file from the guard would widen
`STRUCTURAL_EXEMPT`, which R10 forbids. Fingerprints without the commit
(`file:rule:line`) would hide any future value on those lines, including a real key, so
they are less safe than whole values.

**Rewrite history to remove the strings.** Rejected: it changes every SHA recorded as
evidence in `PROJECT_STATE.json`, every `completion_commit` and every `phase-NN` tag, and
it needs a force-push. That cost buys nothing when the strings were never secrets.

**Disable `generic-api-key`.** Rejected: that rule is the one that would catch an
unknown provider's key, so disabling it hides real leaks along with these false
positives.

## Consequences

**Easier.** CI can go green again, and the gitleaks log names the file and line of
any future finding.

**Harder.** The repository now has a suppression file that a reviewer has to trust. The
same mitigation as the allowlist applies: every entry has a comment and an ADR, and the
file is short enough to read in full.
