#!/usr/bin/env python3
"""Genera tablas RGB de recolor de puentes (PALETTE_TO_STRUCT_* en OpenTTD).

Parsea pseudo-sprites 795–801 de `ogfx1_base.nfo` y escribe mapas fuente→destino
para el remapeo en runtime (`bridge_structure_palette.rs`).

Uso: python3 scripts/gen_bridge_structure_palette.py
"""
from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

from opengfx_palette import dos_palette

REPO = Path(__file__).resolve().parents[1]
OUT_RS = (
    REPO / "crates/openttdrs-client/src/sprites/bridge_structure_palette_data_generated.rs"
)


def find_ogfx1_base_nfo() -> Path | None:
    """NFO clásico 8bpp (pseudo-sprites PALETTE_TO_STRUCT_*).

    En --32bpp vive en `.signal-src-8bpp/` (lo prepara descargar_graficos.sh).
    En --8bpp, en `opengfx-*/sprites/`.
    """
    opengfx = REPO / "assets" / "opengfx"
    candidates = [
        opengfx / ".signal-src-8bpp" / "sprites" / "ogfx1_base.nfo",
        opengfx
        / ".signal-src-8bpp"
        / "extract"
        / "opengfx-8.0"
        / "sprites"
        / "ogfx1_base.nfo",
        *sorted(opengfx.glob("opengfx-*/sprites/ogfx1_base.nfo")),
    ]
    for path in candidates:
        if path.is_file():
            return path
    return None

PALETTE_IDS: list[tuple[str, int]] = [
    ("BROWN", 796),
    ("RED", 798),
    ("CONCRETE", 800),
    ("YELLOW", 801),
]


def load_dos_palette() -> list[tuple[int, int, int]]:
    """Paleta DOS con el índice transparente 0 explícito.

    ``palettes.h`` contiene sólo los 255 ``M(...)`` opacos. Leerlos
    directamente corría todos los índices una posición y hacía que
    ``PALETTE_TO_STRUCT_*`` recoloreara el tono vecino del que usa OpenTTD.
    """
    return list(dos_palette())


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


def build_output() -> str:
    nfo = find_ogfx1_base_nfo()
    if nfo is None:
        raise FileNotFoundError(
            "Falta ogfx1_base.nfo 8bpp para paletas de puente "
            "(assets/opengfx/.signal-src-8bpp/sprites/ o opengfx-*/sprites/). "
            "Ejecutá ./scripts/descargar_graficos.sh --32bpp (o --8bpp)."
        )
    nfo_lines = nfo.read_bytes().decode("latin-1").splitlines()
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

    return "\n".join(lines)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="falla si la tabla versionada difiere")
    args = parser.parse_args(argv)

    try:
        output = build_output()
    except FileNotFoundError as exc:
        print(exc, file=sys.stderr)
        return 2

    current = OUT_RS.read_text(encoding="utf-8") if OUT_RS.is_file() else None
    if args.check:
        if current == output:
            print(f"OK {OUT_RS.relative_to(REPO)}")
            return 0
        print(f"DRIFT {OUT_RS.relative_to(REPO)}", file=sys.stderr)
        return 1

    OUT_RS.write_text(output, encoding="utf-8")
    print(f"Escrito {OUT_RS.relative_to(REPO)} ({len(PALETTE_IDS)} tablas)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
