#!/usr/bin/env python3
"""Recorta solo `industry_<id>.png` faltantes según industry_gfx_data_generated.rs.

Usa los atlas OpenGFX ya extraídos en assets/opengfx/ (sin borrar tiles existentes).
Tras P5 (480 filas / 4 estadios) hace falta regenerar industria si solo tenías estadio 3.

Uso:
  python3 scripts/crop_missing_industry_pngs.py
  python3 scripts/crop_missing_industry_pngs.py --all   # re-corta todos los IDs de la tabla
"""
from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

from nfo_sprite_meta import detect_graphics_mode

try:
    from PIL import Image
except ImportError:
    print("Falta Pillow (pip install pillow).", file=sys.stderr)
    raise SystemExit(1) from None


def industry_sprite_ids(repo: Path) -> list[int]:
    gen = repo / "crates/openttdrs-client/src/sprites/industry_gfx_data_generated.rs"
    text = gen.read_text(encoding="utf-8")
    ids = set(int(x) for x in re.findall(r"ground_sprite_id:\s*(\d+)", text))
    ids |= set(int(x) for x in re.findall(r"sprite_id:\s*(\d+)", text))
    ids.discard(0)
    return sorted(ids)


def opengfx_paths(repo: Path) -> tuple[Path, Path, str, str]:
    mode = detect_graphics_mode(repo) or "8bpp"
    opengfx = repo / "assets/opengfx"
    if mode == "32bpp":
        base = opengfx / "opengfx2-32ez"
        return base / "sprites", base / "sprites" / "ogfx21_base_32ez.nfo", "ogfx21_base_32ez", mode
    version_dirs = sorted(opengfx.glob("opengfx-*"), reverse=True)
    if not version_dirs:
        raise FileNotFoundError(
            "No hay assets/opengfx/opengfx-* — ejecutá ./scripts/descargar_graficos.sh --8bpp"
        )
    base = version_dirs[0]
    return base / "sprites", base / "sprites" / "ogfx1_base.nfo", "ogfx1_base", mode


def load_sheet(png_path: Path, graphics_mode: str) -> Image.Image:
    if graphics_mode == "32bpp":
        img = Image.open(png_path)
        if img.mode == "P":
            pal = img.getpalette()
            transparent_rgb = tuple(pal[0:3]) if pal else None
            img_rgba = img.convert("RGBA")
            if transparent_rgb is not None:
                data = []
                for r, g, b, a in img_rgba.getdata():
                    if (r, g, b) == transparent_rgb:
                        data.append((0, 0, 0, 0))
                    else:
                        data.append((r, g, b, a))
                img_rgba.putdata(data)
            return img_rgba
        return img.convert("RGBA")

    def is_magenta_key(r: int, g: int, b: int) -> bool:
        return r >= 220 and b >= 220 and g <= 40 and abs(r - b) <= 24

    img = Image.open(png_path)
    if img.mode == "P":
        pal = img.getpalette()
        transparent_rgb = tuple(pal[0:3])
        img_rgba = img.convert("RGBA")
        data = []
        for r, g, b, a in img_rgba.getdata():
            if (r, g, b) == transparent_rgb or is_magenta_key(r, g, b):
                data.append((0, 0, 0, 0))
            else:
                data.append((r, g, b, a))
        img_rgba.putdata(data)
        return img_rgba
    img_rgba = img.convert("RGBA")
    data = []
    for r, g, b, a in img_rgba.getdata():
        if is_magenta_key(r, g, b):
            data.append((0, 0, 0, 0))
        else:
            data.append((r, g, b, a))
    img_rgba.putdata(data)
    return img_rgba


def parse_sprite_rect(nfo_path: Path) -> dict[int, tuple[int, int, int, int, str]]:
    pat = re.compile(
        r"^\s*(\d+)\s+(\S*?((?:ogfx1_base|ogfx21_base_32ez)\d+\.(?:32\.png|png|pcx)))\s+(?:8bpp|32bpp)\s+"
        r"(\d+)\s+(\d+)\s+(\d+)\s+(\d+)\s+(-?\d+)\s+(-?\d+)"
    )
    out: dict[int, tuple[int, int, int, int, str]] = {}
    for line in nfo_path.read_text(errors="replace").splitlines():
        m = pat.match(line)
        if m:
            sid = int(m.group(1))
            sheet = Path(m.group(2)).name
            out[sid] = (
                int(m.group(4)),
                int(m.group(5)),
                int(m.group(6)),
                int(m.group(7)),
                sheet,
            )
    return out


