#!/usr/bin/env python3
"""Genera sprites y datos de dibujo de árboles (MP_TREES, todos los climas).

1. Recorta `tree_{i:02d}.png` para todo el rango de sprites usado por
   `_tree_layout_sprite` (1576..2009) desde los sheets OpenGFX ya decodificados,
   con la misma lógica de `descargar_graficos.sh` (`crop_by_id`).
2. Porta `_tree_layout_xy` y las 196 filas de `_tree_layout_sprite` de
   `table/tree_land.h`, incluidas las variantes climáticas y las filas extra
   que usa el dibujo ártico sobre nieve.
3. Emite `crates/openttdrs-client/src/sprites/tree_draw_data_generated.rs`
   con metadatos w/h/xrel/yrel por sprite (vía NFO).

Uso: python3 scripts/gen_tree_draw_data.py
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

from gen_field_draw_data import Cropper, REPO, TILES_DIR
from nfo_sprite_meta import detect_graphics_mode, parse_sprite_offs, sprite_dims_from_assets

SPR_TREES_BASE = 1576
# La fila más alta usa 0x7d3 y el último estadio suma seis sprites.
TREE_SPRITE_COUNT = 0x7D3 + 6 - SPR_TREES_BASE + 1
TREE_LAYOUT_ROWS = 164 + (79 - 48 + 1)

TREE_PALETTE_IDS = {
    "PAL_NONE": 0,
    "PALETTE_TO_PALE_GREEN": 776,
    "PALETTE_TO_PINK": 777,
    "PALETTE_TO_YELLOW": 778,
    "PALETTE_TO_RED": 779,
    "PALETTE_TO_GREEN": 781,
    "PALETTE_TO_CREAM": 784,
    "PALETTE_TO_MAUVE": 785,
    "PALETTE_TO_PURPLE": 786,
    "PALETTE_TO_ORANGE": 787,
    "PALETTE_TO_BROWN": 788,
    "PALETTE_TO_GREY": 789,
    "PALETTE_TO_WHITE": 790,
}

TREE_LAND_H = REPO / "reference" / "openttd-upstream" / "src" / "table" / "tree_land.h"
OUT_RS = REPO / "crates/openttdrs-client/src/sprites/tree_draw_data_generated.rs"

def crop_tree_sprites(mode: str) -> None:
    cropper = Cropper(mode)
    written = 0
    for i in range(TREE_SPRITE_COUNT):
        sid = SPR_TREES_BASE + i
        cropper.crop(sid, f"tree_{i:02d}.png")
        written += 1
    print(f"Recortados {written} sprites de árbol en {TILES_DIR}")


def parse_layout_xy(text: str) -> list[list[tuple[int, int]]]:
    m = re.search(r"_tree_layout_xy\[\]\[4\] = \{(.*?)\n\};", text, re.S)
    if not m:
        sys.exit("no se encontró _tree_layout_xy")
    rows = []
    for line in m.group(1).splitlines():
        pairs = re.findall(r"\{\s*(\d+),\s*(\d+)\s*\}", line)
        if len(pairs) == 4:
            rows.append([(int(a), int(b)) for a, b in pairs])
    if len(rows) != 4:
        sys.exit(f"_tree_layout_xy: esperaba 4 filas, hay {len(rows)}")
    return rows


def parse_layout_sprite(text: str) -> tuple[list[list[int]], list[list[int]]]:
    m = re.search(r"_tree_layout_sprite\[[^\]]*\]\[4\] = \{(.*?)\n\};", text, re.S)
    if not m:
        sys.exit("no se encontró _tree_layout_sprite")
    rows = []
    palettes = []
    for line in m.group(1).splitlines():
        cells = re.findall(
            r"\{\s*0x([0-9a-fA-F]+),\s*([A-Z0-9_]+)\s*\}", line
        )
        if len(cells) == 4:
            rows.append([int(sprite, 16) - SPR_TREES_BASE for sprite, _ in cells])
            palette_row = []
            for _, palette in cells:
                try:
                    palette_row.append(TREE_PALETTE_IDS[palette])
                except KeyError:
                    sys.exit(f"paleta de árbol no soportada: {palette}")
            palettes.append(palette_row)
    if len(rows) != TREE_LAYOUT_ROWS:
        sys.exit(f"_tree_layout_sprite: esperaba {TREE_LAYOUT_ROWS} filas, hay {len(rows)}")
    for r in rows:
        for v in r:
            if not 0 <= v < TREE_SPRITE_COUNT:
                sys.exit(f"índice de sprite fuera de rango: {v}")
    return rows, palettes


def main() -> None:
    mode = detect_graphics_mode(REPO) or "8bpp"
    crop_tree_sprites(mode)

    text = TREE_LAND_H.read_text(encoding="utf-8")
    layout_xy = parse_layout_xy(text)
    layout_sprite, layout_palette = parse_layout_sprite(text)

    nfo = parse_sprite_offs(REPO)
    metas = []
    for i in range(TREE_SPRITE_COUNT):
        sid = SPR_TREES_BASE + i
        png = f"tree_{i:02d}.png"
        w, h, xr, yr, _note = sprite_dims_from_assets(REPO, TILES_DIR, nfo, sid, png, mode)
        metas.append((w, h, xr, yr))

    lines = [
        "// Generado por scripts/gen_tree_draw_data.py — NO EDITAR A MANO.",
        "//",
        "// Sprites vanilla de árboles de OpenTTD (rango 1576..2009) + tablas",
        "// climáticas de `table/tree_land.h`.",
        "#![cfg_attr(rustfmt, rustfmt_skip)]",
        "",
        "/// Metadatos NFO de un sprite de árbol (`tree_{NN}.png`, NN = id − 1576).",
        "#[derive(Debug, Clone, Copy)]",
        "pub struct TreeSpriteMeta {",
        "    pub w: f32,",
        "    pub h: f32,",
        "    pub xrel: f32,",
        "    pub yrel: f32,",
        "}",
        "",
        f"pub const TREE_SPRITE_COUNT: usize = {TREE_SPRITE_COUNT};",
        "",
        "pub static TREE_SPRITE_META: [TreeSpriteMeta; TREE_SPRITE_COUNT] = [",
    ]
    for w, h, xr, yr in metas:
        lines.append(
            f"    TreeSpriteMeta {{ w: {w:.1f}, h: {h:.1f}, xrel: {xr:.1f}, yrel: {yr:.1f} }},"
        )
    lines.append("];")
    lines.append("")
    lines.append("/// `_tree_layout_xy`: posiciones sub-tesela (0..15) de hasta 4 árboles.")
    lines.append("pub static TREE_LAYOUT_XY: [[(u8, u8); 4]; 4] = [")
    for row in layout_xy:
        cells = ", ".join(f"({a}, {b})" for a, b in row)
        lines.append(f"    [{cells}],")
    lines.append("];")
    lines.append("")
    lines.append("/// `_tree_layout_sprite`: índice = tipo (m3) × 4 + variante.")
    lines.append("/// Valor = índice png base de la especie (sumar etapa 0..6).")
    lines.append(
        f"pub static TREE_LAYOUT_SPRITE: [[u16; 4]; {TREE_LAYOUT_ROWS}] = ["
    )
    for row in layout_sprite:
        cells = ", ".join(str(v) for v in row)
        lines.append(f"    [{cells}],")
    lines.append("];")
    lines.append("")

    lines.append(
        f"/// Paleta declarada por `PalSpriteID` para cada entrada del layout ({TREE_LAYOUT_ROWS} filas)."
    )
    lines.append("#[allow(dead_code)]")
    lines.append(
        f"pub static TREE_LAYOUT_PALETTE: [[u16; 4]; {TREE_LAYOUT_ROWS}] = ["
    )
    for row in layout_palette:
        cells = ", ".join(str(value) for value in row)
        lines.append(f"    [{cells}],")
    lines.append("];")

    OUT_RS.write_text("\n".join(lines), encoding="utf-8")
    print(f"Escrito {OUT_RS}")


if __name__ == "__main__":
    main()
