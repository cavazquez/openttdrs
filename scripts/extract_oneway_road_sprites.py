#!/usr/bin/env python3
"""Extrae el fallback Action5 0x09 de calles de sentido único de OpenTTD.

`SPR_ONEWAY_BASE` no vive en los GRF de OpenGFX: OpenTTD carga siempre su
``openttd.grf`` de gráficos extra y éste aporta las 18 imágenes Action5. Por
eso también se usa bajo una base OpenGFX2 32bpp; el fallback oficial sigue
siendo un sprite paletizado 8bpp.

La fuente es ``media/baseset/openttd/oneway.{nfo,png}`` del pin de OpenTTD.
Se generan los PNG ``assets/opengfx/tiles/oneway_00..17.png`` y la tabla Rust
con las anclas NFO. El atlas se empaqueta después con ``gen_tile_atlas.py``.

Uso:
  python3 scripts/extract_oneway_road_sprites.py
  python3 scripts/extract_oneway_road_sprites.py --source-dir /ruta/a/openttd
  python3 scripts/extract_oneway_road_sprites.py --check
"""

from __future__ import annotations

import argparse
import os
import re
import sys
from pathlib import Path

from PIL import Image


ROOT = Path(__file__).resolve().parents[1]
TILES = ROOT / "assets" / "opengfx" / "tiles"
OUT_RS = (
    ROOT
    / "crates"
    / "openttdrs-client"
    / "src"
    / "sprites"
    / "oneway_road_sprites_generated.rs"
)
SPRITE_COUNT = 18
SPR_ONEWAY_BASE = 6105

# ``-1 sprites/oneway.png 8bpp x y w h xrel yrel normal``. El archivo fuente
# incluye sólo las 18 filas de Action5 que nos interesan, pero se conserva una
# expresión completa para detectar cambios del formato en vez de recortar una
# región por posición implícita.
ROW = re.compile(
    r"^\s*-1\s+sprites/oneway\.png\s+8bpp\s+"
    r"(?P<x>\d+)\s+(?P<y>\d+)\s+(?P<w>\d+)\s+(?P<h>\d+)\s+"
    r"(?P<xrel>-?\d+)\s+(?P<yrel>-?\d+)\s+normal\s*$"
)


def default_source_dir() -> Path | None:
    """Localiza el pin o la caché que prepara ``descargar_graficos.sh``."""
    configured = os.environ.get("OPENTTDRS_OPENTTD_EXTRA_DIR")
    candidates = [
        Path(configured) if configured else None,
        ROOT / "reference" / "openttd-upstream" / "media" / "baseset" / "openttd",
        ROOT / ".downloads" / "openttd" / "openttd-extra-15.3",
    ]
    return next(
        (
            candidate
            for candidate in candidates
            if candidate is not None
            and (candidate / "oneway.nfo").is_file()
            and (candidate / "oneway.png").is_file()
        ),
        None,
    )


def parse_rows(nfo: Path) -> list[tuple[int, int, int, int, int, int]]:
    rows: list[tuple[int, int, int, int, int, int]] = []
    for line in nfo.read_text(encoding="utf-8", errors="replace").splitlines():
        match = ROW.match(line)
        if match is None:
            continue
        rows.append(
            tuple(int(match[name]) for name in ("x", "y", "w", "h", "xrel", "yrel"))
        )
    if len(rows) != SPRITE_COUNT:
        raise RuntimeError(
            f"{nfo}: esperados {SPRITE_COUNT} sprites Action5 0x09; hallados {len(rows)}"
        )
    return rows


def source_crop(source: Image.Image, rect: tuple[int, int, int, int, int, int]) -> Image.Image:
    """Recorta un sprite y vuelve transparente el índice 0 azul del GRF."""
    x, y, w, h, _xrel, _yrel = rect
    box = (x, y, x + w, y + h)
    indexed = source.crop(box)
    rgba = indexed.convert("RGBA")
    if indexed.mode == "P":
        # En ``oneway.png`` el índice 0 se almacena como azul (#0000ff), pero
        # en el GRF es transparente. Convertir RGB a RGBA sin esta máscara
        # dejaba un rectángulo azul alrededor de las flechas.
        # ``Image.point`` conserva el modo P y luego ``convert('L')`` aplica
        # la paleta (azul → luminancia 29), no el índice. Construir L desde
        # los índices crudos mantiene el cero realmente transparente.
        alpha = Image.frombytes(
            "L",
            indexed.size,
            bytes(0 if index == 0 else 255 for index in indexed.tobytes()),
        )
        rgba.putalpha(alpha)
    return rgba


