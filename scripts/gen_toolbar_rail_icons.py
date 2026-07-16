#!/usr/bin/env python3
"""Extrae iconos del toolbar ferroviario por tipo de vía (OpenTTD rail_gui).

Iconos normales del set base + variantes eléctrica / mono / maglev
(`table/railtypes.h` + `sprites.h`).

Salida: assets/opengfx/tiles/toolbar_rail_*.png
        assets/opengfx/tiles/toolbar_rail_{electric,mono,maglev}_*.png

Los iconos de vía *eléctrica* (ranuras Action5 elrail 36..39, 44) se recortan
desde OpenGFX clásico 8bpp (`ogfxe_extra`), no desde OpenGFX2 32ez: ese GRF
extra todavía no expone un bloque GUI elrail usable en 32bpp. En modo 32bpp,
`descargar_graficos.sh` deja el NFO en
`assets/opengfx/.signal-src-8bpp/sprites/ogfxe_extra.nfo`.

TODO(32bpp-nativo): cuando OpenGFX2 tenga Action5 tipo 05 (elrail) con iconos
GUI en 32bpp, leerlos de `ogfx2e_extra_32ez` y eliminar la dependencia de
`.signal-src-8bpp` / `write_electric_gui_from_8bpp`.

Uso: python3 scripts/gen_toolbar_rail_icons.py
"""
from __future__ import annotations

import re
from pathlib import Path

from PIL import Image

REPO = Path(__file__).resolve().parents[1]
SPRITES = REPO / "assets" / "opengfx" / "opengfx2-32ez" / "sprites"
TILES = REPO / "assets" / "opengfx" / "tiles"

BASE_NFO = SPRITES / "ogfx21_base_32ez.nfo"
EXTRA_NFO = SPRITES / "ogfx2e_extra_32ez.nfo"

# (sprite_id base, nombre de salida)
BASE_ICONS = [
    (703, "demolish"),
    (714, "remove"),
    (1251, "rail_ns"),
    (1252, "rail_x"),
    (1253, "rail_ew"),
    (1254, "rail_y"),
    (1291, "signals"),
    (1294, "depot"),
    (1298, "station"),
    (2430, "tunnel"),
    (2594, "bridge"),
]

# SPR_OPENTTD_BASE + n para los iconos del GRF extra (Action 5 tipo 0x15).
EXTRA_ICONS = [
    (53, "autorail"),
    (55, "convert"),
    (76, "waypoint"),
]

# Variantes tipadas: OpenTTD OnInit solo cambia estas 8 ranuras.
# Electric: GUI sprites en Action5 elrail (offsets 36..39, 44) + extra offsets.
# Mono/Maglev: sprites base 1255–1262 + túneles 2431/2432 + extra.
TYPED_BASE = {
    "mono": {
        "rail_ns": 1255,
        "rail_x": 1256,
        "rail_ew": 1257,
        "rail_y": 1258,
        "tunnel": 2431,
    },
    "maglev": {
        "rail_ns": 1259,
        "rail_x": 1260,
        "rail_y": 1261,  # 0x4ED
        "rail_ew": 1262,  # 0x4EE
        "tunnel": 2432,
    },
}

TYPED_EXTRA = {
    "electric": {
        "autorail": 57,
        "convert": 59,
        "depot": 61,
    },
    "mono": {
        "autorail": 63,
        "convert": 65,
        "depot": 67,
    },
    "maglev": {
        "autorail": 69,
        "convert": 71,
        "depot": 73,
    },
}

# Action5 elrail (tipo 05): índices dentro del bloque → nombre toolbar.
ELECTRIC_A5_SLOTS = {
    36: "rail_ns",
    37: "rail_x",
    38: "rail_ew",
    39: "rail_y",
    44: "tunnel",
}

ELRAIL_8BPP_NFOS = [
    REPO / "assets" / "opengfx" / ".signal-src-8bpp" / "sprites" / "ogfxe_extra.nfo",
    REPO
    / "assets"
    / "opengfx"
    / ".signal-src-8bpp"
    / "extract"
    / "opengfx-8.0"
    / "sprites"
    / "ogfxe_extra.nfo",
]
PALETTES_H = REPO / "third_party" / "openttd" / "table" / "palettes.h"
A5_ELRAIL_RE = re.compile(r"\*\s*5\s+05\s+05\s+FF\s+30")

