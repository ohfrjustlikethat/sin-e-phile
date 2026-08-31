"""
Test vectors for the posture guard.

EVERY vector here uses an RFC 2606 / RFC 6761 reserved domain — .invalid, .test,
.example — or a synthetic infohash of the right shape. No real forbidden string
appears in this file, or anywhere in this repository, or in its history.

That is the whole point of ADR-0009's third rule: `notarealindexer.invalid` is
*provably* not a real site, because no registry can ever issue `.invalid`. So the
guard can be proven to fire without the repository containing the thing the guard
exists to prevent.

This file is skipped by the guard ENTIRELY (FULLY_EXEMPT in guard.py), because it
is definitionally a list of strings that must trigger every check — including the
undeclared-domain check, which no exemption-by-category could accommodate.

That would be a real hole, so selftest.py closes it from the other side: it audits
every domain appearing here and fails unless each is RFC 2606 reserved, known
infrastructure, allowlisted, or in PINNED_PLACEHOLDERS below. This file is
unscanned, but it is not unchecked.
"""

from __future__ import annotations

# ── Tokens the self-test injects into an in-memory denylist. ──────────────────
#
# These are NOT in the committed denylist.txt, which is empty and correctly so:
# the project ships no content sources, so there is no real token to deny yet.
# Seeding the committed file with these would mean every document that mentions
# them — this file, the ADR, the spec — trips the guard.
#
# Both are RFC 2606 reserved, so they are provably not real sites.
DENYLIST_VECTORS: list[str] = [
    "notarealindexer.invalid",
    "guardcanary.test",
]

# Synthetic domains that exist only so the undeclared-domain check has something
# to fire on. Not real sites, deliberately boring, and pinned so the set cannot
# grow without an explicit edit. selftest.py imports these to audit this file:
# every domain here must be reserved, infrastructure, allowlisted, or listed below.
PINNED_PLACEHOLDERS: frozenset[str] = frozenset({
    "some-metadata-site.io",  # undeclared-domain vector
    "profile.dev",            # TOML table header vector - a section name, not a host
})

# ── Vectors that MUST be caught. (label, content, expected rule) ──────────────

SHOULD_FIRE: list[tuple[str, str, str]] = [
    (
        "magnet URI",
        "let link = \"magnet:?xt=urn:btih:0123456789abcdef0123456789abcdef01234567\";",
        "magnet-uri",
    ),
    (
        "bare hex infohash",
        "const HASH: &str = \"0123456789abcdef0123456789abcdef01234567\";",
        "infohash-hex",
    ),
    (
        "bare base32 infohash",
        "hash = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ234567'",
        "infohash-base32",
    ),
    (
        "tracker announce endpoint",
        "tracker = \"https://tracker.invalid/announce\"",
        "tracker-announce",
    ),
    (
        "tracker scrape endpoint",
        "url = \"http://tracker.test/scrape?info_hash=x\"",
        "tracker-announce",
    ),
    (
        "torrent file URL",
        "download(\"https://files.invalid/ubuntu.torrent\")",
        "torrent-url",
    ),
    (
        "default source URL in config",
        "default_source_url = \"https://sources.invalid/manifest.json\"",
        "default-source-key",
    ),
    (
        "default catalogue URL in config",
        "default_catalogue_url: \"https://catalogue.test/list.json\",",
        "default-source-key",
    ),
    (
        "denylisted token, bare",
        "// see notarealindexer.invalid for the list",
        "denylist",
    ),
    (
        "denylisted token, as a URL",
        "const SRC: &str = \"https://www.notarealindexer.invalid/api\";",
        "denylist",
    ),
    (
        "denylisted token, second vector",
        "fetch('guardcanary.test')",
        "denylist",
    ),
    (
        "undeclared source-shaped domain",
        "let api = \"https://some-metadata-site.io/v1/films\";",
        "undeclared-domain",
    ),
]

# ── Vectors that MUST NOT fire. These are the false positives that would make
#    the guard untrustworthy, and an ignored guard is a dead guard. ────────────

SHOULD_PASS: list[tuple[str, str]] = [
    ("allowlisted: Internet Archive", "let base = \"https://archive.org/advancedsearch.php\";"),
    ("allowlisted subdomain", "url = \"https://ia801234.us.archive.org/item/x\""),
    ("allowlisted: TMDB", "const TMDB: &str = \"https://api.themoviedb.org/3\";"),
    ("allowlisted: AniList", "let gql = \"https://graphql.anilist.co\";"),
    ("infrastructure: crates.io", "See https://crates.io/crates/librqbit for the API."),
    ("infrastructure: docs.rs", "// https://docs.rs/tokio/latest/tokio/"),
    ("infrastructure: MDN", "// https://developer.mozilla.org/en-US/docs/Web/API"),
    ("RFC 2606 example in docs", "Paste a manifest URL: https://example.com/addon/manifest.json"),
    ("reserved .invalid domain alone", "endpoint = \"https://backend.invalid/health\""),
    ("empty default source key", "default_source_url = \"\""),
    ("null default source key", "default_catalogue_url: null,"),
    ("sha256 hex is 64 chars, not 40", "digest = \"" + "a" * 64 + "\""),
    ("short hex is not an infohash", "colour = \"#8C2F39\"  // oxblood"),
    ("release group token is permitted", "Movie.Title.2019.1080p.BluRay.x264-GROUP.mkv"),
    ("redacted site tag is permitted", "[SITE01] Series Title - 07 [1080p][HEVC].mkv"),
    ("prose mentioning torrents", "The torrent engine streams rather than downloads."),
    # The GPL-3.0 text is not ours to edit, and the FSF is legal/development
    # infrastructure rather than a content source - so it belongs in
    # INFRASTRUCTURE_DOMAINS, not in the allowlist (ADR-0010 scopes the
    # allowlist to content and metadata sources only).
    ("LICENSE header", "Copyright (C) 2007 Free Software Foundation, Inc. <https://fsf.org/>"),
    ("Apache-2.0 licence body", "    http://www.apache.org/licenses/LICENSE-2.0"),
    ("TOML table header", "[profile.dev]"),
    ("nested TOML table header", "[tool.poetry.group.dev.dependencies]"),
    # Lockfiles are committed (R8) and are full of registry and funding URLs.
    ("npm lockfile resolved URL", '"resolved": "https://registry.npmjs.org/react/-/react-19.2.8.tgz"'),
    ("funding URL in package metadata", '"url": "https://opencollective.com/eslint"'),
    ("the app's own bundle identifier", '"identifier": "dev.sinephile.app",'),
]
