#!/usr/bin/env python3
"""Genera tablas RGB de rampas de compañía OpenTTD para el cliente.

Fuente: `table/palettes.h` (paleta DOS) + índices de
https://grf.farm/misc/company_colour_indexes.html

Salida: `crates/openttdrs-client/src/sprites/company_palette_data_generated.rs`

Uso: python3 scripts/gen_company_palette_rust.py
"""
from __future__ import annotations

import re
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
PALETTES_H = REPO / "third_party" / "openttd" / "table" / "palettes.h"
OUT_RS = REPO / "crates/openttdrs-client/src/sprites/company_palette_data_generated.rs"

COMPANY_RAMP_INDICES: list[list[int]] = [
    [0xC6, 0xC7, 0xC8, 0xC9, 0xCA, 0xCB, 0xCC, 0xCD],
    [0x60, 0x61, 0x62, 0x63, 0x64, 0x65, 0x66, 0x67],
    [0x2A, 0x2B, 0x2C, 0x2D, 0x2E, 0x2F, 0x30, 0x31],
    [0x3E, 0x3F, 0x40, 0x41, 0x42, 0x43, 0x44, 0x45],
    [0xB3, 0xB4, 0xB5, 0xB6, 0xB7, 0xA4, 0xA5, 0xA6],
    [0x9A, 0x9B, 0x9C, 0x9D, 0x9E, 0x9F, 0xA0, 0xA1],
    [0x52, 0x53, 0x54, 0x55, 0xCE, 0xCF, 0xD0, 0xD1],
    [0x58, 0x59, 0x5A, 0x5B, 0x5C, 0x5D, 0x5E, 0x5F],
    [0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99],
    [0x72, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79],
    [0x80, 0x81, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87],
    [0x88, 0x89, 0x8A, 0x8B, 0x8C, 0x8D, 0x8E, 0x8F],
    [0x40, 0xC0, 0xC1, 0xC2, 0xC3, 0xC4, 0xC5, 0x27],
    [0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27],
    [0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B],
    [0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F],
]

NAMES = [
    "DARK_BLUE",
    "PALE_GREEN",
    "PINK",
    "YELLOW",
    "RED",
    "LIGHT_BLUE",
    "GREEN",
    "DARK_GREEN",
    "BLUE",
    "CREAM",
    "MAUVE",
    "PURPLE",
    "ORANGE",
    "BROWN",
    "GREY",
    "WHITE",
]


def load_dos_palette() -> list[tuple[int, int, int]]:
    if PALETTES_H.is_file():
        text = PALETTES_H.read_text(encoding="utf-8", errors="replace")
    else:
        import urllib.request

        url = "https://raw.githubusercontent.com/OpenTTD/OpenTTD/master/src/table/palettes.h"
        text = urllib.request.urlopen(url, timeout=30).read().decode("utf-8", errors="replace")
    start = text.index("static const Palette _palette")
    end = text.index("};", start)
    block = text[start:end]
    colours = [
        tuple(map(int, m)) for m in re.findall(r"M\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*\)", block)
    ]
    if len(colours) < 206:
        raise SystemExit(f"paleta DOS incompleta ({len(colours)} entradas)")
    return colours


def main() -> None:
    dos = load_dos_palette()
    ramps = [[dos[i] for i in idxs] for idxs in COMPANY_RAMP_INDICES]

    lines = [
        "// Generado por scripts/gen_company_palette_rust.py — NO EDITAR A MANO.",
        "// Rampas de 8 tonos por `Colours` (PALETTE_CC_* / grf.farm).",
        "",
        "/// RGB de cada rampa `[colour][shade]`.",
        f"pub static COMPANY_RAMP_RGB: [[u8; 3]; 16 * 8] = [",
    ]
    for ci, name in enumerate(NAMES):
        for si, (r, g, b) in enumerate(ramps[ci]):
            lines.append(f"    [{r}, {g}, {b}], // {name}[{si}]")
    lines += [
        "];",
        "",
        "pub const COMPANY_RAMP_SHADES: usize = 8;",
        "pub const COMPANY_COLOUR_COUNT: usize = 16;",
        "",
    ]
    OUT_RS.write_text("\n".join(lines), encoding="utf-8")
    print(f"Escrito {OUT_RS.relative_to(REPO)}")


if __name__ == "__main__":
    main()
