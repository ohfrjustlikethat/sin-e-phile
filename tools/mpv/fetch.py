"""Fetch libmpv, pinned and checksum-verified (SPEC.md Phase 8, ADR-0004).

libmpv is the player (ADR-0004: libmpv over libVLC). It is loaded dynamically at run
time, as Spike A proved works with no import library (docs/RISKS.md R1), so the only
thing the build needs is the DLL beside the executable.

The build is shinchiro's mpv-dev for baseline x86-64 — not the "v3" variant, which needs
AVX2 that a Tier 0 machine may not have (SPEC.md §8). It is pinned to one release, at
least two weeks old when chosen, and its SHA-256 is checked before anything is used: a
121 MB native library is exactly what must not be swapped silently.

    python tools/mpv/fetch.py            # -> lib/libmpv-2.dll (git-ignored)
    python tools/mpv/fetch.py --check    # exit 1 if it is missing or wrong

Stdlib only (ADR-0012). The archive is 7z; Windows' own tar.exe (bsdtar) reads it.
"""

from __future__ import annotations

import hashlib
import shutil
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
DEST = ROOT / "lib" / "libmpv-2.dll"

RELEASE = "20261021"
ARCHIVE = "mpv-dev-x86_64-20261021-git-e471496c56.7z"
URL = f"https://github.com/shinchiro/mpv-winbuild-cmake/releases/download/{RELEASE}/{ARCHIVE}"
ARCHIVE_SHA256 = "efc719eff3dc5954b4a5efc800632808bcec01cc9034940dd14e5eb7fe9ea633"
DLL_SHA256 = "04e4d27dc385dc18ddd93fbe69c75e635b864f3c6ec53a9122df54c2f7842f4a"


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def check() -> bool:
    return DEST.is_file() and sha256(DEST) == DLL_SHA256


# The release host has answered 504 twice running (CI, 2026-11-08): a gateway timeout is
# the host's, not the file's, so it is tried again after a pause before giving up.
ATTEMPTS = 4


def download(url: str, to: Path) -> None:
    for attempt in range(1, ATTEMPTS + 1):
        try:
            with urllib.request.urlopen(url, timeout=120) as r, to.open("wb") as out:
                shutil.copyfileobj(r, out)
            return
        except (urllib.error.HTTPError, urllib.error.URLError, TimeoutError) as e:
            code = getattr(e, "code", None)
            # A 4xx other than 429 will not change by waiting.
            if code is not None and 400 <= code < 500 and code != 429:
                raise
            if attempt == ATTEMPTS:
                raise
            wait = 15 * attempt
            print(f"  {e}; trying again in {wait} s ({attempt} of {ATTEMPTS})")
            time.sleep(wait)


def fetch() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        archive = Path(tmp) / ARCHIVE
        print(f"fetching {ARCHIVE} ({RELEASE}) ...")
        download(URL, archive)
        got = sha256(archive)
        if got != ARCHIVE_SHA256:
            sys.exit(f"checksum mismatch for {ARCHIVE}: expected {ARCHIVE_SHA256}, got {got}")
        extracted = Path(tmp) / "x"
        extracted.mkdir()
        tar = Path(r"C:\Windows\System32\tar.exe")
        subprocess.run([str(tar if tar.exists() else "tar"), "-xf", str(archive), "-C", str(extracted)], check=True)
        dll = extracted / "libmpv-2.dll"
        if sha256(dll) != DLL_SHA256:
            sys.exit("the extracted libmpv-2.dll does not match its pinned checksum")
        DEST.parent.mkdir(exist_ok=True)
        shutil.copyfile(dll, DEST)
    print(f"libmpv: {DEST.relative_to(ROOT)} ({DEST.stat().st_size // (1 << 20)} MiB), verified")


def main() -> int:
    if "--check" in sys.argv:
        ok = check()
        print("libmpv:", "present and verified" if ok else f"missing or wrong — run python tools/mpv/fetch.py")
        return 0 if ok else 1
    if check():
        print("libmpv: already present and verified")
        return 0
    fetch()
    return 0 if check() else 1


if __name__ == "__main__":
    sys.exit(main())
