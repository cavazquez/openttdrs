#!/usr/bin/env python3
"""Genera sprites y datos de dibujo de árboles (MP_TREES, clima templado).

1. Recorta `tree_{i:02d}.png` para los sprites 1576..1708 (19 especies × 7
   etapas) desde los sheets OpenGFX ya decodificados, con la misma lógica de
   `descargar_graficos.sh` (`crop_by_id`).
2. Porta `_tree_layout_xy` y las filas templadas (0..47) de
   `_tree_layout_sprite` de OpenTTD `table/tree_land.h`.
3. Emite `crates/openttdrs-client/src/sprites/tree_draw_data_generated.rs`
   con metadatos w/h/xrel/yrel por sprite (vía NFO).

Uso: python3 scripts/gen_tree_draw_data.py
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

from PIL import Image

from nfo_sprite_meta import detect_graphics_mode, parse_sprite_offs, sprite_dims_from_assets

SPR_TREES_BASE = 1576
TREE_SPRITE_COUNT = 133  # 19 especies × 7 etapas (0x628..0x6A6+6)
TEMPERATE_LAYOUT_ROWS = 48  # 12 tipos (m3) × 4 variantes

REPO = Path(__file__).resolve().parents[1]
TILES_DIR = REPO / "assets" / "opengfx" / "tiles"
TREE_LAND_H = REPO / "reference" / "openttd-upstream" / "src" / "table" / "tree_land.h"
OUT_RS = REPO / "crates/openttdrs-client/src/sprites/tree_draw_data_generated.rs"

SHEET_RE = re.compile(
    r"^\s*(\d+)\s+(\S*?((?:ogfx1_base|ogfx21_base_32ez)\d+\.(?:32\.png|png|pcx)))\s+(?:8bpp|32bpp)\s+"
    r"(\d+)\s+(\d+)\s+(\d+)\s+(\d+)\s+(-?\d+)\s+(-?\d+)"
)


def find_sprites_dir() -> Path:
    for cand in (REPO / "assets" / "opengfx").glob("*/sprites"):
        if any(cand.glob("*.nfo")):
            return cand
    sys.exit("No se encontró el directorio de sprites decodificados (corré descargar_graficos.sh)")


def load_sheet(png_path: Path, mode: str) -> Image.Image:
    """Réplica de `load_sheet` de descargar_graficos.sh para el modo activo."""
    img = Image.open(png_path)
    if mode == "32bpp":
        if img.mode == "P":
            pal = img.getpalette()
            transparent_rgb = tuple(pal[0:3]) if pal else None
            img_rgba = img.convert("RGBA")
            if transparent_rgb is not None:
                data = [
                    (0, 0, 0, 0) if (r, g, b) == transparent_rgb else (r, g, b, a)
                    for r, g, b, a in img_rgba.getdata()
                ]
                img_rgba.putdata(data)
            return img_rgba
        return img.convert("RGBA")
    # 8bpp clásico: azul índice 0 → transparente (heurística del script base).
    img_rgba = img.convert("RGBA")
    data = [
        (0, 0, 0, 0) if (r, g, b) == (0, 0, 255) else (r, g, b, a)
        for r, g, b, a in img_rgba.getdata()
    ]
    img_rgba.putdata(data)
    return img_rgba


def crop_tree_sprites(mode: str) -> None:
    sprites_dir = find_sprites_dir()
    # Hay base + extra; `glob` no garantiza orden y el extra puede salir primero.
    sprite_rect: dict[int, tuple[int, int, int, int, str]] = {}
    for nfo in sorted(sprites_dir.glob("*.nfo")):
        for line in nfo.read_text(errors="replace").splitlines():
            m = SHEET_RE.match(line)
            if m:
                sid = int(m.group(1))
                sprite_rect[sid] = (
                    int(m.group(4)),
                    int(m.group(5)),
                    int(m.group(6)),
                    int(m.group(7)),
                    Path(m.group(2)).name,
                )

    sheets: dict[str, Image.Image] = {}
    written = 0
    for i in range(TREE_SPRITE_COUNT):
        sid = SPR_TREES_BASE + i
        if sid not in sprite_rect:
            sys.exit(f"sprite {sid} no está en el NFO")
        x, y, w, h, sheet_name = sprite_rect[sid]
        if sheet_name not in sheets:
            p = sprites_dir / sheet_name
            if not p.is_file():
                p = p.with_suffix(".pcx")
            sheets[sheet_name] = load_sheet(p, mode)
        crop = sheets[sheet_name].crop((x, y, x + w, y + h))
        crop.save(TILES_DIR / f"tree_{i:02d}.png")
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


def parse_layout_sprite(text: str) -> list[list[int]]:
    m = re.search(r"_tree_layout_sprite\[[^\]]*\]\[4\] = \{(.*?)\n\};", text, re.S)
    if not m:
        sys.exit("no se encontró _tree_layout_sprite")
    rows = []
    for line in m.group(1).splitlines():
        ids = re.findall(r"\{\s*0x([0-9a-fA-F]+),\s*PAL_NONE\s*\}", line)
        if len(ids) == 4:
            rows.append([int(s, 16) - SPR_TREES_BASE for s in ids])
    if len(rows) < TEMPERATE_LAYOUT_ROWS:
        sys.exit(f"_tree_layout_sprite: esperaba ≥{TEMPERATE_LAYOUT_ROWS} filas, hay {len(rows)}")
    rows = rows[:TEMPERATE_LAYOUT_ROWS]
    for r in rows:
        for v in r:
            if not 0 <= v < TREE_SPRITE_COUNT:
                sys.exit(f"índice de sprite fuera de rango: {v}")
    return rows


def main() -> None:
    mode = detect_graphics_mode(REPO) or "8bpp"
    crop_tree_sprites(mode)

    text = TREE_LAND_H.read_text(encoding="utf-8")
    layout_xy = parse_layout_xy(text)
    layout_sprite = parse_layout_sprite(text)

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
        "// Sprites de árboles templados de OpenTTD (SPR_TREES_BASE=1576, 19 especies",
        "// × 7 etapas) + tablas de layout de `table/tree_land.h`.",
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
    lines.append("/// `_tree_layout_sprite` filas templadas: índice = tipo (m3) × 4 + variante.")
    lines.append("/// Valor = índice png base de la especie (sumar etapa 0..6).")
    lines.append(
        f"pub static TREE_LAYOUT_SPRITE: [[u16; 4]; {TEMPERATE_LAYOUT_ROWS}] = ["
    )
    for row in layout_sprite:
        cells = ", ".join(str(v) for v in row)
        lines.append(f"    [{cells}],")
    lines.append("];")
    lines.append("")

    OUT_RS.write_text("\n".join(lines), encoding="utf-8")
    print(f"Escrito {OUT_RS}")


if __name__ == "__main__":
    main()
