#!/usr/bin/env python3
"""Extrae el banco Action5 tipo 04 (presignals/PBS) como `rail_{5088..5327}.png`.

OpenTTD carga estos sprites en `SPR_SIGNALS_BASE` (5088). `DrawSingleSignal` usa
`SPR_SIGNALS_BASE - 16` (5072) como base de la fórmula; el bloque eléctrico clásico
sigue en 1275 (`SPR_ORIGINAL_SIGNALS_BASE`).

Fuente: OpenGFX 8bpp `ogfxe_extra.nfo` (sprites NFO 2081–2321, 240 reales).
El `.32.png` del mismo paquete puede estar vacío; se usa el sheet 8bpp.

Salida: `assets/opengfx/tiles/rail_{5088..5327}.png`

Uso: python3 scripts/gen_rail_signal_action5_sprites.py
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

from PIL import Image

from opengfx_palette import dematte_legacy_colorkey, indexed_dos_to_rgba

REPO = Path(__file__).resolve().parents[1]
TILES = REPO / "assets" / "opengfx" / "tiles"
SPR_SIGNALS_BASE = 5088
ACTION5_COUNT = 240
# Primer sprite real tras `05 04 FF 30 00` en ogfxe_extra.nfo
NFO_FIRST = 2081
NFO_LAST_EXCLUSIVE = 2322

ROW_RE = re.compile(
    r"^\s*(\d+)\s+(\S+?\.(?:32\.png|png|pcx))\s+(?:8bpp|32bpp)\s+"
    r"(\d+)\s+(\d+)\s+(\d+)\s+(\d+)\s+(-?\d+)\s+(-?\d+)"
)


def find_extra_nfo() -> Path:
    preferred = REPO / "assets" / "opengfx" / ".signal-src-8bpp" / "sprites" / "ogfxe_extra.nfo"
    if preferred.is_file():
        return preferred
    for nfo in sorted((REPO / "assets" / "opengfx").glob("*/sprites/ogfxe_extra.nfo")):
        return nfo
    sys.exit("No se encontró ogfxe_extra.nfo (corré descargar_graficos.sh)")


def parse_rows(nfo: Path) -> dict[int, tuple[Path, int, int, int, int]]:
    sprites_dir = nfo.parent
    rows: dict[int, tuple[Path, int, int, int, int]] = {}
    for line in nfo.read_text(errors="replace").splitlines():
        m = ROW_RE.match(line)
        if not m:
            continue
        sid = int(m.group(1))
        sheet_name = Path(m.group(2)).name
        # Preferir sheet no vacío; el .32.png del paquete 8bpp a menudo mide 0 bytes.
        if sheet_name.endswith(".32.png"):
            continue
        if sid in rows:
            continue
        path = sprites_dir / sheet_name
        rows[sid] = (
            path,
            int(m.group(3)),
            int(m.group(4)),
            int(m.group(5)),
            int(m.group(6)),
        )
    return rows


def load_sheet(path: Path, cache: dict[str, Image.Image]) -> Image.Image:
    key = path.as_posix()
    if key in cache:
        return cache[key]
    img = Image.open(path)
    if img.mode == "P":
        rgba = indexed_dos_to_rgba(img)
        cache[key] = rgba
        return rgba
    rgba = dematte_legacy_colorkey(img)
    cache[key] = rgba
    return rgba


def dematte(img: Image.Image) -> Image.Image:
    src = img.convert("RGBA")
    data = []
    for r, g, b, a in src.get_flattened_data():
        if a == 0 or (r, g, b) == (0, 0, 255) or (r > 200 and b > 200 and g < 80):
            data.append((0, 0, 0, 0))
        else:
            data.append((r, g, b, a))
    src.putdata(data)
    return src


def main() -> None:
    nfo = find_extra_nfo()
    rows = parse_rows(nfo)
    nfo_ids = [sid for sid in range(NFO_FIRST, NFO_LAST_EXCLUSIVE) if sid in rows]
    if len(nfo_ids) != ACTION5_COUNT:
        sys.exit(
            f"Se esperaban {ACTION5_COUNT} sprites Action5 en {nfo.name}, hay {len(nfo_ids)}"
        )

    TILES.mkdir(parents=True, exist_ok=True)
    cache: dict[str, Image.Image] = {}
    for i, nfo_sid in enumerate(nfo_ids):
        game_id = SPR_SIGNALS_BASE + i
        path, x, y, w, h = rows[nfo_sid]
        if not path.is_file() or path.stat().st_size == 0:
            sys.exit(f"Sheet inválido para NFO {nfo_sid}: {path}")
        crop = dematte(load_sheet(path, cache).crop((x, y, x + w, y + h)))
        opaque = sum(1 for px in crop.get_flattened_data() if px[3] > 0)
        if opaque < 1:
            sys.exit(f"rail_{game_id}.png quedó vacío (NFO {nfo_sid})")
        if w > 32 or h > 40:
            sys.exit(f"rail_{game_id}.png {w}x{h}: no parece señal Action5")
        crop.save(TILES / f"rail_{game_id}.png")

    # PBS eléctrico (SIGTYPE_PATH, variant=0, image=2): 5204
    sample = TILES / "rail_5204.png"
    im = Image.open(sample).convert("RGBA")
    if im.size[1] < 16:
        sys.exit(f"{sample.name} demasiado bajo ({im.size}): ¿sheet incorrecto?")
    print(
        f"Action5 señales: {ACTION5_COUNT} → rail_{SPR_SIGNALS_BASE}.."
        f"rail_{SPR_SIGNALS_BASE + ACTION5_COUNT - 1}.png (fuente: {nfo})"
    )


if __name__ == "__main__":
    main()
