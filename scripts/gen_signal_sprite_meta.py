#!/usr/bin/env python3
"""Genera bbox/offset de señales desde las filas NFO que producen los PNG.

La tabla resultante es la fuente de posicionamiento del cliente. Incluye el
bloque clásico/OpenGFX2 (1275..1699) y Action5 (5088..5327).

Uso:
  python3 scripts/gen_signal_sprite_meta.py
  python3 scripts/gen_signal_sprite_meta.py --check
"""
from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

from PIL import Image

from gen_rail_signal_sprites import (
    ELECTRIC_CLASSIC_ALIASES,
    SIGNAL_RANGE,
    TILES,
    detect_graphics_mode,
    find_sprite_dirs,
    merge_signal_rows,
)

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "crates/openttdrs-client/src/sprites/signal_sprite_meta_generated.rs"
ACTION5_BASE = 5088
ACTION5_COUNT = 240
ACTION5_FIRST_NFO = 2081
ACTION5_LAST_NFO_EXCLUSIVE = 2322
ROW_RE = re.compile(
    r"^\s*(\d+)\s+(\S+?\.(?:32\.png|png|pcx))\s+(?:8bpp|32bpp)\s+"
    r"(\d+)\s+(\d+)\s+(\d+)\s+(\d+)\s+(-?\d+)\s+(-?\d+)"
)

Meta = tuple[int, int, int, int]


def action5_nfo() -> Path:
    preferred = ROOT / "assets/opengfx/.signal-src-8bpp/sprites/ogfxe_extra.nfo"
    if preferred.is_file():
        return preferred
    matches = sorted((ROOT / "assets/opengfx").glob("*/sprites/ogfxe_extra.nfo"))
    if matches:
        return matches[0]
    raise SystemExit("Falta ogfxe_extra.nfo para metadatos Action5")


def collect_action5() -> dict[int, Meta]:
    rows: dict[int, Meta] = {}
    for line in action5_nfo().read_text(encoding="utf-8", errors="replace").splitlines():
        match = ROW_RE.match(line)
        if not match or match.group(2).endswith(".32.png"):
            continue
        sid = int(match.group(1))
        if ACTION5_FIRST_NFO <= sid < ACTION5_LAST_NFO_EXCLUSIVE and sid not in rows:
            rows[sid] = tuple(int(match.group(i)) for i in range(5, 9))  # type: ignore[assignment]
    source_ids = sorted(rows)
    if len(source_ids) != ACTION5_COUNT:
        raise SystemExit(
            f"Action5: se esperaban {ACTION5_COUNT} filas NFO y hay {len(source_ids)}"
        )
    return {ACTION5_BASE + index: rows[sid] for index, sid in enumerate(source_ids)}


def collect_classic() -> dict[int, Meta]:
    mode = detect_graphics_mode(ROOT) or "8bpp"
    cache: dict[str, Image.Image] = {}
    selected = merge_signal_rows(find_sprite_dirs(), mode, cache)
    result: dict[int, Meta] = {}
    for sid in SIGNAL_RANGE:
        if sid in ELECTRIC_CLASSIC_ALIASES:
            continue
        png = TILES / f"rail_{sid}.png"
        if sid not in selected or not png.is_file():
            continue
        row, _kind = selected[sid]
        _sheet, _x, _y, width, height, xrel, yrel = row
        png_size = Image.open(png).size
        if png_size != (width, height):
            raise SystemExit(
                f"{png.name}: PNG {png_size} != bbox NFO {(width, height)}"
            )
        result[sid] = (width, height, xrel, yrel)
    for target, source in ELECTRIC_CLASSIC_ALIASES.items():
        result[target] = result[source]
    return result


def validate_pngs(metadata: dict[int, Meta]) -> None:
    if sum(ACTION5_BASE <= sid < ACTION5_BASE + ACTION5_COUNT for sid in metadata) != ACTION5_COUNT:
        raise SystemExit("Cobertura Action5 incompleta")
    for sid, (width, height, _xrel, _yrel) in metadata.items():
        png = TILES / f"rail_{sid}.png"
        if not png.is_file():
            raise SystemExit(f"Falta {png}")
        if Image.open(png).size != (width, height):
            raise SystemExit(f"{png.name}: dimensiones distintas del NFO")


def render(metadata: dict[int, Meta]) -> str:
    lines = [
        "// GENERADO por scripts/gen_signal_sprite_meta.py — NO EDITAR A MANO.\n",
        "#![cfg_attr(rustfmt, rustfmt_skip)]\n\n",
        "/// `(sprite_id, width, height, xrel, yrel)` de la fila NFO elegida.\n",
        "pub(crate) static SIGNAL_SPRITE_META: &[(u32, i16, i16, i16, i16)] = &[\n",
    ]
    for sid, (width, height, xrel, yrel) in sorted(metadata.items()):
        lines.append(f"    ({sid}, {width}, {height}, {xrel}, {yrel}),\n")
    lines.append("];\n")
    return "".join(lines)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args(argv)

    metadata = collect_classic()
    metadata.update(collect_action5())
    validate_pngs(metadata)
    generated = render(metadata)
    if args.check:
        if not OUT.is_file() or OUT.read_text(encoding="utf-8") != generated:
            print(f"DRIFT: regenerá {OUT.relative_to(ROOT)}", file=sys.stderr)
            return 1
        print(f"OK: {len(metadata)} sprites de señal validados contra NFO/PNG")
        return 0
    OUT.write_text(generated, encoding="utf-8")
    print(f"Generados {len(metadata)} metadatos de señal en {OUT.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
