"""
Prove the posture guard actually works.

SPEC.md Phase 0 exit criterion: "The posture guard fails CI when fed a
deliberately-planted test string, and passes on the clean tree. Verify it
actually works — an unverified guard is worse than none, because it produces
false confidence."

This runs four checks:

  1. Every SHOULD_FIRE vector produces the finding it is supposed to produce.
  2. No SHOULD_PASS vector produces any finding at all. False positives matter
     as much as false negatives: a guard that cries wolf gets `--no-verify`d,
     and then it protects nothing.
  3. The exemption lists have not grown. Those are the guard's only holes, so
     they are pinned here — widening one requires editing this test, which makes
     it a visible decision rather than a quiet one.
  4. vectors.py, which the guard skips entirely, contains no real undeclared
     domains. The guard cannot check that file, so this check does it instead —
     which is what stops the full exemption from being a genuine hole.
"""

from __future__ import annotations

import sys
from pathlib import Path

GUARD_DIR = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(GUARD_DIR))

from guard import (  # noqa: E402
    FULLY_EXEMPT,
    GUARD_DIR,
    STRUCTURAL_EXEMPT,
    domain_is_permitted,
    extract_domains,
    hash_token,
    load_allowlist,
    load_denylist,
    load_salt,
    scan_text,
)
from tests.vectors import (  # noqa: E402
    DENYLIST_VECTORS,
    PINNED_PLACEHOLDERS,
    SHOULD_FIRE,
    SHOULD_PASS,
)

# Pinned deliberately. See checks 3 and 4 above.
EXPECTED_EXEMPTIONS = frozenset(
    {
        "tools/guard/guard.py",
        "SPEC.md",
        "docs/adr/0009-posture-guard-denylist-design.md",
    }
)
EXPECTED_FULL_EXEMPTIONS = frozenset({"tools/guard/tests/vectors.py"})


def run_selftest() -> int:
    salt, allow = load_salt(), load_allowlist()

    # The committed denylist is empty, and correctly so: the project ships no
    # content sources, so there is no real token to deny. Seeding it with test
    # vectors would mean every document mentioning those vectors trips the guard.
    #
    # So the self-test builds its own denylist in memory instead. The hashed
    # matcher is still genuinely exercised — same salt, same hash function, same
    # code path — it just does not require the committed file to be non-empty.
    deny = set(load_denylist()) | {hash_token(v, salt) for v in DENYLIST_VECTORS}
    failures: list[str] = []

    print("guard selftest — all vectors use RFC 2606 reserved domains (ADR-0009)\n")

    print(f"  must fire ({len(SHOULD_FIRE)} vectors)")
    for label, content, expected_rule in SHOULD_FIRE:
        findings = scan_text("selftest_vector.rs", content, salt, deny, allow)
        rules = {f.rule for f in findings}
        if expected_rule in rules:
            print(f"    PASS  {label}  [{expected_rule}]")
        else:
            got = ", ".join(sorted(rules)) or "nothing"
            print(f"    FAIL  {label}  expected [{expected_rule}], got: {got}")
            failures.append(f"missed: {label} (expected {expected_rule}, got {got})")

    print(f"\n  must not fire ({len(SHOULD_PASS)} vectors)")
    for label, content in SHOULD_PASS:
        findings = scan_text("selftest_vector.rs", content, salt, deny, allow)
        if not findings:
            print(f"    PASS  {label}")
        else:
            rules = ", ".join(sorted({f.rule for f in findings}))
            print(f"    FAIL  {label}  false positive: {rules}")
            failures.append(f"false positive: {label} ({rules})")

    print("\n  exemption lists are pinned")
    if STRUCTURAL_EXEMPT == EXPECTED_EXEMPTIONS and FULLY_EXEMPT == EXPECTED_FULL_EXEMPTIONS:
        print(f"    PASS  {len(STRUCTURAL_EXEMPT)} structural + {len(FULLY_EXEMPT)} full, unchanged")
    else:
        print(f"    FAIL  exemptions changed — structural: {sorted(STRUCTURAL_EXEMPT)}, "
              f"full: {sorted(FULLY_EXEMPT)}")
        failures.append("exemption lists changed without updating selftest.py")

    # vectors.py is skipped by the guard entirely, so audit it from this side:
    # every domain in it must be reserved, infrastructure, allowlisted, or a
    # pinned placeholder. This is what stops the full exemption being a hole.
    print("\n  vectors.py contains no real undeclared domains")
    # Line by line, so the same per-line rules the guard applies (TOML headers,
    # code-file handling) apply here too. Scanning the whole file as one blob
    # would silently disagree with the guard.
    vectors_text = (GUARD_DIR / "tests" / "vectors.py").read_text(encoding="utf-8")
    stray = {
        d
        for line in vectors_text.splitlines()
        for d in extract_domains(line)
        if not domain_is_permitted(d, allow) and d.lower() not in PINNED_PLACEHOLDERS
    }
    if not stray:
        print(f"    PASS  all domains reserved, allowlisted, or pinned placeholders")
    else:
        print(f"    FAIL  unaudited domain(s) in vectors.py: {sorted(stray)}")
        failures.append(f"unaudited domains in vectors.py: {sorted(stray)}")

    total = len(SHOULD_FIRE) + len(SHOULD_PASS) + 2
    if failures:
        print(f"\nguard selftest: {len(failures)} FAILURE(S) of {total} checks\n", file=sys.stderr)
        for failure in failures:
            print(f"  - {failure}", file=sys.stderr)
        return 1

    print(f"\nguard selftest: all {total} checks passed — the guard fires when it should "
          f"and stays quiet when it should")
    return 0


if __name__ == "__main__":
    raise SystemExit(run_selftest())
