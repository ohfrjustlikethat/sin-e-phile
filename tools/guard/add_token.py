#!/usr/bin/env python3
"""
Add a token to the hashed denylist without ever writing it in plaintext.

Usage:  python tools/guard/add_token.py "short justification"

Reads the token from stdin WITHOUT ECHOING it (getpass), normalises it exactly
as guard.py does, hashes it with the committed salt, and appends only the hash
plus your justification comment to denylist.txt.

The plaintext token is never printed, never stored, and never reaches the
terminal history. That is the entire point: SPEC.md §2.1 forbids the plaintext
from existing in this repository at all (ADR-0009).

Remember what this is: HYGIENE, NOT SECURITY. The salt is committed, so the
hashes are brute-forceable by anyone determined. This defends against accidental
plaintext and casual discovery. Do not describe it as anything stronger.
"""

from __future__ import annotations

import getpass
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from guard import GUARD_DIR, hash_token, load_denylist, load_salt, normalise_token  # noqa: E402


def main() -> int:
    if len(sys.argv) != 2:
        print(__doc__)
        print("error: exactly one argument required — a short justification comment.",
              file=sys.stderr)
        return 2

    justification = sys.argv[1].strip().replace("\n", " ")
    if not justification:
        print("error: justification must not be empty.", file=sys.stderr)
        return 2

    token = getpass.getpass("token (input hidden, not echoed): ").strip()
    if not token:
        print("error: no token supplied.", file=sys.stderr)
        return 2

    normalised = normalise_token(token)
    if len(normalised) < 3:
        print("error: token too short after normalisation.", file=sys.stderr)
        return 2

    digest = hash_token(normalised, load_salt())

    if digest in load_denylist():
        print("already present — denylist unchanged.")
        return 0

    path = GUARD_DIR / "denylist.txt"
    with path.open("a", encoding="utf-8", newline="\n") as handle:
        handle.write(f"{digest}  # {justification}\n")

    # Deliberately reports the hash and the justification only — never the token,
    # not even the length, which would narrow a brute-force search.
    print(f"added: {digest[:16]}…  # {justification}")
    print("Commit denylist.txt. Do not record the plaintext anywhere.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
