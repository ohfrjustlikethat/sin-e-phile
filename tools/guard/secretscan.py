#!/usr/bin/env python3
"""
Local secret scanner — the pre-commit half of SPEC.md §12.6.

This is deliberately the WEAKER of two scanners (ADR-0012). CI runs a
version-pinned, checksum-verified gitleaks binary and is authoritative. This runs
in the pre-commit hook, needs nothing installed but Python, and exists so that a
machine without gitleaks is not silently unprotected — silent non-functionality
in a safety rail being the worst possible failure mode.

Defence in depth: fast and approximate here, thorough in CI, GitHub push
protection as the backstop if both are bypassed.

Usage:
    python tools/guard/secretscan.py --staged     (pre-commit)
    python tools/guard/secretscan.py --tree

Exit 0 = clean, 1 = findings, 2 = could not run.

If this fires on a genuine false positive — a high-entropy test fixture, a long
hash — `git commit --no-verify` gets you past it locally, but CI still runs
gitleaks and will fail the push. The escape hatch is temporary, never permanent.
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]

# (rule, pattern, description). Ordered roughly by confidence.
SECRET_PATTERNS: list[tuple[str, re.Pattern[str], str]] = [
    (
        "private-key",
        re.compile(r"-----BEGIN (?:RSA |EC |DSA |OPENSSH |PGP )?PRIVATE KEY( BLOCK)?-----"),
        "A private key header.",
    ),
    (
        "aws-access-key",
        re.compile(r"\b(?:AKIA|ASIA)[0-9A-Z]{16}\b"),
        "An AWS access key ID.",
    ),
    (
        "github-token",
        re.compile(r"\bgh[pousr]_[A-Za-z0-9]{36,}\b"),
        "A GitHub personal access / OAuth token.",
    ),
    (
        "slack-token",
        re.compile(r"\bxox[abprs]-[A-Za-z0-9-]{10,}\b"),
        "A Slack token.",
    ),
    (
        "generic-api-key-assignment",
        # A key-shaped NAME assigned a long, secret-shaped VALUE. The value must
        # not look like a placeholder — `.env.example` must stay committable.
        re.compile(
            r"""(?i)\b\w*(?:api[_-]?key|apikey|secret|token|password|passwd|client[_-]?secret|access[_-]?key)\w*\b"""
            r"""\s*[:=]\s*["'`]"""
            # Skip obvious placeholders so .env.example stays committable.
            # Every alternative here must be a DISTINCTIVE word: short, common
            # ones like `a`, `my` or `the` match the start of real secrets
            # (`a1b2c3…` begins with `a`) and silently disable the whole rule.
            r"""(?!(?:your|placeholder|example|changeme|change_me|redacted|dummy|"""
            r"""fake|sample|insert|todo|none|null|xxx|<)[\w-]*["'`])"""
            r"""([A-Za-z0-9_\-./+=]{20,})["'`]"""
        ),
        "A secret-shaped value assigned to a key-shaped name.",
    ),
    (
        "high-entropy-hex",
        re.compile(r"""(?i)\b\w*(?:key|secret|token|hash|salt)\w*\b\s*[:=]\s*["'`]([0-9a-f]{48,})["'`]"""),
        "A long hex value assigned to a secret-shaped name.",
    ),
]

# The guard's own salt is a committed 64-char hex value BY DESIGN (ADR-0009).
# It is not a secret: it exists to make the denylist non-plaintext, and the ADR
# is explicit that a committed salt is brute-forceable. Exempted so the scanner
# does not flag the one high-entropy string the design requires.
EXEMPT_PATHS = frozenset({"tools/guard/denylist.salt", "tools/guard/denylist.txt"})

TEXT_SUFFIXES = frozenset(
    {
        ".rs", ".ts", ".tsx", ".js", ".jsx", ".json", ".toml", ".yaml", ".yml",
        ".md", ".txt", ".py", ".sh", ".ps1", ".html", ".css", ".sql", ".env",
        ".example", ".cfg", ".ini", ".sample", "",
    }
)

SKIP_DIR_PARTS = frozenset({".git", "node_modules", "target", "dist", "data", "__pycache__"})


def redact(value: str) -> str:
    """Never print a suspected secret in full — printing it copies it into logs."""
    return f"{value[:4]}…{value[-2:]} ({len(value)} chars)" if len(value) > 8 else "…"


def scan_text(path: str, text: str) -> list[str]:
    rel = path.replace("\\", "/").split(" (history ", 1)[0]
    if rel in EXEMPT_PATHS:
        return []
    findings = []
    for lineno, line in enumerate(text.splitlines(), start=1):
        if len(line) > 4000:
            continue
        for rule, pattern, description in SECRET_PATTERNS:
            match = pattern.search(line)
            if match:
                captured = match.group(match.lastindex or 0)
                findings.append(f"  {rel}:{lineno}\n      [{rule}] {description}\n"
                                f"      value: {redact(captured)}")
                break
    return findings


def looks_like_text(path: str) -> bool:
    return Path(path).suffix.lower() in TEXT_SUFFIXES


def git(*args: str) -> str:
    result = subprocess.run(["git", *args], cwd=REPO_ROOT, capture_output=True,
                            text=True, encoding="utf-8", errors="replace")
    if result.returncode != 0:
        print(f"secretscan: git {' '.join(args)} failed: {result.stderr.strip()}", file=sys.stderr)
        raise SystemExit(2)
    return result.stdout


def main() -> int:
    parser = argparse.ArgumentParser(description="Local secret scanner (SPEC.md §12.6)")
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--staged", action="store_true")
    mode.add_argument("--tree", action="store_true")
    args = parser.parse_args()

    findings: list[str] = []

    if args.staged:
        what = "staged content"
        for rel in (n.strip() for n in git("diff", "--cached", "--name-only",
                                           "--diff-filter=ACMR").splitlines() if n.strip()):
            if looks_like_text(rel):
                findings += scan_text(rel, git("show", f":{rel}"))
    else:
        what = "working tree"
        # Tracked files only. Walking the filesystem sweeps in whatever happens
        # to be in the directory — a downloaded archive, an extracted tool — and
        # in CI that failed the build on an unpacked binary's own README.
        result = subprocess.run(["git", "ls-files"], cwd=REPO_ROOT,
                                capture_output=True, text=True,
                                encoding="utf-8", errors="replace")
        for rel in (p for p in result.stdout.splitlines() if p):
            if not looks_like_text(rel):
                continue
            try:
                findings += scan_text(rel, (REPO_ROOT / rel).read_text(encoding="utf-8"))
            except (UnicodeDecodeError, OSError, FileNotFoundError):
                continue

    if not findings:
        print(f"secretscan: clean — no credential-shaped values in {what}")
        return 0

    print(f"\nsecretscan: {len(findings)} finding(s) in {what} — SPEC.md §12.6\n", file=sys.stderr)
    for finding in findings:
        print(finding, file=sys.stderr)
    print(
        "\nIf this is a real credential: ROTATE IT FIRST, then remove it and clean\n"
        "history. Rotating matters more than cleaning — assume anything pushed to a\n"
        "public repository is compromised the moment it lands (§12.6).\n"
        "If it is a false positive, `--no-verify` works locally, but CI runs gitleaks\n"
        "and will still fail the push.\n",
        file=sys.stderr,
    )
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
