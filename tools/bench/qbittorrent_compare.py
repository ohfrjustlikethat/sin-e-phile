"""Phase 7 E6: measure qBittorrent on the same torrent, the same way, for comparison.

SPEC.md Phase 7: "Measured against qBittorrent on both axes and both numbers
published: time-to-first-frame on a healthy swarm, and sustained bulk throughput on the
same torrent." The engine's half is `cargo run --release -p sinephile-torrent --example
bench`; this is qBittorrent's.

It runs a SEPARATE qBittorrent instance with a throwaway profile (`--profile`), so a copy
the user already runs — and its torrents — are never touched. That instance's Web API
listens on loopback only, with localhost authentication off in the throwaway profile
alone. The torrent is a file the person running this supplies; nothing about it is
recorded here (SPEC.md §2.1).

"First bytes" for qBittorrent is the moment the pieces covering the first MiB are all
downloaded, with sequential download and first/last-piece priority on — the closest it
offers to streaming. Bulk is bytes downloaded over the same window as the engine's run.

Usage:
    python tools/bench/qbittorrent_compare.py <path-to-.torrent> <scratch-dir> [bulk-seconds]

Stdlib only (ADR-0012).
"""

from __future__ import annotations

import json
import subprocess
import sys
import time
import urllib.request
import uuid
from pathlib import Path

QBT = Path(r"C:\Program Files\qBittorrent\qbittorrent.exe")
PORT = 18081
API = f"http://127.0.0.1:{PORT}/api/v2"


def call(path: str, data: bytes | None = None, headers: dict | None = None) -> bytes:
    req = urllib.request.Request(API + path, data=data, headers=headers or {})
    with urllib.request.urlopen(req, timeout=10) as r:
        return r.read()


def write_profile(profile: Path, downloads: Path) -> None:
    config = profile / "qBittorrent" / "config"
    config.mkdir(parents=True, exist_ok=True)
    (config / "qBittorrent.ini").write_text(
        "[LegalNotice]\nAccepted=true\n\n"
        "[BitTorrent]\n"
        f"Session\\DefaultSavePath={downloads.as_posix()}\n"
        "Session\\Port=0\n\n"
        "[Preferences]\n"
        "WebUI\\Enabled=true\n"
        f"WebUI\\Port={PORT}\n"
        "WebUI\\Address=127.0.0.1\n"
        "WebUI\\LocalHostAuth=false\n"
        "General\\ExitConfirm=false\n",
        encoding="utf-8",
    )


def add(torrent: bytes) -> None:
    boundary = uuid.uuid4().hex
    parts = []
    for name, value in (("sequentialDownload", "true"), ("firstLastPiecePrio", "true")):
        parts.append(f'--{boundary}\r\nContent-Disposition: form-data; name="{name}"\r\n\r\n{value}\r\n'.encode())
    parts.append(
        f'--{boundary}\r\nContent-Disposition: form-data; name="torrents"; filename="t.torrent"\r\n'
        "Content-Type: application/x-bittorrent\r\n\r\n".encode() + torrent + b"\r\n"
    )
    parts.append(f"--{boundary}--\r\n".encode())
    call("/torrents/add", b"".join(parts), {"Content-Type": f"multipart/form-data; boundary={boundary}"})


def main() -> int:
    torrent = Path(sys.argv[1]).read_bytes()
    scratch = Path(sys.argv[2])
    bulk = int(sys.argv[3]) if len(sys.argv) > 3 else 60
    profile, downloads = scratch / "qbt-profile", scratch / "qbt-downloads"
    downloads.mkdir(parents=True, exist_ok=True)
    write_profile(profile, downloads)

    proc = subprocess.Popen([str(QBT), f"--profile={profile}", "--no-splash"])
    try:
        for _ in range(60):
            try:
                call("/app/version")
                break
            except OSError:
                time.sleep(0.5)
        else:
            print("qBittorrent's Web API did not come up")
            return 1

        t0 = time.monotonic()
        add(torrent)
        info = []
        while not info:
            info = json.loads(call("/torrents/info"))
            time.sleep(0.1)
        h = info[0]["hash"]
        props = json.loads(call(f"/torrents/properties?hash={h}"))
        piece = props["piece_size"]
        need = max(1, (1 << 20) // piece + (1 if (1 << 20) % piece else 0))
        while True:
            states = json.loads(call(f"/torrents/pieceStates?hash={h}"))
            if states and all(s == 2 for s in states[:need]):
                break
            if time.monotonic() - t0 > 120:
                print("first MiB not complete in 120 s")
                return 1
            time.sleep(0.05)
        print(f"qBittorrent: add -> first 1 MiB  {(time.monotonic() - t0) * 1e3:>8.0f} ms   (sequential + first/last priority)")

        before = json.loads(call(f"/torrents/info?hashes={h}"))[0]["completed"]
        tb = time.monotonic()
        time.sleep(bulk)
        now = json.loads(call(f"/torrents/info?hashes={h}"))[0]
        rate = (now["completed"] - before) / (time.monotonic() - tb)
        print(f"qBittorrent: bulk over {bulk} s      {rate / (1 << 20):>8.2f} MiB/s   ({now['num_seeds']} seeds connected)")
        call(f"/torrents/delete", f"hashes={h}&deleteFiles=true".encode(),
             {"Content-Type": "application/x-www-form-urlencoded"})
    finally:
        try:
            call("/app/shutdown", b"")
        except OSError:
            pass
        try:
            proc.wait(timeout=20)
        except subprocess.TimeoutExpired:
            proc.kill()
    return 0


if __name__ == "__main__":
    sys.exit(main())
