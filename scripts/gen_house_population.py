#!/usr/bin/env python3
"""Genera tablas de HouseSpec desde `table/town_land.h`.

OpenTTD **no guarda** la población de las ciudades en el save: la reconstruye
al cargar (`RebuildTownCaches`, `town_sl.cpp`) sumando
`HouseSpec::population` de cada tesela MP_HOUSE completada. La población por
HouseID es el 3er argumento de las macros `MS(...)` de
`_original_house_specs` (`table/town_land.h`).

También emite `HOUSE_SIZE_1X1` (`BuildingFlag::Size1x1`) para el poblado
procedural (solo footprint 1×1).

Salida: `crates/openttdrs-core/src/sav/house_population_generated.rs`.

Uso:
  python3 scripts/gen_house_population.py
  python3 scripts/gen_house_population.py --check
"""
from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
TOWN_LAND_H = REPO / "reference" / "openttd-upstream" / "src" / "table" / "town_land.h"
OUT_RS = REPO / "crates" / "openttdrs-core" / "src" / "sav" / "house_population_generated.rs"


def parse_specs(town_land: Path) -> tuple[list[int], list[bool]]:
    text = town_land.read_text(encoding="utf-8")
    m = re.search(r"_original_house_specs\[\] = \{(.*)\};", text, re.S)
    if not m:
        raise SystemExit("no se encontró _original_house_specs")
    body = m.group(1)
    parts = re.split(r"\bMS\(", body)[1:]
    if len(parts) < 100:
        raise SystemExit(f"esperaba ~110 entradas MS, hay {len(parts)}")
    pops: list[int] = []
    size_1x1: list[bool] = []
    for p in parts:
        am = re.match(r"\s*(-?\d+)\s*,[^,]+,\s*(\d+)\s*,", p)
        if not am:
            raise SystemExit(f"MS mal formado: {p[:80]!r}")
        pops.append(int(am.group(2)))
        head = p[:500]
        # Size2x* / Size1x2 ganan sobre Size1x1 si aparecen juntos.
        if re.search(r"BuildingFlag::Size(?:2x1|1x2|2x2)\b", head):
            size_1x1.append(False)
        elif re.search(r"BuildingFlag::Size1x1\b", head):
            size_1x1.append(True)
        else:
            size_1x1.append(False)
    return pops, size_1x1


def fmt_bool_row(vals: list[bool], start: int) -> str:
    chunk = ", ".join("true" if v else "false" for v in vals)
    end = start + len(vals) - 1
    return f"    {chunk}, // {start}..{end}"


def build_content(town_land: Path) -> str:
    pops, size_1x1 = parse_specs(town_land)
    n = len(pops)
    lines = [
        "// Generado por scripts/gen_house_population.py — NO EDITAR A MANO.",
        "//",
        "// `HouseSpec::population` por HouseID original (3er argumento de las",
        "// macros `MS` en `_original_house_specs`, `table/town_land.h`).",
        "// Usado para reconstruir `Town::cache.population` como",
        "// `RebuildTownCaches` (`town_sl.cpp`).",
        "//",
        "// `HOUSE_SIZE_1X1`: `BuildingFlag::Size1x1` (footprint de una tesela).",
        "",
        f"pub(crate) static HOUSE_POPULATION: [u16; {n}] = [",
    ]
    for start in range(0, n, 10):
        chunk = ", ".join(str(p) for p in pops[start : start + 10])
        lines.append(f"    {chunk}, // {start}..{min(start + 10, n) - 1}")
    lines += [
        "];",
        "",
        f"pub(crate) static HOUSE_SIZE_1X1: [bool; {n}] = [",
    ]
    for start in range(0, n, 10):
        lines.append(fmt_bool_row(size_1x1[start : start + 10], start))
    lines += ["];", ""]
    return "\n".join(lines)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="genera en memoria y compara con el archivo versionado (no escribe)",
    )
    parser.add_argument(
        "--town-land",
        type=Path,
        default=TOWN_LAND_H,
        help="ruta a town_land.h (default: referencia OpenTTD pin #109)",
    )
    parser.add_argument(
        "-o",
        "--output",
        type=Path,
        default=OUT_RS,
        help="ruta de salida al regenerar (ignorado con --check)",
    )
    args = parser.parse_args(argv)

    if not args.town_land.is_file():
        # Fallback al árbol OpenTTD del monorepo (desarrollo local).
        fallback = REPO.parent / "OpenTTD" / "src" / "table" / "town_land.h"
        if fallback.is_file():
            args.town_land = fallback
        else:
            print(
                f"Falta {args.town_land}. Ejecutá: ./scripts/fetch-openttd-reference.sh",
                file=sys.stderr,
            )
            return 1

    content = build_content(args.town_land)
    if args.check:
        if not OUT_RS.is_file():
            print(f"Falta salida versionada: {OUT_RS}", file=sys.stderr)
            return 1
        current = OUT_RS.read_text(encoding="utf-8")
        if current != content:
            print(
                "DRIFT: house_population_generated.rs no coincide con el generador.",
                file=sys.stderr,
            )
            print(
                "  Regenerá con: python3 scripts/gen_house_population.py",
                file=sys.stderr,
            )
            print(
                f"  (fuente: {args.town_land}, pin OpenTTD en docs/parity/openttd-reference.json)",
                file=sys.stderr,
            )
            return 1
        print(f"OK: {OUT_RS.relative_to(REPO)} coincide con el generador")
        return 0

    args.output.write_text(content, encoding="utf-8")
    print(f"Escrito {args.output.relative_to(REPO)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
