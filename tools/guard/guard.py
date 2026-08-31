#!/usr/bin/env python3
"""
sin-e-phile posture guard — automated enforcement of SPEC.md §2.1.

Fails the build if this repository contains content sources it must never ship.
See SPEC.md §12.5 for the rules and ADR-0009 / ADR-0010 for why it is built this way.

THREE CHECKS, run over whatever content the chosen mode supplies:

  1. Structural patterns (plaintext).  Shapes that identify forbidden content
     without naming any instance of it: magnet URIs, bare infohashes, tracker
     announce paths, .torrent URLs, non-empty default-source config keys.
     This does most of the real work and generalises to sites nobody thought of.

  2. Hashed denylist (salted SHA-256).  Exact-token matching against
     denylist.txt. Plaintext site names are NEVER written to this repository —
     writing them would itself violate §2.1. This is HYGIENE, NOT SECURITY: the
     salt is committed and therefore brute-forceable. It defends against
     accidental plaintext and casual discovery, nothing more. Its accepted cost
     is exact-match only — no substring, fuzzy, or homoglyph matching.

  3. Allowlist.  Every content/metadata source domain that appears in the
     repository must be declared in allowlist.txt with a justification, or be an
     RFC 2606 reserved domain, or be recognised development infrastructure.
     Anything else is a finding. The denylist is best-effort; the allowlist is
     exhaustive, and it is the half that actually holds the posture.

MODES
  --staged     scan git-staged content only (pre-commit hook: must stay fast)
  --tree       scan the working tree
  --history    scan every blob that has ever existed in this repository (CI)
  --selftest   prove the guard fires, using RFC 2606 vectors (ADR-0009)

Exit code 0 = clean, 1 = findings, 2 = guard could not run.

Python 3.12+, standard library only (ADR-0012), so it works on a fresh clone
before any toolchain is installed.
"""

from __future__ import annotations

import argparse
import hashlib
import re
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
GUARD_DIR = Path(__file__).resolve().parent

# ─────────────────────────────────────────────────────────────────────────────
# Check 1 — structural patterns
#
# Each is a shape, not a name. They are safe to commit in the clear precisely
# because they identify no particular site. Written so they do not match their
# own source text (the regex escaping does this for us: the literal source here
# reads `magnet:\?`, which the compiled pattern `magnet:\?` does not match).
# ─────────────────────────────────────────────────────────────────────────────

STRUCTURAL_PATTERNS: list[tuple[str, re.Pattern[str], str]] = [
    (
        "magnet-uri",
        re.compile(r"magnet:\?[^\s\"'`]*xt=urn:btih:", re.IGNORECASE),
        "A magnet URI. SPEC.md §2.1: no hardcoded magnet links outside clearly-marked legal fixtures.",
    ),
    (
        "infohash-hex",
        re.compile(r"(?<![0-9a-fA-F])[0-9a-fA-F]{40}(?![0-9a-fA-F])"),
        "A bare 40-character hex string — the shape of a BitTorrent v1 infohash.",
    ),
    (
        "infohash-base32",
        re.compile(r"(?<![A-Z2-7])[A-Z2-7]{32}(?![A-Z2-7])"),
        "A bare 32-character base32 string — the shape of a base32-encoded infohash.",
    ),
    (
        "tracker-announce",
        re.compile(r"https?://[^\s\"'`]+/(announce|scrape)\b", re.IGNORECASE),
        "A tracker announce/scrape endpoint.",
    ),
    (
        "torrent-url",
        re.compile(r"https?://[^\s\"'`]+\.torrent\b", re.IGNORECASE),
        "A URL pointing at a .torrent file.",
    ),
    (
        "default-source-key",
        re.compile(
            # A default-source key assigned a NON-EMPTY value: either a quoted
            # string with real content, or a bare token that is not a null-ish
            # literal. `default_source_url = ""` is correct and must not fire.
            r"""\b(?:default_(?:source|indexer|catalogue|catalog|addon)_?(?:url|uri|list)?)\b"""
            r"""\s*[:=]\s*"""
            r"""(?:(["'`])[^"'`\s][^"'`]*\1"""
            r"""|(?!null\b|none\b|nil\b|false\b|\[\s*\]|\{\s*\})[A-Za-z0-9/][^\s,;]*)""",
            re.IGNORECASE,
        ),
        "A default source/indexer/catalogue URL with a non-empty value. SPEC.md §2.1: no default URL ships.",
    ),
]

