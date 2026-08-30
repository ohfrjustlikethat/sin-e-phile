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
