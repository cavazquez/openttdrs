#!/usr/bin/env python3
"""Empaqueta assets/opengfx/tiles/*.png en un texture atlas.

Bevy 2D solo agrupa en un draw call sprites consecutivos (en orden Z) que
comparten textura; con ~2300 PNGs sueltos cada tesela corta el batch. Este
script deduplica por contenido (muchos archivos son aliases), empaqueta las
imágenes únicas en páginas de atlas (shelf packing) y genera:

  - assets/opengfx/atlas/tiles_atlas_{p}.png      (páginas, gitignored)
  - crates/openttdrs-client/src/sprites/tile_atlas_generated.rs (committed)

Correr después de scripts/descargar_graficos.sh.
"""

from __future__ import annotations

import hashlib
import os
import sys

from PIL import Image

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
TILES_DIR = os.path.join(ROOT, "assets/opengfx/tiles")
ATLAS_DIR = os.path.join(ROOT, "assets/opengfx/atlas")
OUT_RS = os.path.join(
    ROOT, "crates/openttdrs-client/src/sprites/tile_atlas_generated.rs"
)

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


def main() -> None:
    files = sorted(f for f in os.listdir(TILES_DIR) if f.endswith(".png"))
    if not files:
        raise SystemExit(f"No hay PNGs en {TILES_DIR}; corré descargar_graficos.sh")

    # Dedupe por contenido: hash -> imagen única; nombre -> hash.
    unique: dict[str, Image.Image] = {}
    name_to_hash: dict[str, str] = {}
    for f in files:
        data = open(os.path.join(TILES_DIR, f), "rb").read()
        h = hashlib.sha1(data).hexdigest()
        name_to_hash[f] = h
        if h not in unique:
            unique[h] = Image.open(os.path.join(TILES_DIR, f)).convert("RGBA")

    # Orden estable: alto desc, ancho desc, hash (mejor relleno de shelf).
    items = sorted(
        ((h, im.width, im.height) for h, im in unique.items()),
        key=lambda t: (-t[2], -t[1], t[0]),
    )
    placed, page_count = shelf_pack(items)

    # Altura real usada por página (recorta la última shelf).
    used_h = [0] * page_count
    for h, im in unique.items():
        page, _x, y = placed[h]
        used_h[page] = max(used_h[page], y + im.height + PAD)

    os.makedirs(ATLAS_DIR, exist_ok=True)
    pages = [Image.new("RGBA", (PAGE_W, used_h[p]), (0, 0, 0, 0)) for p in range(page_count)]
    for h, im in unique.items():
        page, x, y = placed[h]
        pages[page].paste(im, (x, y))
    for p, img in enumerate(pages):
        img.save(os.path.join(ATLAS_DIR, f"tiles_atlas_{p}.png"), optimize=True)

    # Rects únicos agrupados por página (índice dentro de página = posición
    # dentro del rango de su página, que es el índice del TextureAtlasLayout).
    hashes_by_page: list[list[str]] = [[] for _ in range(page_count)]
    for h in unique:
        hashes_by_page[placed[h][0]].append(h)
    for lst in hashes_by_page:
        lst.sort()

    rects = []  # (page, x, y, w, h)
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

    with open(OUT_RS, "w") as out:
        out.write(
            "//! GENERADO por scripts/gen_tile_atlas.py — no editar a mano.\n"
            "//!\n"
            "//! Metadata del texture atlas de assets/opengfx/tiles. Las páginas\n"
            "//! (assets/opengfx/atlas/tiles_atlas_{p}.png) se regeneran con el\n"
            "//! script; este archivo se commitea para que el cliente compile la\n"
            "//! tabla de lookup sin leer los PNGs.\n\n"
        )
        out.write(f"pub(crate) const TILE_ATLAS_PAGE_COUNT: usize = {page_count};\n\n")
        out.write(
            "/// Dimensiones `(ancho, alto)` de cada página del atlas.\n"
            "pub(crate) static TILE_ATLAS_PAGE_SIZES: &[(u32, u32)] = &[\n"
        )
        for p in range(page_count):
            out.write(f"    ({PAGE_W}, {used_h[p]}),\n")
        out.write("];\n\n")
        out.write(
            "/// Rects únicos `(página, x, y, w, h)`, agrupados por página.\n"
            "pub(crate) static TILE_ATLAS_RECTS: &[(u16, u16, u16, u16, u16)] = &[\n"
        )
        for r in rects:
            out.write(f"    ({r[0]}, {r[1]}, {r[2]}, {r[3]}, {r[4]}),\n")
        out.write("];\n\n")
        out.write(
            "/// Rango `[inicio, fin)` de `TILE_ATLAS_RECTS` por página.\n"
            "pub(crate) static TILE_ATLAS_PAGE_RANGES: &[(u32, u32)] = &[\n"
        )
        for a, b in ranges:
            out.write(f"    ({a}, {b}),\n")
        out.write("];\n\n")
        out.write(
            "/// `(archivo, índice en TILE_ATLAS_RECTS)`, ordenado por nombre\n"
            "/// (apto para búsqueda binaria). Incluye aliases: varios nombres\n"
            "/// pueden apuntar al mismo rect.\n"
            "pub(crate) static TILE_ATLAS_NAMES: &[(&str, u32)] = &[\n"
        )
        for f, idx in names:
            out.write(f'    ("{f}", {idx}),\n')
        out.write("];\n")

    total = sum(im.width * im.height for im in unique.values())
    print(
        f"atlas: {len(files)} archivos, {len(unique)} únicos, "
        f"{page_count} página(s) de {PAGE_W}px de ancho "
        f"(alturas {used_h}), ocupación {total / (PAGE_W * sum(used_h)):.0%}"
    )


if __name__ == "__main__":
    sys.exit(main())