# Four files must be able to contain these patterns in order to DEFINE or
# DOCUMENT them: the guard's own source, its test vectors, the section of the
# specification that specifies the guard, and the ADR that designs it.
#
# They are exempt from CHECK 1 ONLY. The denylist and allowlist checks still
# apply to them in full.
#
# This exemption is the guard's one hole, so it is deliberately tiny, and
# selftest.py pins the exact set — widening it requires editing a test, which
# makes it a visible decision rather than a quiet one. R10 is about exactly this
# kind of erosion, so the list must never grow to include ordinary source files.
STRUCTURAL_EXEMPT: frozenset[str] = frozenset(
    {
        "tools/guard/guard.py",
        "SPEC.md",
        "docs/adr/0009-posture-guard-denylist-design.md",
    }
)

# The test vectors are a special case: the file is definitionally a list of
# strings that MUST trigger every check, including the undeclared-domain check.
# It is therefore skipped entirely rather than partially.
#
# That would be a real hole, so selftest.py closes it from the other side: it
# audits every domain appearing in this file and fails unless each one is RFC
# 2606 reserved, known infrastructure, allowlisted, or a pinned synthetic
# placeholder. The file is unscanned by the guard but not unchecked.
FULLY_EXEMPT: frozenset[str] = frozenset({"tools/guard/tests/vectors.py"})

# ─────────────────────────────────────────────────────────────────────────────
# Check 3 — domain classification
# ─────────────────────────────────────────────────────────────────────────────

# RFC 2606 / RFC 6761 reserved names. No registry can ever issue these, so a
# string ending in one is provably not a real site (ADR-0009).
RESERVED_SUFFIXES = (".invalid", ".test", ".example", ".localhost", ".local")
RESERVED_EXACT = frozenset({"example.com", "example.net", "example.org"})

# Development infrastructure. Explicitly out of scope per ADR-0010 — scanning it
# produces constant noise for zero posture benefit.
INFRASTRUCTURE_DOMAINS = frozenset(
    {
        "crates.io", "docs.rs", "rust-lang.org", "npmjs.com", "nodejs.org",
        "github.com", "githubusercontent.com", "github.io", "gitlab.com",
        "python.org", "pypi.org", "tauri.app", "vitejs.dev", "tailwindcss.com",
        "react.dev", "typescriptlang.org", "developer.mozilla.org", "mozilla.org",
        "w3.org", "ietf.org", "rfc-editor.org", "unicode.org", "sqlite.org",
        "ffmpeg.org", "videolan.org", "mpv.io", "onnxruntime.ai", "huggingface.co",
        # Licence and standards bodies. Cited by canonical licence texts we do not
        # own and must not edit, so they can never be removed from the tree.
        "gnu.org", "fsf.org", "apache.org", "opensource.org", "mit-license.org",
        "microsoft.com", "wikipedia.org", "schema.org",
        "json-schema.org", "semver.org", "conventionalcommits.org", "keepachangelog.com",
        "rustup.rs", "git-scm.com", "visualstudio.microsoft.com", "gitleaks.io",
        "adr.github.io", "pola.rs", "serde.rs",
        # Package registries and funding links that appear in generated lockfiles.
        # Lockfiles are committed deliberately (R8), so these must be permitted —
        # but as INFRASTRUCTURE, not on the allowlist, which ADR-0010 scopes to
        # content and metadata sources.
        "npmjs.org", "eslint.org", "opencollective.com", "tidelift.com",
    }
)

