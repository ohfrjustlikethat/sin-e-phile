# The sin-e-phile addon protocol, version 1

**Status:** implemented (Phase 6) · **Decides:** [ADR-0006](../adr/0006-ships-empty-source-posture.md) (declarative, ships empty), [ADR-0017](../adr/0017-dual-license-extracted-crates.md) (permissive licence)
**Code:** `crates/source-protocol/` (types, manifest and wire validators, MIT OR Apache-2.0) · `crates/sources/` (the app's resolver and addon client) · [`docs/schemas/addon-manifest.schema.json`](../schemas/addon-manifest.schema.json)
**Example:** [`tools/protocol/example_addon.py`](../../tools/protocol/example_addon.py), a complete addon in one stdlib Python file

This document is written for someone who has never seen this repository and wants
sin-e-phile to find films through a service they run. Everything an addon must do is
here. Where this document and the reference validator disagree, that is a bug in one of
them; report it.

---

## 1. What an addon is

An addon is **a manifest plus a web service you run**. The manifest is a small JSON
document that says what the addon is called, what it finds, and where its two
endpoints are. The service answers two kinds of request: *find* ("what do you have for
this film?") and *resolve* ("how do I play this one?").

sin-e-phile never runs addon code. It sends your service a query, reads your JSON
answer, checks it, and shows it next to every other source's answers. That is the whole
relationship, and it is deliberately that small: **parsing untrusted data is a bounded
problem, and executing untrusted code is not.**

The app ships with **no** addons and no list of addons. A user adds yours by pasting
your manifest's address, reading the plain-language summary the app makes of it, and
confirming.

```
user pastes   https://example.com/addon/manifest.json
app GETs      the manifest ── checks it ── shows the user what it claims ── user confirms
on search     POST https://example.com/addon/find      { query }  →  { candidates }
on play       POST https://example.com/addon/resolve   { key }    →  { stream }
```

All examples in this document use `example.com` and other RFC 2606 reserved names. Use
your own host.

## 2. The manifest

Served at any address you like, by `GET`, as `application/json`, UTF-8.

```json
{
  "protocol": 1,
  "id": "com.example.films",
  "name": "Example Films",
  "version": "1.0.0",
  "description": "Public-domain features, served over HTTPS.",
  "media": ["film"],
  "transports": ["http"],
  "endpoints": {
    "find": "https://example.com/addon/find",
    "resolve": "https://example.com/addon/resolve"
  },
  "homepage": "https://example.com/about"
}
```

| Field | Required | Rule |
|---|---|---|
| `protocol` | yes | The integer `1`. Any other value is refused with a message naming the version. |
| `id` | yes | 3–64 characters: lowercase ASCII letters, digits, `.`, `-`, `_`, starting with a letter. Reverse-DNS is the convention. **Never change it**: the user's settings and your addon's health record are stored under it. |
| `name` | yes | 1–64 characters, not only whitespace. Shown to the user. |
| `version` | yes | 1–32 characters, free-form; semver suggested. Shown to the user. |
| `description` | no | Up to 500 characters; may contain newlines. Shown before the user confirms. |
| `media` | yes | 1–8 of `"film"`, `"episode"`. At least one must be a value this version knows; unknown values are ignored. |
| `transports` | yes | 1–8 of `"http"`, `"p2p"`, `"local"` — how your candidates' bytes travel. Shown to the user as sentences. |
| `endpoints.find`, `endpoints.resolve` | yes | See §2.1. |
| `homepage` | no | An address shown as a link. Same address rules as endpoints, except it may be on any host. |

**No other field is allowed**, at the top level or in `endpoints`. A manifest with an
unknown field is refused. This is what keeps a manifest declarative: there is nowhere
to put a script, a template, a URL pattern or a selector, so an addon cannot turn the
app into something that fetches and interprets a website.

**Display text** — `name`, `version`, `description` — may not contain control characters
(other than a newline in `description`), zero-width characters, or bidirectional
embeddings, overrides and isolates (U+200B–U+200F, U+202A–U+202E, U+2066–U+2069,
U+FEFF). Those are how one name is made to render as another.

### 2.1 Addresses

- `https://` always. Plain `http://` only when the host is this computer
  (`localhost`, `127.0.0.0/8`, `[::1]`) — which is what makes local development possible.
- No user name or password in the address.
- **Both endpoints must be on the same host and port as the manifest itself.** A
  manifest on one site may not send the user's queries to another.
- At most 2,048 characters.

### 2.2 Fetching

The app fetches a manifest with `GET`, **does not follow redirects**, and refuses any
status but `200`. A redirect would be a way round §2.1's same-host rule, so a `3xx` is
reported to the user with the advice to paste the final address instead.

The whole document may be at most **65,536 bytes** and may nest at most **8 levels**
deep. Both are checked before the JSON is parsed. A real manifest is a few hundred bytes.

The app stores the manifest the user confirmed and re-checks it every time it loads.
To update your addon, change the manifest at the same address; the user updates by
pasting it again, and keeps their position and on/off setting.

## 3. Find

```
POST <endpoints.find>
Content-Type: application/json
Accept: application/json
```

The body is a **query**:

```json
{
  "kind": "film",
  "title": "Nosferatu",
  "year": 1922,
  "ids": { "imdb": "tt0013442" },
  "season": null,
  "episode": null
}
```

| Field | Type | Meaning |
|---|---|---|
| `kind` | `"film"` or `"episode"` | You are only sent kinds your manifest lists. |
| `title` | string | The catalogue's primary title, or what the user typed. |
| `year` | integer or `null` | Release year, when known. |
| `ids` | object | Any of `imdb` (string, `tt` + digits), `tmdb` (integer), `anilist` (integer). Absent keys are unknown, not empty. |
| `season`, `episode` | integer or `null` | Set for an episode. |

Answer `200` with:

```json
{
  "candidates": [
    {
      "key": "nosferatu-1922-sd",
      "title": "Nosferatu",
      "year": 1922,
      "transport": "http",
      "quality": { "resolution": "sd", "hdr": false },
      "languages": ["de"],
      "subtitles": ["en"],
      "codec": "h264",
      "size_bytes": 401195646,
      "health": { "kind": "direct" },
      "locator": "https://cdn.example/nosferatu.mp4",
      "provenance": { "page": "https://example.com/films/nosferatu", "licence": "public domain" }
    }
  ]
}
```

An empty list is a normal answer. Every field of a candidate:

| Field | Required | Rule |
|---|---|---|
| `key` | yes | 1–2,048 characters, opaque to the app. It is what you will be sent back in *resolve*. |
| `title` | yes | 1–300 characters, display-text rules as §2. What *you* call it. |
| `transport` | yes | `"http"`, `"p2p"` or `"local"`. |
| `year` | no | Integer. |
| `quality.resolution` | no | `"sd"` (≤ 576 lines), `"hd"` (720), `"full_hd"` (1080), `"uhd"` (2160). |
| `quality.hdr` | no | Boolean. |
| `languages`, `subtitles` | no | Up to 16 each, BCP 47 in shape (`en`, `pt-BR`): 2–35 letters, digits and hyphens, starting with a letter. |
| `codec` | no | `"h264"`, `"h265"`, `"av1"`, `"vp9"`, `"mpeg4"`. |
| `size_bytes` | no | Integer. |
| `health` | no | `{"kind":"direct"}` for a file a host serves, or `{"kind":"swarm","seeders":n,"peers":n}`. |
| `locator` | no | Up to 2,048 characters naming *what the bytes are* — a URL, an info hash. Two candidates with the same locator, from you or from another source, are shown as one. Omit it if you cannot say. |
| `provenance.page` | no | A page about the item. Address rules as §2.1 (any host). An unsafe one is dropped. |
| `provenance.licence` | no | Up to 100 characters, e.g. `public domain`. |

What the app does with your answer:

- The response may be at most **1 MiB** and nest at most 8 levels. Over that, the whole
  answer is refused.
- At most **200** candidates are read; the rest are ignored.
- **A candidate that breaks a rule is dropped on its own**; the others are kept.
- Unknown fields in a candidate are ignored, and an unknown value of an enumerated field
  (`transport`, `resolution`, `codec`, `health.kind`) reads as "unknown" rather than
  failing — so a newer addon still works with an older app.
- `provenance.backend` and `provenance.also_from` are **overwritten**. The app knows
  which source answered; a candidate cannot claim to come from another.

## 4. Resolve

When the user chooses one of your candidates:

```
POST <endpoints.resolve>
Content-Type: application/json

{ "key": "nosferatu-1922-sd" }
```

Answer `200` with one stream, tagged by `kind`:

```json
{ "stream": { "kind": "http", "url": "https://cdn.example/nosferatu.mp4",
              "content_type": "video/mp4", "size_bytes": 401195646 } }
```

```json
{ "stream": { "kind": "torrent", "info_hash": "<40 hexadecimal characters>",
              "file_index": 0 } }
```

- `http`: an `http://` or `https://` address with a host and no credentials. It may be on
  any host — a CDN is normal. `content_type` and `size_bytes` are optional.
- `torrent`: a 40-hex-character BitTorrent v1 info hash; `file_index` is optional, and so
  is `trackers` — up to 32 `http`, `https` or `udp` tracker addresses that know the swarm.
  Without them the app finds peers through DHT from the hash alone, which is slower.
- **An addon may not return a `file` stream.** A remote service has no business naming a
  path on the user's computer; such an answer is refused.
- At most 16 KiB.

A key you no longer recognise should be answered with a `404`; the user sees that the
source no longer has it.

## 5. Time, failure and health

- **Each search gives every source its own budget of 8 seconds.** Answer faster and your
  candidates are shown at once; other sources are never held up by you, and you are never
  held up by them. Past the budget your request is abandoned and the search goes on
  without you.
- Any status but `200` is a failure. A `3xx` is reported as a redirect, which is not
  allowed. Every request is a `POST` with a JSON body; the app sends no cookies and no
  credentials.
- The app keeps a health record per source — answers, failures, time-outs, a weighted
  typical latency, and the last error — and shows it to the user in words ("Answered 12
  of 14 searches, usually in 340 ms"). The user can switch you off or move you down the
  order.

## 6. Catalogues

A catalogue is a list of addons someone recommends. The app opens one only from an
address the user types; none is built in. It is listing, not installing: each entry's
manifest goes through §2 exactly as if pasted by hand.

```json
{
  "protocol": 1,
  "name": "Example picks",
  "addons": [
    { "name": "Example Films", "description": "Public-domain features.",
      "manifest": "https://example.com/addon/manifest.json" }
  ]
}
```

`name` and each entry's `name` follow §2's `name` rule, `description` §2's description
rule; each `manifest` follows §2.1 and may be on any host. At most 500 entries and
256 KiB. Unknown fields are refused.

## 7. Versioning

This is protocol `1`. A change that an existing addon could not ignore — a new required
field, a changed meaning — will be protocol `2`, and the app will say so to the user
rather than half-understand a manifest. Additions an existing addon *can* ignore (a new
optional candidate field, a new enum value) stay in version 1, which is why §3 tells the
app to tolerate unknown values in answers. Manifests, which the user reads and confirms,
do not get that tolerance: an unknown manifest field is refused.

## 8. Checking your addon

Point the app's conformance test at your manifest. It fetches it exactly as the app does,
runs a find, and resolves the first candidate:

```bash
SINEPHILE_ADDON=https://example.com/addon/manifest.json \
SINEPHILE_ADDON_QUERY="Nosferatu" \
  cargo test -p sinephile-sources --test conformance -- --ignored --nocapture
```

For local development, serve on `http://127.0.0.1:<port>`; plain HTTP is allowed there
and only there. `tools/protocol/example_addon.py` is a working addon in one file to start
from:

```bash
python tools/protocol/example_addon.py --port 8765
SINEPHILE_ADDON=http://127.0.0.1:8765/manifest.json \
  cargo test -p sinephile-sources --test conformance -- --ignored --nocapture
```

The manifest's JSON Schema, for your own validation, is
[`docs/schemas/addon-manifest.schema.json`](../schemas/addon-manifest.schema.json). Three
rules cannot be written in JSON Schema and are enforced only by the app: the same-host
rule, plain HTTP only on loopback, and the size and depth limits.

## 9. Writing a backend inside the app instead

An addon is the way to add a source without touching sin-e-phile. A source built *into*
the app — the Internet Archive, the user's own links — implements the Rust trait the
protocol is the wire form of (`crates/source-protocol/src/backend.rs`):

```rust
#[async_trait]
pub trait SourceBackend: Send + Sync {
    fn id(&self) -> &str;
    fn capabilities(&self) -> BackendCapabilities;
    async fn find(&self, query: &MediaQuery) -> Result<Vec<SourceCandidate>, SourceError>;
    async fn resolve(&self, candidate: &SourceCandidate) -> Result<PlayableStream, SourceError>;
}
```

One file under `crates/sources/src/backends/`, one line in `backends::all()`, and
nothing else: the registry, the ordering, the health record, the settings screen and the
resolver all find it from that list (Phase 6 E1, commit `bd80a1f`). **No built-in
backend may name or reach an unlicensed content source** — `SPEC.md` §2.1, enforced by
`tools/guard` over the whole history.
