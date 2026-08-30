#!/usr/bin/env python3
"""
Generate PROJECT_STATE.json's phase table by PARSING SPEC.md §15.

SPEC.md §10.1 requires PROJECT_STATE.json to enumerate all 28 phases with their
exit criteria. Transcribing ~150 checkboxes by hand would be tedious and, more to
the point, would silently drift the moment §15 is amended — and drift between the
spec and the state file is precisely what §2.8 exists to prevent.

So the phase skeleton is derived, not typed. Run this after any amendment to
§15 and the state file re-syncs.

  python tools/state/build_state.py --check    verify state matches SPEC.md (CI)
  python tools/state/build_state.py --write    regenerate, preserving live fields

PRESERVED across regeneration (these are session state, not spec content):
  status, completion_commit, subtasks, exit-criterion `met` and `evidence`,
  and everything outside the "phases" array.

Python 3.12+, standard library only (ADR-0012).
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
SPEC = REPO_ROOT / "SPEC.md"
STATE = REPO_ROOT / "PROJECT_STATE.json"

PHASE_HEADING = re.compile(r"^### Phase (\d+) — (.+?)\s*$")
DEPENDS = re.compile(r"\*\*Depends on:\*\*\s*(.+?)\.?\s*\*\*Sessions:\*\*\s*(.+?)\.?\s*$")
CRITERION = re.compile(r"^- \[[ xX]\] (.+?)\s*$")


def slugify(title: str) -> str:
    """Short branch-style slug matching SPEC.md's own convention.

    §10.5's example branch is `phase/07-torrent-engine` for a phase titled
    "Torrent Engine and Streaming Server", and §10.1's example state file uses
    the slug "torrent-engine". So: cut at an em-dash or at " and ", which yields
    the head noun phrase and reproduces the spec's examples exactly.

        'Torrent Engine and Streaming Server'            -> 'torrent-engine'
        'Player Core — **MILESTONE: FIRST DEMOABLE...**' -> 'player-core'
        'Bootstrap and Project Infrastructure'           -> 'bootstrap'
    """
    head = re.split(r"\s+—\s+|\s+-\s+|\s+and\s+", title)[0]
    head = re.sub(r"\*\*|`|:", "", head)
    return re.sub(r"[^a-z0-9]+", "-", head.lower()).strip("-")


def parse_spec() -> list[dict]:
    """Walk §15 and pull each phase's title, dependencies, sessions and criteria."""
    lines = SPEC.read_text(encoding="utf-8").split("\n")
    phases: list[dict] = []
    current: dict | None = None
    in_exit_block = False

    for line in lines:
        heading = PHASE_HEADING.match(line)
        if heading:
            if current:
                phases.append(current)
            number, title = int(heading.group(1)), heading.group(2)
            current = {
                "number": number,
                "slug": slugify(title),
                "title": re.sub(r"\*\*", "", title),
                "depends_on": None,
                "sessions": None,
                "status": "not_started",
                "completion_commit": None,
                "exit_criteria": [],
            }
            in_exit_block = False
            continue

        if current is None:
            continue

        dep = DEPENDS.search(line)
        if dep:
            current["depends_on"] = dep.group(1).strip()
            current["sessions"] = dep.group(2).strip()
            continue

        if line.startswith("**Exit criteria"):
            in_exit_block = True
            continue

        if in_exit_block:
            criterion = CRITERION.match(line)
            if criterion:
                index = len(current["exit_criteria"]) + 1
                current["exit_criteria"].append({
                    "id": f"E{index}",
                    "text": re.sub(r"\*\*", "", criterion.group(1)),
                    "met": False,
                    "evidence": None,
                })
            elif line.strip().startswith("**Learning note"):
                in_exit_block = False

    if current:
        phases.append(current)
    return phases


def merge(generated: list[dict], existing: list[dict]) -> list[dict]:
    """Overlay live session state onto the freshly-parsed spec skeleton."""
    by_number = {p.get("number"): p for p in existing if isinstance(p.get("number"), int)}
    for phase in generated:
        old = by_number.get(phase["number"])
        if not old:
            continue
        phase["status"] = old.get("status", phase["status"])
        phase["completion_commit"] = old.get("completion_commit")
        if "subtasks" in old:
            phase["subtasks"] = old["subtasks"]
        if "note" in old:
            phase["note"] = old["note"]
        old_criteria = {c.get("id"): c for c in old.get("exit_criteria", [])}
        for criterion in phase["exit_criteria"]:
            prior = old_criteria.get(criterion["id"])
            # Carry evidence forward ONLY when the criterion text is unchanged.
            # An amended criterion is a different claim and must be re-evidenced.
            if prior and prior.get("text") == criterion["text"]:
                criterion["met"] = prior.get("met", False)
                criterion["evidence"] = prior.get("evidence")
    return generated


def main() -> int:
    parser = argparse.ArgumentParser(description="Sync PROJECT_STATE.json phases with SPEC.md §15")
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--check", action="store_true", help="fail if state disagrees with the spec")
    mode.add_argument("--write", action="store_true", help="regenerate, preserving live fields")
    args = parser.parse_args()

    generated = parse_spec()
    if len(generated) != 28:
        print(f"build_state: parsed {len(generated)} phases from SPEC.md §15, expected 28. "
              f"Has the heading format changed?", file=sys.stderr)
        return 2

    state = json.loads(STATE.read_text(encoding="utf-8")) if STATE.exists() else {}
    merged = merge(generated, state.get("phases", []))

    if args.check:
        current = state.get("phases", [])
        spec_view = [{"number": p["number"], "slug": p["slug"],
                      "criteria": [c["text"] for c in p["exit_criteria"]]} for p in merged]
        state_view = [{"number": p.get("number"), "slug": p.get("slug"),
                       "criteria": [c.get("text") for c in p.get("exit_criteria", [])]}
                      for p in current]
        if spec_view != state_view:
            print("build_state: PROJECT_STATE.json disagrees with SPEC.md §15.\n"
                  "Run: python tools/state/build_state.py --write", file=sys.stderr)
            return 1
        total = sum(len(p["exit_criteria"]) for p in merged)
        print(f"build_state: in sync — 28 phases, {total} exit criteria")
        return 0

    state["phases"] = merged
    STATE.write_text(json.dumps(state, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    total = sum(len(p["exit_criteria"]) for p in merged)
    print(f"build_state: wrote 28 phases, {total} exit criteria to PROJECT_STATE.json")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
