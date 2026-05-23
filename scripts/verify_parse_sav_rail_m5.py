#!/usr/bin/env python3
"""
Comprueba que ``parse_sav`` conserva el plano ``m5`` en teselas ``MP_RAILWAY``.

Valida roundtrip .sav → ``export_ottdmap_from_chunks`` y el fixture ``sp3_slope_lab``
(HORZ/VERT plano y en pendiente) usado como referencia cuando no hay .sav con doble vía.

Uso (desde la raíz del repo):

  python3 scripts/verify_parse_sav_rail_m5.py
  python3 scripts/verify_parse_sav_rail_m5.py tests/fixtures/stationlist-test.sav
"""

from __future__ import annotations

import importlib.util
import sys
from pathlib import Path

# ``MAPT`` nibble alto: ``TileType::Railway`` == 1 (`tile_type.h`).
MP_RAILWAY_TT = 1
RAIL_TB_HORZ = 0x0C
RAIL_TB_VERT = 0x30


def _load_parse_sav(repo_root: Path):
    script = repo_root / "scripts" / "parse_sav.py"
    spec = importlib.util.spec_from_file_location("parse_sav", script)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"No se pudo cargar {script}")
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def _rail_m5_mismatches(
    mapt: bytes, src_m5: bytes, out_m5: bytes, dim_x: int, dim_y: int, limit: int = 8
) -> list[str]:
    n = dim_x * dim_y
    errs: list[str] = []
    for i in range(n):
        if (mapt[i] >> 4) & 0xF != MP_RAILWAY_TT:
            continue
        if src_m5[i] != out_m5[i]:
            x, y = i % dim_x, i // dim_x
            tb_s = src_m5[i] & 0x3F
            tb_o = out_m5[i] & 0x3F
            errs.append(
                f"({x},{y}) m5 save=0x{src_m5[i]:02x} ottdmap=0x{out_m5[i]:02x} "
                f"trackbits save=0x{tb_s:02x} ottdmap=0x{tb_o:02x}"
            )
            if len(errs) >= limit:
                break
    return errs


def verify_sav_roundtrip(parse_sav, sav_path: Path) -> list[str]:
    raw = sav_path.read_bytes()
    data, version = parse_sav.decompress(raw)
    chunks = parse_sav.parse_chunks(data)
    dim_x, dim_y = parse_sav.dimensions_from_chunks(chunks)
    expected = dim_x * dim_y
    mapt = chunks["MAPT"][:expected]
    src_m5 = chunks.get("MAP5", b"")[:expected].ljust(expected, b"\x00")

    body = parse_sav.export_ottdmap_from_chunks(chunks, version)
    _, _, out_m5 = parse_sav.ottdmap_dense_m5_plane(body)

    mism = _rail_m5_mismatches(mapt, src_m5, out_m5, dim_x, dim_y)
    if mism:
        return [f"{sav_path.name}: {e}" for e in mism]
    return []


def _tile_m5(parse_sav, data: bytes, x: int, y: int) -> tuple[int, int, int]:
    dim_x, dim_y, m5 = parse_sav.ottdmap_dense_m5_plane(data)
    if not (0 <= x < dim_x and 0 <= y < dim_y):
        raise IndexError(f"({x},{y}) fuera de {dim_x}×{dim_y}")
    n = dim_x * dim_y
    mapt = data[16 : 16 + n]
    idx = y * dim_x + x
    tt = (mapt[idx] >> 4) & 0xF
    return tt, m5[idx], m5[idx] & 0x3F


def verify_sp3_slope_lab(parse_sav, fixture: Path) -> list[str]:
    data = fixture.read_bytes()
    errs: list[str] = []

    cases: list[tuple[int, int, int, str]] = [
        (13, 1, RAIL_TB_HORZ, "HORZ plano"),
        (15, 1, RAIL_TB_VERT, "VERT plano"),
        (1, 16, RAIL_TB_HORZ, "HORZ pendiente NE"),
        (1, 18, RAIL_TB_VERT, "VERT pendiente NE"),
    ]
    for x, y, want_tb, label in cases:
        tt, _m5, tb = _tile_m5(parse_sav, data, x, y)
        if tt != MP_RAILWAY_TT:
            errs.append(f"{fixture.name} ({x},{y}) {label}: tt={tt}, esperado Railway")
        elif tb != want_tb:
            errs.append(
                f"{fixture.name} ({x},{y}) {label}: trackbits=0x{tb:02x}, esperado 0x{want_tb:02x}"
            )

    dim_x, dim_y, m5 = parse_sav.ottdmap_dense_m5_plane(data)
    n = dim_x * dim_y
    mapt = data[16 : 16 + n]
    horz = vert = 0
    for i in range(n):
        if (mapt[i] >> 4) & 0xF != MP_RAILWAY_TT:
            continue
        tb = m5[i] & 0x3F
        if tb == RAIL_TB_HORZ:
            horz += 1
        elif tb == RAIL_TB_VERT:
            vert += 1
    if horz < 5:
        errs.append(f"{fixture.name}: HORZ tiles={horz}, esperado ≥5")
    if vert < 5:
        errs.append(f"{fixture.name}: VERT tiles={vert}, esperado ≥5")
    return errs


def rail_trackbit_histogram(parse_sav, sav_path: Path) -> dict[int, int]:
    raw = sav_path.read_bytes()
    data, _ver = parse_sav.decompress(raw)
    chunks = parse_sav.parse_chunks(data)
    dim_x, dim_y = parse_sav.dimensions_from_chunks(chunks)
    n = dim_x * dim_y
    mapt = chunks["MAPT"][:n]
    map5 = chunks.get("MAP5", b"")[:n].ljust(n, b"\x00")
    counts: dict[int, int] = {}
    for i in range(n):
        if (mapt[i] >> 4) & 0xF != MP_RAILWAY_TT:
            continue
        tb = map5[i] & 0x3F
        counts[tb] = counts.get(tb, 0) + 1
    return counts


def main(argv: list[str] | None = None) -> int:
    repo = Path.cwd()
    parse_sav = _load_parse_sav(repo)
    args = list(argv or sys.argv[1:])
    errs: list[str] = []

    sav_paths = [Path(p) for p in args if Path(p).suffix == ".sav"]
    if not sav_paths:
        default = repo / "tests/fixtures/stationlist-test.sav"
        if default.is_file():
            sav_paths = [default]

    for sav in sav_paths:
        if not sav.is_file():
            errs.append(f"no existe {sav}")
            continue
        errs.extend(verify_sav_roundtrip(parse_sav, sav))
        hist = rail_trackbit_histogram(parse_sav, sav)
        total = sum(hist.values())
        horz = hist.get(RAIL_TB_HORZ, 0)
        vert = hist.get(RAIL_TB_VERT, 0)
        print(
            f"{sav.name}: {total} teselas Railway — "
            f"HORZ={horz} VERT={vert} histograma={dict(sorted(hist.items()))}"
        )

    lab = repo / "crates/openttdrs-core/tests/fixtures/sp3_slope_lab.ottdmap"
    if lab.is_file():
        errs.extend(verify_sp3_slope_lab(parse_sav, lab))
    else:
        print(f"AVISO: sin fixture {lab}", file=sys.stderr)

    if errs:
        print("verify_parse_sav_rail_m5: fallos:", file=sys.stderr)
        for e in errs:
            print(f"  {e}", file=sys.stderr)
        return 1

    print("OK: m5 de vía conservado (parse_sav → .ottdmap) + sp3_slope_lab HORZ/VERT")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
