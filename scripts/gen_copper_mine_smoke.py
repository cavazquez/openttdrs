#!/usr/bin/env python3
"""Extrae el humo de la mina de cobre (SPR_SMOKE_0..4).

OpenTTD crea un `EffectVehicle` `EV_COPPER_MINE_SMOKE` en la tesela
`GFX_COPPER_MINE_CHIMNEY` (gfx 49) en `(+6, +6, z=43)` y cicla los sprites
2040..2044 (`SmokeTick`).

Uso: python3 scripts/gen_copper_mine_smoke.py
"""
from __future__ import annotations

from pathlib import Path

from gen_field_draw_data import REPO, TILES_DIR, Cropper
from nfo_sprite_meta import detect_graphics_mode, parse_sprite_offs, sprite_dims_from_assets

SPR_SMOKE_0 = 2040
FRAMES = 5
OUT_RS = REPO / "crates/openttdrs-client/src/sprites/copper_smoke_draw_data_generated.rs"


def main() -> None:
    mode = detect_graphics_mode(REPO) or "8bpp"
    cropper = Cropper(mode)
    for i in range(FRAMES):
        cropper.crop(SPR_SMOKE_0 + i, f"mine_smoke_{i}.png")
    print(f"Recortados {FRAMES} frames de humo mina cobre en {TILES_DIR}")

    nfo = parse_sprite_offs(REPO)
    lines = [
        "// Generado por scripts/gen_copper_mine_smoke.py — NO EDITAR A MANO.",
        "//",
        "// Humo mina cobre (`SPR_SMOKE_0..4`, EffectVehicle gfx 49).",
        "",
        "pub const COPPER_MINE_SMOKE_FRAMES: usize = 5;",
        "",
        "/// (w, h, xrel, yrel) de `mine_smoke_{i}.png`.",
        "pub static COPPER_MINE_SMOKE_META: [(f32, f32, f32, f32); 5] = [",
    ]
    for i in range(FRAMES):
        sid = SPR_SMOKE_0 + i
        png = f"mine_smoke_{i}.png"
        w, h, xr, yr, _ = sprite_dims_from_assets(REPO, TILES_DIR, nfo, sid, png, mode)
        lines.append(f"    ({w:.1f}, {h:.1f}, {xr:.1f}, {yr:.1f}),")
    lines += ["];", ""]
    OUT_RS.write_text("\n".join(lines), encoding="utf-8")
    print(f"Escrito {OUT_RS}")


if __name__ == "__main__":
    main()
