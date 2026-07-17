#!/usr/bin/env python3
"""Genera HOUSE_DRAW_DATA desde OpenTTD `table/town_land.h`.

OpenTTD: `_town_draw_tile_data[house_id * 16 + TileHash2Bit * 4 + stage]`.
110 casas originales (HouseID 0..109) → 1760 filas.

Offsets w/h/xrel/yrel: NFO + PNG `house_s{id}.png` por capa s1/s2.

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
GRASS_GROUND = {0, 0xF54, 3924, 3981}
FALLBACK = (64.0, 48.0, -32.0, -32.0)


def parse_sprite_constants(repo: Path) -> dict[str, int]:
    """`SPR_*` → id desde OpenTTD `table/sprites.h`."""
    path = repo / "reference" / "openttd-upstream" / "src" / "table" / "sprites.h"
    if not path.is_file():
        return {}
    pat = re.compile(r"static const SpriteID\s+(SPR_[A-Z0-9_]+)\s*=\s*(\d+)")
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


def parse_macro_rows(path: Path, spr: dict[str, int]) -> list[tuple[str, str, int, int, int, int]]:
    pat = re.compile(
        r"^\s*M\(\s*([^,]+),\s*[^,]+,\s*([^,]+),\s*[^,]+,\s*"
        r"(-?\d+),\s*(-?\d+),\s*(-?\d+),\s*(-?\d+),"
    )
    out: list[tuple[str, str, int, int, int, int]] = []
    for line in path.read_text(encoding="utf-8").splitlines():
        m = pat.match(line)
        if not m:
            continue
        try:
            out.append(
                (
                    m.group(1).strip(),
                    m.group(2).strip(),
                    int(m.group(3)),
                    int(m.group(4)),
                    int(m.group(5)),
                    int(m.group(6)),
                )
            )
        except ValueError:
            continue
    return out


def map_ground(s1: int) -> int:
    return 0 if s1 in GRASS_GROUND else s1


def spec_line(
    s1: int,
    s1_dims: tuple[float, float, float, float],
    s2: int,
    s2_dims: tuple[float, float, float, float],
) -> str:
    sw, sh, sx, sy = s1_dims
    bw, bh, bx, by = s2_dims
    return (
        f"    HouseDrawSpec {{ s1: {s1}, s1_w: {sw:.1f}, s1_h: {sh:.1f}, "
        f"s1_xrel: {sx:.1f}, s1_yrel: {sy:.1f}, s2: {s2}, s2_w: {bw:.1f}, "
        f"s2_h: {bh:.1f}, s2_xrel: {bx:.1f}, s2_yrel: {by:.1f} }},"
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

    for s1_raw, s2_raw, dx, dy, sx, sy in rows_macro[:ROWS]:
        s1 = map_ground(parse_atom(s1_raw, spr))
        s2 = parse_atom(s2_raw, spr)
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

        body_rows.append(spec_line(s1, s1_dims, s2, s2_dims))

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
            "(no vendorizados; ver docs/parity/GENERATED_TABLES.md)",
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