# The application's own reverse-DNS bundle identifier. Domain-shaped by
# convention (Tauri, Windows and macOS all require this form), but it is not a
# host anything connects to — it is this project's name for itself.
#
# Deliberately NOT in the allowlist (ADR-0010 scopes that to content and metadata
# SOURCES) and not in INFRASTRUCTURE_DOMAINS (it is not third-party tooling).
# A separate one-entry set keeps both of those honest, and keeps this visible.
SELF_IDENTIFIERS = frozenset({"dev.sinephile.app"})

# Domains are found two ways, because a path segment like `/manifest.json` and a
# filename like `Movie.2019.x264-GROUP.mkv` otherwise look exactly like domains.
#
#   URL_RE          — pulls the host out of a real URL. Authoritative.
#   BARE_DOMAIN_RE  — a bare domain in prose or code. Must NOT be preceded by `/`
#                     (that makes it a path segment), and its final label must not
#                     be a file extension (that makes it a filename).
URL_RE = re.compile(
    r"\b(?:https?|ftp)://(?:[^\s/@\"'`]+@)?"
    r"((?:[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z]{2,24})",
    re.IGNORECASE,
)

# `[profile.dev]`, `[tool.poetry.group.dev.dependencies]` — a section header, not
# a host. Anchored to the whole line so it cannot swallow anything else.
TOML_TABLE_RE = re.compile(r"^\s*\[\[?[A-Za-z0-9_.\-]+\]\]?\s*(#.*)?$")

BARE_DOMAIN_RE = re.compile(
    r"(?<![/\w.@-])((?:[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z]{2,24})(?![\w-])",
    re.IGNORECASE,
)

# A bare domain only counts if its final label is a plausible TLD.
#
# This is an ALLOWLIST of endings rather than a blocklist of file extensions,
# because the false-positive space is far larger than it first appears. Dotted
# code identifiers (`core.hooksPath`), scene filenames
# (`Movie.Title.2019.1080p.BluRay`), and prose (`The.Batman`) all match the shape
# of a domain perfectly. Enumerating everything that ISN'T a domain is unbounded;
# enumerating what IS one is finite.
#
# A real domain on an exotic TLD written *bare* will be missed. That is the
# accepted cost, and it is small: written as a URL it is still caught by URL_RE,
# which does not consult this set.
KNOWN_TLDS = frozenset(
    {
        # Reserved — RFC 2606 / RFC 6761. Load-bearing for this whole design.
        "test", "invalid", "example", "local", "localhost",
        # Generic
        # `.name` is a real gTLD but is overwhelmingly attribute access in practice
        # (`os.name`, `result.name`), so it is deliberately omitted.
        "com", "org", "net", "int", "edu", "gov", "mil", "info", "biz",
        "io", "co", "tv", "cc", "me", "dev", "app", "ai", "xyz", "online", "site",
        "club", "link", "live", "stream", "media", "studio", "moe", "fm", "gg",
        "sh", "to", "ws", "la", "li", "ly", "gl", "is", "im", "st", "so", "re",
        # Country codes seen in practice around media and metadata
        "uk", "de", "fr", "jp", "cn", "ru", "br", "in", "nl", "se", "no", "fi",
        "dk", "pl", "es", "it", "ca", "au", "nz", "us", "eu", "ch", "at", "be",
        "pt", "gr", "cz", "hu", "ro", "ua", "tr", "kr", "tw", "hk", "sg", "za",
        "mx", "ar", "cl", "id", "th", "vn", "ph", "my", "ir", "il", "sa", "ae",
    }
)

# Candidate tokens for the hashed denylist: domain-ish or word-ish runs.
TOKEN_RE = re.compile(r"[A-Za-z0-9][A-Za-z0-9._-]{2,63}")

TEXT_SUFFIXES = frozenset(
    {
        ".rs", ".ts", ".tsx", ".js", ".jsx", ".json", ".jsonc", ".toml", ".yaml", ".yml",
        ".md", ".txt", ".py", ".sh", ".ps1", ".html", ".css", ".sql", ".graphql",
        ".cfg", ".ini", ".env", ".example", ".lock", ".gitignore", ".sample", "",
    }
)

