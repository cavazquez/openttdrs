#!/usr/bin/env python3
"""Extrae sprites de puente (vanilla 13 tipos) y genera tablas Rust.

Salida PNG: `assets/opengfx/tiles/bridge_{id}.png`
Salida RS: `crates/openttdrs-client/src/sprites/bridge_sprites_generated.rs`

Uso: python3 scripts/gen_bridge_sprites.py
"""
from __future__ import annotations

import sys
from pathlib import Path

from PIL import Image

from nfo_sprite_meta import (
    SpriteRect,
    detect_graphics_mode,
    parse_global_sprite_rects,
    parse_sprite_offs,
    pick_sprite_meta,
    sprite_dims_from_assets,
)
from opengfx_palette import dematte_legacy_colorkey, indexed_dos_to_rgba

REPO = Path(__file__).resolve().parents[1]
TILES = REPO / "assets" / "opengfx" / "tiles"
OUT_RS = REPO / "crates/openttdrs-client/src/sprites/bridge_sprites_generated.rs"

# `src/table/bridge_land.h` de OpenTTD organiza cada tramo en ocho filas:
# rail X/Y, road X/Y, monorail X/Y y maglev X/Y. Las capas front y pillar son
# comunes a esos cuatro medios de transporte. Conservamos la tabla completa
# aquí para que el atlas y el renderer no conviertan monorriel/maglev en riel
# normal ni intercambien los ejes de puentes complejos.
#
# (rear rail X/Y, rear road X/Y, rear mono X/Y, rear maglev X/Y,
#  front X/Y, pillar X/Y)
Deck = tuple[int, int, int, int, int, int, int, int, int, int, int, int]

# Cabezas/rampas del puente de madera. Los cuatro primeros son la cabeza
# inclinada sobre una ladera; los cuatro últimos son la subida/bajada desde
# terreno plano. El orden coincide con BRIDGE_PIECE_HEAD de OpenTTD.
WOOD_RAMP_IDS = {
    2529, 2530, 2531, 2532, 2533, 2534, 2535, 2536,
    2537, 2538, 2539, 2540, 2541, 2542, 2543, 2544,
    4352, 4353, 4354, 4355, 4356, 4357, 4358, 4359,
    4392, 4393, 4394, 4395, 4396, 4397, 4398, 4399,
}

