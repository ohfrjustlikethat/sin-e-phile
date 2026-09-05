#!/usr/bin/env python3
"""
Validate PROJECT_STATE.json against docs/schemas/project-state.schema.json,
and regenerate PROGRESS.md from it.

  python tools/state/validate_state.py --check      validate (CI, session start)
  python tools/state/validate_state.py --progress   regenerate PROGRESS.md

WHY A HAND-ROLLED VALIDATOR. `jsonschema` is a pip dependency, and ADR-0012 fixed
these tools as standard-library-only so they run on a fresh clone before anything
is installed. This implements the subset of Draft 2020-12 the schema actually
uses — type, required, enum, const, pattern, minLength, minimum/maximum,
minItems/maxItems, additionalProperties, items, $ref, $defs, allOf, if/then, not.

That subset is small and the schema is ours, so the trade is worth it. If the
schema ever needs a construct this does not implement, the validator says so
loudly rather than passing silently — see `unsupported`.

PROGRESS.md is GENERATED, never edited by hand (SPEC.md §10.1: "Regenerated from
PROJECT_STATE.json at the end of every session so the two can never disagree").
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path
from typing import Any

REPO_ROOT = Path(__file__).resolve().parents[2]
STATE = REPO_ROOT / "PROJECT_STATE.json"
SCHEMA = REPO_ROOT / "docs" / "schemas" / "project-state.schema.json"
PROGRESS = REPO_ROOT / "PROGRESS.md"

KNOWN_KEYWORDS = {
    "$schema", "$id", "$defs", "$ref", "title", "description", "type", "required",
    "properties", "additionalProperties", "items", "enum", "const", "pattern",
    "minLength", "maxLength", "minimum", "maximum", "minItems", "maxItems",
    "allOf", "anyOf", "oneOf", "if", "then", "else", "not", "format",
    "propertyNames", "_comment",
}

TYPES: dict[str, Any] = {
    "object": dict, "array": list, "string": str, "boolean": bool, "null": type(None),
}


class Validator:
    def __init__(self, schema: dict) -> None:
        self.root = schema
        self.errors: list[str] = []
        self.unsupported: set[str] = set()

    def resolve(self, schema: dict) -> dict:
        if "$ref" not in schema:
            return schema
        ref = schema["$ref"]
        if not ref.startswith("#/"):
            self.unsupported.add(f"external $ref {ref}")
            return {}
        node: Any = self.root
        for part in ref[2:].split("/"):
            node = node.get(part.replace("~1", "/").replace("~0", "~"), {})
        return node

    def check(self, value: Any, schema: dict, path: str) -> None:
        schema = self.resolve(schema)
        if not schema:
            return

        for keyword in schema:
            if keyword not in KNOWN_KEYWORDS:
                self.unsupported.add(keyword)

        if "type" in schema:
            expected = schema["type"]
            names = [expected] if isinstance(expected, str) else expected
            # bool is a subclass of int in Python; integer must not accept True.
            ok = False
            for name in names:
                if name == "integer":
                    ok = ok or (isinstance(value, int) and not isinstance(value, bool))
                elif name == "number":
                    ok = ok or (isinstance(value, (int, float)) and not isinstance(value, bool))
                else:
                    ok = ok or isinstance(value, TYPES.get(name, object))
            if not ok:
                self.errors.append(f"{path}: expected type {expected}, got {type(value).__name__}")
                return

        if "const" in schema and value != schema["const"]:
            self.errors.append(f"{path}: must equal {schema['const']!r}, got {value!r}")
        if "enum" in schema and value not in schema["enum"]:
            self.errors.append(f"{path}: {value!r} not one of {schema['enum']}")

        if isinstance(value, str):
            if "pattern" in schema and not re.search(schema["pattern"], value):
                self.errors.append(f"{path}: {value[:60]!r} does not match /{schema['pattern']}/")
            if "minLength" in schema and len(value) < schema["minLength"]:
                self.errors.append(
                    f"{path}: length {len(value)} is below minimum {schema['minLength']}"
                    + (f" — value: {value!r}" if len(value) < 40 else "")
                )
            if "maxLength" in schema and len(value) > schema["maxLength"]:
                self.errors.append(f"{path}: length {len(value)} exceeds {schema['maxLength']}")

        if isinstance(value, (int, float)) and not isinstance(value, bool):
            if "minimum" in schema and value < schema["minimum"]:
                self.errors.append(f"{path}: {value} below minimum {schema['minimum']}")
            if "maximum" in schema and value > schema["maximum"]:
                self.errors.append(f"{path}: {value} above maximum {schema['maximum']}")

        if isinstance(value, list):
            if "minItems" in schema and len(value) < schema["minItems"]:
                self.errors.append(f"{path}: {len(value)} items, minimum {schema['minItems']}")
            if "maxItems" in schema and len(value) > schema["maxItems"]:
                self.errors.append(f"{path}: {len(value)} items, maximum {schema['maxItems']}")
            if "items" in schema:
                for index, item in enumerate(value):
                    self.check(item, schema["items"], f"{path}[{index}]")

        if isinstance(value, dict):
            for key in schema.get("required", []):
                if key not in value:
                    self.errors.append(f"{path}: missing required property {key!r}")
            properties = schema.get("properties", {})
            for key, sub in properties.items():
                if key in value:
                    self.check(value[key], sub, f"{path}.{key}")
            if schema.get("additionalProperties") is False:
                for key in value:
                    if key not in properties:
                        self.errors.append(f"{path}: unexpected property {key!r}")

        for sub in schema.get("allOf", []):
            self.check(value, sub, path)

        if "if" in schema:
            if self.matches(value, schema["if"]):
                if "then" in schema:
                    self.check(value, schema["then"], path)
            elif "else" in schema:
                self.check(value, schema["else"], path)

        if "not" in schema and self.matches(value, schema["not"]):
            self.errors.append(f"{path}: matches a forbidden shape — {schema['not'].get('description', schema['not'])}")

    def matches(self, value: Any, schema: dict) -> bool:
        probe = Validator(self.root)
        probe.check(value, schema, "?")
        return not probe.errors


def load() -> tuple[dict, dict]:
    return (json.loads(STATE.read_text(encoding="utf-8")),
            json.loads(SCHEMA.read_text(encoding="utf-8")))



def check_current_phase_mirrors(state: dict) -> list[str]:
    """`current_phase` and its `phases[]` entry must agree.

    The state file holds the live checklists TWICE — once in `phases[]`, once in
    `current_phase` — and nothing made them agree. In one session that let E5 be marked
    met in `phases[]` while `current_phase` still said false, and then let the same
    thing happen again to E7 an hour later. Both times only `tools/statecheck` caught
    it, indirectly, through the phase document's checkbox, which is a long way from the
    cause and says nothing useful about it.

    Recorded as debt D23 the first time. Fixed here the second, because a class of
    error that recurs within a session is not going to be prevented by remembering.
    """
    current = state.get("current_phase")
    if not isinstance(current, dict):
        return []
    number = current.get("number")
    entry = next(
        (p for p in state.get("phases", []) if p.get("number") == number), None
    )
    if entry is None:
        return [f"current_phase is phase {number}, which has no entry in phases[]"]

    errors: list[str] = []

    def compare(kind: str, key: str, fields: tuple[str, ...]) -> None:
        mine = {item.get("id"): item for item in current.get(kind, [])}
        theirs = {item.get("id"): item for item in entry.get(kind, [])}
        for ident in sorted(set(mine) | set(theirs)):
            a, b = mine.get(ident), theirs.get(ident)
            if a is None or b is None:
                errors.append(
                    f"{kind} {ident} exists in "
                    f"{'phases[]' if a is None else 'current_phase'} only"
                )
                continue
            for field in fields:
                if a.get(field) != b.get(field):
                    errors.append(
                        f"{kind} {ident}: current_phase.{field} is {a.get(field)!r} "
                        f"but phases[{number}].{field} is {b.get(field)!r}"
                    )

    compare("exit_criteria", "id", ("met", "evidence"))
    compare("subtasks", "id", ("status", "commit"))
    return errors


def check_phase_progression(state: dict) -> list[str]:
    """Every phase before the current one must be a CLOSED record.

    SPEC.md §10.8 says a criterion is met only with an artefact. The schema enforces
    that per-criterion, but it could not see a whole phase left half-written: the
    Phase 2 record sat at `not_started` with no evidence for a day after Phase 2 was
    merged and tagged, because the finished record had been written to
    `current_phase` and never copied back into `phases`. Nothing complained.

    That is exactly the failure this file exists to prevent, and it gets worse the
    later it happens — at Phase 19 nobody is reading Phase 2's record closely enough
    to notice it is empty. So: advancing `current_phase` closes every phase behind it.

    A phase deliberately not built (Tier C and D are optional — Appendix E) may be
    `skipped` instead, but must say why.
    """
    errors: list[str] = []
    current = state["current_phase"]["number"]

    for phase in state["phases"]:
        n = phase["number"]
        if n >= current:
            continue

        status = phase.get("status")
        if status == "skipped":
            if not (phase.get("skip_reason") or "").strip():
                errors.append(
                    f"phases[{n}] is skipped but gives no skip_reason. "
                    f"A phase not built must record why."
                )
            continue

        if status != "complete":
            errors.append(
                f"phases[{n}] has status '{status}' but current_phase is {current}. "
                f"A phase behind the current one must be 'complete' (or 'skipped' "
                f"with a reason)."
            )
        if not (phase.get("completion_commit") or "").strip():
            errors.append(
                f"phases[{n}] is behind current_phase {current} but has no "
                f"completion_commit. Record the merge commit that closed it."
            )
        for criterion in phase["exit_criteria"]:
            if not criterion["met"]:
                errors.append(
                    f"phases[{n}].{criterion['id']} is not met, but phase {n} is "
                    f"behind current_phase {current}. Either evidence it, or say in "
                    f"SESSION_LOG.md why the phase closed without it."
                )
            elif not (criterion.get("evidence") or "").strip():
                errors.append(
                    f"phases[{n}].{criterion['id']} is met with no evidence (§10.8)."
                )

    # The current phase must actually exist in the table it indexes into.
    if not any(p["number"] == current for p in state["phases"]):
        errors.append(f"current_phase is {current}, which is not in the phases table.")

    return errors


def do_check() -> int:
    state, schema = load()
    validator = Validator(schema)
    validator.check(state, schema, "$")

    if validator.unsupported:
        print(f"validate_state: schema uses constructs this validator does not implement: "
              f"{sorted(validator.unsupported)} — extend validate_state.py.", file=sys.stderr)
        return 2

    if validator.errors:
        print(f"\nvalidate_state: {len(validator.errors)} schema violation(s) "
              f"in PROJECT_STATE.json\n", file=sys.stderr)
        for error in validator.errors:
            print(f"  {error}", file=sys.stderr)
        print("\nSPEC.md §10.8: a criterion marked met MUST carry an artefact. If evidence\n"
              "cannot be produced, the criterion is not met — say so and say what is\n"
              "blocking it.\n", file=sys.stderr)
        return 1

    mirrors = check_current_phase_mirrors(state)
    if mirrors:
        print("", file=sys.stderr)
        print(f"validate_state: {len(mirrors)} disagreement(s) between current_phase "
              f"and phases[] in PROJECT_STATE.json", file=sys.stderr)
        print("", file=sys.stderr)
        for error in mirrors:
            print(f"  {error}", file=sys.stderr)
        print("", file=sys.stderr)
        print("The live checklists are stored twice and must agree. Update BOTH, or "
              "the phase document and the state file will disagree about what is "
              "done — which is how a criterion gets marked met in one place and not "
              "the other (debt D23).", file=sys.stderr)
        return 1

    progression = check_phase_progression(state)
    if progression:
        print("", file=sys.stderr)
        print(f"validate_state: {len(progression)} phase-progression violation(s) "
              f"in PROJECT_STATE.json", file=sys.stderr)
        print("", file=sys.stderr)
        for error in progression:
            print(f"  {error}", file=sys.stderr)
        print("", file=sys.stderr)
        print("Advancing current_phase closes every phase behind it: status "
              "'complete', a completion_commit, and evidence on every criterion.",
              file=sys.stderr)
        print("", file=sys.stderr)
        return 1

    phases = state["phases"]
    criteria = sum(len(p["exit_criteria"]) for p in phases)
    met = sum(1 for p in phases for c in p["exit_criteria"] if c["met"])
    print(f"validate_state: PROJECT_STATE.json valid — {len(phases)} phases, "
          f"{criteria} exit criteria, {met} met with evidence")
    return 0


def do_progress() -> int:
    state, _ = load()
    phases = state["phases"]
    current = state["current_phase"]

    tier_of = {}
    for number in range(0, 9):
        tier_of[number] = "A"
    for number in list(range(9, 19)) + [21, 27]:
        tier_of[number] = "B"
    for number in (19, 20, 23):
        tier_of[number] = "C"
    for number in (22, 24, 25, 26):
        tier_of[number] = "D"

    mark = {"complete": "x", "in_progress": "~", "blocked": "!",
            "awaiting_review": "?", "not_started": " "}

    lines = [
        "# Progress",
        "",
        "> **Generated file — do not edit.** Regenerated from `PROJECT_STATE.json` by",
        "> `python tools/state/validate_state.py --progress` (`SPEC.md` §10.1, so the two",
        "> can never disagree). Edit the state file, then regenerate.",
        "",
        f"**Spec version {state['spec_version']}** · "
        f"{state['sessions_completed']} session(s) completed · "
        f"last updated {state['last_updated'][:10]}",
        "",
        "---",
        "",
        "## Where we are right now",
        "",
    ]

    met = sum(1 for c in current["exit_criteria"] if c["met"])
    total = len(current["exit_criteria"])
    lines += [
        f"**Phase {current['number']} — {current.get('title', current['slug'])}** "
        f"(`{current['status']}`, branch `{current['branch']}`)",
        "",
        f"{met} of {total} exit criteria met with evidence.",
        "",
    ]
    if current.get("note"):
        lines += [f"> {current['note']}", ""]

    if current.get("subtasks"):
        done = sum(1 for s in current["subtasks"] if s["status"] == "complete")
        lines += [f"### Subtasks — {done}/{len(current['subtasks'])} complete", ""]
        for subtask in current["subtasks"]:
            sha = f" · `{subtask['commit']}`" if subtask["commit"] else ""
            lines.append(f"- [{mark[subtask['status']]}] **{subtask['id']}** {subtask['text']}{sha}")
        lines.append("")

    lines += ["### Exit criteria", ""]
    for criterion in current["exit_criteria"]:
        lines.append(f"- [{'x' if criterion['met'] else ' '}] **{criterion['id']}** {criterion['text']}")
        if criterion["evidence"]:
            lines.append(f"      - *Evidence:* {criterion['evidence']}")
    lines += ["", "---", "", "## What's next", "", f"{state['next_action']}", ""]

    if state.get("blockers"):
        lines += ["---", "", "## Blockers", ""]
        for blocker in state["blockers"]:
            flag = " **(needs you)**" if blocker["needs_user"] else ""
            lines.append(f"- **{blocker['id']}**{flag} {blocker['description']}")
        lines.append("")
    else:
        lines += ["---", "", "## Blockers", "", "None.", ""]

    lines += [
        "---",
        "",
        "## All 28 phases",
        "",
        "Tiers are the legitimate stopping points from `SPEC.md` Appendix E. "
        "**Tier B is the definition of done** — complete it and the project has succeeded.",
        "",
        "| | # | Phase | Tier | Depends on | Sessions | Criteria met |",
        "|---|---|---|---|---|---|---|",
    ]
    for phase in phases:
        met = sum(1 for c in phase["exit_criteria"] if c["met"])
        total = len(phase["exit_criteria"])
        tier = tier_of.get(phase["number"], "?")
        milestone = " 🏁" if phase["number"] in (8, 18) else ""
        lines.append(
            f"| [{mark[phase['status']]}] | {phase['number']} | "
            f"{phase.get('title', phase['slug'])}{milestone} | {tier} | "
            f"{phase.get('depends_on') or '—'} | {phase.get('sessions') or '—'} | {met}/{total} |"
        )

    lines += [
        "",
        "Legend: `[x]` complete · `[~]` in progress · `[!]` blocked · `[?]` awaiting review · "
        "`[ ]` not started. 🏁 marks Phase 8 (first demoable build) and Phase 18 "
        "(complete product).",
        "",
    ]

    if state.get("known_debt"):
        lines += ["---", "", "## Known debt", ""]
        for debt in state["known_debt"]:
            lines.append(f"- **{debt['id']}** (raised in Phase {debt['phase_raised']}) {debt['text']}")
        lines.append("")

    if state.get("decisions_pending"):
        lines += ["---", "", "## Decisions pending", ""]
        for decision in state["decisions_pending"]:
            by = f" — decide by Phase {decision['decide_by_phase']}" if decision.get("decide_by_phase") is not None else ""
            lines.append(f"- **{decision['id']}**{by} {decision['text']}")
        lines.append("")

    PROGRESS.write_text("\n".join(lines), encoding="utf-8", newline="\n")
    print(f"validate_state: regenerated PROGRESS.md ({len(lines)} lines)")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description="Validate state; regenerate PROGRESS.md")
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--check", action="store_true")
    mode.add_argument("--progress", action="store_true")
    args = parser.parse_args()
    return do_check() if args.check else do_progress()


if __name__ == "__main__":
    raise SystemExit(main())