SKIP_DIR_PARTS = frozenset({".git", "node_modules", "target", "dist", "data", "__pycache__"})


class Finding:
    """One violation. Rendered as `path:line: [rule] detail`."""

    def __init__(self, path: str, line: int, rule: str, detail: str, excerpt: str) -> None:
        self.path, self.line, self.rule = path, line, rule
        self.detail, self.excerpt = detail, excerpt

    def __str__(self) -> str:
        return f"  {self.path}:{self.line}\n      [{self.rule}] {self.detail}\n      > {self.excerpt}"


# ─────────────────────────────────────────────────────────────────────────────
# Denylist and allowlist loading
# ─────────────────────────────────────────────────────────────────────────────

def normalise_token(token: str) -> str:
    """Fold a candidate to its comparison form.

    Must match add_token.py exactly, or hashes will never line up: lowercase,
    drop scheme, drop a leading `www.`, strip surrounding punctuation.
    """
    t = token.strip().lower()
    t = re.sub(r"^[a-z][a-z0-9+.-]*://", "", t)
    t = t.removeprefix("www.")
    return t.strip("./-_[](){}<>\"'`,;:!?")


def hash_token(token: str, salt: str) -> str:
    return hashlib.sha256((salt + normalise_token(token)).encode("utf-8")).hexdigest()


def load_salt() -> str:
    path = GUARD_DIR / "denylist.salt"
    if not path.exists():
        die(f"missing {path.relative_to(REPO_ROOT)} — the denylist cannot be checked without it")
    return path.read_text(encoding="utf-8").strip()


def load_denylist() -> set[str]:
    path = GUARD_DIR / "denylist.txt"
    if not path.exists():
        return set()
    out: set[str] = set()
    for raw in path.read_text(encoding="utf-8").splitlines():
        line = raw.split("#", 1)[0].strip()
        if line:
            out.add(line.lower())
    return out


def load_allowlist() -> set[str]:
    path = GUARD_DIR / "allowlist.txt"
    if not path.exists():
        return set()
    out: set[str] = set()
    for raw in path.read_text(encoding="utf-8").splitlines():
        line = raw.split("#", 1)[0].strip().lower()
        if line:
            out.add(line.removeprefix("www."))
    return out


def extract_domains(line: str, urls_only: bool = False) -> list[str]:
    """Every domain in a line: hosts from real URLs, plus bare domains in prose.

    `urls_only` restricts this to URL hosts, for source files where attribute
    access is indistinguishable from a bare domain (see CODE_SUFFIXES).

    Bare candidates whose final label is not a plausible TLD are dropped — see
    KNOWN_TLDS for why that matters more than it sounds like it should.
    """
    found: list[str] = [m.group(1) for m in URL_RE.finditer(line)]
    if urls_only:
        return found
    # A TOML table header is not a domain. `[profile.dev]` and `[tool.x.dev]`
    # match the shape exactly, and config files are scanned for bare domains.
    if TOML_TABLE_RE.match(line):
        return found
    for match in BARE_DOMAIN_RE.finditer(line):
        candidate = match.group(1)
        if candidate.rsplit(".", 1)[-1].lower() in KNOWN_TLDS:
            found.append(candidate)
    return found


def domain_is_permitted(domain: str, allowlist: set[str]) -> bool:
    """A domain passes if it is reserved, infrastructure, or explicitly allowed.

    Suffix matching means an allowlist entry covers its subdomains: allowing
    `archive.org` also allows `ia801234.us.archive.org`.
    """
    d = domain.lower().removeprefix("www.")
    if d in SELF_IDENTIFIERS:
        return True
    if d in RESERVED_EXACT or d.endswith(RESERVED_SUFFIXES):
        return True
    for known in (INFRASTRUCTURE_DOMAINS | allowlist):
        if d == known or d.endswith("." + known):
            return True
    return False


# ─────────────────────────────────────────────────────────────────────────────
# The scan
# ─────────────────────────────────────────────────────────────────────────────

