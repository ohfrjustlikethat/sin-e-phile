# 0046 — An improvement backlog, scheduled into the phases

- **Status:** Accepted 2026-11-16. Asked for by the author: "suggest me any and all improvements
  that can be made for the app to make it function better, or have it look better, or so its
  usability is better, can be a very small change, or can be an added feature. after you have
  suggested everything, save it to the spec.md and start working on finishing the app".
- **Date:** 2026-11-16
- **Phase:** 16 onward
- **Amends:** `SPEC.md` §15 (preamble), new Appendix F; spec_version 4.1.0

## Context

- The app was reviewed as a person would use it (the release build's screens, the pad flow's
  screenshots, the known-debt list, and Phases 17–28 as specified).
- 78 improvements came out: small fixes (a search row whose labels touch the focus ring), missing
  everyday functions (mark as watched, a sleep timer, a working Watchlist page), and features
  (local artwork without a key, `.nfo` identification, a system status page).
- A list kept outside the spec is forgotten; a list with no schedule is never built; and a list
  bolted onto exit criteria would redefine what each phase's "done" means (rule 11).

## Decision

- **`SPEC.md` Appendix F holds the backlog**: one row per item with an id (`I01`–`I78`), the phase
  that builds it, a size, and a Done column.
- **Each item is a deliverable of its phase, not an exit criterion.** The phase plan lists its
  items; `tools/phasedoc` prints them in the phase doc so a session reading only its two files
  sees them.
- **At phase close every item is either done (commit in the Done column) or carried** to a named
  later phase with a reason in the retrospective. Never dropped silently. Filling Done or moving
  an item's phase is bookkeeping, not an amendment.
- **Seven items are built now, in Phase 16**, because they are what a person using the release
  build meets first: `I01`, `I21`, `I33`, `I38`, `I39`, `I71`, `I72`.
- **Three items are marked ⚑**: taste calls, or a change to an earlier decision. They are shown
  both ways in Phase 19's mockups and the author chooses; the default is no change.
- **Constraints win.** No item may breach §2.1 (no content sources), §9.0, a locked technology or
  a performance budget; an item that turns out to is cut with a note, not bent to fit.

## Consequences

- Phases 17–28 each grow by a handful of mostly small items; Phases 19, 21 and 24 grow most.
- The schedule (ADR-0043: everything, at full depth) is unchanged; the items ride on phases that
  already touch the same code.
- Found during the review and **not** an improvement: Phase 16's picture panel has brightness,
  contrast and saturation but not the **aspect and zoom** the phase specifies. That is a gap in
  Phase 16 itself and is fixed there.

## Alternatives considered

- **A separate `docs/IMPROVEMENTS.md`.** Rejected: the author asked for the spec, and a second
  list drifts from the first (debt D23's failure).
- **One exit criterion per item.** Rejected: 78 new criteria would make every remaining phase's
  definition of done about polish, and force a carry for each one that slips.
- **A new "polish" phase at the end.** Rejected: that is where improvements go to die, and most
  are cheapest while the file is already open.
