#!/usr/bin/env python3
"""Genera tablas de HouseSpec desde `table/town_land.h`.

OpenTTD **no guarda** la población de las ciudades en el save: la reconstruye
al cargar (`RebuildTownCaches`, `town_sl.cpp`) sumando
`HouseSpec::population` de cada tesela MP_HOUSE completada. La población por
HouseID es el 3er argumento de las macros `MS(...)` de
`_original_house_specs` (`table/town_land.h`).

También emite tablas runtime para P3.5/P3.6/P3.7:
`HOUSE_MAIL_GENERATION`, `HOUSE_SIZE_1X1`, años, zonas, flags, aceptación,
`minimum_life` y `probability`.

Salida: `crates/openttdrs-core/src/sav/house_population_generated.rs`.

Uso:
  python3 scripts/gen_house_population.py
  python3 scripts/gen_house_population.py --check
"""
from __future__ import annotations

import argparse
import re
import sys
from dataclasses import dataclass
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
TOWN_LAND_H = REPO / "reference" / "openttd-upstream" / "src" / "table" / "town_land.h"
OUT_RS = REPO / "crates" / "openttdrs-core" / "src" / "sav" / "house_population_generated.rs"

MAX_YEAR = 5_000_000

ZONE_BITS = {
    "TownEdge": 0,
    "TownOutskirt": 1,
    "TownOuterSuburb": 2,
    "TownInnerSuburb": 3,
    "TownCentre": 4,
    "ClimateSubarcticAboveSnow": 11,
    "ClimateTemperate": 12,
    "ClimateSubarcticBelowSnow": 13,
    "ClimateSubtropic": 14,
    "ClimateToyland": 15,
}

FLAG_BITS = {
    "Size1x1": 0,
    "NotSloped": 1,
    "Size2x1": 2,
    "Size1x2": 3,
    "Size2x2": 4,
    "IsAnimated": 5,
    "IsChurch": 6,
    "IsStadium": 7,
}

# Índices estables para el runtime Rust (`HouseAcceptCargo`).
# Toyland candy/fizzy → Goods (mismo slot de mercancías urbanas).
CARGO_IDX = {
    "CT_PASSENGERS": 0,
    "CT_MAIL": 1,
    "CT_GOODS": 2,
    "CT_FOOD": 3,
    "CT_WATER": 4,
    "CT_CANDY": 2,
    "CT_FIZZY_DRINKS": 2,
}


@dataclass
class Spec:
    min_year: int
    max_year: int
    population: int
    mail: int
    size_1x1: bool
    availability: int
    building_flags: int
    cargo_acceptance: tuple[int, int, int]
    accepts_cargo: tuple[int, int, int]
    probability: int
    minimum_life: int


def parse_year(tok: str) -> int:
    tok = tok.strip()
    if tok == "CalendarTime::MAX_YEAR":
        return MAX_YEAR
    return int(tok)


def parse_flags(block: str) -> int:
    bits = 0
    for name, bit in FLAG_BITS.items():
        if re.search(rf"BuildingFlag::{name}\b", block):
            bits |= 1 << bit
    return bits


def parse_zones(block: str) -> int:
    bits = 0
    for name, bit in ZONE_BITS.items():
        if re.search(rf"HouseZone::{name}\b", block):
            bits |= 1 << bit
    return bits


def parse_cargo_triple(tail: str) -> tuple[int, int, int]:
    # El cierre de MS es `CT_A, CT_B, CT_C), // nn`.
    m = re.search(
        r"(CT_\w+)\s*,\s*(CT_\w+)\s*,\s*(CT_\w+)\s*\)\s*,",
        tail,
    )
    if not m:
        raise SystemExit(f"cargos no encontrados: {tail[-120]!r}")
    out = []
    for g in m.groups():
        if g not in CARGO_IDX:
            raise SystemExit(f"cargo desconocido {g}")
        out.append(CARGO_IDX[g])
    return out[0], out[1], out[2]


def parse_specs(town_land: Path) -> list[Spec]:
    text = town_land.read_text(encoding="utf-8")
    m = re.search(r"_original_house_specs\[\] = \{(.*)\};", text, re.S)
    if not m:
        raise SystemExit("no se encontró _original_house_specs")
    body = m.group(1)
    parts = re.split(r"\bMS\(", body)[1:]
    if len(parts) < 100:
        raise SystemExit(f"esperaba ~110 entradas MS, hay {len(parts)}")

    specs: list[Spec] = []
    for p in parts:
        am = re.match(
            r"\s*(-?\d+|CalendarTime::MAX_YEAR)\s*,\s*"
            r"(CalendarTime::MAX_YEAR|\d+)\s*,\s*"
            r"(\d+)\s*,",
            p,
        )
        if not am:
            raise SystemExit(f"MS mal formado: {p[:80]!r}")
        mm = re.search(r"STR_\w+,\s*\d+,\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*,", p)
        if not mm:
            raise SystemExit(f"MS sin mail/acceptance: {p[:100]!r}")
        min_year = parse_year(am.group(1))
        max_year = parse_year(am.group(2))
        population = int(am.group(3))
        mail = int(mm.group(1))
        ca1, ca2, ca3 = int(mm.group(2)), int(mm.group(3)), int(mm.group(4))
        # El bloque de flags/zonas va entre la aceptación y los CT_*.
        head = p[:800]
        flags = parse_flags(head)
        zones = parse_zones(head)
        size_1x1 = bool(flags & (1 << FLAG_BITS["Size1x1"])) and not (
            flags
            & (
                (1 << FLAG_BITS["Size2x1"])
                | (1 << FLAG_BITS["Size1x2"])
                | (1 << FLAG_BITS["Size2x2"])
            )
        )
        accepts = parse_cargo_triple(p)
        # Defaults del macro MS: probability=16, minimum_life=0.
        specs.append(
            Spec(
                min_year=min_year,
                max_year=max_year,
                population=population,
                mail=mail,
                size_1x1=size_1x1,
                availability=zones,
                building_flags=flags,
                cargo_acceptance=(ca1, ca2, ca3),
                accepts_cargo=accepts,
                probability=16,
                minimum_life=0,
            )
        )
    return specs