ROW_RE = re.compile(
    r"^\s*(\d+)\s+(\S+?\.png)\s+(8bpp)\s+(\d+)\s+(\d+)\s+(\d+)\s+(\d+)\s+(-?\d+)\s+(-?\d+)"
)
A5_GUI_RE = re.compile(r"^\s*(\d+)\s+\*\s+\d+\s+05 95 FF ([0-9A-F]{2}) 00 FF ([0-9A-F]{2}) 00")


def load_dos_palette() -> list[tuple[int, int, int]]:
    text = PALETTES_H.read_text(encoding="utf-8", errors="replace")
    start = text.index("static const Palette _palette")
    end = text.index("};", start)
    colours = [
        tuple(map(int, m))
        for m in re.findall(r"M\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*\)", text[start:end])
    ]
    if len(colours) < 206:
        raise SystemExit(f"paleta DOS incompleta ({len(colours)} entradas)")
    # Completar a 256 por si el archivo trae menos (índices altos raros).
    while len(colours) < 256:
        colours.append((0, 0, 0))
    return colours


def parse_rows(nfo: Path) -> dict[int, tuple[str, int, int, int, int]]:
    rows: dict[int, tuple[str, int, int, int, int]] = {}
    for line in nfo.read_text(errors="replace").splitlines():
        m = ROW_RE.match(line)
        if not m:
            continue
        sid = int(m.group(1))
        if sid not in rows:  # primera fila 8bpp por sprite
            rows[sid] = (
                Path(m.group(2)).name,
                int(m.group(4)),
                int(m.group(5)),
                int(m.group(6)),
                int(m.group(7)),
            )
    return rows


def gui_offset_map(nfo: Path) -> dict[int, int]:
    """offset GUI (n de SPR_OPENTTD_BASE+n) → spritenum NFO."""
    out: dict[int, int] = {}
    for line in nfo.read_text(errors="replace").splitlines():
        m = A5_GUI_RE.match(line)
        if not m:
            continue
        header_num = int(m.group(1))
        count = int(m.group(2), 16)
        offset = int(m.group(3), 16)
        for i in range(count):
            out.setdefault(offset + i, header_num + 1 + i)
    return out


def dematte_blue(img: Image.Image) -> Image.Image:
    img = img.convert("RGBA")
    px = img.load()
    for j in range(img.height):
        for i in range(img.width):
            if px[i, j][:3] == (0, 0, 255):
                px[i, j] = (0, 0, 0, 0)
    return img


