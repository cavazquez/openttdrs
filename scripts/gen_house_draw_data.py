#!/usr/bin/env python3
"""Genera HOUSE_DRAW_DATA desde OpenTTD `table/town_land.h`.

OpenTTD: `_town_draw_tile_data[house_id * 16 + TileHash2Bit * 4 + stage]`.
110 casas originales (HouseID 0..109) → 1760 filas.

Offsets w/h/xrel/yrel: NFO + PNG `house_s{id}.png` por capa s1/s2.
Bounds `sort_*`: caja de mundo `dx/dy/sx/sy/sz` de cada `M(...)` para el
`AddSortableSpriteToDraw` del edificio.

Uso:
  python3 scripts/gen_house_draw_data.py
  python3 scripts/gen_house_draw_data.py --check
"""
from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

from nfo_sprite_meta import detect_graphics_mode, parse_sprite_offs, sprite_dims_from_assets

ORIGINAL_HOUSE_COUNT = 110
ROWS = ORIGINAL_HOUSE_COUNT * 16
FALLBACK = (64.0, 48.0, -32.0, -32.0)


def parse_sprite_constants(repo: Path) -> dict[str, int]:
    """Constantes `SpriteID`/`PaletteID` usadas por `town_land.h`.

    La tabla de casas no sólo referencia `SPR_*`: las dos capas de cada
    entrada de ``M(...)`` también llevan una ``PaletteID``. Conservar esa
    segunda parte es esencial: muchos edificios vanilla reutilizan el mismo
    PNG y OpenTTD les aplica una paleta de estructura, iglesia o compañía al
    dibujarlos.
    """
    path = repo / "reference" / "openttd-upstream" / "src" / "table" / "sprites.h"
    if not path.is_file():
        return {}
    pat = re.compile(
        r"static const (?:SpriteID|PaletteID)\s+([A-Z][A-Z0-9_]*)\s*=\s*(\d+)"
    )
    out: dict[str, int] = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        m = pat.match(line.strip())
        if m:
            out[m.group(1)] = int(m.group(2))
    return out


def parse_atom(a: str, spr: dict[str, int]) -> int:
    a = a.split("|")[0].strip()
    if a in spr:
        return spr[a]
    if a.startswith("0x"):
        return int(a, 16)
    if a.isdigit():
        return int(a)
    return 0


def parse_macro_rows(
    path: Path, spr: dict[str, int]
) -> list[tuple[str, str, str, str, int, int, int, int, int, int]]:
    # M(s1, p1, s2, p2, dx, dy, sx, sy, sz, p) — `p` = draw_proc (1 = lift).
    pat = re.compile(
        r"^\s*M\(\s*([^,]+),\s*([^,]+),\s*([^,]+),\s*([^,]+),\s*"
        r"(-?\d+),\s*(-?\d+),\s*(-?\d+),\s*(-?\d+),\s*(-?\d+),\s*(-?\d+)\s*\)"
    )
    out: list[tuple[str, str, str, str, int, int, int, int, int, int]] = []
    for line in path.read_text(encoding="utf-8").splitlines():
        m = pat.match(line)
        if not m:
            continue
        try:
            out.append(
                (
                    m.group(1).strip(),
                    m.group(2).strip(),
                    m.group(3).strip(),
                    m.group(4).strip(),
                    int(m.group(5)),
                    int(m.group(6)),
                    int(m.group(7)),
                    int(m.group(8)),
                    int(m.group(9)),
                    int(m.group(10)),  # draw_proc (`p`)
                )
            )
        except ValueError:
            continue
    return out


def spec_line(
    s1: int,
    s1_palette: int,
    s1_dims: tuple[float, float, float, float],
    s2: int,
    s2_palette: int,
    s2_dims: tuple[float, float, float, float],
    sort_bounds: tuple[int, int, int, int, int, int],
    draw_proc: int,
) -> str:
    sw, sh, sx, sy = s1_dims
    bw, bh, bx, by = s2_dims
    sox, soy, soz, sex, sey, sez = sort_bounds
    return (
        f"    HouseDrawSpec {{ s1: {s1}, s1_palette: {s1_palette}, s1_w: {sw:.1f}, s1_h: {sh:.1f}, "
        f"s1_xrel: {sx:.1f}, s1_yrel: {sy:.1f}, s2: {s2}, s2_palette: {s2_palette}, s2_w: {bw:.1f}, "
        f"s2_h: {bh:.1f}, s2_xrel: {bx:.1f}, s2_yrel: {by:.1f}, "
        f"sort_ox: {sox}, sort_oy: {soy}, sort_oz: {soz}, sort_ex: {sex}, sort_ey: {sey}, sort_ez: {sez}, "
        f"draw_proc: {draw_proc} }},"
    )


