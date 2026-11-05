# Internet Archive fixtures

Real responses from the Internet Archive's public APIs, fetched 2026-11-05 for the
query *Nosferatu*, so `crates/sources/tests/archive.rs` tests the backend against
what the Archive actually returns rather than what its documentation describes.

Two edits, and only these:

- **Thumbnails and item tiles were removed** from the `metadata-*.json` file lists
  (dozens of `.jpg` entries per item that the backend ignores by format).
- **Licence URLs point at `licence.example`** instead of the Creative Commons site.
  The posture guard admits only reserved or allowlisted domains (ADR-0009, ADR-0010),
  and a licence link is neither a content source nor worth an allowlist entry. The
  path after the domain is unchanged, so the value still reads as the licence it was.

Nothing else is changed: titles, identifiers, years, formats, sizes and heights are
as served.
