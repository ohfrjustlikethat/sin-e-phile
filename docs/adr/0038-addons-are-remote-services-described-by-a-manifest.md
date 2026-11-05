# 0038 — An addon is a remote service described by a manifest

- **Status:** Accepted 2026-11-05 by the author ("do whatever you think is good", on a plan that named this as decision 1 with this as the default)
- **Date:** 2026-11-05
- **Phase:** 6
- **Builds on:** [ADR-0006](0006-ships-empty-source-posture.md) (ships empty, declarative manifests)
- **Specification:** [`docs/specs/addon-protocol.md`](../specs/addon-protocol.md)

## Context

ADR-0006 and SPEC.md §2.1 require "a declarative addon manifest format (TOML/JSON, no
executable code) so a source can be described as data rather than shipped as an
implementation". That rules out plugins that run code. It does not say what a
declarative description *describes*. Two readings were on the table:

- **(a)** The manifest describes a **service someone else runs** that speaks this
  protocol: two JSON endpoints, *find* and *resolve*. The app sends a query and parses
  an answer. (Stremio's model.)
- **(b)** The manifest describes **how to read a website**: URL templates with `{title}`
  placeholders and rules for pulling results out of pages.

## Decision

**(a).** An addon is a manifest — name, version, what it finds, how its bytes travel,
and the addresses of its two endpoints — plus the service at those addresses.

The manifest format is closed: unknown fields are refused, so there is nowhere to put a
template, a selector or a script. The service's answers are parsed as hostile — size
capped, every string bounded, display text free of bidirectional tricks, a bad
candidate dropped alone, a stream that names a local file refused — and every request
goes through a transport that does not follow redirects.

## Why not (b)

(b) is a scraper written as data. It would make the app the thing that fetches and
interprets a site's pages, which is exactly what §2.1 forbids shipping, arriving as
configuration instead of code — the back door ADR-0029 names for qBittorrent's search
plugins. It would also be much harder to bound: a template language grows conditionals,
pagination and escaping until it is a programming language, and then it is not
declarative any more.

## Consequences

- Writing an addon needs a server. That is the honest cost, and the one the author
  accepted when choosing (a). `tools/protocol/example_addon.py` is a complete one in a
  single stdlib file, to make the cost small.
- The app's whole exposure to an addon is parsing two kinds of JSON answer, which is a
  bounded problem with tests (`crates/source-protocol/tests/hostile_manifests.rs`,
  `src/wire.rs`, `crates/sources/tests/addons.rs`).
- An addon learns what the user searches for. The confirm screen says so in words,
  naming the host ("Your searches are sent to …") before anything is stored.
- Addons share the registry with built-ins under a namespaced id (`addon:<id>`), so one
  cannot install itself as `internet-archive` and take over its settings.