def fmt_bool_row(vals: list[bool], start: int) -> str:
    chunk = ", ".join("true" if v else "false" for v in vals)
    end = start + len(vals) - 1
    return f"    {chunk}, // {start}..{end}"


def fmt_u(v: int) -> str:
    """Formato Rust con separadores de miles para literales largos (clippy)."""
    if abs(v) >= 1_000:
        return f"{v:_}"
    return str(v)


def fmt_u_row(vals: list[int], start: int) -> str:
    chunk = ", ".join(fmt_u(v) for v in vals)
    end = start + len(vals) - 1
    return f"    {chunk}, // {start}..{end}"


def build_content(town_land: Path) -> str:
    specs = parse_specs(town_land)
    n = len(specs)
    lines = [
        "// Generado por scripts/gen_house_population.py — NO EDITAR A MANO.",
        "//",
        "// `HouseSpec` runtime desde macros `MS` en `_original_house_specs`",
        "// (`table/town_land.h`). Usado por RebuildTownCaches, TryBuildTownHouse,",
        "// AddAcceptedCargo_Town y renovación urbana (P3.5–P3.7).",
        "",
        f"pub(crate) const HOUSE_SPEC_COUNT: usize = {n};",
        f"pub(crate) const HOUSE_MAX_YEAR: u32 = {MAX_YEAR:_};",
        "",
        f"pub(crate) static HOUSE_POPULATION: [u16; {n}] = [",
    ]
    pops = [s.population for s in specs]
    mails = [s.mail for s in specs]
    size_1x1 = [s.size_1x1 for s in specs]
    for start in range(0, n, 10):
        lines.append(fmt_u_row(pops[start : start + 10], start))
    lines += ["];", "", f"pub(crate) static HOUSE_MAIL_GENERATION: [u16; {n}] = ["]
    for start in range(0, n, 10):
        lines.append(fmt_u_row(mails[start : start + 10], start))
    lines += ["];", "", f"pub(crate) static HOUSE_SIZE_1X1: [bool; {n}] = ["]
    for start in range(0, n, 10):
        lines.append(fmt_bool_row(size_1x1[start : start + 10], start))
    lines += [
        "];",
        "",
        f"pub(crate) static HOUSE_MIN_YEAR: [u32; {n}] = [",
    ]
    for start in range(0, n, 10):
        lines.append(fmt_u_row([s.min_year for s in specs][start : start + 10], start))
    lines += ["];", "", f"pub(crate) static HOUSE_MAX_YEAR_OF: [u32; {n}] = ["]
    for start in range(0, n, 10):
        lines.append(fmt_u_row([s.max_year for s in specs][start : start + 10], start))
    lines += ["];", "", f"pub(crate) static HOUSE_AVAILABILITY: [u16; {n}] = ["]
    for start in range(0, n, 10):
        chunk = ", ".join(f"0x{s.availability:04x}" for s in specs[start : start + 10])
        end = start + len(specs[start : start + 10]) - 1
        lines.append(f"    {chunk}, // {start}..{end}")
    lines += ["];", "", f"pub(crate) static HOUSE_BUILDING_FLAGS: [u8; {n}] = ["]
    for start in range(0, n, 10):
        chunk = ", ".join(f"0x{s.building_flags:02x}" for s in specs[start : start + 10])
        end = start + len(specs[start : start + 10]) - 1
        lines.append(f"    {chunk}, // {start}..{end}")
    lines += ["];", "", f"pub(crate) static HOUSE_PROBABILITY: [u8; {n}] = ["]
    for start in range(0, n, 10):
        lines.append(fmt_u_row([s.probability for s in specs][start : start + 10], start))
    lines += ["];", "", f"pub(crate) static HOUSE_MINIMUM_LIFE: [u8; {n}] = ["]
    for start in range(0, n, 10):
        lines.append(fmt_u_row([s.minimum_life for s in specs][start : start + 10], start))
    lines += [
        "];",
        "",
        f"pub(crate) static HOUSE_CARGO_ACCEPTANCE: [[u8; 3]; {n}] = [",
    ]
    for i, s in enumerate(specs):
        a, b, c = s.cargo_acceptance
        lines.append(f"    [{a}, {b}, {c}], // {i}")
    lines += [
        "];",
        "",
        f"pub(crate) static HOUSE_ACCEPTS_CARGO: [[u8; 3]; {n}] = [",
    ]
    for i, s in enumerate(specs):
        a, b, c = s.accepts_cargo
        lines.append(f"    [{a}, {b}, {c}], // {i}")
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
            return 1
        print(f"OK: {OUT_RS.relative_to(REPO)} coincide con el generador")
        return 0

    args.output.write_text(content, encoding="utf-8")
    print(f"Escrito {args.output.relative_to(REPO)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
