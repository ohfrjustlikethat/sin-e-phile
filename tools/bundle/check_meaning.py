"""Refuse to build an installer that carries a bad copy of search by meaning (ADR-0053).

The installer carries the language model, its tokenizer and the published meaning file in
`meaning/` beside the app, so an installed copy searches by meaning with nothing to
download. They are copied from the data folder of the machine that builds the release, and
the author's own download of the meaning file was corrupt in its middle with a sound header
(D90): so each file is hashed in full here, against the published file's sha256, before it
can be packed.

Usage: python tools/bundle/check_meaning.py [data folder, default ./data]
Stdlib only (ADR-0012).
"""

import hashlib
import os
import sys

# Whole-file sha256 of each file as published: the model and tokenizer at their pinned
# revision, and the embeddings-v3 release asset (858,170 vectors; the artefact's own
# internal checksum is a different number, see crates/catalogue/src/assets.rs).
FILES = {
    "embeddings-bge-small-en-v1.5-int8.bin": "7f11415afcf1603e500aec54ad77a6542c64980597f0221cb193ddc15337fe0e",
    os.path.join("models", "bge-small-en-v1.5-int8.onnx"): "6c9c6101a956d62dfb5e7190c538226c0c5bb9cb27b651234b6df063ee7dbfe4",
    os.path.join("models", "bge-small-en-v1.5-tokenizer.json"): "d241a60d5e8f04cc1b2b3e9ef7a4921b27bf526d9f6050ab90f9267a1f9e5c66",
}


def sha256(path):
    digest = hashlib.sha256()
    with open(path, "rb") as f:
        for block in iter(lambda: f.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def main():
    data = sys.argv[1] if len(sys.argv) > 1 else "data"
    bad = []
    for name, expected in FILES.items():
        path = os.path.join(data, name)
        if not os.path.isfile(path):
            bad.append(f"{path}: missing")
            continue
        got = sha256(path)
        if got != expected:
            bad.append(f"{path}: sha256 {got[:16]}..., the published file is {expected[:16]}...")
    if bad:
        print("check_meaning: the installer would carry a bad copy of search by meaning:")
        for line in bad:
            print("  " + line)
        print("Fetch them again (the app's Search -> Download, into this data folder) and rerun.")
        return 1
    print(f"check_meaning: all {len(FILES)} files match the published ones")
    return 0


if __name__ == "__main__":
    sys.exit(main())