# Source files where bare-domain detection produces more noise than signal.
#
# `os.name`, `result.name`, `df.to`, `Path.is` — attribute access is shaped
# exactly like a bare domain, and no lookbehind distinguishes them reliably. So
# inside code, only URL_RE applies (a real URL with a scheme, which is precise);
# bare domains are checked in prose and configuration, where they actually occur.
#
# The residual gap is a scheme-less forbidden domain sitting in a source file.
# That is caught by the denylist matcher, which runs everywhere regardless.
CODE_SUFFIXES = frozenset(
    {".rs", ".ts", ".tsx", ".js", ".jsx", ".py", ".go", ".java", ".kt", ".rb",
     ".c", ".h", ".cpp", ".hpp", ".cs", ".sh", ".ps1", ".bat", ".sql"}
)


def scan_text(path: str, text: str, salt: str, deny: set[str], allow: set[str]) -> list[Finding]:
    findings: list[Finding] = []
    rel = path.replace("\\", "/")
    # History entries arrive as "path (history abcdef12)"; match on the path part.
    bare = rel.split(" (history ", 1)[0]
    if bare in FULLY_EXEMPT:
        return findings
    structural_exempt = bare in STRUCTURAL_EXEMPT
    urls_only = Path(bare).suffix.lower() in CODE_SUFFIXES

    for lineno, line in enumerate(text.splitlines(), start=1):
        if len(line) > 4000:  # minified or generated; not human-authored content
            continue
        excerpt = line.strip()[:160]

        if not structural_exempt:
            for rule, pattern, detail in STRUCTURAL_PATTERNS:
                if pattern.search(line):
                    findings.append(Finding(rel, lineno, rule, detail, excerpt))

        # Check 2 — hashed denylist over every candidate token.
        if deny:
            for match in TOKEN_RE.finditer(line):
                token = normalise_token(match.group(0))
                if len(token) < 3:
                    continue
                if hash_token(token, salt) in deny:
                    findings.append(
                        Finding(rel, lineno, "denylist",
                                "Matches a denylisted token (see ADR-0009; the token is not printed).",
                                excerpt)
                    )
                    break

        # Check 3 — every source-shaped domain must be declared.
        for domain in extract_domains(line, urls_only=urls_only):
            if not domain_is_permitted(domain, allow):
                findings.append(
                    Finding(rel, lineno, "undeclared-domain",
                            f"'{domain}' is not on the allowlist, not reserved, and not known "
                            f"infrastructure. Add it to tools/guard/allowlist.txt with an ADR "
                            f"(ADR-0010), use an RFC 2606 example domain, or remove it.",
                            excerpt)
                )

    return findings


def looks_like_text(path: str) -> bool:
    return Path(path).suffix.lower() in TEXT_SUFFIXES


def git(*args: str) -> str:
    result = subprocess.run(
        ["git", *args], cwd=REPO_ROOT, capture_output=True, text=True,
        encoding="utf-8", errors="replace",
    )
    if result.returncode != 0:
        die(f"git {' '.join(args)} failed: {result.stderr.strip()}")
    return result.stdout


def tracked_files() -> list[str]:
    """Files git knows about. Tree scans consider ONLY these.

    Walking the filesystem instead sweeps in whatever happens to be sitting in
    the working directory — a downloaded archive, a scratch file, an extracted
    tool. In CI that produced a false failure when an unpacked binary's own
    README (containing an example key) was scanned as though it were project
    source. Untracked content is caught at `git add` time by the pre-commit
    hook, which is the right moment for it.
    """
    result = subprocess.run(
        ["git", "ls-files"], cwd=REPO_ROOT, capture_output=True,
        text=True, encoding="utf-8", errors="replace",
    )
    if result.returncode != 0:
        return []
    return [p for p in result.stdout.splitlines() if p]


def tree_sources():
    """(path, text) for every tracked Rust file — what the architecture check reads."""
    for rel in tracked_files():
        if not rel.endswith(".rs"):
            continue
        try:
            yield rel, (REPO_ROOT / rel).read_text(encoding="utf-8")
        except (UnicodeDecodeError, OSError, FileNotFoundError):
            continue


