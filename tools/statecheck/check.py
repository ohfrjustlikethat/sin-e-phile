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
  6. CI was red for 42 days with an empty blockers array, and the `adrs` array
     listed 23 ADRs while `docs/adr/` held 34 (D48). Neither was a schema violation,
     so every other check here stayed green.

Runs in CI and on pre-push. Python 3.12, stdlib only (ADR-0012).

    python tools/statecheck/check.py
    python tools/statecheck/check.py --selftest
"""

from __future__ import annotations

import argparse
import json
import os
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


class Skip:
    """A check that could not run. Reported, never counted as a failure."""

    def __init__(self, reason: str):
        self.reason = reason


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
    # Compare NORMALISED, restore EXACT. The two halves are different problems and
    # the first version got both wrong in the same line.
    #
    # Restore must be byte-exact: `write_text` translates newlines to the platform
    # default, so the original silently rewrote PROGRESS.md from LF to CRLF on every
    # run, while comparing normalised text — blind to the damage it was doing.
    #
    # Comparison must ignore newlines: git checks out with `core.autocrlf` on the
    # Windows CI runner, so the working tree is CRLF while the generator writes LF.
    # A byte comparison there fails on every run for a file whose content is
    # identical, which is a check that cries wolf until it is deleted.
    def normalise(raw: bytes) -> bytes:
        return raw.replace(b"\r\n", b"\n")

    before = PROGRESS.read_bytes() if PROGRESS.exists() else b""
    subprocess.run(
        [sys.executable, str(VALIDATE), "--progress"],
        cwd=REPO, capture_output=True, text=True,
    )
    after = PROGRESS.read_bytes() if PROGRESS.exists() else b""
    PROGRESS.write_bytes(before)
    if normalise(after) != normalise(before):
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


# ---------------------------------------------------------------------------
# 6 — repository facts the state file must agree with (D48)
# ---------------------------------------------------------------------------

ADR_DIR = REPO / "docs" / "adr"
ADR_FILE = re.compile(r"^(\d{4})-.+\.md$")
# A blocker "records" red CI if its description names CI as a word. Loose on
# purpose: the failure being caught is an EMPTY blockers array while CI is red,
# not a blocker worded differently.
MENTIONS_CI = re.compile(r"\bCI\b")


def adr_problems(files: list[str], state: dict) -> list[Problem]:
    """Every ADR on disk is in the `adrs` array, and every entry has a file."""
    on_disk = {m.group(1) for f in files if (m := ADR_FILE.match(f))}
    recorded = {str(a.get("id")) for a in state.get("adrs", [])}
    problems = []
    if missing := sorted(on_disk - recorded):
        problems.append(Problem(
            "repository facts",
            f"docs/adr/ holds ADR(s) the adrs array does not: {', '.join(missing)}",
            "add an {id, title, date} entry to PROJECT_STATE.json adrs for each",
        ))
    if orphaned := sorted(recorded - on_disk):
        problems.append(Problem(
            "repository facts",
            f"the adrs array lists ADR(s) with no file in docs/adr/: {', '.join(orphaned)}",
            "restore the ADR file, or remove the entry if it was never written",
        ))
    return problems


def ci_problems(runs: list[dict], state: dict) -> list[Problem]:
    """The latest finished CI run is green, or a blocker says it is not.

    Red CI alone is not a failure here. This runs on pre-push, so failing on red CI
    would refuse the very push that fixes it. What went wrong in B6 was red CI that
    nothing in the state file mentioned, so that is what fails.
    """
    finished = [r for r in runs if r.get("status") == "completed"]
    if not finished:
        return []
    latest = finished[0]
    if latest.get("conclusion") == "success":
        return []
    if any(MENTIONS_CI.search(b.get("description", "")) for b in state.get("blockers", [])):
        return []
    return [Problem(
        "repository facts",
        f"the latest CI run is '{latest.get('conclusion')}' and no blocker records it: "
        f"{latest.get('url', '')}",
        "fix CI, or raise a blocker naming CI with what is failing and why",
    )]


def check_repository_facts(state: dict) -> list[Problem] | Skip:
    files = [p.name for p in ADR_DIR.glob("*.md")]
    problems = adr_problems(files, state)

    # The CI half asks GitHub, so it degrades to a skip, never a failure, when it
    # cannot ask. Inside CI the latest run is the one still executing this script,
    # so the question has no answer there.
    reason = None
    branch = git("rev-parse", "--abbrev-ref", "HEAD")
    if os.environ.get("GITHUB_ACTIONS") == "true":
        reason = "CI half skipped inside CI, which cannot judge itself"
    elif not branch or branch == "HEAD":
        reason = "CI half skipped: no branch checked out"
    else:
        try:
            result = subprocess.run(
                ["gh", "run", "list", "--branch", branch, "--workflow", "ci.yml",
                 "--limit", "10", "--json", "status,conclusion,url"],
                cwd=REPO, capture_output=True, text=True, encoding="utf-8",
                errors="replace", timeout=20,
            )
        except (OSError, subprocess.TimeoutExpired):
            result = None
        if result is None or result.returncode != 0:
            reason = "CI half skipped: gh is absent, offline or unauthenticated"
        else:
            problems += ci_problems(json.loads(result.stdout or "[]"), state)

    if reason and not problems:
        return Skip(f"ADRs ok; {reason}")
    return problems


CHECKS = [
    ("state file is schema-valid", check_schema),
    ("phase document exists and matches", check_phase_doc),
    ("PROGRESS.md in sync", check_progress),
    ("phases behind current are closed", check_phases_closed),
    ("next_action is an instruction", check_next_action),
    ("state follows code", check_state_follows_code),
    ("repository facts: ADRs and CI", check_repository_facts),
]


def run() -> int:
    state = load()
    problems: list[Problem] = []

    print(f"{BOLD}statecheck{RESET}  {DIM}can a cold session tell where it is?{RESET}\n")
    for label, check in CHECKS:
        found = check(state)
        if isinstance(found, Skip):
            print(f"  {DIM}skip{RESET}  {label}  {DIM}({found.reason}){RESET}")
            continue
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

    quiet = {"skipped phase is allowed", "red CI recorded by a blocker",
             "ADRs agree", "green CI", "no finished CI run"}
    for label, check, payload in cases:
        fired = bool(check({**state, **payload}))
        if report(label, fired, label not in quiet):
            failures.append(label)

    # The D48 halves take repository facts as arguments, so they are fed fake ones.
    red = [{"status": "completed", "conclusion": "failure", "url": "run/2"}]
    ci_blocker = {"blockers": [{"description": "CI red: gitleaks finds 5"}]}
    adrs = {"adrs": [{"id": "0001"}, {"id": "0002"}]}
    fact_cases = [
        ("ADR on disk, not in adrs", adr_problems(
            ["0001-a.md", "0002-b.md", "0003-c.md", "README.md"], adrs)),
        ("ADR in adrs, not on disk", adr_problems(["0001-a.md"], adrs)),
        ("ADRs agree", adr_problems(["0001-a.md", "0002-b.md", "README.md"], adrs)),
        ("red CI, no blocker", ci_problems(red, {"blockers": []})),
        ("red CI, blocker about something else", ci_problems(
            red, {"blockers": [{"description": "the zip pin is yanked"}]})),
        ("red CI recorded by a blocker", ci_problems(red, ci_blocker)),
        ("green CI", ci_problems(
            [{"status": "in_progress", "conclusion": ""},
             {"status": "completed", "conclusion": "success"}], {"blockers": []})),
        ("no finished CI run", ci_problems(
            [{"status": "queued", "conclusion": ""}], {"blockers": []})),
    ]
    for label, found in fact_cases:
        if report(label, bool(found), label not in quiet):
            failures.append(label)

    # And the real ones, which must be quiet on a healthy tree.
    real = 0
    for label, check in CHECKS:
        if check in (check_next_action, check_phases_closed):  # covered above
            continue
        real += 1
        found = check(state)
        if found and not isinstance(found, Skip):
            failures.append(f"{label} fires on a healthy tree")
            print(f"  {RED}FAIL{RESET}  {label} fires on a healthy tree")
        else:
            print(f"  {GREEN}ok  {RESET}  {label}  {DIM}(quiet when healthy){RESET}")

    if failures:
        print(f"\n{RED}statecheck selftest: {len(failures)} failure(s){RESET}", file=sys.stderr)
        return 1
    print(f"\nstatecheck selftest: all {len(cases) + len(fact_cases) + real} checks passed\n")
    return 0


def report(label: str, fired: bool, should_fire: bool) -> bool:
    """Print one selftest line; return True if it went wrong."""
    ok = fired == should_fire
    print(f"  {GREEN + 'ok  ' + RESET if ok else RED + 'FAIL' + RESET}  {label}"
          f"  {DIM}({'fires' if should_fire else 'stays quiet'}){RESET}")
    return not ok


def main() -> int:
    parser = argparse.ArgumentParser(description="is the repository resumable?")
    parser.add_argument("--selftest", action="store_true", help="prove each check fires")
    args = parser.parse_args()
    return selftest() if args.selftest else run()


if __name__ == "__main__":
    raise SystemExit(main())
