#!/usr/bin/env python3
"""Programmsymbole aus assets/ct-editor.svg erzeugen / build the program icons.

- icons/ct-editor-<größe>.png  (16–256, einkompiliert und für Linux)
- ct-editor.ico                (Windows, in die .exe eingebettet: build.rs)
- ct-editor.icns               (macOS, im .app)

Braucht / needs: rsvg-convert (librsvg), magick (ImageMagick).
Aufruf / usage:  python3 assets/make-icons.py
"""

import struct
import subprocess
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
SVG = HERE / "ct-editor.svg"
PNG_SIZES = [16, 24, 32, 48, 64, 128, 256]
# ICNS-Typen mit PNG-Inhalt / ICNS types holding PNG data: (Typ, Kantenlänge)
ICNS_TYPES = [
    (b"icp4", 16), (b"icp5", 32), (b"icp6", 64), (b"ic07", 128), (b"ic08", 256),
    (b"ic09", 512), (b"ic10", 1024), (b"ic11", 32), (b"ic12", 64), (b"ic13", 256),
    (b"ic14", 512),
]


def render(size: int, out: Path) -> None:
    subprocess.run(["rsvg-convert", "-w", str(size), "-h", str(size), str(SVG), "-o", str(out)],
                   check=True)


def main() -> None:
    icons = HERE / "icons"
    icons.mkdir(exist_ok=True)
    for size in PNG_SIZES:
        render(size, icons / f"ct-editor-{size}.png")
    subprocess.run(["magick", *[str(icons / f"ct-editor-{s}.png") for s in PNG_SIZES],
                    str(HERE / "ct-editor.ico")], check=True)
    with tempfile.TemporaryDirectory() as tmp:
        blocks = b""
        for kind, size in ICNS_TYPES:
            png = Path(tmp) / f"{size}.png"
            if not png.exists():
                render(size, png)
            data = png.read_bytes()
            blocks += kind + struct.pack(">I", len(data) + 8) + data
        (HERE / "ct-editor.icns").write_bytes(b"icns" + struct.pack(">I", len(blocks) + 8) + blocks)
    print("icons, ct-editor.ico, ct-editor.icns erzeugt / written")


if __name__ == "__main__":
    main()
