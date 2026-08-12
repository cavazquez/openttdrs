#!/usr/bin/env python3
"""Genera sprites y datos de campos de cultivo (MP_CLEAR Fields) y cercas.

1. Recorta `field_{estado}_{off:02d}.png` (9 estados × 19 offsets de pendiente,
   sprites 4126..4296 = SPR_FARMLAND_BARE + estado×19 + SlopeToSpriteOffset).
2. Recorta `fence_{tipo}_{var}.png` (6 tipos × 6 variantes, sprites 4090..4125,
   `_clear_land_fence_sprites` + `_fence_mod_by_tileh_*`).
3. Porta las tablas `_fence_mod_by_tileh_*` de `table/clear_land.h` y emite
   `crates/openttdrs-client/src/sprites/field_draw_data_generated.rs` con
   metadatos NFO (w/h/xrel/yrel) de las cercas.

Uso: python3 scripts/gen_field_draw_data.py
"""
from __future__ import annotations

import os
import re
import sys
from pathlib import Path

from PIL import Image

from nfo_sprite_meta import (
    active_global_sprite_nfo,
    detect_graphics_mode,
    parse_global_sprite_rects,
    parse_sprite_offs,
    sprite_dims_from_assets,
)
from opengfx_palette import dematte_legacy_colorkey, indexed_dos_to_rgba

SPR_FARMLAND_BARE = 4126
FIELD_STATES = 9  # bare, state 1..7, haypacks
FIELD_SLOPE_OFFSETS = 19
SPR_HEDGE_BASE = 4090  # SPR_HEDGE_BUSHES; 6 tipos × 6 variantes
FENCE_TYPES = 6
FENCE_VARIANTS = 6

REPO = Path(__file__).resolve().parents[1]
TILES_DIR = REPO / "assets" / "opengfx" / "tiles"
OUT_RS = REPO / "crates/openttdrs-client/src/sprites/field_draw_data_generated.rs"


def find_clear_land_h() -> Path:
    rel = Path("src/table/clear_land.h")
    candidates = [
        REPO / "reference" / "openttd-upstream" / rel,
        REPO.parent / "OpenTTD" / rel,
    ]
    if env := os.environ.get("OPENTTD_SRC"):
        candidates.insert(0, Path(env) / rel)
    for path in candidates:
        if path.is_file():
            return path
    sys.exit(
        "No se encontró clear_land.h. Corré ./scripts/fetch-openttd-reference.sh "
        "o definí OPENTTD_SRC."
    )

def load_sheet(png_path: Path, mode: str) -> Image.Image:
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
    if img.mode == "P":
        return indexed_dos_to_rgba(img)
    return dematte_legacy_colorkey(img)


class Cropper:
    """Recorta un ``SpriteID`` global de OpenTTD desde el GRF base activo."""

    def __init__(self, mode: str) -> None:
        self.mode = mode
        nfo_path = active_global_sprite_nfo(REPO, mode)
        if nfo_path is None:
            sys.exit("No se encontró el NFO OpenGFX base activo (corré descargar_graficos.sh)")
        self.sprites_dir = nfo_path.parent
        self.rect = parse_global_sprite_rects(nfo_path, mode)
        self.sheets: dict[str, Image.Image] = {}

    def crop(self, sid: int, out_name: str) -> None:
        if sid not in self.rect:
            sys.exit(f"sprite {sid} no está en el NFO")
        rect = self.rect[sid]
        if rect.sheet not in self.sheets:
            p = self.sprites_dir / rect.sheet
            if not p.is_file():
                p = p.with_suffix(".pcx")
            self.sheets[rect.sheet] = load_sheet(p, self.mode)
        self.sheets[rect.sheet].crop(
            (rect.x, rect.y, rect.x + rect.w, rect.y + rect.h)
        ).save(TILES_DIR / out_name)


