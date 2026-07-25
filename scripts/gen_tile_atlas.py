#!/usr/bin/env python3
"""Empaqueta assets/opengfx/tiles/*.png en un texture atlas.

Bevy 2D solo agrupa en un draw call sprites consecutivos (en orden Z) que
comparten textura; con ~2300 PNGs sueltos cada tesela corta el batch. Este
script deduplica por contenido (muchos archivos son aliases), empaqueta las
imágenes únicas en páginas de atlas (shelf packing) y genera:

  - assets/opengfx/atlas/tiles_atlas_{p}.png      (páginas, gitignored)
  - crates/openttdrs-client/src/sprites/tile_atlas_generated.rs (committed)

Correr después de scripts/descargar_graficos.sh.

Uso:
  python3 scripts/gen_tile_atlas.py
  python3 scripts/gen_tile_atlas.py --check   # solo compara el .rs (no escribe)
"""

from __future__ import annotations

import argparse
import hashlib
import os
import sys
from pathlib import Path

from PIL import Image

ROOT = Path(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
TILES_DIR = ROOT / "assets" / "opengfx" / "tiles"
ATLAS_DIR = ROOT / "assets" / "opengfx" / "atlas"
OUT_RS = ROOT / "crates" / "openttdrs-client" / "src" / "sprites" / "tile_atlas_generated.rs"

PAGE_W = 2048
PAGE_H = 4096
# 1 px de separación: con sampler nearest y sin mipmaps evita el bleeding.
PAD = 1


def shelf_pack(items):
    """items: [(key, w, h)] -> {key: (page, x, y)}; shelf packing por filas."""
    placed = {}
    page = 0
    x = y = shelf_h = 0
    for key, w, h in items:
        if w + 2 * PAD > PAGE_W or h + 2 * PAD > PAGE_H:
            raise SystemExit(f"sprite {key} ({w}x{h}) no entra en una página")
        if x + w + 2 * PAD > PAGE_W:
            x = 0
            y += shelf_h
            shelf_h = 0
        if y + h + 2 * PAD > PAGE_H:
            page += 1
            x = y = shelf_h = 0
        placed[key] = (page, x + PAD, y + PAD)
        x += w + 2 * PAD
        shelf_h = max(shelf_h, h + 2 * PAD)
    return placed, page + 1


def assets_available() -> bool:
    return TILES_DIR.is_dir() and any(TILES_DIR.glob("*.png"))


def build_atlas():
    """Devuelve (rs_text, pages, used_h, files, unique) sin escribir disco."""
    files = sorted(f for f in os.listdir(TILES_DIR) if f.endswith(".png"))
    if not files:
        raise SystemExit(f"No hay PNGs en {TILES_DIR}; corré descargar_graficos.sh")

    unique: dict[str, Image.Image] = {}
    name_to_hash: dict[str, str] = {}
    for f in files:
        data = (TILES_DIR / f).read_bytes()
        h = hashlib.sha1(data).hexdigest()
        name_to_hash[f] = h
        if h not in unique:
            unique[h] = Image.open(TILES_DIR / f).convert("RGBA")

    items = sorted(
        ((h, im.width, im.height) for h, im in unique.items()),
        key=lambda t: (-t[2], -t[1], t[0]),
    )
    placed, page_count = shelf_pack(items)

    used_h = [0] * page_count
    for h, im in unique.items():
        page, _x, y = placed[h]
        used_h[page] = max(used_h[page], y + im.height + PAD)

    pages = [Image.new("RGBA", (PAGE_W, used_h[p]), (0, 0, 0, 0)) for p in range(page_count)]
    for h, im in unique.items():
        page, x, y = placed[h]
        pages[page].paste(im, (x, y))

    hashes_by_page: list[list[str]] = [[] for _ in range(page_count)]
    for h in unique:
        hashes_by_page[placed[h][0]].append(h)
    for lst in hashes_by_page:
        lst.sort()

    rects = []
    rect_index: dict[str, int] = {}
    ranges = []
    for p, lst in enumerate(hashes_by_page):
        start = len(rects)
        for h in lst:
            im = unique[h]
            _page, x, y = placed[h]
            rect_index[h] = len(rects)
            rects.append((p, x, y, im.width, im.height))
        ranges.append((start, len(rects)))

    names = sorted((f, rect_index[name_to_hash[f]]) for f in files)

    lines = [
        "//! GENERADO por scripts/gen_tile_atlas.py — no editar a mano.\n"
        "//!\n"
        "//! Metadata del texture atlas de assets/opengfx/tiles. Las páginas\n"
        "//! (assets/opengfx/atlas/tiles_atlas_{p}.png) se regeneran con el\n"
        "//! script; este archivo se commitea para que el cliente compile la\n"
        "//! tabla de lookup sin leer los PNGs.\n"
        "#![cfg_attr(rustfmt, rustfmt_skip)]\n\n",
        f"pub(crate) const TILE_ATLAS_PAGE_COUNT: usize = {page_count};\n\n",
        "/// Dimensiones `(ancho, alto)` de cada página del atlas.\n"
        "pub(crate) static TILE_ATLAS_PAGE_SIZES: &[(u32, u32)] = &[\n",
    ]
    for p in range(page_count):
        lines.append(f"    ({PAGE_W}, {used_h[p]}),\n")
    lines.append("];\n\n")
    lines.append(
        "/// Rects únicos `(página, x, y, w, h)`, agrupados por página.\n"
        "pub(crate) static TILE_ATLAS_RECTS: &[(u16, u16, u16, u16, u16)] = &[\n"
    )
    for r in rects:
        lines.append(f"    ({r[0]}, {r[1]}, {r[2]}, {r[3]}, {r[4]}),\n")
    lines.append("];\n\n")
    lines.append(
        "/// Rango `[inicio, fin)` de `TILE_ATLAS_RECTS` por página.\n"
        "pub(crate) static TILE_ATLAS_PAGE_RANGES: &[(u32, u32)] = &[\n"
    )
    for a, b in ranges:
        lines.append(f"    ({a}, {b}),\n")
    lines.append("];\n\n")
    lines.append(
        "/// `(archivo, índice en TILE_ATLAS_RECTS)`, ordenado por nombre\n"
        "/// (apto para búsqueda binaria). Incluye aliases: varios nombres\n"
        "/// pueden apuntar al mismo rect.\n"
        "pub(crate) static TILE_ATLAS_NAMES: &[(&str, u32)] = &[\n"
    )
    for f, idx in names:
        lines.append(f'    ("{f}", {idx}),\n')
    lines.append("];\n")

    rs_text = "".join(lines)
    return rs_text, pages, used_h, files, unique


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="empaqueta en memoria y compara el .rs versionado (no escribe PNG ni .rs)",
    )
    args = parser.parse_args(argv)

    if not assets_available():
        if args.check:
            print(
                "SKIP: --check de tile_atlas requiere assets/opengfx/tiles/*.png",
                file=sys.stderr,
            )
            return 2
        raise SystemExit(f"No hay PNGs en {TILES_DIR}; corré descargar_graficos.sh")

    rs_text, pages, used_h, files, unique = build_atlas()

    if args.check:
        current = OUT_RS.read_text(encoding="utf-8")
        if current != rs_text:
            print(
                "DRIFT: tile_atlas_generated.rs no coincide con el generador.",
                file=sys.stderr,
            )
            print("  Regenerá con: python3 scripts/gen_tile_atlas.py", file=sys.stderr)
            return 1
        print(f"OK: {OUT_RS.relative_to(ROOT)} coincide ({len(files)} archivos)")
        return 0

    ATLAS_DIR.mkdir(parents=True, exist_ok=True)
    for p, img in enumerate(pages):
        img.save(ATLAS_DIR / f"tiles_atlas_{p}.png", optimize=True)
    OUT_RS.write_text(rs_text, encoding="utf-8")

    total = sum(im.width * im.height for im in unique.values())
    page_count = len(pages)
    print(
        f"atlas: {len(files)} archivos, {len(unique)} únicos, "
        f"{page_count} página(s) de {PAGE_W}px de ancho "
        f"(alturas {used_h}), ocupación {total / (PAGE_W * sum(used_h)):.0%}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