def load_sheets(sprites_dir: Path, sheet_prefix: str, graphics_mode: str) -> dict[str, Image.Image]:
    sheets: dict[str, Image.Image] = {}
    for p in sorted(sprites_dir.glob(f"{sheet_prefix}*.png")):
        try:
            if p.stat().st_size == 0:
                continue
            sheets[p.name] = load_sheet(p, graphics_mode)
        except OSError:
            continue
    for p in sorted(sprites_dir.glob(f"{sheet_prefix}*.pcx")):
        try:
            if p.stat().st_size == 0:
                continue
            sheets[p.name] = load_sheet(p, graphics_mode)
        except OSError:
            continue
    return sheets


def crop_sprite(
    sid: int,
    out_path: Path,
    sprite_rect: dict[int, tuple[int, int, int, int, str]],
    sheets: dict[str, Image.Image],
) -> str:
    if sid not in sprite_rect:
        return "no_nfo"
    x, y, w, h, sheet = sprite_rect[sid]
    sheet_key = sheet
    if sheet_key not in sheets:
        alt = Path(sheet).with_suffix(".pcx").name
        if alt in sheets:
            sheet_key = alt
        else:
            return "no_sheet"
    crop = sheets[sheet_key].crop((x, y, x + w, y + h))
    out_path.parent.mkdir(parents=True, exist_ok=True)
    crop.save(out_path)
    return "ok"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--all",
        action="store_true",
        help="Re-cortar todos los IDs de la tabla (no solo faltantes)",
    )
    args = parser.parse_args()

    repo = Path(__file__).resolve().parents[1]
    tiles_dir = repo / "assets/opengfx/tiles"
    try:
        sprites_dir, nfo_path, sheet_prefix, graphics_mode = opengfx_paths(repo)
    except FileNotFoundError as e:
        print(e, file=sys.stderr)
        return 1

    if not nfo_path.is_file():
        print(f"Falta NFO: {nfo_path}", file=sys.stderr)
        print("Ejecutá: ./scripts/descargar_graficos.sh --8bpp", file=sys.stderr)
        return 1
    if not any(sprites_dir.glob(f"{sheet_prefix}*.png")) and not any(
        sprites_dir.glob(f"{sheet_prefix}*.pcx")
    ):
        print(f"Sin atlas en {sprites_dir} — ¿grfcodec pendiente?", file=sys.stderr)
        return 1

    ids = industry_sprite_ids(repo)
    todo = ids if args.all else [i for i in ids if not (tiles_dir / f"industry_{i}.png").is_file()]
    if not todo:
        print(f"Nada que hacer: {len(ids)} IDs y todos los PNG existen en {tiles_dir}")
        return 0

    print(f"Modo {graphics_mode}, NFO {nfo_path.name}, recortando {len(todo)}/{len(ids)} sprites…")
    sprite_rect = parse_sprite_rect(nfo_path)
    sheets = load_sheets(sprites_dir, sheet_prefix, graphics_mode)

    ok = no_nfo = no_sheet = 0
    for sid in todo:
        out = tiles_dir / f"industry_{sid}.png"
        status = crop_sprite(sid, out, sprite_rect, sheets)
        if status == "ok":
            ok += 1
        elif status == "no_nfo":
            no_nfo += 1
            print(f"  (omitido industry_{sid}.png: no en NFO)")
        else:
            no_sheet += 1
            print(f"  (omitido industry_{sid}.png: sheet ausente)")

    print(f"Listo: {ok} creados, {no_nfo} sin NFO, {no_sheet} sin sheet")
    if ok > 0:
        print("Siguiente paso: python3 scripts/gen_tile_atlas.py  (incluir PNGs nuevos en el atlas)")
    still_missing = [
        i for i in ids if not (tiles_dir / f"industry_{i}.png").is_file()
    ]
    if still_missing:
        print(f"Aún faltan {len(still_missing)} PNG (ej. {still_missing[:8]})", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