def parse_fence_mods(text: str) -> dict[str, list[int]]:
    out: dict[str, list[int]] = {}
    for side in ("sw", "se", "ne", "nw"):
        m = re.search(
            rf"_fence_mod_by_tileh_{side}\[32\] = \{{(.*?)\}};", text, re.S
        )
        if not m:
            sys.exit(f"no se encontró _fence_mod_by_tileh_{side}")
        vals = [int(v) for v in re.findall(r"\d+", m.group(1))]
        if len(vals) != 32:
            sys.exit(f"_fence_mod_by_tileh_{side}: esperaba 32 valores, hay {len(vals)}")
        out[side] = vals
    return out


def main() -> None:
    mode = detect_graphics_mode(REPO) or "8bpp"
    cropper = Cropper(mode)

    for state in range(FIELD_STATES):
        for off in range(FIELD_SLOPE_OFFSETS):
            sid = SPR_FARMLAND_BARE + state * FIELD_SLOPE_OFFSETS + off
            cropper.crop(sid, f"field_{state}_{off:02d}.png")
    for ftype in range(FENCE_TYPES):
        for var in range(FENCE_VARIANTS):
            sid = SPR_HEDGE_BASE + ftype * FENCE_VARIANTS + var
            cropper.crop(sid, f"fence_{ftype}_{var}.png")
    print(
        f"Recortados {FIELD_STATES * FIELD_SLOPE_OFFSETS} sprites de campo y "
        f"{FENCE_TYPES * FENCE_VARIANTS} de cerca en {TILES_DIR}"
    )

    mods = parse_fence_mods(find_clear_land_h().read_text(encoding="utf-8"))

    nfo = parse_sprite_offs(REPO)
    fence_meta = []
    for ftype in range(FENCE_TYPES):
        row = []
        for var in range(FENCE_VARIANTS):
            sid = SPR_HEDGE_BASE + ftype * FENCE_VARIANTS + var
            png = f"fence_{ftype}_{var}.png"
            w, h, xr, yr, _ = sprite_dims_from_assets(REPO, TILES_DIR, nfo, sid, png, mode)
            row.append((w, h, xr, yr))
        fence_meta.append(row)

    lines = [
        "// Generado por scripts/gen_field_draw_data.py — NO EDITAR A MANO.",
        "//",
        "// Campos de cultivo (`SPR_FARMLAND_*`, 9 estados × 19 pendientes) y",
        "// cercas (`SPR_HEDGE_*`, 6 tipos × 6 variantes) de `table/clear_land.h`.",
        "#![cfg_attr(rustfmt, rustfmt_skip)]",
        "",
        "/// Metadatos NFO de un sprite de cerca (`fence_{tipo}_{var}.png`).",
        "#[derive(Debug, Clone, Copy)]",
        "pub struct FenceSpriteMeta {",
        "    pub w: f32,",
        "    pub h: f32,",
        "    pub xrel: f32,",
        "    pub yrel: f32,",
        "}",
        "",
        f"pub const FIELD_STATES: usize = {FIELD_STATES};",
        "",
        "pub static FENCE_SPRITE_META: [[FenceSpriteMeta; 6]; 6] = [",
    ]
    for row in fence_meta:
        cells = ", ".join(
            f"FenceSpriteMeta {{ w: {w:.1f}, h: {h:.1f}, xrel: {xr:.1f}, yrel: {yr:.1f} }}"
            for w, h, xr, yr in row
        )
        lines.append(f"    [{cells}],")
    lines.append("];")
    for side in ("sw", "se", "ne", "nw"):
        lines.append("")
        lines.append(f"/// `_fence_mod_by_tileh_{side}`: variante de sprite por pendiente.")
        lines.append(
            f"pub static FENCE_MOD_BY_TILEH_{side.upper()}: [u8; 32] = ["
        )
        vals = mods[side]
        for i in range(0, 32, 8):
            lines.append("    " + ", ".join(str(v) for v in vals[i : i + 8]) + ",")
        lines.append("];")
    lines.append("")

    OUT_RS.write_text("\n".join(lines), encoding="utf-8")
    print(f"Escrito {OUT_RS}")


if __name__ == "__main__":
    main()