# `_bridge_sprite_table_generic_*_heads` de OpenTTD: rail, road, monorail y
# maglev; para cada transporte hay cuatro cabezas sobre ladera y cuatro rampas
# desde terreno plano. La paleta del tipo de puente se aplica al dibujarlas.
GENERIC_RAMP_IDS = {
    2437, 2438, 2439, 2440, 2441, 2442, 2443, 2444,
    2445, 2446, 2447, 2448, 2449, 2450, 2451, 2452,
    4326, 4327, 4328, 4329, 4330, 4331, 4332, 4333,
    4366, 4367, 4368, 4369, 4370, 4371, 4372, 4373,
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

# Sprites de tablero vanilla por tipo y pieza. Sincronizado contra
# `OpenTTD/src/table/bridge_land.h` y `src/table/sprites.h`.
BRIDGE_DECKS: dict[tuple[int, int], Deck] = {}

def deck(rx, ry, rox, roy, mx, my, gx, gy, fx, fy, px, py) -> Deck:
    return (rx, ry, rox, roy, mx, my, gx, gy, fx, fy, px, py)


def set_all_pieces(bt: int, d: Deck) -> None:
    for p in range(6):
        BRIDGE_DECKS[(bt, p)] = d


WOOD = deck(2546, 2545, 2548, 2547, 4361, 4360, 4401, 4400, 2550, 2549, 2552, 2551)
CONC = deck(2493, 2494, 2495, 2496, 4344, 4345, 4384, 4385, 2497, 2498, 2505, 2506)
GIRDER = deck(2499, 2500, 2501, 2502, 4324, 4325, 4364, 4365, 2503, 2504, 2505, 2506)
GIRDER_ALT = deck(2553, 2554, 2555, 2556, 4362, 4363, 4402, 4403, 2557, 2558, 2505, 2506)

set_all_pieces(0, WOOD)
set_all_pieces(1, CONC)
set_all_pieces(2, GIRDER)
set_all_pieces(9, GIRDER_ALT)

# Suspensión (tablas TILE A/B/C/D/E/F). El orden X/Y coincide con
# `GetBridgeMiddleAxisBaseOffset` (X = fila 0, Y = fila 1).
SUSP_CONC = {
    0: deck(2473, 2469, 2461, 2453, 4338, 4334, 4378, 4374, 2463, 2455, 2481, 2477),
    1: deck(2474, 2470, 2462, 2454, 4339, 4335, 4379, 4375, 2464, 2456, 2482, 2478),
    2: deck(2476, 2472, 2466, 2458, 4341, 4337, 4381, 4377, 2468, 2460, 2484, 2480),
    3: deck(2475, 2471, 2465, 2457, 4340, 4336, 4380, 4376, 2467, 2459, 2483, 2479),
    4: deck(2486, 2485, 2488, 2487, 4343, 4342, 4383, 4382, 2490, 2489, 2492, 2491),
    5: deck(2493, 2494, 2495, 2496, 4344, 4345, 4384, 4385, 2497, 2498, 0, 0),
}
for p, d in SUSP_CONC.items():
    BRIDGE_DECKS[(3, p)] = d

# Suspensión acero / amarilla (mismas piezas, paleta distinta en juego; PNG base).
for bt in (4, 5):
    for p, d in SUSP_CONC.items():
        BRIDGE_DECKS[(bt, p)] = d

# Cantilever acero/brown/red: las formas cambian por pieza, no por color.
CAN_MID = deck(2508, 2511, 2514, 2517, 4347, 4350, 4387, 4390, 2520, 2523, 2526, 2527)
CAN_NORTH = deck(2509, 2510, 2515, 2516, 4348, 4349, 4388, 4389, 2521, 2522, 0, 0)
CAN_SOUTH = deck(2507, 2512, 2513, 2518, 4346, 4351, 4386, 4391, 2519, 2524, 2525, 2528)

for bt in (6, 7, 8):
    BRIDGE_DECKS[(bt, 0)] = CAN_NORTH
    BRIDGE_DECKS[(bt, 1)] = CAN_SOUTH
    for p in (2, 3, 4, 5):
        BRIDGE_DECKS[(bt, p)] = CAN_MID

# Tubular (BEG/MID/END como cantilever en layout).
TUB_MID = deck(2570, 2573, 2576, 2579, 2582, 2585, 2588, 2591, 2560, 2563, 2566, 2567)
TUB_NORTH = deck(2571, 2572, 2577, 2578, 2583, 2584, 2589, 2590, 2561, 2562, 0, 0)
TUB_SOUTH = deck(2569, 2574, 2575, 2580, 2581, 2586, 2587, 2592, 2559, 2564, 2565, 2568)

for bt in (10, 11, 12):
    BRIDGE_DECKS[(bt, 0)] = TUB_NORTH
    BRIDGE_DECKS[(bt, 1)] = TUB_SOUTH
    for p in (2, 3, 4, 5):
        BRIDGE_DECKS[(bt, p)] = TUB_MID

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


def load_sheet(path: Path, mode: str) -> Image.Image:
    img = Image.open(path)
    if img.mode == "P":
        if mode != "32bpp":
            return indexed_dos_to_rgba(img)
        pal = img.getpalette()
        key = tuple(pal[0:3]) if pal else None
        out = img.convert("RGBA")
        if key:
            data = [
                (0, 0, 0, 0) if px[:3] == key else px for px in out.get_flattened_data()
            ]
            out.putdata(data)
        return out
    return img.convert("RGBA") if mode == "32bpp" else dematte_legacy_colorkey(img)


def crop_sprite(
    sid: int,
    rects: dict[int, SpriteRect],
    sprites_dir: Path,
    sheets: dict[str, Image.Image],
    mode: str,
) -> Image.Image | None:
    if sid == 0 or sid not in rects:
        return None
    rect = rects[sid]
    if rect.w <= 0 or rect.h <= 0:
        return None
    if rect.sheet not in sheets:
        p = sprites_dir / rect.sheet
        alt = p.with_suffix(".pcx")
        load_path = alt if alt.is_file() else p
        if not load_path.is_file():
            return None
        sheets[rect.sheet] = load_sheet(load_path, mode)
    crop = sheets[rect.sheet].crop((rect.x, rect.y, rect.x + rect.w, rect.y + rect.h))
    return crop


def main() -> None:
    nfo = parse_sprite_offs(REPO)
    sprites_dir, nfo_path, mode = opengfx_paths(REPO)
    print(f"NFO base: {nfo_path.relative_to(REPO)} ({mode})", file=sys.stderr)

    all_ids: set[int] = set()
    for d in BRIDGE_DECKS.values():
        all_ids.update(x for x in d if x != 0)
    all_ids.update(WOOD_RAMP_IDS)
    all_ids.update(GENERIC_RAMP_IDS)
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

    rects = parse_global_sprite_rects(nfo_path, mode)
    prefer = mode

    sheets: dict[str, Image.Image] = {}
    meta_by_id: dict[int, tuple[float, float, float, float]] = {}

    for sid in sorted(all_ids):
        out_name = legacy.get(sid, f"bridge_{sid}.png")
        crop = crop_sprite(sid, rects, sprites_dir, sheets, mode)
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
        "use openttdrs_core::{BridgePiece, BridgeType, RailType};",
        "",
        "/// Sprites de tablero: rear rail/road/mono/maglev X/Y, front X/Y y pillar X/Y.",
        "pub struct BridgeDeckSpriteIds {",
        "    pub rear_rail: [u32; 2],",
        "    pub rear_road: [u32; 2],",
        "    pub rear_mono: [u32; 2],",
        "    pub rear_maglev: [u32; 2],",
        "    pub front: [u32; 2],",
        "    pub pillar: [u32; 2],",
        "}",
        "",
        "impl BridgeDeckSpriteIds {",
        "    pub const fn empty() -> Self {",
        "        Self {",
        "            rear_rail: [0, 0],",
        "            rear_road: [0, 0],",
        "            rear_mono: [0, 0],",
        "            rear_maglev: [0, 0],",
        "            front: [0, 0],",
        "            pillar: [0, 0],",
        "        }",
        "    }",
        "",
        "    pub const fn rear(&self, rail: bool, axis: usize) -> u32 {",
        "        if rail { self.rear_rail[axis] } else { self.rear_road[axis] }",
        "    }",
        "",
        "    /// Capa trasera integrada que corresponde al medio de transporte.",
        "    pub const fn rear_for_transport(&self, rail: bool, rail_type: RailType, axis: usize) -> u32 {",
        "        if !rail { return self.rear_road[axis]; }",
        "        match rail_type {",
        "            RailType::Rail | RailType::Electric => self.rear_rail[axis],",
        "            RailType::Monorail => self.rear_mono[axis],",
        "            RailType::Maglev => self.rear_maglev[axis],",
        "        }",
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
            "/// Sprite de cabeza/rampa vanilla (`BRIDGE_PIECE_HEAD`).",
            "///",
            "/// La cabeza contiene la transición del suelo al tablero; no puede",
            "/// sustituirse por el sprite recto del vano. `dir` son los bits bajos de m5.",
            "pub fn bridge_ramp_sprite_id(",
            "    bridge_type: BridgeType,",
            "    rail: bool,",
            "    rail_type: RailType,",
            "    tileh: u8,",
            "    dir: u8,",
            ") -> u32 {",
            "    // GetBridgeRampDirectionBaseOffset: SW, SE, NE, NW.",
            "    let direction = [2usize, 1, 0, 3][usize::from(dir & 3)];",
            "    let transport = if !rail { 1 } else {",
            "        match rail_type {",
            "            RailType::Rail | RailType::Electric => 0,",
            "            RailType::Monorail => 2,",
            "            RailType::Maglev => 3,",
            "        }",
            "    };",
            "    let flat = usize::from(tileh == 0);",
            "    let table = if bridge_type == BridgeType::Wooden {",
            "        // `_bridge_sprite_table_wood_heads`.",
            "        [[2542, 2541, 2544, 2543], [2538, 2537, 2539, 2540],",
            "         [2534, 2533, 2536, 2535], [2530, 2529, 2531, 2532],",
            "         [4357, 4356, 4359, 4358], [4353, 4352, 4354, 4355],",
            "         [4397, 4396, 4399, 4398], [4393, 4392, 4394, 4395]][transport * 2 + flat]",
            "    } else {",
            "        // `_bridge_sprite_table_generic_*_heads` (la paleta define el puente).",
            "        [[2438, 2440, 2437, 2439], [2442, 2444, 2441, 2443],",
            "         [2446, 2448, 2445, 2447], [2450, 2452, 2449, 2451],",
            "         [4327, 4329, 4326, 4328], [4331, 4333, 4330, 4332],",
            "         [4367, 4369, 4366, 4368], [4371, 4373, 4370, 4372]][transport * 2 + flat]",
            "    };",
            "    table[direction]",
            "}",
            "",
            "/// Compatibilidad con los tests y consumidores anteriores de madera.",
            "pub fn wooden_bridge_ramp_sprite_id(rail: bool, tileh: u8, dir: u8) -> u32 {",
            "    bridge_ramp_sprite_id(BridgeType::Wooden, rail, RailType::Rail, tileh, dir)",
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
                f"rear_road: [{d[2]}, {d[3]}], rear_mono: [{d[4]}, {d[5]}], "
                f"rear_maglev: [{d[6]}, {d[7]}], front: [{d[8]}, {d[9]}], "
                f"pillar: [{d[10]}, {d[11]}] }},"
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
