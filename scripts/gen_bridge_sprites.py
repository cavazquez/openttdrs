#!/usr/bin/env python3
"""Extrae sprites de puente (vanilla 13 tipos) y genera tablas Rust.

Salida PNG: `assets/opengfx/tiles/bridge_{id}.png`
Salida RS: `crates/openttdrs-client/src/sprites/bridge_sprites_generated.rs`

Uso: python3 scripts/gen_bridge_sprites.py
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

from PIL import Image

from nfo_sprite_meta import (
    detect_graphics_mode,
    parse_sprite_offs,
    pick_sprite_meta,
    sprite_dims_from_assets,
)

REPO = Path(__file__).resolve().parents[1]
TILES = REPO / "assets" / "opengfx" / "tiles"
OUT_RS = REPO / "crates/openttdrs-client/src/sprites/bridge_sprites_generated.rs"

# (rear_rail_x, rear_rail_y, rear_road_x, rear_road_y, front_x, front_y, pillar_x, pillar_y)
Deck = tuple[int, int, int, int, int, int, int, int]

# Cabezas/rampas del puente de madera. Los cuatro primeros son la cabeza
# inclinada sobre una ladera; los cuatro últimos son la subida/bajada desde
# terreno plano. El orden coincide con BRIDGE_PIECE_HEAD de OpenTTD.
WOOD_RAMP_IDS = {
    2529, 2530, 2531, 2532, 2533, 2534, 2535, 2536,
    2537, 2538, 2539, 2540, 2541, 2542, 2543, 2544,
}

# Piezas 0..5 = north, south, inner_n, inner_s, mid_odd, mid_even (índice OpenTTD).
BRIDGE_TYPE_NAMES = [
    "wood",
    "concrete",
    "girder",
    "susp_conc",
    "susp_steel",
    "susp_yellow",
    "can_steel",
    "can_brown",
    "can_red",
    "girder_alt",
    "tub_steel",
    "tub_yellow",
    "tub_silicon",
]

PIECE_NAMES = [
    "north",
    "south",
    "inner_n",
    "inner_s",
    "mid_odd",
    "mid_even",
]

# Sprites de tablero por tipo y pieza (grupos rail X/Y + road X/Y del NFO base).
BRIDGE_DECKS: dict[tuple[int, int], Deck] = {}

def deck(rx, ry, rox, roy, fx, fy, px, py) -> Deck:
    return (rx, ry, rox, roy, fx, fy, px, py)


def set_all_pieces(bt: int, d: Deck) -> None:
    for p in range(6):
        BRIDGE_DECKS[(bt, p)] = d


WOOD = deck(2546, 2545, 2548, 2547, 2550, 2549, 2552, 2551)
CONC = deck(2493, 2494, 2495, 2496, 2497, 2498, 2505, 2506)
GIRDER = deck(2499, 2500, 2501, 2502, 2503, 2504, 2505, 2506)
GIRDER_ALT = deck(2553, 2554, 2555, 2556, 2557, 2558, 2505, 2506)

set_all_pieces(0, WOOD)
set_all_pieces(1, CONC)
set_all_pieces(2, GIRDER)
set_all_pieces(9, GIRDER_ALT)

# Suspensión hormigón (tablas TILE A/B/C/D/E/F).
SUSP_CONC = {
    0: deck(2469, 2470, 2487, 2488, 2463, 2455, 2481, 2477),  # north A
    1: deck(2470, 2469, 2488, 2487, 2464, 2456, 2482, 2478),  # south B
    2: deck(2472, 2471, 2488, 2487, 2468, 2460, 2484, 2480),  # inner C/D
    3: deck(2471, 2472, 2487, 2488, 2467, 2459, 2483, 2479),
    4: deck(2485, 2494, 2487, 2488, 2489, 2497, 2491, 2491),  # mid odd E
    5: deck(2493, 2494, 2495, 2496, 2497, 2498, 0, 0),  # mid even F
}
for p, d in SUSP_CONC.items():
    BRIDGE_DECKS[(3, p)] = d

# Suspensión acero / amarilla (mismas piezas, paleta distinta en juego; PNG base).
for bt in (4, 5):
    for p, d in SUSP_CONC.items():
        BRIDGE_DECKS[(bt, p)] = d

# Cantilever acero (bronce) — mismas formas que rojo en IDs base.
CAN_MID = deck(2508, 2511, 2514, 2517, 2520, 2523, 2526, 2527)
CAN_NORTH = deck(2509, 2510, 2515, 2516, 2521, 2522, 0, 0)
CAN_SOUTH = deck(2507, 2512, 2518, 2518, 2519, 2524, 2525, 2528)

for bt in (6, 7, 8):
    BRIDGE_DECKS[(bt, 0)] = CAN_NORTH
    BRIDGE_DECKS[(bt, 1)] = CAN_SOUTH
    for p in (2, 3, 4, 5):
        BRIDGE_DECKS[(bt, p)] = CAN_MID

# Tubular (BEG/MID/END como cantilever en layout).
TUB_MID = deck(2570, 2571, 2574, 2575, 2560, 2563, 0, 0)
TUB_NORTH = deck(2569, 2572, 2573, 2576, 2559, 2562, 0, 0)
TUB_SOUTH = deck(2571, 2570, 2575, 2574, 2561, 2564, 0, 0)

for bt in (10, 11, 12):
    BRIDGE_DECKS[(bt, 0)] = TUB_NORTH
    BRIDGE_DECKS[(bt, 1)] = TUB_SOUTH
    for p in (2, 3, 4, 5):
        BRIDGE_DECKS[(bt, p)] = TUB_MID

ROW_RE = re.compile(
    r"^\s*(\d+)\s+(\S*?((?:ogfx1_base|ogfx21_base_32ez)\d+\.(?:32\.png|png|pcx)))\s+(?:8bpp|32bpp)\s+"
    r"(\d+)\s+(\d+)\s+(\d+)\s+(\d+)\s+(-?\d+)\s+(-?\d+)"
)


def opengfx_paths(repo: Path) -> tuple[Path, Path, str]:
    """Directorio de sprites + NFO base (puentes viven en ogfx1 / ogfx21, no en *extra*)."""
    mode = detect_graphics_mode(repo) or "8bpp"
    opengfx = repo / "assets" / "opengfx"
    if mode == "32bpp":
        base = opengfx / "opengfx2-32ez"
        return base / "sprites", base / "sprites" / "ogfx21_base_32ez.nfo", mode
    version_dirs = sorted(opengfx.glob("opengfx-*"), reverse=True)
    if not version_dirs:
        signal = opengfx / ".signal-src-8bpp"
        if (signal / "sprites" / "ogfx1_base.nfo").is_file():
            return signal / "sprites", signal / "sprites" / "ogfx1_base.nfo", mode
        raise FileNotFoundError(
            "No hay assets/opengfx/opengfx-* — ejecutá ./scripts/descargar_graficos.sh"
        )
    base = version_dirs[0]
    return base / "sprites", base / "sprites" / "ogfx1_base.nfo", mode


def parse_rows(nfo_path: Path, sprites_dir: Path) -> dict[int, tuple[str, int, int, int, int]]:
    rows: dict[int, tuple[str, int, int, int, int]] = {}
    for line in nfo_path.read_text(errors="replace").splitlines():
        m = ROW_RE.match(line)
        if not m:
            continue
        sid = int(m.group(1))
        sheet = (sprites_dir / Path(m.group(2)).name).as_posix()
        rows[sid] = (
            sheet,
            int(m.group(4)),
            int(m.group(5)),
            int(m.group(6)),
            int(m.group(7)),
        )
    return rows


def load_sheet(path: Path, mode: str) -> Image.Image:
    img = Image.open(path)
    if img.mode == "P":
        pal = img.getpalette()
        key = tuple(pal[0:3]) if pal else None
        out = img.convert("RGBA")
        if key:
            data = [
                (0, 0, 0, 0) if px[:3] == key else px for px in out.get_flattened_data()
            ]
            out.putdata(data)
        return out
    out = img.convert("RGBA")
    if mode != "32bpp":
        data = [
            (0, 0, 0, 0) if px[:3] == (0, 0, 255) else px
            for px in out.get_flattened_data()
        ]
        out.putdata(data)
    return out


def crop_sprite(
    sid: int,
    rows: dict[int, tuple[str, int, int, int, int]],
    sheets: dict[str, Image.Image],
    mode: str,
) -> Image.Image | None:
    if sid == 0 or sid not in rows:
        return None
    sheet_path, x, y, w, h = rows[sid]
    if w <= 0 or h <= 0:
        return None
    if sheet_path not in sheets:
        p = Path(sheet_path)
        alt = p.with_suffix(".pcx")
        load_path = alt if alt.is_file() else p
        if not load_path.is_file():
            return None
        sheets[sheet_path] = load_sheet(load_path, mode)
    crop = sheets[sheet_path].crop((x, y, x + w, y + h))
    return crop


def main() -> None:
    nfo = parse_sprite_offs(REPO)
    sprites_dir, nfo_path, mode = opengfx_paths(REPO)
    print(f"NFO base: {nfo_path.relative_to(REPO)} ({mode})", file=sys.stderr)

    all_ids: set[int] = set()
    for d in BRIDGE_DECKS.values():
        all_ids.update(x for x in d if x != 0)
    all_ids.update(WOOD_RAMP_IDS)
    # Alias wood legacy names
    legacy = {
        2545: "bridge_wood_rail_y.png",
        2546: "bridge_wood_rail_x.png",
        2547: "bridge_wood_road_y.png",
        2548: "bridge_wood_road_x.png",
        2549: "bridge_wood_y_front.png",
        2550: "bridge_wood_x_front.png",
        2551: "bridge_wood_y_pillar.png",
        2552: "bridge_wood_x_pillar.png",
    }

    rows = parse_rows(nfo_path, sprites_dir)
    prefer = mode

    sheets: dict[str, Image.Image] = {}
    meta_by_id: dict[int, tuple[float, float, float, float]] = {}

    for sid in sorted(all_ids):
        out_name = legacy.get(sid, f"bridge_{sid}.png")
        crop = crop_sprite(sid, rows, sheets, mode)
        if crop is None:
            print(f"  (omitido sprite {sid})", file=sys.stderr)
            continue
        (TILES / out_name).parent.mkdir(parents=True, exist_ok=True)
        crop.save(TILES / out_name)
        w, h, xr, yr, note = sprite_dims_from_assets(
            REPO, TILES, nfo, sid, out_name, prefer
        )
        if note not in ("sin_nfo", "macro"):
            meta_by_id[sid] = (w, h, xr, yr)
        print(f"  {out_name} ← {sid}")

    lines = [
        "// Generado por scripts/gen_bridge_sprites.py — NO EDITAR A MANO.",
        "",
        "use openttdrs_core::{BridgePiece, BridgeType};",
        "",
        "/// Sprites de tablero: rear rail X/Y, rear road X/Y, front X/Y, pillar X/Y.",
        "pub struct BridgeDeckSpriteIds {",
        "    pub rear_rail: [u32; 2],",
        "    pub rear_road: [u32; 2],",
        "    pub front: [u32; 2],",
        "    pub pillar: [u32; 2],",
        "}",
        "",
        "impl BridgeDeckSpriteIds {",
        "    pub const fn empty() -> Self {",
        "        Self {",
        "            rear_rail: [0, 0],",
        "            rear_road: [0, 0],",
        "            front: [0, 0],",
        "            pillar: [0, 0],",
        "        }",
        "    }",
        "",
        "    pub fn rear(&self, rail: bool, axis: usize) -> u32 {",
        "        if rail { self.rear_rail[axis] } else { self.rear_road[axis] }",
        "    }",
        "",
        "    pub fn atlas_name(sid: u32) -> String {",
        "        match sid {",
    ]
    for sid, name in sorted(legacy.items()):
        lines.append(f'            {sid} => "{name}".to_string(),')
    lines.append('            other => format!("bridge_{other}.png"),')
    lines.extend(
        [
            "        }",
            "    }",
            "}",
            "",
            "/// Sprite de cabeza/rampa del puente de madera.",
            "/// `dir` es el valor de dirección almacenado en los bits bajos de m5.",
            "pub fn wooden_bridge_ramp_sprite_id(rail: bool, tileh: u8, dir: u8) -> u32 {",
            "    // GetBridgeRampDirectionBaseOffset: SW, SE, NE, NW.",
            "    let direction = [2usize, 1, 0, 3][usize::from(dir & 3)];",
            "    let table = if rail {",
            "        if tileh == 0 {",
            "            [2542, 2541, 2544, 2543]",
            "        } else {",
            "            [2538, 2537, 2539, 2540]",
            "        }",
            "    } else if tileh == 0 {",
            "        [2534, 2533, 2536, 2535]",
            "    } else {",
            "        [2530, 2529, 2531, 2532]",
            "    };",
            "    table[direction]",
            "}",
            "",
            "/// Offsets NFO (w, h, xrel, yrel) por sprite id.",
            "pub fn bridge_sprite_meta(sid: u32) -> Option<(f32, f32, f32, f32)> {",
            "    match sid {",
        ]
    )
    for sid, (w, h, xr, yr) in sorted(meta_by_id.items()):
        lines.append(f"        {sid} => Some(({w:.1f}, {h:.1f}, {xr:.1f}, {yr:.1f})),")
    lines.append("        _ => None,")
    lines.extend(["    }", "}", ""])

    lines.append("const DECK_TABLE: [[BridgeDeckSpriteIds; 6]; 13] = [")
    for bt in range(13):
        lines.append("    [")
        for p in range(6):
            d = BRIDGE_DECKS.get((bt, p), WOOD)
            lines.append(
                f"        BridgeDeckSpriteIds {{ rear_rail: [{d[0]}, {d[1]}], "
                f"rear_road: [{d[2]}, {d[3]}], front: [{d[4]}, {d[5]}], "
                f"pillar: [{d[6]}, {d[7]}] }},"
            )
        lines.append("    ],")
    lines.append("];")
    lines.extend(
        [
            "",
            "pub fn bridge_deck_sprite_ids(",
            "    bridge_type: BridgeType,",
            "    piece: BridgePiece,",
            ") -> &'static BridgeDeckSpriteIds {",
            "    let bt = bridge_type.as_u8() as usize;",
            "    let pi = match piece {",
            "        BridgePiece::North => 0,",
            "        BridgePiece::South => 1,",
            "        BridgePiece::InnerNorth => 2,",
            "        BridgePiece::InnerSouth => 3,",
            "        BridgePiece::MiddleOdd => 4,",
            "        BridgePiece::MiddleEven => 5,",
            "    };",
            "    &DECK_TABLE[bt][pi]",
            "}",
            "",
        ]
    )

    OUT_RS.write_text("\n".join(lines), encoding="utf-8")
    print(f"Escrito {OUT_RS.relative_to(REPO)}")


if __name__ == "__main__":
    main()
