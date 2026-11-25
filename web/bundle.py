#!/usr/bin/env python3
"""Packs the prepared resources for the browser build into web/dist/fs.

    python3 web/bundle.py [--resources .local/horizon-client/26.3] [--out web/dist/fs] [--index-only]

- `startup.bundle`: the files the game reads before the title screen and its first world. The page
  streams it into the game's file system before the game starts.
- `later.bundle`: the structure templates (already compressed, and read only when a structure
  generates). The page loads it once the game runs.
- `index.txt`: every file (`size<TAB>/res/path` per line). A file in no bundle (sounds, the other
  languages, the optional resource packs) is fetched from `res/` when the game first reads it.

A bundle is `HZB1`, the index's length (u32, little-endian), the index (`size<TAB>/res/path` per
line) and the files' bytes in its order. Each is also written brotli-compressed (`.br`), which a
server sends with `Content-Encoding: br`.
"""

import argparse
import os
import struct
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

LAZY = (
    "assets/minecraft/sounds/",
    "assets/minecraft/resourcepacks/",
    "classes/",
    "client-classes/",
)
LANGUAGES = {"assets/minecraft/lang/en_us.json", "assets/minecraft/lang/deprecated.json"}
LATER = ("data/minecraft/structure/", "worldgen/structure/")


def kind(path: str) -> str:
    if path.endswith(".log") or path.startswith(LAZY):
        return "lazy"
    if path.startswith("assets/minecraft/lang/") and path not in LANGUAGES:
        return "lazy"
    if path.startswith(LATER):
        return "later"
    return "startup"


def write_bundle(resources: Path, files: list[str], out: Path) -> int:
    index = "".join(f"{(resources / f).stat().st_size}\t/res/{f}\n" for f in files).encode()
    total = 0
    with out.open("wb") as bundle:
        bundle.write(b"HZB1" + struct.pack("<I", len(index)) + index)
        for f in files:
            data = (resources / f).read_bytes()
            bundle.write(data)
            total += len(data)
    subprocess.run(["brotli", "-q", "9", "-f", "-o", f"{out}.br", str(out)], check=True)
    return total


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--resources", type=Path, default=ROOT / ".local/horizon-client/26.3")
    parser.add_argument("--out", type=Path, default=ROOT / "web/dist/fs")
    parser.add_argument("--index-only", action="store_true", help="only index.txt (web/www/probe.html)")
    args = parser.parse_args()
    resources = args.resources.resolve()
    args.out.mkdir(parents=True, exist_ok=True)
    every = sorted(
        Path(directory, name).relative_to(resources).as_posix()
        for directory, _, names in os.walk(resources)
        for name in names
    )
    (args.out / "index.txt").write_text(
        "".join(f"{(resources / f).stat().st_size}\t/res/{f}\n" for f in every)
    )
    if args.index_only:
        return
    for name in ("startup", "later"):
        files = [f for f in every if kind(f) == name]
        size = write_bundle(resources, files, args.out / f"{name}.bundle")
        packed = (args.out / f"{name}.bundle.br").stat().st_size
        print(f"{name}.bundle: {len(files)} files, {size / 1e6:.1f} MB, {packed / 1e6:.1f} MB brotli")


if __name__ == "__main__":
    main()
