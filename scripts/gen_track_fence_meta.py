#!/usr/bin/env python3
"""Genera anclas NFO para `SPR_TRACK_FENCE_*` del perfil gráfico activo.

Los ocho PNG se recortan al descargar OpenGFX; su posición isométrica no se
puede recuperar de una caja fija porque las variantes verticales y de pendiente
tienen dimensiones/anclas distintas. El NFO activo conserva ese contrato tanto
en OpenGFX 8bpp como en OpenGFX2 32bpp.

Uso:
  python3 scripts/gen_track_fence_meta.py
  python3 scripts/gen_track_fence_meta.py --check
"""
from __future__ import annotations

import argparse
import sys
from pathlib import Path

from PIL import Image

from nfo_sprite_meta import active_global_sprite_nfo, detect_graphics_mode, parse_global_sprite_rects

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "crates/openttdrs-client/src/sprites/track_fence_meta_generated.rs"
SPRITE_IDS = tuple(range(1301, 1309))
Meta = tuple[int, int, int, int]


def collect(repo: Path) -> list[Meta]:
    """Devuelve `(width, height, xrel, yrel)` del baseset que está activo."""
    mode = detect_graphics_mode(repo)
    if mode is None:
        raise SystemExit("No se pudo detectar assets/opengfx/.graphics_mode")
    nfo = active_global_sprite_nfo(repo, mode)
    if nfo is None:
        raise SystemExit(f"No se encontró NFO global para el perfil {mode}")
    rects = parse_global_sprite_rects(nfo, mode)
    tiles = repo / "assets/opengfx/tiles"
    metadata: list[Meta] = []
    for index, sprite_id in enumerate(SPRITE_IDS):
        rect = rects.get(sprite_id)
        if rect is None:
            raise SystemExit(f"NFO sin sprite global {sprite_id}")
        png = tiles / f"track_fence_{index}.png"
        if not png.is_file():
            raise SystemExit(f"Falta {png}")
        with Image.open(png) as image:
            if image.size != (rect.w, rect.h):
                raise SystemExit(
                    f"{png.name}: PNG {image.size} != NFO {(rect.w, rect.h)} ({mode})"
                )
        metadata.append((rect.w, rect.h, rect.xrel, rect.yrel))
    return metadata


def render(metadata: list[Meta]) -> str:
    lines = [
        "// GENERADO por scripts/gen_track_fence_meta.py — NO EDITAR A MANO.\n",
        "#![cfg_attr(rustfmt, rustfmt_skip)]\n\n",
        "/// `(width, height, xrel, yrel)` de `SPR_TRACK_FENCE_FLAT_X..SLOPE_NW`.\n",
        "pub(crate) static TRACK_FENCE_SPRITE_META: &[(i16, i16, i16, i16)] = &[\n",
    ]
    for width, height, xrel, yrel in metadata:
        lines.append(f"    ({width}, {height}, {xrel}, {yrel}),\n")
    lines.append("];\n")
    return "".join(lines)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args(argv)

    generated = render(collect(ROOT))
    if args.check:
        if not OUT.is_file() or OUT.read_text(encoding="utf-8") != generated:
            print(f"DRIFT: regenerá {OUT.relative_to(ROOT)}", file=sys.stderr)
            return 1
        print(f"OK: {len(SPRITE_IDS)} metadatos de cercas validados contra NFO/PNG")
        return 0
    OUT.write_text(generated, encoding="utf-8")
    print(f"Generados {len(SPRITE_IDS)} metadatos de cercas en {OUT.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
