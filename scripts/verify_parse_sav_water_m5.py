#!/usr/bin/env python3
"""
Comprueba que ``parse_sav`` conserva el plano ``m5`` en teselas ``MP_WATER``.

Compara MAP5 del .sav con el plano m5 del .ottdmap generado en memoria
(``export_ottdmap_from_chunks``). También valida el fixture SP3 con costa explícita.

Uso (desde la raíz del repo):

  python3 scripts/verify_parse_sav_water_m5.py
"""

from __future__ import annotations

import importlib.util
import struct
import sys
from pathlib import Path


def _load_parse_sav(repo_root: Path):
    script = repo_root / "scripts" / "parse_sav.py"
    spec = importlib.util.spec_from_file_location("parse_sav", script)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"No se pudo cargar {script}")
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def _water_m5_mismatches(
    mapt: bytes, src_m5: bytes, out_m5: bytes, dim_x: int, dim_y: int, limit: int = 8
) -> list[str]:
    n = dim_x * dim_y
    errs: list[str] = []
    for i in range(n):
        if (mapt[i] >> 4) & 0xF != 6:  # MP_WATER
            continue
        if src_m5[i] != out_m5[i]:
            x, y = i % dim_x, i // dim_x
            errs.append(
                f"({x},{y}) m5 save=0x{src_m5[i]:02x} ottdmap=0x{out_m5[i]:02x} "
                f"wtt={(src_m5[i] >> 4) & 0xF}"
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

    mism = _water_m5_mismatches(mapt, src_m5, out_m5, dim_x, dim_y)
    if mism:
        return [f"{sav_path.name}: {e}" for e in mism]
    return []


def verify_sp3_fixture(parse_sav, fixture: Path) -> list[str]:
    data = fixture.read_bytes()
    dim_x, dim_y, m5 = parse_sav.ottdmap_dense_m5_plane(data)
    n = dim_x * dim_y
    base = 16
    mapt = data[base : base + n]
    coast = 0
    for i in range(n):
        if (mapt[i] >> 4) & 0xF != 6:
            continue
        wtt = (m5[i] >> 4) & 0xF
        if wtt == 1:
            coast += 1
    if coast < 1:
        return [f"{fixture.name}: falta al menos una tesela Coast (m5>>4==1)"]
    return []


def main() -> int:
    repo = Path.cwd()
    parse_sav = _load_parse_sav(repo)
    errs: list[str] = []

    sav = repo / "tests/fixtures/stationlist-test.sav"
    if sav.is_file():
        errs.extend(verify_sav_roundtrip(parse_sav, sav))
    else:
        print(f"AVISO: sin fixture {sav}", file=sys.stderr)

    sp3 = repo / "crates/openttdrs-core/tests/fixtures/sp3_visual_checklist.ottdmap"
    if sp3.is_file():
        errs.extend(verify_sp3_fixture(parse_sav, sp3))
        # Costa explícita en (3,7)
        _, _, m5 = parse_sav.ottdmap_dense_m5_plane(sp3.read_bytes())
        idx = 7 * 12 + 3
        if m5[idx] != 0x10:
            errs.append(f"sp3: (3,7) m5 esperado 0x10, obtuvo 0x{m5[idx]:02x}")
    else:
        print(f"AVISO: sin fixture {sp3}", file=sys.stderr)

    if errs:
        print("verify_parse_sav_water_m5: fallos:", file=sys.stderr)
        for e in errs:
            print(f"  {e}", file=sys.stderr)
        return 1

    print("OK: m5 de agua conservado (parse_sav → .ottdmap)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
