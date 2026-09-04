"""Measure how much of `not in catalogue` is Japanese long-vowel romanisation.

Run:  python tools/eval/measure_vowels.py
Needs `ingest anime` to have written data/anilist-unmatched.tsv.

RESULT, 2026-09-04: 200 of 5,997 (3.3%) recoverable, against 27,070 new collisions
created across the catalogue. Folding was rejected on those numbers. Kept so the
number can be reproduced rather than taken on trust — see docs/eval-results.md.

The hypothesis: AniList writes "Obake no Q-tarou" where IMDb writes "Q-Taro the
Ghost" / "Obake no Q-taro". Japanese long vowels are romanised inconsistently -
`ou`/`oo`/`o`/`o-macron` are all the same sound, as are `uu`/`u`/`u-macron`.

This measures ONLY. It builds no index and changes nothing, because the decision
of whether to fold is worth making on a number rather than on the one example
that prompted the question.
"""
import os
import re
import sqlite3
import sys
import unicodedata
from collections import Counter

sys.stdout.reconfigure(encoding="utf-8")

# The data directory, so this is runnable from a checkout rather than only from the
# machine it was first written on. `ingest` writes both files there.
DATA = os.environ.get("SINEPHILE_DATA_DIR") or os.path.join(
    os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))), "data"
)
DB = os.path.join(DATA, "sinephile.db")
TSV = os.path.join(DATA, "anilist-unmatched.tsv")


def normalise(text):
    """Mirror of matching.rs::normalise — alphanumerics kept, everything else a space."""
    out = []
    last_space = True
    for ch in text:
        ch = ch.lower()
        if ch.isalnum():
            out.append(ch)
            last_space = False
        elif not last_space:
            out.append(" ")
            last_space = True
    return "".join(out).rstrip()


def fold_long_vowels(text):
    """Collapse the ways Japanese long vowels get romanised.

    Macrons and circumflexes first (o-macron -> o), then doubled vowels, then the
    `ou` digraph. Order matters: `ou` must go after `oo` or "kooushi" folds oddly.
    """
    text = unicodedata.normalize("NFD", text)
    text = "".join(c for c in text if unicodedata.category(c) != "Mn")
    text = re.sub(r"([aeiou])\1+", r"\1", text)
    text = text.replace("ou", "o")
    return text


def main():
    con = sqlite3.connect(f"file:{DB}?mode=ro", uri=True)

    print("loading catalogue titles...", flush=True)
    exact = set()
    folded = {}
    rows = 0
    for (n,) in con.execute("SELECT DISTINCT normalised FROM titles WHERE normalised IS NOT NULL"):
        rows += 1
        exact.add(n)
        f = fold_long_vowels(n)
        if f != n:
            folded.setdefault(f, 0)
            folded[f] += 1
        else:
            folded.setdefault(n, 0)
            folded[n] += 1
    print(f"  {rows:,} distinct normalised titles, {len(folded):,} distinct folded forms")

    if not os.path.exists(TSV):
        print(f"no unmatched file at {TSV} yet")
        return

    reasons = Counter()
    recoverable = Counter()
    examples = []
    total = 0
    with open(TSV, encoding="utf-8") as fh:
        next(fh)
        for line in fh:
            parts = line.rstrip("\n").split("\t")
            if len(parts) < 7:
                continue
            total += 1
            _id, romaji, english, native, year, fmt, reason = parts[:7]
            bucket = reason.split(" (")[0]
            reasons[bucket] += 1
            if bucket != "not in catalogue":
                continue

            # Would folding have found it, when the exact form did not?
            for form in (romaji, english):
                if not form:
                    continue
                n = normalise(form)
                if n in exact:
                    continue  # not a vowel problem; something else refused it
                f = fold_long_vowels(n)
                if f in folded:
                    recoverable[bucket] += 1
                    if len(examples) < 20:
                        examples.append((form, n, f))
                    break

    print(f"\n{total:,} unmatched entries")
    for reason, count in reasons.most_common():
        print(f"  {count:6,}  {reason}")

    nic = reasons["not in catalogue"]
    rec = recoverable["not in catalogue"]
    if nic:
        print(f"\nof {nic:,} 'not in catalogue', {rec:,} ({100 * rec / nic:.1f}%) "
              f"would find a candidate under long-vowel folding")
    print("\nexamples (anilist form -> normalised -> folded):")
    for form, n, f in examples:
        print(f"  {form:<44} {n:<40} {f}")


main()
