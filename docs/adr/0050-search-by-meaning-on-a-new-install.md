# 0050 — Search by meaning downloads itself on a new install

- **Status:** Accepted 2026-11-17 (the author's decision, round 8).
- **Date:** 2026-11-17
- **Phase:** 16
- **Amends:** ADR-0014 (consent for static downloads), `SPEC.md` §2.7; spec_version 4.4.0.
- **Source:** `docs/design/author-brief-2026-11-17.md`, round 8.

## Context

ADR-0014 lets the app fetch static assets (the IMDb datasets, the search model, the published embedding artefact)
but requires each optional one to be consented to, with its size shown, before it happens. Search by meaning was
therefore a button: in onboarding (since 4cc59d2) and under the search results.

The author reported "search by meaning still does not work". Their install had never fetched it: setup was finished
before the onboarding offer existed, and the offer under the results appears only after a search, below the hits,
where it was not noticed. Asked how it should arrive, the author chose: **a new install downloads it during setup;
an existing install gets a clear offer instead** (on the search page and in Settings, with its size).

## Decision

- **A new install** (the first catalogue build, `FIRST_BUILD`) starts the download itself as soon as the catalogue's
  stage A is done (the meaning index is built over the catalogue's titles). Setup says so, with the size, while it
  happens: setting up a new install is consent to what setup says it fetches, as it already is for the datasets.
- **An existing install** is offered it, never fetched unasked: at the top of the search page before anything is
  typed, and in Settings → Library, each with the size and one button.
- Nothing else changes: the download is the same pinned, checksummed artefact (ADR-0014), and search works without it.

## Consequences

- §2.7's "never fetched in the background, never assumed" gains one stated exception, a new install's setup.
- A first run on a slow connection spends 364 MB more during setup; search by keyword works throughout.
- One download at a time: pressing the button while setup's download runs shows that download, not a second.
