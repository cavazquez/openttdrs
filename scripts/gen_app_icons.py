#!/usr/bin/env python3
"""Genera tamaños hicolor desde static/app/openttdrs-icon.png."""
from __future__ import annotations

from pathlib import Path

try:
    from PIL import Image
except ImportError:
    raise SystemExit("Instala Pillow: pip install Pillow") from None

ROOT = Path(__file__).resolve().parents[1]
SRC = ROOT / "static/app/openttdrs-icon.png"
OUT = ROOT / "static/app/icons"

SIZES = (16, 32, 48, 64, 128, 256)


def main() -> int:
    if not SRC.is_file():
        print(f"Falta {SRC}", flush=True)
        return 1
    OUT.mkdir(parents=True, exist_ok=True)
    img = Image.open(SRC).convert("RGBA")
    for size in SIZES:
        path = OUT / f"{size}x{size}.png"
        img.resize((size, size), Image.Resampling.LANCZOS).save(path)
        print(f"Escrito {path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
