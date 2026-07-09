#!/usr/bin/env python3
"""Extrae catenaria Action5 (tipo 05) de ogfxe_extra a tiles/.

OpenGFX guarda wires/postes en Action5 del GRF *extra*, no en ogfx1_base
(IDs 1039–1074 del base son vía/depósito/plataforma). Este script:

  - rail_{1039..1062}.png          wires WSO 0..23
  - rail_catenary_entrance_{0..3}.png  WSO_ENTRANCE 24..27
  - rail_pylon_{0..7}.png          PSO 28..35

Busca NFO/sheets en:
  assets/opengfx/opengfx-*/sprites/ogfxe_extra*
  assets/opengfx/.signal-src-8bpp/sprites/ogfxe_extra*
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
TILES = ROOT / "assets" / "opengfx" / "tiles"

A5_PAT = re.compile(r"\*\s*5\s+05\s+05\s+FF\s+30")
SPRITE_PAT = re.compile(
    r"^\s*(\d+)\s+(\S+)\s+(?:8bpp|32bpp)\s+"
    r"(\d+)\s+(\d+)\s+(\d+)\s+(\d+)\s+(-?\d+)\s+(-?\d+)"
)


def find_extra_nfo() -> Path | None:
    base = ROOT / "assets" / "opengfx"
    candidates = [
        *sorted(base.glob("opengfx-*/sprites/ogfxe_extra.nfo")),
        base / ".signal-src-8bpp" / "sprites" / "ogfxe_extra.nfo",
        base
        / ".signal-src-8bpp"
        / "extract"
        / "opengfx-8.0"
        / "sprites"
        / "ogfxe_extra.nfo",
    ]
    for p in candidates:
        if p.is_file():
            return p
    return None


def dematte_palette0(img: Image.Image) -> Image.Image:
    if img.mode == "P":
        pal = img.getpalette()
        key = tuple(pal[0:3]) if pal else None
        rgba = img.convert("RGBA")
        if key is None:
            return rgba
        pix = rgba.load()
        w, h = rgba.size
        for y in range(h):
            for x in range(w):
                r, g, b, a = pix[x, y]
                if (r, g, b) == key or (r, g, b) == (0, 0, 255):
                    pix[x, y] = (0, 0, 0, 0)
        return rgba
    rgba = img.convert("RGBA")
    pix = rgba.load()
    w, h = rgba.size
    for y in range(h):
        for x in range(w):
            r, g, b, a = pix[x, y]
            if (r, g, b) == (0, 0, 255):
                pix[x, y] = (0, 0, 0, 0)
    return rgba


def load_sheet(sheet_dir: Path, name: str) -> Image.Image | None:
    for cand in (
        name,
        name.replace(".png", ".32.png"),
        Path(name).with_suffix(".pcx").name,
    ):
        p = sheet_dir / cand
        if p.is_file() and p.stat().st_size > 0:
            return dematte_palette0(Image.open(p))
    return None


def main() -> int:
    nfo = find_extra_nfo()
    if nfo is None:
        print("  (omitido elrail Action5: no hay ogfxe_extra.nfo)")
        return 0
    sheet_dir = nfo.parent
    lines = nfo.read_text(errors="replace").splitlines()
    start = next((i + 1 for i, l in enumerate(lines) if A5_PAT.search(l)), None)
    if start is None:
        print(f"  (omitido elrail Action5: sin bloque 05 05 en {nfo})")
        return 0
    sprites: list[tuple] = []
    for line in lines[start : start + 80]:
        m = SPRITE_PAT.match(line)
        if not m:
            if len(sprites) >= 48:
                break
            continue
        sprites.append(m.groups())
        if len(sprites) >= 48:
            break
    if len(sprites) < 36:
        print(f"  (omitido elrail Action5: solo {len(sprites)} sprites en {nfo})")
        return 0

    TILES.mkdir(parents=True, exist_ok=True)
    sheets: dict[str, Image.Image] = {}
    written = 0
    for i, g in enumerate(sprites[:48]):
        sheet_name = Path(g[1]).name
        if sheet_name not in sheets:
            img = load_sheet(sheet_dir, sheet_name)
            if img is None:
                print(f"  (omitido elrail_{i:02d}: sheet {sheet_name})")
                continue
            sheets[sheet_name] = img
        x, y, w, h = map(int, g[2:6])
        crop = sheets[sheet_name].crop((x, y, x + w, y + h))
        if i < 24:
            out = TILES / f"rail_{1039 + i}.png"
        elif i < 28:
            out = TILES / f"rail_catenary_entrance_{i - 24}.png"
        elif i < 36:
            out = TILES / f"rail_pylon_{i - 28}.png"
        else:
            # GUI / extras Action5 — no usados por el cliente aún.
            continue
        crop.save(out)
        written += 1
    print(f"  elrail Action5: {written} tiles desde {nfo.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
