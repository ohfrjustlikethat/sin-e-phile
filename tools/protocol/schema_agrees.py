"""Check that the addon manifest JSON Schema agrees with the reference validator.

docs/schemas/addon-manifest.schema.json is what a third party validates against;
crates/source-protocol/src/manifest.rs is what the app enforces. Two descriptions of
one format drift unless something compares them, so this runs the schema over the
same corpus the Rust tests use (fixtures/addon-manifests/):

- every file in valid/ must pass the schema, except where the app is deliberately
  more lenient than the schema (LENIENT, below);
- every file in hostile/ must FAIL the schema, except the rules JSON Schema cannot
  express, which only the validator enforces (VALIDATOR_ONLY, below).

Both exception lists are explicit, so a new fixture that disagrees fails here until
someone decides which side is right.

Usage: python tools/protocol/schema_agrees.py [--selftest]
Stdlib only (ADR-0012); reuses the evaluator in tools/state/validate_state.py.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools" / "state"))
from validate_state import Validator  # noqa: E402

SCHEMA = ROOT / "docs" / "schemas" / "addon-manifest.schema.json"
CORPUS = ROOT / "fixtures" / "addon-manifests"

# Valid for the app, invalid for the schema: forward compatibility. A newer addon may
# list a kind this build has never heard of; the app ignores it, the schema (which
# describes protocol 1 for authors) does not allow it.
LENIENT = {"unknown-media-alongside-known.json"}

# Hostile, but only the validator can tell: these rules compare the manifest with
# where it was fetched from, or need a real URL parser.
VALIDATOR_ONLY = {
    "endpoint-on-another-host.json": "endpoints must be on the manifest's own host",
    "endpoint-on-another-port.json": "endpoints must be on the manifest's own port",
}


def errors(schema: dict, doc: object) -> list[str]:
    v = Validator(schema)
    v.check(doc, schema, "$")
    if v.unsupported:
        return [f"schema uses keywords the evaluator ignores: {sorted(v.unsupported)}"]
    return v.errors


def load(path: Path) -> object | None:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (ValueError, UnicodeDecodeError):
        return None  # not JSON at all; the schema has nothing to say


def run(schema: dict) -> list[str]:
    problems: list[str] = []
    for path in sorted((CORPUS / "valid").glob("*.json")):
        errs = errors(schema, load(path))
        if path.name in LENIENT:
            if not errs:
                problems.append(f"valid/{path.name} is in LENIENT but the schema accepts it")
        elif errs:
            problems.append(f"valid/{path.name} fails the schema: {errs[0]}")
    for path in sorted((CORPUS / "hostile").glob("*.json")):
        doc = load(path)
        if doc is None:
            continue
        accepted = not errors(schema, doc)
        if path.name in VALIDATOR_ONLY:
            if not accepted:
                problems.append(f"hostile/{path.name} is in VALIDATOR_ONLY but the schema rejects it")
        elif accepted:
            problems.append(f"hostile/{path.name} PASSES the schema")
    return problems


def main() -> int:
    schema = json.loads(SCHEMA.read_text(encoding="utf-8"))
    if "--selftest" in sys.argv:
        # Seen to fail: drop the display-text rule and the bidi fixture must slip through.
        schema["$defs"]["displayText"] = {"type": "string"}
        problems = run(schema)
        ok = any("name-bidi-override" in p for p in problems)
        print("schema_agrees selftest:", "ok — a weakened schema is caught" if ok else "FAILED")
        return 0 if ok else 1
    problems = run(schema)
    for p in problems:
        print("  " + p)
    n = len(list(CORPUS.glob("*/*.json")))
    print(f"schema_agrees: {'clean' if not problems else f'{len(problems)} disagreement(s)'} over {n} fixtures")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
