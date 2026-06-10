#!/usr/bin/env python3
"""Extrae el humo de chimenea de la central eléctrica (SPR_CHIMNEY_SMOKE_0..7).

OpenTTD crea un `EffectVehicle` en la tesela `GFX_POWERPLANT_CHIMNEY` (gfx 8)
que cicla los sprites 3701..3708 (`ChimneySmokeTick`, un frame cada 8 ticks).
Recorta `chimney_smoke_{i}.png` y emite los metadatos NFO en
`crates/openttdrs-client/src/sprites/smoke_draw_data_generated.rs`.

Uso: python3 scripts/gen_chimney_smoke.py
"""
from __future__ import annotations

from pathlib import Path

from gen_field_draw_data import REPO, TILES_DIR, Cropper
from nfo_sprite_meta import detect_graphics_mode, parse_sprite_offs, sprite_dims_from_assets

SPR_CHIMNEY_SMOKE_0 = 3701
FRAMES = 8
OUT_RS = REPO / "crates/openttdrs-client/src/sprites/smoke_draw_data_generated.rs"


def main() -> None:
    mode = detect_graphics_mode(REPO) or "8bpp"
    cropper = Cropper(mode)
    for i in range(FRAMES):
        cropper.crop(SPR_CHIMNEY_SMOKE_0 + i, f"chimney_smoke_{i}.png")
    print(f"Recortados {FRAMES} frames de humo en {TILES_DIR}")

    nfo = parse_sprite_offs(REPO)
    lines = [
        "// Generado por scripts/gen_chimney_smoke.py — NO EDITAR A MANO.",
        "//",
        "// Humo de chimenea (`SPR_CHIMNEY_SMOKE_0..7`, EffectVehicle de la",
        "// central eléctrica). Metadatos NFO (w, h, xrel, yrel) por frame.",
        "",
        "pub const CHIMNEY_SMOKE_FRAMES: usize = 8;",
        "",
        "/// (w, h, xrel, yrel) de `chimney_smoke_{i}.png`.",
        "pub static CHIMNEY_SMOKE_META: [(f32, f32, f32, f32); 8] = [",
    ]
    for i in range(FRAMES):
        sid = SPR_CHIMNEY_SMOKE_0 + i
        png = f"chimney_smoke_{i}.png"
        w, h, xr, yr, _ = sprite_dims_from_assets(REPO, TILES_DIR, nfo, sid, png, mode)
        lines.append(f"    ({w:.1f}, {h:.1f}, {xr:.1f}, {yr:.1f}),")
    lines += ["];", ""]
    OUT_RS.write_text("\n".join(lines), encoding="utf-8")
    print(f"Escrito {OUT_RS}")


if __name__ == "__main__":
    main()