def to_toolbar_canvas(img: Image.Image) -> Image.Image:
    w, h = img.size
    scale = min(2.0, 63.0 / w, 51.0 / h)
    sw, sh = max(1, round(w * scale)), max(1, round(h * scale))
    icon = img.resize((sw, sh), Image.NEAREST)
    canvas = Image.new("RGBA", (63, 51), (0, 0, 0, 0))
    canvas.alpha_composite(icon, ((63 - sw) // 2, (51 - sh) // 2))
    return canvas


def crop(rows: dict[int, tuple[str, int, int, int, int]], sid: int, out_name: str) -> None:
    sheet_name, x, y, w, h = rows[sid]
    sheet = Image.open(SPRITES / sheet_name)
    img = dematte_blue(sheet.crop((x, y, x + w, y + h)))
    to_toolbar_canvas(img).save(TILES / out_name)
    print(f"  {out_name} <- {sheet_name} sprite {sid} ({w}x{h})")


def crop_indexed_dos(
    sheet: Image.Image,
    dos: list[tuple[int, int, int]],
    x: int,
    y: int,
    w: int,
    h: int,
) -> Image.Image:
    """Recorta 8bpp y aplica paleta DOS (la paleta embebida del PNG miente)."""
    crop_img = sheet.crop((x, y, x + w, y + h))
    if crop_img.mode != "P":
        return dematte_blue(crop_img)
    # Pillow 10+: get_flattened_data; getdata() queda deprecado (Pillow 14).
    idx = list(crop_img.get_flattened_data())
    out = Image.new("RGBA", (w, h))
    px = out.load()
    for row in range(h):
        for col in range(w):
            p = idx[row * w + col]
            if p == 0:
                px[col, row] = (0, 0, 0, 0)
            else:
                r, g, b = dos[p]
                px[col, row] = (r, g, b, 255)
    return out


def find_elrail_8bpp_nfo() -> Path | None:
    for p in ELRAIL_8BPP_NFOS:
        if p.is_file():
            return p
    return None


def write_electric_gui_from_8bpp(dos: list[tuple[int, int, int]]) -> None:
    nfo = find_elrail_8bpp_nfo()
    if nfo is None:
        raise SystemExit(
            "falta ogfxe_extra.nfo 8bpp para iconos eléctricos "
            "(assets/opengfx/.signal-src-8bpp/…). "
            "En --32bpp lo prepara descargar_graficos.sh; si corrés este "
            "script solo, ejecutá antes ese pipeline. "
            "TODO(32bpp-nativo): reemplazar por Action5 elrail de OpenGFX2."
        )
    lines = nfo.read_text(errors="replace").splitlines()
    start = next((i + 1 for i, l in enumerate(lines) if A5_ELRAIL_RE.search(l)), None)
    if start is None:
        raise SystemExit(f"sin bloque Action5 elrail en {nfo}")
    sprites: list[re.Match[str]] = []
    for line in lines[start : start + 80]:
        m = ROW_RE.match(line)
        if m:
            sprites.append(m)
        if len(sprites) >= 48:
            break
    if len(sprites) < 45:
        raise SystemExit(f"Action5 elrail incompleto ({len(sprites)} sprites)")
    sheets: dict[str, Image.Image] = {}
    sheet_dir = nfo.parent
    for slot, name in ELECTRIC_A5_SLOTS.items():
        m = sprites[slot]
        sheet_name = Path(m.group(2)).name
        if sheet_name not in sheets:
            sheets[sheet_name] = Image.open(sheet_dir / sheet_name)
        x, y, w, h = map(int, m.group(4, 5, 6, 7))
        rgba = crop_indexed_dos(sheets[sheet_name], dos, x, y, w, h)
        out_name = f"toolbar_rail_electric_{name}.png"
        to_toolbar_canvas(rgba).save(TILES / out_name)
        print(f"  {out_name} <- {sheet_name} A5[{slot}] DOS ({w}x{h})")


def main() -> None:
    TILES.mkdir(parents=True, exist_ok=True)
    dos = load_dos_palette()
    base_rows = parse_rows(BASE_NFO)
    for sid, name in BASE_ICONS:
        crop(base_rows, sid, f"toolbar_rail_{name}.png")

    extra_rows = parse_rows(EXTRA_NFO)
    offsets = gui_offset_map(EXTRA_NFO)
    for off, name in EXTRA_ICONS:
        sid = offsets.get(off)
        if sid is None or sid not in extra_rows:
            raise SystemExit(f"icono extra GUI offset {off:#x} ({name}) no encontrado")
        crop(extra_rows, sid, f"toolbar_rail_{name}.png")

    write_electric_gui_from_8bpp(dos)
    for name, off in TYPED_EXTRA["electric"].items():
        sid = offsets.get(off)
        if sid is None or sid not in extra_rows:
            raise SystemExit(f"electric extra offset {off:#x} ({name}) no encontrado")
        crop(extra_rows, sid, f"toolbar_rail_electric_{name}.png")

    for railtype, mapping in TYPED_BASE.items():
        for name, sid in mapping.items():
            crop(base_rows, sid, f"toolbar_rail_{railtype}_{name}.png")
        for name, off in TYPED_EXTRA[railtype].items():
            sid = offsets.get(off)
            if sid is None or sid not in extra_rows:
                raise SystemExit(f"{railtype} extra offset {off:#x} ({name}) no encontrado")
            crop(extra_rows, sid, f"toolbar_rail_{railtype}_{name}.png")


if __name__ == "__main__":
    main()
