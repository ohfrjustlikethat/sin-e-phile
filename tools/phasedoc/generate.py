#!/usr/bin/env python3
"""Generate and maintain `docs/phases/phase-NN-<slug>.md`.

WHY THIS EXISTS. The lean profile (ADR-0016 A5) removed per-phase documents, and the
session-start ritual kept pointing at files that were never created again. A session
resuming cold followed a pointer to nothing, and the drift went unnoticed for two
phases. The author reinstated them (2026-09-01) with a narrower job:

    **This is the single file a session reads to know what it is doing.**

It is a working file, not a document. Terse, bullets, no prose. Three lifecycle
stages:

    open   generated at phase start from SPEC.md §15 — goal, deliverables, exit
           criteria as a live checklist, dependencies, risks
    log    appended during the phase — one line per completed subtask, with commit
    close  written at phase end — outcome, evidence per criterion, deviations,
           debt incurred, and an unambiguous "next phase starts by"

Python 3.12, stdlib only (ADR-0012).

    python tools/phasedoc/generate.py --open 4
    python tools/phasedoc/generate.py --log 4
    python tools/phasedoc/generate.py --close 4
    python tools/phasedoc/generate.py --check
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
SPEC = REPO / "SPEC.md"
STATE = REPO / "PROJECT_STATE.json"
PHASES = REPO / "docs" / "phases"
RISKS = REPO / "docs" / "RISKS.md"

MARK_LOG = "<!-- subtask log: appended during the phase -->"
MARK_CLOSE = "<!-- closed: written at phase end -->"


def load_state() -> dict:
    return json.loads(STATE.read_text(encoding="utf-8"))


def phase_record(state: dict, number: int) -> dict:
    for phase in state["phases"]:
        if phase["number"] == number:
            return phase
    raise SystemExit(f"phasedoc: no phase {number} in PROJECT_STATE.json")


def spec_section(number: int) -> str:
    """The §15 entry for one phase, verbatim.

    Anchored on the heading rather than an index, because §15's phases are the
    authoritative source (amendment 12) and their order must not be assumed.
    """
    text = SPEC.read_text(encoding="utf-8")
    start = re.search(rf"^### Phase {number} — .+$", text, re.M)
    if not start:
        raise SystemExit(f"phasedoc: SPEC.md §15 has no '### Phase {number} — ...' heading")
    rest = text[start.start():]
    nxt = re.search(r"^### Phase \d+ — ", rest[1:], re.M)
    return rest[: nxt.start() + 1] if nxt else rest


def field(section: str, label: str) -> str:
    """Pull a `**Label.** ...` paragraph out of the spec section."""
    match = re.search(rf"\*\*{label}\.?\*\*\s*(.+?)(?=\n\n|\n\*\*|\Z)", section, re.S)
    return " ".join(match.group(1).split()) if match else ""


def dependencies(section: str) -> str:
    match = re.search(r"\*\*Depends on:\*\*\s*([^.]+)\.", section)
    return match.group(1).strip() if match else "nothing"


def sessions(section: str) -> str:
    match = re.search(r"\*\*Sessions:\*\*\s*([^.\n]+)", section)
    return match.group(1).strip() if match else "?"


def risks_for(section: str, number: int) -> list[str]:
    """Risks this phase carries, looked up in BOTH directions.

    A phase entry naming R3 is the obvious case. The important one is the reverse:
    R4's owner row says "Phase 4" and the Phase 4 entry never mentions R4 at all.
    Scanning only the phase text produced an empty Risks section for the phase whose
    named risk is the entire reason to measure before committing to a shape.
    """
    if not RISKS.exists():
        return []
    risk_text = RISKS.read_text(encoding="utf-8")

    # `## R4 — Catalogue ingestion is far larger or slower than expected`, followed
    # by a small table whose Owner row names the phases that carry it.
    blocks = re.split(r"^## (R\d+) — ", risk_text, flags=re.M)
    titles: dict[str, str] = {}
    owners: dict[str, str] = {}
    for i in range(1, len(blocks), 2):
        rid, body = blocks[i], blocks[i + 1]
        titles[rid] = body.splitlines()[0].strip()
        owner = re.search(r"\|\s*\*\*Owner\*\*\s*\|\s*(.+?)\s*\|", body)
        owners[rid] = owner.group(1) if owner else ""

    ids = {r for r in re.findall(r"\bR\d+\b", section) if r in titles}
    ids |= {r for r, owner in owners.items() if re.search(rf"\bPhase {number}\b", owner)}

    return [
        f"- **{rid}** — {titles[rid]}"
        for rid in sorted(ids, key=lambda r: int(r[1:]))
    ]


def doc_path(phase: dict) -> Path:
    return PHASES / f"phase-{phase['number']:02d}-{phase['slug']}.md"


# ---------------------------------------------------------------------------
# open
# ---------------------------------------------------------------------------

def render_open(phase: dict) -> str:
    number = phase["number"]
    section = spec_section(number)
    risks = risks_for(section, number)

    lines = [
        f"# Phase {number} — {phase['title']}",
        "",
        f"**Status:** {phase['status']} · **Depends on:** {dependencies(section)} · "
        f"**Sessions:** {sessions(section)}",
        "",
        "> The single file a session reads to know what it is doing. Generated from",
        "> `SPEC.md` §15 by `tools/phasedoc/generate.py`. Working file, not a document.",
        "",
        "## Goal",
        "",
        field(section, "Goal") or "_not stated in SPEC.md §15_",
        "",
        "## Deliverables",
        "",
        field(section, "Deliverables") or "_not stated in SPEC.md §15_",
        "",
        "## Exit criteria",
        "",
    ]

    for criterion in phase["exit_criteria"]:
        box = "x" if criterion["met"] else " "
        lines.append(f"- [{box}] **{criterion['id']}** {criterion['text']}")

    lines += ["", "## Subtasks", ""]
    if phase.get("subtasks"):
        for subtask in phase["subtasks"]:
            box = "x" if subtask["status"] == "complete" else " "
            lines.append(f"- [{box}] **{subtask['id']}** {subtask['text']}")
    else:
        lines.append("_none authored yet — write them when the phase is planned._")

    if risks:
        lines += ["", "## Risks named by this phase", ""] + risks

    lines += [
        "",
        "## Learning note",
        "",
        field(section, "Learning note") or "_not stated_",
        "",
        "---",
        "",
        MARK_LOG,
        "",
        "## Work log",
        "",
    ]
    return "\n".join(lines) + "\n"


def do_open(number: int, force: bool) -> int:
    state = load_state()
    phase = phase_record(state, number)
    path = doc_path(phase)
    PHASES.mkdir(parents=True, exist_ok=True)

    if path.exists() and not force:
        print(f"phasedoc: {path.relative_to(REPO)} already exists (use --force to regenerate)")
        return 0

    # --force overwrites. Phase 0's document was hand-written before this tool
    # existed and held a retrospective that regeneration destroyed; it was restored
    # from git. A document with no generator marker was not produced by this tool,
    # so refuse to overwrite it without being told twice.
    if path.exists() and force and MARK_LOG not in path.read_text(encoding="utf-8"):
        print(
            f"phasedoc: {path.relative_to(REPO)} was not generated by this tool and may "
            f"contain hand-written content. Move it aside first if you really mean to "
            f"replace it.",
            file=sys.stderr,
        )
        return 1

    path.write_text(render_open(phase), encoding="utf-8")
    print(f"phasedoc: wrote {path.relative_to(REPO)}")
    return 0


# ---------------------------------------------------------------------------
# log — refresh the checklists and append newly-completed subtasks
# ---------------------------------------------------------------------------

def do_log(number: int) -> int:
    state = load_state()
    phase = phase_record(state, number)
    path = doc_path(phase)
    if not path.exists():
        return do_open(number, force=False)

    text = path.read_text(encoding="utf-8")
    head, _, tail = text.partition(MARK_LOG)

    # Regenerate the head so the checklists track PROJECT_STATE.json rather than
    # drifting from it — the drift is the whole failure this file exists to prevent.
    fresh = render_open(phase)
    new_head = fresh.partition(MARK_LOG)[0]

    logged = {m.group(1) for m in re.finditer(r"^- \*\*(\d+\.\d+)\*\*", tail, re.M)}
    additions = [
        f"- **{s['id']}** {s['text'][:96]}"
        + (f" · `{s['commit']}`" if s.get("commit") else "")
        for s in phase.get("subtasks", [])
        if s["status"] == "complete" and s["id"] not in logged
    ]

    body = tail.rstrip() + ("\n" + "\n".join(additions) if additions else "")
    path.write_text(new_head + MARK_LOG + body + "\n", encoding="utf-8")
    print(
        f"phasedoc: refreshed {path.relative_to(REPO)}"
        + (f", logged {len(additions)} subtask(s)" if additions else "")
    )
    return 0


# ---------------------------------------------------------------------------
# close
# ---------------------------------------------------------------------------

def do_close(number: int, next_action: str | None) -> int:
    state = load_state()
    phase = phase_record(state, number)
    path = doc_path(phase)
    if not path.exists():
        do_open(number, force=False)

    unevidenced = [
        c["id"] for c in phase["exit_criteria"]
        if not c["met"] or not (c.get("evidence") or "").strip()
    ]
    if unevidenced:
        print(
            f"phasedoc: refusing to close phase {number} — "
            f"{', '.join(unevidenced)} lack evidence (SPEC.md §10.8).",
            file=sys.stderr,
        )
        return 1

    do_log(number)
    text = path.read_text(encoding="utf-8")
    if MARK_CLOSE in text:
        text = text.partition(MARK_CLOSE)[0].rstrip() + "\n\n"

    debt = [d for d in state.get("known_debt", []) if d.get("phase_raised") == number]
    action = next_action or state.get("next_action", "")

    lines = [
        "",
        MARK_CLOSE,
        "",
        "---",
        "",
        "## Outcome",
        "",
        f"**Complete.** Merged as `{phase.get('completion_commit') or 'pending'}`.",
        "",
        "### Evidence per criterion",
        "",
    ]
    for criterion in phase["exit_criteria"]:
        lines += [f"**{criterion['id']}** — {criterion['text']}", "", f"> {criterion['evidence']}", ""]

    if debt:
        lines += ["### Debt incurred", ""]
        lines += [f"- **{d['id']}** {d['text']}" for d in debt]
        lines.append("")

    lines += [
        "### Next phase starts by",
        "",
        action or "_not set — this is a bug; `next_action` must be an instruction._",
        "",
    ]

    path.write_text(text.rstrip() + "\n" + "\n".join(lines), encoding="utf-8")
    print(f"phasedoc: closed {path.relative_to(REPO)}")
    return 0


# ---------------------------------------------------------------------------
# check
# ---------------------------------------------------------------------------

def do_check() -> int:
    state = load_state()
    current = state["current_phase"]
    problems: list[str] = []

    phase = phase_record(state, current["number"])
    path = doc_path(phase)
    if not path.exists():
        problems.append(
            f"the current phase has no document: {path.relative_to(REPO)} does not exist. "
            f"Run: python tools/phasedoc/generate.py --open {current['number']}"
        )
    else:
        text = path.read_text(encoding="utf-8")
        heading = f"# Phase {current['number']} — {current['title']}"
        if not text.startswith(heading):
            problems.append(
                f"{path.relative_to(REPO)} does not match current_phase "
                f"(expected heading {heading!r})"
            )
        for criterion in current["exit_criteria"]:
            box = "x" if criterion["met"] else " "
            if f"- [{box}] **{criterion['id']}**" not in text:
                problems.append(
                    f"{path.relative_to(REPO)}: {criterion['id']} checkbox disagrees with "
                    f"PROJECT_STATE.json. Run: python tools/phasedoc/generate.py --log "
                    f"{current['number']}"
                )

    for message in problems:
        print(f"  {message}", file=sys.stderr)
    return 1 if problems else 0


def main() -> int:
    parser = argparse.ArgumentParser(description="phase document lifecycle")
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--open", type=int, metavar="N", help="generate the doc at phase start")
    mode.add_argument("--log", type=int, metavar="N", help="refresh checklists, log subtasks")
    mode.add_argument("--close", type=int, metavar="N", help="write the closing section")
    mode.add_argument("--check", action="store_true", help="doc exists and matches state")
    parser.add_argument("--force", action="store_true", help="overwrite an existing doc")
    parser.add_argument("--next-action", help="override the 'next phase starts by' line")
    args = parser.parse_args()

    if args.check:
        return do_check()
    if args.open is not None:
        return do_open(args.open, args.force)
    if args.log is not None:
        return do_log(args.log)
    return do_close(args.close, args.next_action)


if __name__ == "__main__":
    raise SystemExit(main())
