#!/usr/bin/env python3
"""Genera tablas RGB de recolor de puentes (PALETTE_TO_STRUCT_* en OpenTTD).

Parsea pseudo-sprites 795–801 de `ogfx1_base.nfo` y escribe mapas fuente→destino
para el remapeo en runtime (`bridge_structure_palette.rs`).

Uso: python3 scripts/gen_bridge_structure_palette.py
"""
from __future__ import annotations

import re
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
PALETTES_H = REPO / "third_party" / "openttd" / "table" / "palettes.h"
NFO = (
    REPO
    / "assets"
    / "opengfx"
    / ".signal-src-8bpp"
    / "sprites"
    / "ogfx1_base.nfo"
)
OUT_RS = (
    REPO / "crates/openttdrs-client/src/sprites/bridge_structure_palette_data_generated.rs"
)

PALETTE_IDS: list[tuple[str, int]] = [
    ("BROWN", 796),
    ("RED", 798),
    ("CONCRETE", 800),
    ("YELLOW", 801),
]


def load_dos_palette() -> list[tuple[int, int, int]]:
    text = PALETTES_H.read_text(encoding="utf-8", errors="replace")
    colours = [
        tuple(map(int, m))
        for m in re.findall(r"M\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*\)", text)
    ]
    if len(colours) < 256:
        raise SystemExit(f"paleta DOS incompleta ({len(colours)} entradas)")
    return colours


def parse_recolor_block(lines: list[str]) -> list[int]:
    table = [0] * 256
    idx = 0
    blob = " ".join(lines)
    blob = re.sub(r"^\s*\d+\s+\*\s+257\s+", "", blob)
    i = 0
    while i < len(blob) and idx < 256:
        while i < len(blob) and blob[i].isspace():
            i += 1
        if i >= len(blob):
            break
        if blob[i] == '"':
            j = i + 1
            raw: list[str] = []
            while j < len(blob):
                if blob[j] == "\\" and j + 1 < len(blob):
                    raw.append(blob[j + 1])
                    j += 2
                elif blob[j] == '"':
                    break
                else:
                    raw.append(blob[j])
                    j += 1
            for ch in raw:
                if idx >= 256:
                    break
                table[idx] = ord(ch)
                idx += 1
            i = j + 1
            continue
        m = re.match(r"([0-9A-Fa-f]{2})", blob[i:])
        if not m:
            i += 1
            continue
        table[idx] = int(m.group(1), 16)
        idx += 1
        i += len(m.group(1))
    if idx != 256:
        raise SystemExit(f"tabla recolor incompleta ({idx}/256)")
    return table


def extract_sprite(nfo_lines: list[str], sid: int) -> list[int]:
    start = next(
        i for i, line in enumerate(nfo_lines) if re.match(rf"^\s*{sid}\s+\*\s+257\b", line)
    )
    chunk: list[str] = []
    for j in range(start, min(start + 8, len(nfo_lines))):
        if j > start and re.match(r"^\s*\d+\s+\*", nfo_lines[j]):
            break
        if j > start and re.match(r"^\s*\d+\s+\S+\.(?:png|pcx)", nfo_lines[j]):
            break
        chunk.append(nfo_lines[j])
    return parse_recolor_block(chunk)


def rgb_remap(pal: list[tuple[int, int, int]], table: list[int]) -> list[tuple[tuple[int, int, int], tuple[int, int, int]]]:
    out: list[tuple[tuple[int, int, int], tuple[int, int, int]]] = []
    seen: set[tuple[int, int, int]] = set()
    for src in range(256):
        dst = table[src]
        if dst == src:
            continue
        key = pal[src]
        if key in seen:
            continue
        seen.add(key)
        out.append((key, pal[dst]))
    return out


def main() -> None:
    if not NFO.is_file():
        raise SystemExit(f"Falta {NFO} — ejecutá ./scripts/descargar_graficos.sh")
    nfo_lines = NFO.read_bytes().decode("latin-1").splitlines()
    pal = load_dos_palette()

    lines = [
        "// Generado por scripts/gen_bridge_structure_palette.py — NO EDITAR A MANO.",
        "// Remapeos RGB de `PALETTE_TO_STRUCT_*` (pseudo-sprites 795–801).",
        "",
    ]
    for name, sid in PALETTE_IDS:
        table = extract_sprite(nfo_lines, sid)
        pairs = rgb_remap(pal, table)
        lines.append(f"/// `{name}` (sprite recolor {sid}).")
        lines.append(f"pub static STRUCT_REMAP_{name}: [([u8; 3], [u8; 3]); {len(pairs)}] = [")
        for src, dst in pairs:
            lines.append(f"    ([{src[0]}, {src[1]}, {src[2]}], [{dst[0]}, {dst[1]}, {dst[2]}]),")
        lines.append("];")
        lines.append("")

    OUT_RS.write_text("\n".join(lines), encoding="utf-8")
    print(f"Escrito {OUT_RS.relative_to(REPO)} ({len(PALETTE_IDS)} tablas)")


if __name__ == "__main__":
    main()
