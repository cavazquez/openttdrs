#!/usr/bin/env python3
"""Genera INDUSTRY_GFX_DATA para openttdrs desde OpenTTD src/table/industry_land.h.

OpenTTD: `_industry_draw_tile_data[gfx * 4 + construction_stage]` (estadios 0–3).
Offsets w/h/xrel/yrel por capa desde NFO + PNG (`industry_<id>.png`).
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

from nfo_sprite_meta import (
    detect_graphics_mode,
    parse_sprite_offs,
    sprite_dims_from_assets,
)

GRASS_S1 = 0xF54
STAGES = 4
GFX_COUNT = 120
FALLBACK = (64.0, 48.0, -32.0, -32.0)


def parse_atom(a: str) -> int:
    a = a.split("|")[0].strip()
    return int(a, 16) if a.startswith("0x") else int(a)


def parse_macro_rows(path: Path) -> list[tuple[int, int, int, int, int, int]]:
    """s1, s2, dx, dy, sx, sy por cada fila M()."""
    pat = re.compile(
        r"^\s*M\(\s*([^,]+),\s*[^,]+,\s*([^,]+),\s*[^,]+,\s*"
        r"(-?\d+),\s*(-?\d+),\s*(-?\d+),\s*(-?\d+),"
    )
    out: list[tuple[int, int, int, int, int, int]] = []
    for line in path.read_text(encoding="utf-8").splitlines():
        m = pat.match(line)
        if not m:
            continue
        try:
            out.append(
                (
                    parse_atom(m.group(1)),
                    parse_atom(m.group(2)),
                    int(m.group(3)),
                    int(m.group(4)),
                    int(m.group(5)),
                    int(m.group(6)),
                )
            )
        except ValueError:
            continue
    return out


def industry_row_line(
    s2: int,
    gid: int,
    bw: float,
    bh: float,
    bx: float,
    by: float,
    gw: float,
    gh: float,
    gx: float,
    gy: float,
) -> str:
    return (
        f"    IndustryGfxSprite {{ sprite_id: {s2}, ground_sprite_id: {gid}, "
        f"w: {bw:.1f}, h: {bh:.1f}, xrel: {bx:.1f}, yrel: {by:.1f}, "
        f"ground_w: {gw:.1f}, ground_h: {gh:.1f}, "
        f"ground_xrel: {gx:.1f}, ground_yrel: {gy:.1f} }},"
    )


def dims_for_macro_row(
    repo: Path,
    tiles_dir: Path,
    nfo: dict,
    prefer_bpp: str | None,
    s1: int,
    s2: int,
    dx: int,
    dy: int,
    sx: int,
    sy: int,
) -> tuple[tuple[float, float, float, float], tuple[float, float, float, float], str, str]:
    gid = 0 if s1 == GRASS_S1 else s1
    gw, gh, gx, gy, gnote = sprite_dims_from_assets(
        repo,
        tiles_dir,
        nfo,
        gid,
        f"industry_{gid}.png",
        prefer_bpp,
        macro_dx=dx,
        macro_dy=dy,
        macro_sx=sx,
        macro_sy=sy,
        fallback=FALLBACK,
    )
    bw, bh, bx, by, bnote = sprite_dims_from_assets(
        repo,
        tiles_dir,
        nfo,
        s2,
        f"industry_{s2}.png",
        prefer_bpp,
        macro_dx=dx,
        macro_dy=dy,
        macro_sx=sx,
        macro_sy=sy,
        fallback=FALLBACK,
    )
    if s2 == 0 and gid == 0:
        bw, bh, bx, by = FALLBACK
    return (gw, gh, gx, gy), (bw, bh, bx, by), gnote, bnote


def main() -> int:
    repo = Path(__file__).resolve().parents[1]
    upstream = repo / "third_party" / "openttd" / "industry_land.h"
    if len(sys.argv) >= 2:
        upstream = Path(sys.argv[1])
    if not upstream.is_file():
        print(
            f"Falta {upstream}. Copiá industry_land.h desde OpenTTD o pasá ruta como argv[1].",
            file=sys.stderr,
        )
        return 1

    rows_macro = parse_macro_rows(upstream)
    need = GFX_COUNT * STAGES
    if len(rows_macro) < need:
        print(f"Entries insuficientes: {len(rows_macro)} < {need}", file=sys.stderr)
        return 1

    tiles_dir = repo / "assets" / "opengfx" / "tiles"
    out_path = (
        repo / "crates" / "openttdrs-client" / "src" / "sprites" / "industry_gfx_data_generated.rs"
    )
    nfo = parse_sprite_offs(repo)
    prefer_bpp = detect_graphics_mode(repo)

    body_rows: list[str] = []
    nfo_bld = nfo_gnd = macro_cal = fallback_cal = 0
    for gfx in range(GFX_COUNT):
        for stage in range(STAGES):
            idx = gfx * STAGES + stage
            s1, s2, dx, dy, sx, sy = rows_macro[idx]
            (gw, gh, gx, gy), (bw, bh, bx, by), gnote, bnote = dims_for_macro_row(
                repo, tiles_dir, nfo, prefer_bpp, s1, s2, dx, dy, sx, sy
            )

            if s2 == 0 and (s1 == GRASS_S1 or s1 == 0):
                fallback_cal += 1
            elif s2 == 0:
                if gnote.startswith("nfo"):
                    nfo_gnd += 1
                elif gnote == "macro":
                    macro_cal += 1
            elif s1 == GRASS_S1 or s1 == 0:
                if bnote.startswith("nfo"):
                    nfo_bld += 1
                elif bnote == "macro":
                    macro_cal += 1
            else:
                if bnote.startswith("nfo"):
                    nfo_bld += 1
                if gnote.startswith("nfo"):
                    nfo_gnd += 1
                if bnote == "macro" or gnote == "macro":
                    macro_cal += 1

            gid = 0 if s1 == GRASS_S1 else s1
            body_rows.append(industry_row_line(s2, gid, bw, bh, bx, by, gw, gh, gx, gy))

    total = GFX_COUNT * STAGES
    lines = [
        "// @generated by scripts/gen_industry_gfx_data.py — no editar a mano.",
        "// Fuente: OpenTTD _industry_draw_tile_data (gfx*4+stage, stage 0..3).",
        "// Offsets: NFO + PNG por capa (suelo / edificio).",
        "",
        f"#[allow(clippy::large_const_arrays)]\n"
        f"pub const INDUSTRY_GFX_DATA: [IndustryGfxSprite; {total}] = [\n"
        + "\n".join(body_rows)
        + "\n];",
        "",
    ]
    out_path.write_text("\n".join(lines), encoding="utf-8")
    print(
        f"Escrito {out_path} ({total} filas, nfo_bld={nfo_bld} nfo_gnd={nfo_gnd} "
        f"macro={macro_cal} fallback={fallback_cal})"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
