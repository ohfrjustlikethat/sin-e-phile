#!/usr/bin/env python3
"""Fail if the repository cannot tell a cold session where it is.

WHY. Every one of these checks exists because the corresponding thing actually went
wrong in this project, silently, and was found by reading rather than by a tool:

  1. The session ritual pointed at `docs/phases/phase-NN-*.md` for two phases after
     those files stopped being generated.
  2. CI once failed on a stale `PROGRESS.md` that had been stale for a day.
  3. Phase 2's record sat at `not_started` with no evidence after Phase 2 was merged
     and tagged, because the finished record went to `current_phase` and was never
     copied back.
  4. A `next_action` said "continue the torrent engine", which is not an instruction.
  5. Work was finished and the state file was not updated — the failure mode the
     author called out directly: *"it makes 'finished work but forgot to record it'
     mechanically impossible rather than something you have to remember."*

Runs in CI and on pre-push. Python 3.12, stdlib only (ADR-0012).

    python tools/statecheck/check.py
    python tools/statecheck/check.py --selftest
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
STATE = REPO / "PROJECT_STATE.json"
PROGRESS = REPO / "PROGRESS.md"
PHASEDOC = REPO / "tools" / "phasedoc" / "generate.py"
VALIDATE = REPO / "tools" / "state" / "validate_state.py"

CODE_PATHS = ("src-tauri/", "src/", "crates/", "tools/ingest/")

# How far back "keeps pace" looks. Five is enough to catch a session that built
# something and never recorded it, and loose enough that a trailing comment fix does
# not demand its own state edit — see check_state_follows_code.
WINDOW = 5

BOLD, RED, GREEN, DIM, RESET = "\033[1m", "\033[31m", "\033[32m", "\033[2m", "\033[0m"


class Problem:
    def __init__(self, check: str, what: str, fix: str):
        self.check, self.what, self.fix = check, what, fix


def git(*args: str) -> str:
    result = subprocess.run(
        ["git", *args], cwd=REPO, capture_output=True, text=True,
        encoding="utf-8", errors="replace",
    )
    return result.stdout.strip() if result.returncode == 0 else ""


def load() -> dict:
    return json.loads(STATE.read_text(encoding="utf-8"))


# ---------------------------------------------------------------------------
# 0 — the state file is valid at all
# ---------------------------------------------------------------------------

def check_schema(_: dict) -> list[Problem]:
    """Run the schema and evidence validator.

    statecheck did not do this, so a subtask marked complete with a null commit
    reached CI and failed there instead of at pre-push. Pre-push is meant to be the
    stricter gate, not the looser one — anything CI checks about the state file
    should fail here first.
    """
    result = subprocess.run(
        [sys.executable, str(VALIDATE), "--check"],
        cwd=REPO, capture_output=True, text=True, encoding="utf-8", errors="replace",
    )
    if result.returncode == 0:
        return []
    detail = [
        line.strip() for line in (result.stderr or "").splitlines()
        if line.strip().startswith("$.")
    ]
    return [Problem(
        "state file schema",
        "; ".join(detail[:3]) or "validate_state --check failed",
        "python tools/state/validate_state.py --check",
    )]


# ---------------------------------------------------------------------------
# 1 — the current phase has a document, and it matches
# ---------------------------------------------------------------------------

def check_phase_doc(state: dict) -> list[Problem]:
    result = subprocess.run(
        [sys.executable, str(PHASEDOC), "--check"],
        cwd=REPO, capture_output=True, text=True, encoding="utf-8", errors="replace",
    )
    if result.returncode == 0:
        return []
    detail = (result.stderr or result.stdout).strip() or "phasedoc --check failed"
    return [Problem(
        "phase document",
        detail,
        f"python tools/phasedoc/generate.py --log {state['current_phase']['number']}",
    )]


# ---------------------------------------------------------------------------
# 2 — PROGRESS.md is what PROJECT_STATE.json generates
# ---------------------------------------------------------------------------

def check_progress(_: dict) -> list[Problem]:
    """Regenerate and compare, then put back what was there.

    A check that silently repaired the thing it checks would hide the drift it
    exists to report, so the original is restored either way and the fix is left
    to the caller.
    """
    # BYTES, not text. `read_text` normalises newlines and `write_text` translates
    # them back to the platform default, so the first version silently rewrote
    # PROGRESS.md from LF to CRLF on every run — and then compared normalised text,
    # which made it blind to the damage it was doing. CI saw a 118-line diff on a
    # file whose content had not changed at all.
    before = PROGRESS.read_bytes() if PROGRESS.exists() else b""
    subprocess.run(
        [sys.executable, str(VALIDATE), "--progress"],
        cwd=REPO, capture_output=True, text=True,
    )
    after = PROGRESS.read_bytes() if PROGRESS.exists() else b""
    if after != before:
        PROGRESS.write_bytes(before)
        return [Problem(
            "PROGRESS.md",
            "is out of sync with PROJECT_STATE.json",
            "python tools/state/validate_state.py --progress",
        )]
    return []


# ---------------------------------------------------------------------------
# 3 — every phase behind the current one is closed
# ---------------------------------------------------------------------------

def check_phases_closed(state: dict) -> list[Problem]:
    current = state["current_phase"]["number"]
    problems = []
    for phase in state["phases"]:
        n = phase["number"]
        if n >= current:
            continue
        if phase.get("status") == "skipped":
            continue
        if phase.get("status") != "complete":
            problems.append(Problem(
                "closed phases",
                f"phase {n} is '{phase.get('status')}' but current_phase is {current}",
                "close it, or mark it skipped with a skip_reason",
            ))
        if not (phase.get("completion_commit") or "").strip():
            problems.append(Problem(
                "closed phases",
                f"phase {n} has no completion_commit",
                "set it to the merge commit that closed the phase",
            ))
    return problems


# ---------------------------------------------------------------------------
# 4 — next_action is an instruction, not a gesture
# ---------------------------------------------------------------------------

# Openers that are a direction rather than an action. "Continue the torrent engine"
# is the real example from SPEC.md §10's own warning about this.
VAGUE_OPENERS = re.compile(
    r"^\s*(continue|carry on|keep going|work on|finish|resume|proceed|carry\b|"
    r"pick up|move on|start work)\b",
    re.I,
)
# Something a session can actually open: a path, or a spec/ADR reference.
CONCRETE = re.compile(
    r"(?:[\w./-]+/[\w.-]+\.\w+)"          # a path with an extension
    r"|(?:SPEC\.md\s*[§\d])"               # a spec section
    r"|(?:ADR-\d{4})"                      # an ADR
    r"|(?:docs/[\w./-]+)"                  # anything under docs/
    r"|(?:tools/[\w./-]+)"                 # anything under tools/
    r"|(?:`[^`]+`)",                       # an explicitly quoted symbol or command
    re.I,
)


def check_next_action(state: dict) -> list[Problem]:
    action = (state.get("next_action") or "").strip()
    fix = ("Write it as an instruction naming a file and an approach. Not 'continue "
           "the torrent engine' but 'implement SubtitleAligner::estimate_framerate_scale "
           "in crates/subtitle-align/src/lib.rs; approach in docs/specs/"
           "subtitle-alignment.md §3'.")

    if not action:
        return [Problem("next_action", "is empty", fix)]
    if len(action) < 40:
        return [Problem("next_action", f"is too short to be an instruction: {action!r}", fix)]
    if VAGUE_OPENERS.match(action) and not CONCRETE.search(action):
        return [Problem("next_action", f"is a direction, not an instruction: {action[:70]!r}", fix)]
    if not CONCRETE.search(action):
        return [Problem(
            "next_action",
            "names no file, command, spec section or ADR — a session cannot act on it",
            fix,
        )]
    return []


# ---------------------------------------------------------------------------
# 5 — code changes are recorded in the state file
# ---------------------------------------------------------------------------

def check_state_follows_code(_: dict) -> list[Problem]:
    """State must keep pace with code: no run of commits changes code in silence.

    The author's framing: this makes "finished work but forgot to record it"
    mechanically impossible rather than something to remember.

    The rule is a WINDOW, not a strict ordering, and that is a deliberate weakening
    made after the first version fired on real work. Requiring the most recent code
    commit specifically to touch the state file meant a trailing comment fix in a
    migration — work that was already fully recorded one commit earlier — was
    refused at push. A rule that demands a meaningless state edit to get past it is
    a rule that gets `--no-verify`'d within a week, and then protects nothing.

    So: if any of the last WINDOW commits changed code, at least one of them must
    have touched PROJECT_STATE.json. A session cannot build for a whole run of
    commits without recording it, and a fixup does not need its own ceremony.
    """
    window = git("log", f"-{WINDOW}", "--format=%H").splitlines()
    if not window:
        return []

    touched_code: list[str] = []
    touched_state = False
    for sha in window:
        files = [f for f in git("show", "--name-only", "--format=", sha).splitlines() if f]
        if any(f.startswith(CODE_PATHS) for f in files):
            touched_code.append(sha)
        if "PROJECT_STATE.json" in files:
            touched_state = True

    if not touched_code or touched_state:
        return []

    subjects = [git("log", "-1", "--format=%s", sha)[:44] for sha in touched_code[:3]]
    return [Problem(
        "state follows code",
        f"{len(touched_code)} of the last {WINDOW} commits changed code and none "
        f"touched PROJECT_STATE.json: " + "; ".join(f"{s!r}" for s in subjects),
        "update subtask status, evidence and next_action, then commit the state file",
    )]


CHECKS = [
    ("state file is schema-valid", check_schema),
    ("phase document exists and matches", check_phase_doc),
    ("PROGRESS.md in sync", check_progress),
    ("phases behind current are closed", check_phases_closed),
    ("next_action is an instruction", check_next_action),
    ("state follows code", check_state_follows_code),
]


def run() -> int:
    state = load()
    problems: list[Problem] = []

    print(f"{BOLD}statecheck{RESET}  {DIM}can a cold session tell where it is?{RESET}\n")
    for label, check in CHECKS:
        found = check(state)
        problems += found
        mark = f"{RED}FAIL{RESET}" if found else f"{GREEN}ok  {RESET}"
        print(f"  {mark}  {label}")

    if not problems:
        current = state["current_phase"]
        print(f"\nstatecheck: clean — phase {current['number']} "
              f"({current['slug']}), state is resumable\n")
        return 0

    print(f"\n{RED}{len(problems)} problem(s):{RESET}\n", file=sys.stderr)
    for problem in problems:
        print(f"  {BOLD}{problem.check}{RESET} — {problem.what}", file=sys.stderr)
        print(f"      {DIM}fix:{RESET} {problem.fix}\n", file=sys.stderr)
    return 1


# ---------------------------------------------------------------------------
# selftest — every check must be seen to fail
# ---------------------------------------------------------------------------

def selftest() -> int:
    """A check never seen to fail is not evidence (CLAUDE.md, evidence protocol)."""
    state = load()
    failures = []
    print(f"{BOLD}statecheck selftest{RESET}\n")

    cases = [
        ("next_action empty", check_next_action, {"next_action": ""}),
        ("next_action vague", check_next_action,
         {"next_action": "continue the torrent engine and keep going with it please"}),
        ("next_action has no file", check_next_action,
         {"next_action": "implement the thing that was discussed in the previous session"}),
        ("phase behind is not complete", check_phases_closed,
         {"current_phase": {"number": 2}, "phases": [
             {"number": 1, "status": "in_progress", "completion_commit": "abc1234"}]}),
        ("phase behind has no commit", check_phases_closed,
         {"current_phase": {"number": 2}, "phases": [
             {"number": 1, "status": "complete", "completion_commit": ""}]}),
        ("skipped phase is allowed", check_phases_closed,
         {"current_phase": {"number": 2}, "phases": [
             {"number": 1, "status": "skipped", "completion_commit": ""}]}),
    ]

    for label, check, payload in cases:
        should_fire = label != "skipped phase is allowed"
        fired = bool(check({**state, **payload}))
        ok = fired == should_fire
        print(f"  {GREEN + 'ok  ' + RESET if ok else RED + 'FAIL' + RESET}  {label}"
              f"  {DIM}({'fires' if should_fire else 'stays quiet'}){RESET}")
        if not ok:
            failures.append(label)

    # And the real ones, which must be quiet on a healthy tree.
    for label, check in CHECKS:
        if check in (check_next_action, check_phases_closed):  # covered above
            continue
        if check(state):
            failures.append(f"{label} fires on a healthy tree")
            print(f"  {RED}FAIL{RESET}  {label} fires on a healthy tree")
        else:
            print(f"  {GREEN}ok  {RESET}  {label}  {DIM}(quiet when healthy){RESET}")

    if failures:
        print(f"\n{RED}statecheck selftest: {len(failures)} failure(s){RESET}", file=sys.stderr)
        return 1
    print(f"\nstatecheck selftest: all {len(cases) + 3} checks passed\n")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description="is the repository resumable?")
    parser.add_argument("--selftest", action="store_true", help="prove each check fires")
    args = parser.parse_args()
    return selftest() if args.selftest else run()


if __name__ == "__main__":
    raise SystemExit(main())
