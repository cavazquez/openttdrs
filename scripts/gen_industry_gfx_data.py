#!/usr/bin/env python3
"""Genera ``INDUSTRY_GFX_DATA`` desde ``industry_land.h`` de OpenTTD.

OpenTTD indexa ``_industry_draw_tile_data[gfx * 4 + construction_stage]``
(estadios 0–3). Cada fila ``M()`` conserva dos contratos distintos:

* dimensiones/anclas de pantalla, obtenidas del NFO y PNG del perfil gráfico
  activo (8bpp o 32bpp);
* caja 3D ``dx, dy, sx, sy, sz`` usada por ``AddSortableSpriteToDraw``.

Mantener ambos evita mezclar las anclas 32bpp con recortes 8bpp y permite que
la traza ``world-draw`` compare el orden espacial contra OpenTTD.
"""
from __future__ import annotations

import argparse
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
GFX_COUNT = 175
FALLBACK = (64.0, 48.0, -32.0, -32.0)


def parse_atom(a: str) -> int:
    a = a.split("|")[0].strip()
    return int(a, 16) if a.startswith("0x") else int(a)


def parse_macro_rows(path: Path) -> list[tuple[int, int, int, int, int, int, int]]:
    """``s1, s2, dx, dy, sx, sy, sz`` por cada fila ``M()``."""
    pat = re.compile(
        r"^\s*M\(\s*([^,]+),\s*[^,]+,\s*([^,]+),\s*[^,]+,\s*"
        r"(-?\d+),\s*(-?\d+),\s*(-?\d+),\s*(-?\d+),\s*(-?\d+),"
    )
    out: list[tuple[int, int, int, int, int, int, int]] = []
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
                    int(m.group(7)),
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
    sort_ox: int,
    sort_oy: int,
    sort_oz: int,
    sort_ex: int,
    sort_ey: int,
    sort_ez: int,
) -> str:
    return (
        f"    IndustryGfxSprite {{ sprite_id: {s2}, ground_sprite_id: {gid}, "
        f"w: {bw:.1f}, h: {bh:.1f}, xrel: {bx:.1f}, yrel: {by:.1f}, "
        f"ground_w: {gw:.1f}, ground_h: {gh:.1f}, "
        f"ground_xrel: {gx:.1f}, ground_yrel: {gy:.1f}, "
        f"sort_ox: {sort_ox}, sort_oy: {sort_oy}, sort_oz: {sort_oz}, "
        f"sort_ex: {sort_ex}, sort_ey: {sort_ey}, sort_ez: {sort_ez} }},"
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


def build_content(repo: Path, upstream: Path) -> tuple[str, tuple[int, int, int, int]]:
    """Construye la tabla sin escribirla; facilita ``--check`` y regresiones."""
    rows_macro = parse_macro_rows(upstream)
    need = GFX_COUNT * STAGES
    if len(rows_macro) < need:
        raise ValueError(f"Entries insuficientes: {len(rows_macro)} < {need}")

    tiles_dir = repo / "assets" / "opengfx" / "tiles"
    nfo = parse_sprite_offs(repo)
    prefer_bpp = detect_graphics_mode(repo)

    body_rows: list[str] = []
    nfo_bld = nfo_gnd = macro_cal = fallback_cal = 0
    for gfx in range(GFX_COUNT):
        for stage in range(STAGES):
            idx = gfx * STAGES + stage
            s1, s2, dx, dy, sx, sy, sz = rows_macro[idx]
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
            body_rows.append(
                industry_row_line(
                    s2,
                    gid,
                    bw,
                    bh,
                    bx,
                    by,
                    gw,
                    gh,
                    gx,
                    gy,
                    dx,
                    dy,
                    0,
                    sx,
                    sy,
                    sz,
                )
            )

    total = GFX_COUNT * STAGES
    lines = [
        "// @generated by scripts/gen_industry_gfx_data.py — no editar a mano.",
        "// Fuente: OpenTTD _industry_draw_tile_data (gfx*4+stage, stage 0..3).",
        "// Offsets: NFO + PNG por capa (suelo / edificio) del perfil gráfico activo.",
        "// Bounds: M(dx, dy, sx, sy, sz) de OpenTTD para AddSortableSpriteToDraw.",
        "",
        f"#[allow(clippy::large_const_arrays)]\n"
        f"pub const INDUSTRY_GFX_DATA: [IndustryGfxSprite; {total}] = [\n"
        + "\n".join(body_rows)
        + "\n];",
        "",
    ]
    return "\n".join(lines), (nfo_bld, nfo_gnd, macro_cal, fallback_cal)


def main(argv: list[str] | None = None) -> int:
    repo = Path(__file__).resolve().parents[1]
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("upstream", nargs="?", help="ruta alternativa a industry_land.h")
    parser.add_argument("--check", action="store_true", help="verificar sin escribir")
    args = parser.parse_args(argv)
    upstream = Path(args.upstream) if args.upstream else repo / "third_party" / "openttd" / "industry_land.h"
    if not upstream.is_file():
        print(
            f"Falta {upstream}. Copiá industry_land.h desde OpenTTD o pasá una ruta.",
            file=sys.stderr,
        )
        return 1

    try:
        content, (nfo_bld, nfo_gnd, macro_cal, fallback_cal) = build_content(repo, upstream)
    except (OSError, ValueError) as error:
        print(f"No se pudo generar INDUSTRY_GFX_DATA: {error}", file=sys.stderr)
        return 1

    out_path = (
        repo / "crates" / "openttdrs-client" / "src" / "sprites" / "industry_gfx_data_generated.rs"
    )
    if args.check:
        if out_path.is_file() and out_path.read_text(encoding="utf-8") == content:
            print(f"OK {out_path} ({GFX_COUNT * STAGES} filas)")
            return 0
        print(f"DRIFT {out_path}: ejecutá scripts/gen_industry_gfx_data.py", file=sys.stderr)
        return 1

    out_path.write_text(content, encoding="utf-8")
    print(
        f"Escrito {out_path} ({GFX_COUNT * STAGES} filas, nfo_bld={nfo_bld} nfo_gnd={nfo_gnd} "
        f"macro={macro_cal} fallback={fallback_cal})"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
