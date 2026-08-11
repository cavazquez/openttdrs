#!/usr/bin/env python3
"""Extrae los overlays PBS inclinados de puentes vanilla a ``tiles/``.

``DrawBridgeMiddle`` / ``DrawTile_TunnelBridge`` usan estos sprites sólo
cuando una rampa ferroviaria tiene una reserva PBS visible:

  - 5401..5404: rail;
  - 5405..5408: monorail;
  - 5409..5412: maglev.

Están en el GRF *extra* (no en el NFO base donde vive el resto de la vía),
por eso deben extraerse antes de regenerar el atlas. En OpenGFX2 32bpp
`grfcodec` conserva los IDs runtime 5401..5412; en OpenGFX 8bpp clásico el
equivalente se declara como Action5 tipo 0x0F y sus filas NFO tienen IDs
locales. Ambos formatos se normalizan a los mismos nombres de tile.

Uso::

  python3 scripts/extract_bridge_pbs_reservation_sprites.py
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

from PIL import Image

from nfo_sprite_meta import detect_graphics_mode

ROOT = Path(__file__).resolve().parents[1]
OPENGFX = ROOT / "assets" / "opengfx"
TILES = OPENGFX / "tiles"
SPRITE_IDS = tuple(range(5401, 5413))

# Una fila principal lleva el ID; las alternativas 32bpp del mismo sprite
# comienzan con ``|``. Conservamos el ID anterior para poder elegir la variante
# del modo gráfico activo sin confundirla con los rectángulos zi4 posteriores.
ENTRY_RE = re.compile(
    r"^\s*(?:(?P<sprite_id>\d+)|\|)\s+(?P<sheet>\S+)\s+"
    r"(?P<bpp>8bpp|32bpp)\s+(?P<x>\d+)\s+(?P<y>\d+)\s+"
    r"(?P<w>\d+)\s+(?P<h>\d+)\s+-?\d+\s+-?\d+"
)
# Action5 0x0F = `SPR_TRACKS_FOR_SLOPES_BASE`, que comienza en 5401 en
# OpenTTD 15.3. El offset se omite en el GRF clásico cuando vale cero.
A5_TRACKS_FOR_SLOPES_RE = re.compile(
    r"^\s*\d+\s+\*\s+\d+\s+05 (?:0F|8F) FF ([0-9A-F]{2}) 00(?: FF ([0-9A-F]{2}) 00)?"
)


def source_for_active_graphics() -> tuple[Path, Path, str] | None:
    """Devuelve ``(directorio, nfo, bpp)`` para el set extra disponible."""
    source_32 = OPENGFX / "opengfx2-32ez" / "sprites"
    nfo_32 = source_32 / "ogfx2e_extra_32ez.nfo"
    # Si existe el marcador, respetarlo: el árbol puede conservar ambos sets
    # después de una descarga en otro modo.
    if detect_graphics_mode(ROOT) != "8bpp" and nfo_32.is_file():
        return source_32, nfo_32, "32bpp"

    for source_8 in sorted(OPENGFX.glob("opengfx-*/sprites"), reverse=True):
        nfo_8 = source_8 / "ogfxe_extra.nfo"
        if nfo_8.is_file():
            return source_8, nfo_8, "8bpp"

    source_8 = OPENGFX / ".signal-src-8bpp" / "sprites"
    nfo_8 = source_8 / "ogfxe_extra.nfo"
    if nfo_8.is_file():
        return source_8, nfo_8, "8bpp"
    return None


def parse_rects(nfo_path: Path, bpp: str) -> dict[int, tuple[str, int, int, int, int]]:
    rects: dict[int, tuple[str, int, int, int, int]] = {}
    current_id: int | None = None
    for line in nfo_path.read_text(encoding="utf-8", errors="replace").splitlines():
        match = ENTRY_RE.match(line)
        if match is None:
            continue
        if match["sprite_id"] is not None:
            current_id = int(match["sprite_id"])
        if current_id not in SPRITE_IDS or match["bpp"] != bpp:
            continue
        # La primera entrada del bpp pedido es el sprite ``normal``. Las
        # siguientes son los rectángulos zi4 que no se dibujan por separado.
        rects.setdefault(
            current_id,
            (
                Path(match["sheet"]).name,
                int(match["x"]),
                int(match["y"]),
                int(match["w"]),
                int(match["h"]),
            ),
        )
    return rects


def parse_action5_slope_rects(
    nfo_path: Path, bpp: str
) -> dict[int, tuple[str, int, int, int, int]]:
    """Lee Action5 0x0F y lo indexa con los IDs runtime 5401..5412.

    El NFO 8bpp de OpenGFX no numera estas filas con el ID global al que las
    instala OpenTTD. La cabecera Action5 sí conserva el contrato: 12 sprites
    desde offset cero, en orden rail/mono/maglev y cuatro direcciones cada uno.
    """
    lines = nfo_path.read_text(encoding="utf-8", errors="replace").splitlines()
    rects: dict[int, tuple[str, int, int, int, int]] = {}
    for i, line in enumerate(lines):
        header = A5_TRACKS_FOR_SLOPES_RE.match(line)
        if header is None:
            continue
        count = int(header.group(1), 16)
        offset = int(header.group(2), 16) if header.group(2) is not None else 0
        got = 0
        for row_line in lines[i + 1 :]:
            row = ENTRY_RE.match(row_line)
            if row is not None:
                if row["bpp"] == bpp and got < count:
                    slot = offset + got
                    if slot < len(SPRITE_IDS):
                        rects[SPRITE_IDS[slot]] = (
                            Path(row["sheet"]).name,
                            int(row["x"]),
                            int(row["y"]),
                            int(row["w"]),
                            int(row["h"]),
                        )
                    got += 1
                    if got == count:
                        break
                continue
            if got and re.search(r"\*\s+\d+\s+05 ", row_line):
                break
        if got:
            return rects
    return rects


def load_rgba(path: Path) -> Image.Image:
    """Convierte el color transparente de grfcodec en alfa real."""
    image = Image.open(path)
    if image.mode == "P":
        rgba = image.convert("RGBA")
        source = image.load()
        pixels = rgba.load()
        for y in range(image.height):
            for x in range(image.width):
                if source[x, y] == 0:
                    pixels[x, y] = (0, 0, 0, 0)
        return rgba

    rgba = image.convert("RGBA")
    # Las hojas 8bpp que grfcodec entrega como PNG RGB usan azul chroma.
    # En 32bpp se conserva el alfa nativo y no se altera ningún color válido.
    if image.mode != "RGBA":
        pixels = rgba.load()
        for y in range(rgba.height):
            for x in range(rgba.width):
                r, g, b, a = pixels[x, y]
                if a and (r, g, b) == (0, 0, 255):
                    pixels[x, y] = (0, 0, 0, 0)
    return rgba


def main() -> int:
    source = source_for_active_graphics()
    if source is None:
        print("No hay NFO OpenGFX extra; ejecutá scripts/descargar_graficos.sh primero.", file=sys.stderr)
        return 1

    sprites_dir, nfo_path, bpp = source
    rects = parse_rects(nfo_path, bpp)
    if any(sid not in rects for sid in SPRITE_IDS):
        # OpenGFX clásico declara el bloque con Action5 0x0F; sus IDs NFO no
        # coinciden con los IDs runtime. OpenGFX2 32bpp llega por la ruta
        # directa anterior y conserva sus metadatos históricos sin cambios.
        rects.update(parse_action5_slope_rects(nfo_path, bpp))
    missing = [sid for sid in SPRITE_IDS if sid not in rects]
    if missing:
        print(
            f"Faltan sprites PBS {missing} en {nfo_path.relative_to(ROOT)} ({bpp}).",
            file=sys.stderr,
        )
        return 1

    TILES.mkdir(parents=True, exist_ok=True)
    sheets: dict[str, Image.Image] = {}
    for sid in SPRITE_IDS:
        sheet_name, x, y, width, height = rects[sid]
        sheet = sheets.get(sheet_name)
        if sheet is None:
            sheet_path = sprites_dir / sheet_name
            if not sheet_path.is_file():
                print(f"Falta hoja {sheet_path.relative_to(ROOT)}", file=sys.stderr)
                return 1
            sheet = load_rgba(sheet_path)
            sheets[sheet_name] = sheet
        sheet.crop((x, y, x + width, y + height)).save(TILES / f"rail_{sid}.png")

    print(
        f"  bridge PBS: {len(SPRITE_IDS)} tiles desde "
        f"{nfo_path.relative_to(ROOT)} ({bpp})"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