def staged_sources():
    """Same, but the staged blob — so the hook sees what is about to be committed."""
    names = git("diff", "--cached", "--name-only", "--diff-filter=ACMR")
    for rel in (n.strip() for n in names.splitlines() if n.strip()):
        if not rel.endswith(".rs"):
            continue
        try:
            yield rel, git("show", f":{rel}")
        except SystemExit:
            continue


def scan_tree(salt, deny, allow) -> list[Finding]:
    findings = []
    for rel in tracked_files():
        if not looks_like_text(rel):
            continue
        path = REPO_ROOT / rel
        try:
            text = path.read_text(encoding="utf-8")
        except (UnicodeDecodeError, OSError, FileNotFoundError):
            continue
        findings += scan_text(rel, text, salt, deny, allow)
    return findings


def scan_staged(salt, deny, allow) -> list[Finding]:
    findings = []
    names = git("diff", "--cached", "--name-only", "--diff-filter=ACMR")
    for rel in (n.strip() for n in names.splitlines() if n.strip()):
        if not looks_like_text(rel):
            continue
        try:
            text = git("show", f":{rel}")
        except SystemExit:
            continue
        findings += scan_text(rel, text, salt, deny, allow)
    return findings


def scan_history(salt, deny, allow) -> list[Finding]:
    """Scan every blob that has ever existed — content deleted later is still caught.

    SPEC.md §12.5 requires history scanning, because a mistake that was committed
    and then removed is still in the repository.
    """
    findings, seen = [], set()
    listing = git("rev-list", "--objects", "--all")
    for line in listing.splitlines():
        parts = line.split(maxsplit=1)
        if len(parts) != 2:
            continue
        sha, name = parts
        if sha in seen or not looks_like_text(name):
            continue
        seen.add(sha)
        kind = subprocess.run(
            ["git", "cat-file", "-t", sha], cwd=REPO_ROOT,
            capture_output=True, text=True,
        ).stdout.strip()
        if kind != "blob":
            continue
        content = subprocess.run(
            ["git", "cat-file", "blob", sha], cwd=REPO_ROOT,
            capture_output=True, text=True, encoding="utf-8", errors="replace",
        ).stdout
        findings += scan_text(f"{name} (history {sha[:8]})", content, salt, deny, allow)
    return findings



# -----------------------------------------------------------------------------
# The architecture check (ADR-0022)
# -----------------------------------------------------------------------------
#
# `cargo test` cannot run inside `src-tauri` on Windows: every test binary dies at
# load with STATUS_ENTRYPOINT_NOT_FOUND. So logic that ought to be tested must not
# live there. The data layer is the largest instance of this — SPEC.md Phase 3
# requires integration tests for migrations forward and backward, which is
# impossible if the SQL lives in the Tauri crate.
#
# The author's ruling (2026-09-01): "make it enforceable rather than remembered".
# So this is a check, not a convention. It runs with --tree and --staged, which
# means pre-commit and CI both enforce it.

SQL_STATEMENT_RE = re.compile(
    r'"\s*(SELECT|INSERT\s+INTO|UPDATE|DELETE\s+FROM|CREATE\s+(TABLE|INDEX|VIEW|TRIGGER)'
    r'|ALTER\s+TABLE|DROP\s+(TABLE|INDEX|VIEW)|PRAGMA|BEGIN\s+TRANSACTION)',
    re.IGNORECASE,
)
SQLX_RE = re.compile(r"sqlx")

# A re-export module may contain only these statements. Checked per STATEMENT, not
# per line: a rustfmt-wrapped `pub use foo::{A, B, C};` spans four lines, three of
# which look like nothing in particular. The line-based first version rejected every
# multi-line re-export in the file it was written to protect.
REEXPORT_OK_RE = re.compile(
    r"^(pub\s+use|use|pub\s+mod|mod|pub\s+type|pub\s+crate::|#!?\[)", re.S
)


