"""A complete sin-e-phile addon in one file — docs/specs/addon-protocol.md, protocol 1.

Written from the specification document alone, as a third party would: it is the
evidence for Phase 6 E5 ("complete enough that a third party could implement a backend
from it alone"), and a starting point for anyone writing an addon.

It serves a manifest, a find endpoint and a resolve endpoint over plain HTTP on
loopback (allowed there and only there, spec §2.1), from a two-film list whose stream
addresses are on a reserved example domain. It does not stream anything itself.

Usage:
    python tools/protocol/example_addon.py --port 8765
    SINEPHILE_ADDON=http://127.0.0.1:8765/manifest.json \\
        cargo test -p sinephile-sources --test conformance -- --ignored --nocapture

Stdlib only (ADR-0012).
"""

from __future__ import annotations

import argparse
import json
import re
import unicodedata
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

# What this addon "has". A real addon would look these up somewhere it is entitled to.
FILMS = [
    {
        "key": "nosferatu-1922",
        "title": "Nosferatu",
        "year": 1922,
        "url": "https://cdn.example/nosferatu-1922.mp4",
        "height": 480,
    },
    {
        "key": "the-general-1926",
        "title": "The General",
        "year": 1926,
        "url": "https://cdn.example/the-general-1926.mp4",
        "height": 720,
    },
]


def fold(text: str) -> str:
    """Lowercase, accents removed, punctuation to spaces — so "NOSFERATU." matches."""
    text = unicodedata.normalize("NFD", text)
    text = "".join(c for c in text if not unicodedata.combining(c)).lower()
    return " ".join(re.sub(r"[^\w]+", " ", text).split())


def resolution(height: int) -> str:
    # Spec §3: sd ≤ 576 lines, hd 720, full_hd 1080, uhd 2160.
    if height <= 576:
        return "sd"
    if height <= 720:
        return "hd"
    if height <= 1080:
        return "full_hd"
    return "uhd"


def manifest(base: str) -> dict:
    # Spec §2: every required field, nothing else; endpoints on the manifest's own host.
    return {
        "protocol": 1,
        "id": "com.example.reference",
        "name": "Reference Films",
        "version": "1.0.0",
        "description": "The protocol's example addon. Two silent films, served by nobody.",
        "media": ["film"],
        "transports": ["http"],
        "endpoints": {"find": f"{base}/find", "resolve": f"{base}/resolve"},
    }


def find(query: dict) -> dict:
    # Spec §3: kind, title, year; answer {"candidates": [...]}, empty is normal.
    if query.get("kind") != "film":
        return {"candidates": []}
    wanted, year = fold(query.get("title", "")), query.get("year")
    out = []
    for f in FILMS:
        if fold(f["title"]) != wanted or (year is not None and year != f["year"]):
            continue
        out.append({
            "key": f["key"],
            "title": f["title"],
            "year": f["year"],
            "transport": "http",
            "quality": {"resolution": resolution(f["height"]), "hdr": False},
            "codec": "h264",
            "health": {"kind": "direct"},
            "locator": f["url"],
            "provenance": {"licence": "public domain"},
        })
    return {"candidates": out}


def resolve(body: dict) -> tuple[int, dict]:
    # Spec §4: {"key"} -> {"stream": {...}}; an unknown key is a 404.
    for f in FILMS:
        if f["key"] == body.get("key"):
            return 200, {"stream": {"kind": "http", "url": f["url"], "content_type": "video/mp4"}}
    return 404, {"error": "no such key"}


class Handler(BaseHTTPRequestHandler):
    def _send(self, status: int, payload: dict) -> None:
        data = json.dumps(payload).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def _base(self) -> str:
        host, port = self.server.server_address[:2]
        return f"http://{host}:{port}"

    def do_GET(self) -> None:  # noqa: N802 (http.server's naming)
        if self.path == "/manifest.json":
            self._send(200, manifest(self._base()))
        else:
            self._send(404, {"error": "not found"})

    def do_POST(self) -> None:  # noqa: N802
        length = int(self.headers.get("Content-Length") or 0)
        try:
            body = json.loads(self.rfile.read(length) or b"{}")
        except ValueError:
            self._send(400, {"error": "not JSON"})
            return
        if self.path == "/find":
            self._send(200, find(body))
        elif self.path == "/resolve":
            self._send(*resolve(body))
        else:
            self._send(404, {"error": "not found"})

    def log_message(self, fmt: str, *args) -> None:
        print(f"example_addon: {self.command} {self.path} -> {args[1] if len(args) > 1 else ''}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--port", type=int, default=8765)
    args = parser.parse_args()
    server = ThreadingHTTPServer(("127.0.0.1", args.port), Handler)
    print(f"example_addon: http://127.0.0.1:{args.port}/manifest.json")
    server.serve_forever()


if __name__ == "__main__":
    main()
