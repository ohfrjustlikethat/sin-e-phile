#!/usr/bin/env python3
"""
Contrast audit — SPEC.md §9.1 and Phase 2's exit criteria.

Fails the build when any declared text/background pair drops below WCAG AA.
§9.1 names the pair most likely to fail: `--ink-faint` on `--surface`. **Check it.**

Why a declared list rather than scanning the CSS: a scanner cannot know which
pairs actually meet on screen, so it either misses real problems or invents
imaginary ones. `pairs.txt` states the pairs the design genuinely uses, which
makes the audit exact and makes adding a pair a deliberate act.

  python tools/contrast/audit.py            check, fail on violation
  python tools/contrast/audit.py --report   print every pair with its ratio

Exit 0 clean, 1 violations, 2 could not run.

Python 3.12+, standard library only (ADR-0012), so it runs in a hook or on a
fresh clone before anything is installed.
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
TOKENS = REPO_ROOT / "src" / "styles" / "tokens.css"
PAIRS = Path(__file__).resolve().parent / "pairs.txt"

# WCAG 2.1 minimum contrast ratios.
AA_BODY = 4.5  # text under 18.66px regular / 24px bold
AA_LARGE = 3.0  # text at or above that
AA_UI = 3.0  # borders, icons and other non-text that must be distinguishable

# `decorative` is recorded but NOT enforced. It exists so a hairline separator is
# visibly considered rather than silently omitted from the audit. It is deliberately
# narrow: use it only where WCAG imposes no minimum — a rule that separates content
# but identifies no component. A control boundary is `ui` and is enforced.
THRESHOLD = {"body": AA_BODY, "large": AA_LARGE, "ui": AA_UI, "decorative": None}


def parse_tokens() -> dict[str, str]:
    """Every `--name: value;` in tokens.css."""
    text = TOKENS.read_text(encoding="utf-8")
    # Strip comments so a hex inside prose is not mistaken for a declaration.
    text = re.sub(r"/\*.*?\*/", "", text, flags=re.S)
    return {m[0]: m[1].strip() for m in re.findall(r"(--[\w-]+)\s*:\s*([^;]+);", text)}


def to_rgb(value: str, tokens: dict[str, str], depth: int = 0) -> tuple[float, float, float] | None:
    """Resolve a token value to RGB, following var() one level at a time."""
    value = value.strip()
    if depth > 8:
        return None

    ref = re.fullmatch(r"var\(\s*(--[\w-]+)\s*\)", value)
    if ref:
        target = tokens.get(ref.group(1))
        return to_rgb(target, tokens, depth + 1) if target else None

    hexed = re.fullmatch(r"#([0-9a-fA-F]{3}|[0-9a-fA-F]{6})", value)
    if hexed:
        h = hexed.group(1)
        if len(h) == 3:
            h = "".join(c * 2 for c in h)
        return (int(h[0:2], 16), int(h[2:4], 16), int(h[4:6], 16))

    rgba = re.fullmatch(r"rgba?\(([^)]+)\)", value)
    if rgba:
        parts = [p.strip() for p in rgba.group(1).replace("/", ",").split(",")]
        if len(parts) >= 3:
            try:
                return tuple(float(p) for p in parts[:3])  # type: ignore[return-value]
            except ValueError:
                return None
    return None


def composite(fg: tuple[float, float, float], alpha: float,
              bg: tuple[float, float, float]) -> tuple[float, float, float]:
    """Flatten a translucent colour onto its background.

    Without this, a token like --oxblood-wash (alpha 0.16) would be measured as if
    it were opaque, which is not what anyone sees.
    """
    return tuple(fg[i] * alpha + bg[i] * (1 - alpha) for i in range(3))  # type: ignore[return-value]


def alpha_of(value: str, tokens: dict[str, str], depth: int = 0) -> float:
    value = value.strip()
    if depth > 8:
        return 1.0
    ref = re.fullmatch(r"var\(\s*(--[\w-]+)\s*\)", value)
    if ref and ref.group(1) in tokens:
        return alpha_of(tokens[ref.group(1)], tokens, depth + 1)
    m = re.fullmatch(r"rgba\(([^)]+)\)", value)
    if m:
        parts = [p.strip() for p in m.group(1).replace("/", ",").split(",")]
        if len(parts) == 4:
            try:
                return float(parts[3])
            except ValueError:
                return 1.0
    return 1.0


def relative_luminance(rgb: tuple[float, float, float]) -> float:
    """WCAG 2.1 relative luminance."""
    def channel(c: float) -> float:
        c = c / 255.0
        return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4

    r, g, b = (channel(x) for x in rgb)
    return 0.2126 * r + 0.7152 * g + 0.0722 * b


def contrast(a: tuple[float, float, float], b: tuple[float, float, float]) -> float:
    la, lb = relative_luminance(a), relative_luminance(b)
    lighter, darker = max(la, lb), min(la, lb)
    return (lighter + 0.05) / (darker + 0.05)


def load_pairs() -> list[tuple[str, str, str, str]]:
    """(foreground, background, class, why) from pairs.txt."""
    out = []
    for lineno, raw in enumerate(PAIRS.read_text(encoding="utf-8").splitlines(), 1):
        line = raw.split("#", 1)[0].strip()
        if not line:
            continue
        parts = [p.strip() for p in line.split("|")]
        if len(parts) != 4:
            print(f"pairs.txt:{lineno}: expected 4 fields, got {len(parts)}", file=sys.stderr)
            raise SystemExit(2)
        out.append((parts[0], parts[1], parts[2], parts[3]))
    return out


def main() -> int:
    parser = argparse.ArgumentParser(description="WCAG AA audit of the design tokens")
    parser.add_argument("--report", action="store_true", help="print every pair, not only failures")
    args = parser.parse_args()

    tokens = parse_tokens()
    if not tokens:
        print("audit: no tokens parsed from tokens.css", file=sys.stderr)
        return 2

    rows, failures = [], []
    for fg_name, bg_name, size_class, why in load_pairs():
        if size_class not in THRESHOLD:
            print(f"audit: unknown class {size_class!r} for {fg_name} on {bg_name}", file=sys.stderr)
            return 2
        if fg_name not in tokens or bg_name not in tokens:
            missing = fg_name if fg_name not in tokens else bg_name
            print(f"audit: {missing} is not defined in tokens.css", file=sys.stderr)
            return 2

        bg = to_rgb(tokens[bg_name], tokens)
        fg = to_rgb(tokens[fg_name], tokens)
        if bg is None or fg is None:
            print(f"audit: could not resolve {fg_name} on {bg_name}", file=sys.stderr)
            return 2

        # Flatten translucency onto the background before measuring.
        a = alpha_of(tokens[fg_name], tokens)
        if a < 1.0:
            fg = composite(fg, a, bg)

        ratio = contrast(fg, bg)
        need = THRESHOLD[size_class]
        ok = True if need is None else ratio >= need
        rows.append((ok, fg_name, bg_name, size_class, ratio, need, why))
        if not ok:
            failures.append((fg_name, bg_name, size_class, ratio, need, why))

    if args.report or failures:
        print(f"\n{'':2} {'foreground':<20} {'background':<16} {'class':<6} {'ratio':>7}  {'min':>5}")
        print("   " + "-" * 66)
        for ok, fg_name, bg_name, size_class, ratio, need, why in rows:
            mark = "--" if THRESHOLD[size_class] is None else ("ok" if ok else "XX")
            need_s = "  n/a" if need is None else f"{need:>4.1f}"
            print(f"{mark:2} {fg_name:<20} {bg_name:<16} {size_class:<11} {ratio:>6.2f}:1  {need_s}")
            if not ok:
                print(f"   {'':2} {why}")

    if failures:
        print(f"\naudit: {len(failures)} pair(s) below WCAG AA — SPEC.md §9.1\n", file=sys.stderr)
        for fg_name, bg_name, size_class, ratio, need, why in failures:
            short = need - ratio
            print(f"  {fg_name} on {bg_name} ({size_class}): {ratio:.2f}:1, "
                  f"needs {need:.1f}:1 — short by {short:.2f}", file=sys.stderr)
            print(f"      used for: {why}", file=sys.stderr)
        print("\nFix the TOKEN, not the component. A one-off override reintroduces the\n"
              "problem everywhere else the pair is used.\n", file=sys.stderr)
        return 1

    enforced = sum(1 for r in rows if THRESHOLD[r[3]] is not None)
    print(f"contrast: {enforced} enforced pairs pass WCAG AA "
          f"({len(rows) - enforced} decorative recorded, not enforced)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