def rust_statements(text: str):
    """Yield (line_number, statement) with comments stripped.

    Deliberately simple: this is a structural check on a module that is allowed to
    contain almost nothing, not a Rust parser.
    """
    out, buf, start_line = [], [], 1
    for n, raw in enumerate(text.splitlines(), 1):
        line = raw.split("//")[0].rstrip()
        if not line.strip():
            continue
        if not buf:
            start_line = n
        buf.append(line.strip())
        if line.rstrip().endswith(";") or line.strip() in ("}", "};"):
            out.append((start_line, " ".join(buf)))
            buf = []
    if buf:
        out.append((start_line, " ".join(buf)))
    return out


def check_architecture(paths_and_text) -> list[str]:
    """No SQL under src-tauri/, and src-tauri/src/persistence/ holds re-exports only."""
    errors: list[str] = []
    for path, text in paths_and_text:
        rel = path.replace("\\", "/")
        if not rel.startswith("src-tauri/") or not rel.endswith(".rs"):
            continue

        in_persistence = rel.startswith("src-tauri/src/persistence/")

        for n, line in enumerate(text.splitlines(), 1):
            if SQLX_RE.search(line) or SQL_STATEMENT_RE.search(line):
                errors.append(
                    f"{rel}:{n}: SQL or sqlx under src-tauri/. Raw SQL lives in "
                    f"crates/persistence/ (ADR-0022); src-tauri re-exports it."
                )

        if in_persistence:
            for n, statement in rust_statements(text):
                if not REEXPORT_OK_RE.match(statement):
                    errors.append(
                        f"{rel}:{n}: src-tauri/src/persistence/ contains re-exports "
                        f"and nothing else. This is logic: {statement[:60]}"
                    )
    return errors


def die(message: str) -> None:
    print(f"guard: {message}", file=sys.stderr)
    raise SystemExit(2)


def main() -> int:
    parser = argparse.ArgumentParser(description="sin-e-phile posture guard (SPEC.md §12.5)")
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--staged", action="store_true", help="scan staged content (pre-commit)")
    mode.add_argument("--tree", action="store_true", help="scan the working tree")
    mode.add_argument("--history", action="store_true", help="scan all blobs ever committed (CI)")
    mode.add_argument("--selftest", action="store_true", help="prove the guard fires")
    args = parser.parse_args()

    if args.selftest:
        from tests.selftest import run_selftest  # noqa: PLC0415
        return run_selftest()

    salt, deny, allow = load_salt(), load_denylist(), load_allowlist()

    arch: list[str] = []
    if args.staged:
        findings, what = scan_staged(salt, deny, allow), "staged content"
        arch = check_architecture(staged_sources())
    elif args.tree:
        findings, what = scan_tree(salt, deny, allow), "working tree"
        arch = check_architecture(tree_sources())
    else:
        findings, what = scan_history(salt, deny, allow), "full history"

    if arch:
        print("", file=sys.stderr)
        print(f"guard: {len(arch)} architecture violation(s) in {what} — ADR-0022",
              file=sys.stderr)
        print("", file=sys.stderr)
        for error in arch:
            print(f"  {error}", file=sys.stderr)
        print("", file=sys.stderr)
        print("cargo test cannot run inside src-tauri, so logic that must be tested "
              "cannot live there.", file=sys.stderr)
        print("", file=sys.stderr)
        return 1

    if not findings:
        print(f"guard: clean — {what} contains no content-source violations "
              f"({len(deny)} denylist entries, {len(allow)} allowlisted domains)")
        return 0

    print(f"\nguard: {len(findings)} finding(s) in {what} — SPEC.md §2.1\n", file=sys.stderr)
    for finding in findings:
        print(finding, file=sys.stderr)
    print(
        "\nThe fix is to REMOVE the content. If it is already committed, rewrite\n"
        "history before pushing. Never suppress the guard to make CI pass (§12.5).\n",
        file=sys.stderr,
    )
    return 1


if __name__ == "__main__":
    sys.path.insert(0, str(GUARD_DIR))
    raise SystemExit(main())
