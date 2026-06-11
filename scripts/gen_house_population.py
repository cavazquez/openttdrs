#!/usr/bin/env python3
"""Genera la tabla de población por HouseID desde `table/town_land.h`.

OpenTTD **no guarda** la población de las ciudades en el save: la reconstruye
al cargar (`RebuildTownCaches`, `town_sl.cpp`) sumando
`HouseSpec::population` de cada tesela MP_HOUSE completada. La población por
HouseID es el 3er argumento de las macros `MS(...)` de
`_original_house_specs` (`table/town_land.h`).

Salida: `crates/openttdrs-core/src/sav/house_population_generated.rs`.

Uso: python3 scripts/gen_house_population.py
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
TOWN_LAND_H = REPO / "reference" / "openttd-upstream" / "src" / "table" / "town_land.h"
OUT_RS = REPO / "crates/openttdrs-core/src/sav/house_population_generated.rs"


def main() -> None:
    text = TOWN_LAND_H.read_text(encoding="utf-8")
    m = re.search(r"_original_house_specs\[\] = \{(.*)\};", text, re.S)
    if not m:
        sys.exit("no se encontró _original_house_specs")
    entries = re.findall(r"\bMS\(\s*(-?\d+)\s*,[^,]+,\s*(\d+)\s*,", m.group(1))
    if len(entries) < 100:
        sys.exit(f"esperaba ~110 entradas MS, hay {len(entries)}")
    pops = [int(p) for _, p in entries]

    lines = [
        "// Generado por scripts/gen_house_population.py — NO EDITAR A MANO.",
        "//",
        "// `HouseSpec::population` por HouseID original (3er argumento de las",
        "// macros `MS` en `_original_house_specs`, `table/town_land.h`).",
        "// Usado para reconstruir `Town::cache.population` como",
        "// `RebuildTownCaches` (`town_sl.cpp`).",
        "",
        f"pub(crate) static HOUSE_POPULATION: [u16; {len(pops)}] = [",
    ]
    for start in range(0, len(pops), 10):
        chunk = ", ".join(str(p) for p in pops[start : start + 10])
        lines.append(f"    {chunk}, // {start}..{min(start + 10, len(pops)) - 1}")
    lines += ["];", ""]
    OUT_RS.write_text("\n".join(lines), encoding="utf-8")
    print(f"Escrito {OUT_RS.relative_to(REPO)} ({len(pops)} HouseIDs)")


if __name__ == "__main__":
    main()
