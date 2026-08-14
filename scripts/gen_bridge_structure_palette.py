#!/usr/bin/env python3
"""Genera tablas indexadas de recolor de estructuras vanilla.

Parsea pseudo-sprites de estructura/iglesia de `ogfx1_base.nfo` y escribe las 256 entradas
fuente→destino que usa OpenTTD. El remapeo debe conservar el índice original:
dos entradas DOS pueden compartir RGB y aun así tener destinos distintos. Hacer
la conversión a RGB antes del remapeo recorta o recolorea incorrectamente la
silueta de los puentes (caso visible: cantilever rojo de Kale).

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
    ("BLUE", 795),
    ("BROWN", 796),
    ("WHITE", 797),
    ("RED", 798),
    ("GREEN", 799),
    ("CONCRETE", 800),
    ("YELLOW", 801),
    ("CHURCH_RED", 1438),
    ("CHURCH_CREAM", 1439),
]


def load_dos_palette() -> list[tuple[int, int, int]]:
    """Paleta DOS con su índice transparente cero explícito."""

    return list(dos_palette())


PSEUDO_HEADER_RE = re.compile(r"^\s*(?P<id>\d+)\s+\*\s+(?P<size>\d+)\b")
NORMAL_SPRITE_RE = re.compile(r"^\s*\d+\s+\S+?\.(?:png|pcx)\b")


def _decode_nfo_bytes(blob: str, expected: int) -> tuple[int, ...]:
    """Decodifica bytes hex/literales de una pseudo-sprite sin perder Action0.

    El primer byte de las pseudo-sprites de paleta es la acción ``00``;
    OpenTTD consume los 256 bytes posteriores como la tabla de sustitución.
    """

    data: list[int] = []
    index = 0
    while index < len(blob) and len(data) < expected:
        while index < len(blob) and blob[index].isspace():
            index += 1
        if index >= len(blob):
            break
        if blob[index] == '"':
            index += 1
            while index < len(blob):
                char = blob[index]
                if char == "\\" and index + 1 < len(blob):
                    data.append(ord(blob[index + 1]))
                    index += 2
                elif char == '"':
                    index += 1
                    break
                else:
                    data.append(ord(char))
                    index += 1
                if len(data) == expected:
                    break
            continue
        pair = blob[index : index + 2]
        if len(pair) == 2 and all(char in "0123456789abcdefABCDEF" for char in pair):
            data.append(int(pair, 16))
            index += 2
        else:
            index += 1
    if len(data) != expected:
        raise ValueError(f"pseudo-sprite incompleta: {len(data)}/{expected} bytes")
    return tuple(data)


def parse_recolour_table(lines: list[str], sprite_id: int) -> tuple[int, ...]:
    """Devuelve exactamente la tabla índice→índice de un pseudo-sprite 257."""

    start = next(
        (
            index
            for index, line in enumerate(lines)
            if (match := PSEUDO_HEADER_RE.match(line))
            and int(match.group("id")) == sprite_id
            and int(match.group("size")) == 257
        ),
        None,
    )
    if start is None:
        raise ValueError(f"no encontré pseudo-sprite {sprite_id} (*257)")
    chunk = [lines[start]]
    for line in lines[start + 1 :]:
        if PSEUDO_HEADER_RE.match(line) or NORMAL_SPRITE_RE.match(line):
            break
        chunk.append(line)
    header = PSEUDO_HEADER_RE.match(chunk[0])
    assert header is not None
    raw = _decode_nfo_bytes(
        chunk[0][header.end() :] + "\n" + "\n".join(chunk[1:]), 257
    )
    if raw[0] != 0:
        raise ValueError(f"pseudo-sprite {sprite_id} no empieza con acción 00")
    return raw[1:]


def rgb_remap(
    palette: list[tuple[int, int, int]], table: tuple[int, ...]
) -> list[tuple[tuple[int, int, int], tuple[int, int, int]]]:
    """Convierte índice→índice a las parejas RGB de los tiles extraídos.

    Los PNG de tiles ya fueron convertidos a RGBA con la paleta DOS. La tabla
    NFO debe, no obstante, leerse con el desplazamiento correcto: el primer
    byte de una pseudo-sprite es la Action0 y *no* la entrada cero del remapeo.
    """

    pairs: list[tuple[tuple[int, int, int], tuple[int, int, int]]] = []
    seen: set[tuple[int, int, int]] = set()
    for source_index, destination_index in enumerate(table):
        if destination_index == source_index:
            continue
        source = palette[source_index]
        if source in seen:
            continue
        seen.add(source)
        pairs.append((source, palette[destination_index]))
    return pairs


def build_output() -> str:
    nfo = find_ogfx1_base_nfo()
    if nfo is None:
        raise FileNotFoundError(
            "Falta ogfx1_base.nfo 8bpp para paletas de puente "
            "(assets/opengfx/.signal-src-8bpp/sprites/ o opengfx-*/sprites/). "
            "Ejecutá ./scripts/descargar_graficos.sh --32bpp (o --8bpp)."
        )
    nfo_lines = nfo.read_bytes().decode("latin-1").splitlines()
    palette = load_dos_palette()

    lines = [
        "// Generado por scripts/gen_bridge_structure_palette.py — NO EDITAR A MANO.",
        "// Remapeos RGB de estructura/iglesia de OpenTTD (pseudo-sprites 795–801, 1438–1439).",
        "",
    ]
    for name, sid in PALETTE_IDS:
        table = parse_recolour_table(nfo_lines, sid)
        pairs = rgb_remap(palette, table)
        lines.append(f"/// `{name}` (sprite recolor {sid}).")
        lines.append(
            f"pub static STRUCT_REMAP_{name}: [([u8; 3], [u8; 3]); {len(pairs)}] = ["
        )
        for source, destination in pairs:
            lines.append(
                f"    ([{source[0]}, {source[1]}, {source[2]}], "
                f"[{destination[0]}, {destination[1]}, {destination[2]}]),"
            )
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
