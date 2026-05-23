#!/usr/bin/env python3
"""Laboratorio visual SP3: agua + vía en pendiente (mapa dedicado, sin ruido).

Salida: `crates/openttdrs-core/tests/fixtures/sp3_slope_lab.ottdmap`

Layout (16×20, origen arriba-izquierda; paso 3 en x para pendientes):

```
y=1   · vía plana Y · X · T · cruce · HORZ · VERT ·          (referencia)
y=3   · lago Clear 3×3 (x=2–4, y=3–5) ·                  (centro abierto en 3,4)
y=4   · costa explícita (8,4) ·
y=8   · recta Y en pendiente NE · SE · SW · NW ·
y=11  · cruce X|Y en pendiente NE · SE · SW · NW ·
y=14  · T (0x07) en pendiente NE · SE · SW · NW ·
y=16  · HORZ (0x0C) en pendiente NE · SE · SW · NW ·
y=17  · (buffer hierba; esquinas de la fila y=16) ·
y=18  · VERT (0x30) en pendiente NE · SE · SW · NW ·
y=19  · (buffer hierba) ·
```

Todas las teselas de agua usan `height=DEFAULT_H` (4) para evitar hundir el rombo.

Regenerar: `python3 scripts/gen_sp3_slope_lab_ottdmap.py`

Cargar:

    OTTDMAP_FILE=crates/openttdrs-core/tests/fixtures/sp3_slope_lab.ottdmap \\
      cargo run -p openttdrs-client
"""

from __future__ import annotations

import sys
from dataclasses import replace
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from gen_sp3_visual_checklist_ottdmap import (  # noqa: E402
    DEFAULT_H,
    MP_RAILWAY,
    MP_WATER,
    TileSpec,
    apply_ne_slope,
    apply_nw_slope,
    apply_se_slope,
    apply_sw_slope,
    build_map1,
    put,
)

OUT = (
    Path(__file__).resolve().parents[1]
    / "crates/openttdrs-core/tests/fixtures/sp3_slope_lab.ottdmap"
)

# Referencia plana y filas de pendiente (mismo patrón x que el checklist).
SLOPE_X = (1, 4, 7, 10)
SLOPE_FNS = (apply_ne_slope, apply_se_slope, apply_sw_slope, apply_nw_slope)
# tileh esperado por OpenTTD en cada pendiente diagonal.
SLOPE_TILEH = (12, 6, 3, 9)


def place_rail_slope(
    tiles: dict[tuple[int, int], TileSpec],
    tx: int,
    ty: int,
    slope_fn,
    m5: int,
) -> None:
    slope_fn(tiles, tx, ty)
    cur = tiles.get((tx, ty), TileSpec())
    put(tiles, tx, ty, replace(cur, tt=MP_RAILWAY, m5=m5))


def main() -> None:
    w, h = 16, 20
    tiles: dict[tuple[int, int], TileSpec] = {}

    # --- y=1: referencia plana ---
    for x, m5 in [(1, 0x02), (4, 0x01), (7, 0x07), (10, 0x03), (13, 0x0C), (15, 0x30)]:
        put(tiles, x, 1, TileSpec(tt=MP_RAILWAY, m5=m5))

    # --- y=3–5: lago Clear 3×3 + costa explícita ---
    for tx in range(2, 5):
        for ty in range(3, 6):
            put(tiles, tx, ty, TileSpec(tt=MP_WATER, height=DEFAULT_H, m5=0x00))
    put(tiles, 8, 4, TileSpec(tt=MP_WATER, height=DEFAULT_H, m5=0x10))

    # --- y=8: recta Y en 4 pendientes ---
    for tx, slope_fn in zip(SLOPE_X, SLOPE_FNS, strict=True):
        place_rail_slope(tiles, tx, 8, slope_fn, 0x02)

    # --- y=11: cruce X|Y en 4 pendientes ---
    for tx, slope_fn in zip(SLOPE_X, SLOPE_FNS, strict=True):
        place_rail_slope(tiles, tx, 11, slope_fn, 0x03)

    # --- y=14: T en 4 pendientes ---
    for tx, slope_fn in zip(SLOPE_X, SLOPE_FNS, strict=True):
        place_rail_slope(tiles, tx, 14, slope_fn, 0x07)

    # --- y=16: HORZ (UPPER+LOWER) en 4 pendientes ---
    for tx, slope_fn in zip(SLOPE_X, SLOPE_FNS, strict=True):
        place_rail_slope(tiles, tx, 16, slope_fn, 0x0C)

    # --- y=17: VERT (LEFT+RIGHT) en 4 pendientes (y=18 deja y=19 libre) ---
    for tx, slope_fn in zip(SLOPE_X, SLOPE_FNS, strict=True):
        place_rail_slope(tiles, tx, 18, slope_fn, 0x30)

    OUT.parent.mkdir(parents=True, exist_ok=True)
    data = build_map1(w, h, tiles)
    OUT.write_bytes(data)
    print(f"Escrito {OUT} ({len(data)} bytes, {w}×{h})")
    print(
        "Cargar: OTTDMAP_FILE=crates/openttdrs-core/tests/fixtures/sp3_slope_lab.ottdmap "
        "cargo run -p openttdrs-client"
    )


if __name__ == "__main__":
    main()