def build_content(repo: Path, upstream: Path) -> tuple[str, int, int, int, list[int]]:
    spr = parse_sprite_constants(repo)
    if not spr:
        print("Aviso: sin sprites.h; SPR_* en town_land.h quedarán como 0", file=sys.stderr)

    rows_macro = parse_macro_rows(upstream, spr)
    if len(rows_macro) < ROWS:
        raise SystemExit(f"Entries insuficientes: {len(rows_macro)} < {ROWS}")

    tiles_dir = repo / "assets" / "opengfx" / "tiles"
    nfo = parse_sprite_offs(repo)
    prefer_bpp = detect_graphics_mode(repo)

    body_rows: list[str] = []
    nfo_cal = macro_cal = fallback_cal = 0
    sprite_ids: set[int] = set()

    for s1_raw, s1_palette_raw, s2_raw, s2_palette_raw, dx, dy, sx, sy, sz, draw_proc in rows_macro[:ROWS]:
        # `DrawTile_Town` entrega siempre `ground.sprite` a
        # `DrawGroundSprite`. En particular, `SPR_FLAT_BARE_LAND` (3924) no
        # significa «usar césped»: es un sprite de suelo real y tiene que
        # mantenerse para que el renderer pueda distinguirlo de 3981
        # (`SPR_FLAT_GRASS_TILE`). Antes se colapsaban ambos a 0, lo que
        # convertía las parcelas de casas vanilla en césped y hacía imposible
        # contrastar la traza contra OpenTTD.
        s1 = parse_atom(s1_raw, spr)
        s1_palette = parse_atom(s1_palette_raw, spr)
        s2 = parse_atom(s2_raw, spr)
        s2_palette = parse_atom(s2_palette_raw, spr)
        if s1:
            sprite_ids.add(s1)
        if s2:
            sprite_ids.add(s2)

        if s1 == 0:
            s1_dims = (0.0, 0.0, 0.0, 0.0)
        else:
            dims = sprite_dims_from_assets(
                repo,
                tiles_dir,
                nfo,
                s1,
                f"house_s{s1}.png",
                prefer_bpp,
                macro_dx=dx,
                macro_dy=dy,
                macro_sx=sx,
                macro_sy=sy,
                fallback=FALLBACK,
            )
            s1_dims = dims[:4]
            if dims[4].startswith("nfo"):
                nfo_cal += 1
            elif dims[4] == "macro":
                macro_cal += 1

        if s2 == 0:
            s2_dims = (0.0, 0.0, 0.0, 0.0)
        else:
            dims = sprite_dims_from_assets(
                repo,
                tiles_dir,
                nfo,
                s2,
                f"house_s{s2}.png",
                prefer_bpp,
                macro_dx=dx,
                macro_dy=dy,
                macro_sx=sx,
                macro_sy=sy,
                fallback=FALLBACK,
            )
            s2_dims = dims[:4]
            if dims[4].startswith("nfo"):
                nfo_cal += 1
            elif dims[4] == "macro":
                macro_cal += 1
            elif dims[4] == "none" and s2 == 0:
                fallback_cal += 1

        body_rows.append(
            spec_line(
                s1,
                s1_palette,
                s1_dims,
                s2,
                s2_palette,
                s2_dims,
                (dx, dy, 0, sx, sy, sz),
                draw_proc,
            )
        )

    lines = [
        "// @generated by scripts/gen_house_draw_data.py — no editar a mano.",
        "// Fuente: OpenTTD _town_draw_tile_data (110 casas × 16 filas).",
        "// Índice: house_id * 16 + TileHash2Bit * 4 + building_stage.",
        "#![allow(clippy::large_const_arrays)]",
        "#![cfg_attr(rustfmt, rustfmt_skip)]",
        "",
        "use super::HouseDrawSpec;",
        "",
        f"pub const HOUSE_DRAW_DATA: [HouseDrawSpec; {ROWS}] = [",
        *body_rows,
        "];",
        "",
    ]
    missing = sorted(i for i in sprite_ids if i and not (tiles_dir / f"house_s{i}.png").is_file())
    return "\n".join(lines), nfo_cal, macro_cal, fallback_cal, missing


def assets_available(repo: Path) -> bool:
    tiles = repo / "assets" / "opengfx" / "tiles"
    if not tiles.is_dir():
        return False
    # Muestra mínima: el set local típico tiene cientos de house_s*.png
    return any(tiles.glob("house_s*.png"))


def main(argv: list[str] | None = None) -> int:
    repo = Path(__file__).resolve().parents[1]
    default_upstream = repo / "reference" / "openttd-upstream" / "src" / "table" / "town_land.h"
    out_path = (
        repo / "crates" / "openttdrs-client" / "src" / "sprites" / "house_draw_data_generated.rs"
    )

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "upstream",
        nargs="?",
        type=Path,
        default=default_upstream,
        help="ruta a town_land.h",
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="genera en memoria y compara (no escribe); requiere PNG OpenGFX locales",
    )
    args = parser.parse_args(argv)

    if not args.upstream.is_file():
        print(f"Falta {args.upstream}. Ejecutá scripts/fetch-openttd-reference.sh", file=sys.stderr)
        return 1

    if args.check and not assets_available(repo):
        print(
            "SKIP: --check de house_draw_data requiere assets/opengfx/tiles/house_s*.png "
            "(no vendorizados; ver docs/PARIDAD.md)",
            file=sys.stderr,
        )
        return 2

    content, nfo_cal, macro_cal, fallback_cal, missing = build_content(repo, args.upstream)

    if args.check:
        current = out_path.read_text(encoding="utf-8")
        if current != content:
            print(
                "DRIFT: house_draw_data_generated.rs no coincide con el generador.",
                file=sys.stderr,
            )
            print(
                "  Regenerá con: python3 scripts/gen_house_draw_data.py",
                file=sys.stderr,
            )
            return 1
        print(
            f"OK: {out_path.relative_to(repo)} coincide "
            f"(nfo={nfo_cal}, macro={macro_cal}, fallback={fallback_cal})"
        )
        return 0

    out_path.write_text(content, encoding="utf-8")
    print(
        f"Escrito {out_path} ({ROWS} filas, nfo={nfo_cal}, macro={macro_cal}, fallback={fallback_cal})"
    )
    if missing:
        print(f"PNG ausentes ({len(missing)}): {missing[:20]}{'...' if len(missing) > 20 else ''}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
