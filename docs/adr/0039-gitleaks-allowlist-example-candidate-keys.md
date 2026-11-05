# 0039 — Two example candidate keys are gitleaks false positives

- **Status:** Accepted 2026-11-05
- **Date:** 2026-11-05
- **Phase:** 6
- **Under:** [ADR-0035](0035-gitleaks-false-positives-by-value.md) (one ADR per allowlisted value)

## Context

CI's authoritative secret scan (`gitleaks detect` over the full history) failed on commit
`4e81c94`. Its `generic-api-key` rule reads `"key": "<value>"` as a credential when the
value is varied enough, and the addon protocol's candidates have a field that is
genuinely called `key` — an opaque handle the addon gets back in *resolve*
(`docs/specs/addon-protocol.md` §3). The flagged values are the examples in that
document and in `tools/protocol/example_addon.py`:

- `nosferatu-1922-sd`
- `nosferatu-1922`

Renaming the examples did not help: `films/nosferatu/sd` was flagged too. And a new
commit cannot fix it, because the scan reads history.

## Decision

Add exactly those two values to `.gitleaks.toml`, anchored `^…$`, for `generic-api-key`
only. Neither is or ever was a credential: both are a film title, a year and a quality
word, written as illustrations.

## Alternatives

- **Rewrite the pushed branch's history** to remove `4e81c94`'s examples. Destructive
  for a value that is not a secret; CLAUDE.md rule 14's history rewrite exists for real
  credentials, which are rotated first.
- **Rename the protocol field.** The field is right; the scanner's heuristic is what
  misreads it.

## Consequences

Future examples of a candidate key will be flagged the same way. Write them with low
entropy (`the-general-1926` passes), or add them here by value with a new ADR.