def generated_rust(rows: list[tuple[int, int, int, int, int, int]]) -> str:
    lines = [
        "//! GENERADO por scripts/extract_oneway_road_sprites.py — no editar a mano.\n",
        "//!\n",
        "//! Fallback Action5 0x09 de `openttd.grf` (OpenTTD 15.3). El GRF\n",
        "//! oficial es 8bpp también cuando la base seleccionada es OpenGFX2 32bpp.\n\n",
        f"pub const SPR_ONEWAY_BASE: u32 = {SPR_ONEWAY_BASE};\n",
        f"pub const ONEWAY_ROAD_SPRITE_COUNT: usize = {SPRITE_COUNT};\n\n",
        "/// `(w, h, xrel, yrel)` NFO de `oneway_{00..17}.png`.\n",
        "pub static ONEWAY_ROAD_SPRITE_META: [(f32, f32, f32, f32); ONEWAY_ROAD_SPRITE_COUNT] = [\n",
    ]
    for _x, _y, w, h, xrel, yrel in rows:
        lines.append(f"    ({w}.0, {h}.0, {xrel}.0, {yrel}.0),\n")
    lines.append(
        "];\n\n"
        "/// ID global que el draw proc de OpenTTD ve para un slot Action5 0x09.\n"
        "#[must_use]\n"
        "pub const fn oneway_road_sprite_id(slot: usize) -> Option<u32> {\n"
        "    if slot < ONEWAY_ROAD_SPRITE_COUNT {\n"
        "        Some(SPR_ONEWAY_BASE + slot as u32)\n"
        "    } else {\n"
        "        None\n"
        "    }\n"
        "}\n"
    )
    return "".join(lines)


def compare_or_write(path: Path, expected: bytes, check: bool) -> bool:
    actual = path.read_bytes() if path.is_file() else None
    if actual == expected:
        return False
    if check:
        print(f"DRIFT: {path.relative_to(ROOT)} no coincide con la fuente OpenTTD", file=sys.stderr)
        return True
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(expected)
    print(f"Escrito {path.relative_to(ROOT)}")
    return True


def image_differs(path: Path, expected: Image.Image) -> bool:
    if not path.is_file():
        return True
    with Image.open(path) as actual:
        actual_rgba = actual.convert("RGBA")
    return actual_rgba.size != expected.size or actual_rgba.tobytes() != expected.tobytes()


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-dir", type=Path, help="directorio con oneway.nfo y oneway.png")
    parser.add_argument("--check", action="store_true", help="verifica sin escribir")
    args = parser.parse_args(argv)

    source_dir = args.source_dir or default_source_dir()
    if source_dir is None:
        print(
            "SKIP: falta media/baseset/openttd/oneway.{nfo,png}; "
            "corré descargar_graficos.sh o prepará el pin OpenTTD",
            file=sys.stderr,
        )
        return 2
    nfo = source_dir / "oneway.nfo"
    png = source_dir / "oneway.png"
    if not nfo.is_file() or not png.is_file():
        print(f"ERROR: fuente incompleta en {source_dir}", file=sys.stderr)
        return 2

    try:
        rows = parse_rows(nfo)
        with Image.open(png) as source:
            crops = [source_crop(source, row) for row in rows]
    except (OSError, RuntimeError) as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 1

    drift = compare_or_write(OUT_RS, generated_rust(rows).encode("utf-8"), args.check)

    # En CI se valida la tabla Rust desde el pin de OpenTTD, pero los PNG de
    # trabajo/atlas no se versionan. Si no existe ninguna flecha local, no se
    # puede distinguir ese caso legítimo de un asset perdido; el check local
    # sí compara las 18 en cuanto la familia está presente.
    check_tiles = not args.check or (
        TILES.is_dir() and any(TILES.glob("oneway_*.png"))
    )
    if check_tiles:
        for slot, crop in enumerate(crops):
            path = TILES / f"oneway_{slot:02}.png"
            if not image_differs(path, crop):
                continue
            if args.check:
                print(f"DRIFT: {path.relative_to(ROOT)} no coincide con la fuente OpenTTD", file=sys.stderr)
                drift = True
                continue
            path.parent.mkdir(parents=True, exist_ok=True)
            crop.save(path)
            print(f"Escrito {path.relative_to(ROOT)}")

    if args.check:
        if drift:
            return 1
        suffix = "" if check_tiles else "; PNG locales ausentes, se verificó metadata"
        print(f"OK: fallback Action5 one-way (18 sprites + metadata) coincide{suffix}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
