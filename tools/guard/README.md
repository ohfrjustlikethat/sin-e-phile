# The posture guard

Automated enforcement of `SPEC.md` §2.1: **this application ships zero content
sources.** No indexer URLs, no tracker names, no bundled addon list, no default
source URL — not in code, config, fixtures, documentation, or git history.

`SPEC.md` §12.5 explains why this is automated rather than trusted to discipline:

> A human rule that must hold across 28 phases and dozens of sessions will
> eventually be broken by accident. So automate it.

Risk **R10** in `docs/RISKS.md` rates the consequence as Severe — a takedown, and
the portfolio piece disappears.

---

## Running it

```bash
python tools/guard/guard.py --selftest   # prove the guard fires (run this first)
python tools/guard/guard.py --staged     # staged content   (the pre-commit hook)
python tools/guard/guard.py --tree       # tracked files
python tools/guard/guard.py --history    # every blob ever committed (CI)

python tools/guard/secretscan.py --staged   # local secret scan (§12.6)
python tools/guard/secretscan.py --tree
```

Exit 0 clean, 1 findings, 2 could not run. Python 3.12+, standard library only
(ADR-0012), so it runs on a fresh clone before any toolchain exists.

The hooks activate via `core.hooksPath`, which `tools/doctor/doctor.py` sets.
**A fresh clone is unprotected until doctor has run once** — that is why the
guard also runs in CI, and why `docs/SETUP.md` makes doctor step one.

---

## How it decides

Three checks, designed in [ADR-0009](../../docs/adr/0009-posture-guard-denylist-design.md)
and [ADR-0010](../../docs/adr/0010-source-allowlist-and-governance.md).

### 1. Structural patterns — plaintext, and the main event

Shapes that identify forbidden content without naming any instance of it: magnet
URIs, bare 40-hex and 32-char base32 infohashes, `/announce` and `/scrape` paths,
`.torrent` URLs, and non-empty `default_source_url`-style config keys.

These are safe to commit in the clear because they name nothing, and they
generalise to sites nobody has thought of — which the denylist cannot.

### 2. Hashed denylist — the backstop

`denylist.txt` holds `SHA-256(salt || normalised_token)`. Plaintext site names are
never written to this repository; doing so would itself violate §2.1.

```bash
python tools/guard/add_token.py "why this token is denylisted"
# reads the token from stdin WITHOUT echoing it; appends only the hash
```

**This is hygiene, not security.** The salt is committed, so the hashes are
brute-forceable by anyone determined. It defends against accidental plaintext and
casual discovery — a grep, a code search, someone scrolling the file tree. Nothing
more. Do not describe it as anything stronger.

Its accepted cost is **exact-token matching only**: no substring, fuzzy, or
homoglyph matching, because all of those need the plaintext it deliberately does
not hold.

`denylist.txt` is currently **empty, and that is correct** — the project ships no
content sources, so there is no real token to deny yet. An empty denylist is not
an unverified one; run `--selftest`, which builds its own in memory.

### 3. Allowlist — the exhaustive half

`allowlist.txt` declares every content/metadata source domain permitted to appear
in shipped code. Anything source-shaped that is not listed, not RFC 2606 reserved,
and not recognised development infrastructure is a finding.

**Adding a line requires an ADR.** The denylist is best-effort; this list is
complete, so it is the half that actually holds the posture — and a one-line
change here can silently reverse §2.1 while looking innocuous in a diff.

---

## Test vectors use RFC 2606 reserved domains

Every self-test vector, fixture and documentation example uses `.invalid`,
`.test`, `.example`, or `example.com`. No registry can ever issue those, so
`notarealindexer.invalid` is *provably* not a real site.

This is what lets Phase 0 satisfy "the guard fails CI when fed a
deliberately-planted test string" without a real forbidden string ever entering
the repository or its history.

---

## The two holes, and why they are safe

The guard cannot scan files that must contain the patterns in order to define
them. Both exemptions are pinned by `tests/selftest.py`, so widening either
requires editing a test — a visible decision rather than a quiet one.

| Path | Exemption | Why it is not a hole |
|---|---|---|
| `guard.py` | structural only | Defines the patterns. Denylist and allowlist checks still apply. |
| `SPEC.md` | structural only | §12.5 documents the patterns. Same. |
| `docs/adr/0009-…` | structural only | Designs them. Same. |
| `tests/vectors.py` | **everything** | Definitionally a list of strings that must fire every check. Closed from the other side: the self-test audits every domain in it. Unscanned, but not unchecked. |

---

## False positives matter as much as false negatives

A guard that cries wolf gets `--no-verify`d, and then it protects nothing. The
self-test therefore runs 16 vectors that **must not** fire alongside the 12 that
must.

Getting there took three rounds against the real tree — 118 findings, then 12,
then 0 — and the fixes are worth knowing, because each was a genuine design flaw:

1. **A bare domain counts only if its final label is a plausible TLD.**
   `main.rs`, `manifest.json` and `Movie.2019.BluRay.x264-GROUP.mkv` are all
   shaped exactly like domains. Enumerating what *is* a TLD is finite;
   enumerating what is not is unbounded.
2. **URL hosts are extracted separately from bare domains.** A path segment
   (`/manifest.json`) otherwise reads as a domain.
3. **Inside source files, only URLs are checked.** Attribute access — `os.name`,
   `result.name`, or any dotted call ending in a two-letter word that happens to
   be a country-code TLD — is indistinguishable from a bare domain, and no
   lookbehind separates them reliably. Bare domains are still checked in prose
   and configuration, where they actually occur; the denylist runs everywhere
   regardless.

   *(Writing this section is itself a demonstration: an earlier draft named a
   real example inline and the guard blocked the commit. The fix was to remove
   the content, per §12.5 — not to add an exemption.)*
4. **Tree scans consider only git-tracked files.** Walking the filesystem swept
   in a downloaded archive's README and failed CI on someone else's example key.

---

## When it fires

Remove the content. If it is already committed, **rewrite history before
pushing**. If a real credential was committed, **rotate it first**, then clean
history — assume anything pushed to a public repository is compromised the moment
it lands (§12.6).

**Never suppress the guard to make CI pass.** `SPEC.md` §12.5 is unambiguous
about this, and R10's mitigation depends on it.
