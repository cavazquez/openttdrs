#!/usr/bin/env bash
# Descarga OpenGFX — gráficos de reemplazo libre para OpenTTD.
#
# Los archivos se extraen en assets/opengfx/ (carpeta ignorada por git).
# Luego extrae los sprites de tesela a assets/opengfx/tiles/ para el renderer.
# Versión configurable con la variable de entorno OPENGFX_VERSION.
#
# Uso:
#   ./scripts/descargar_graficos.sh
#   OPENGFX_VERSION=7.1 ./scripts/descargar_graficos.sh
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION="${OPENGFX_VERSION:-8.0}"
DEST="${ROOT}/assets/opengfx"
CDN="https://cdn.openttd.org/opengfx-releases/${VERSION}/opengfx-${VERSION}-all.zip"

if [[ -d "${DEST}" && -n "$(ls -A "${DEST}" 2>/dev/null)" ]]; then
  echo "OpenGFX ya está en ${DEST}. Borrá la carpeta para re-descargar."
else
  mkdir -p "${DEST}"

  echo "Descargando OpenGFX ${VERSION} desde ${CDN} ..."
  TMP="$(mktemp -d)"
  trap 'rm -rf "${TMP}"' EXIT

  curl -fL "${CDN}" -o "${TMP}/opengfx.zip"
  unzip -q "${TMP}/opengfx.zip" -d "${TMP}/opengfx"

  shopt -s dotglob
  cp -r "${TMP}/opengfx/"*/* "${DEST}/" 2>/dev/null || cp -r "${TMP}/opengfx/"* "${DEST}/"

  echo ""
  echo "Archivos descargados en ${DEST}/:"
  ls -1 "${DEST}/"
fi

# ── Extracción de sprites de tesela para el renderer isométrico ───────────────
SPRITES_DIR="${DEST}/opengfx-${VERSION}/sprites"
TILES_DIR="${DEST}/tiles"

if [[ ! -f "${SPRITES_DIR}/ogfx1_base00.png" ]]; then
  if command -v grfcodec &>/dev/null; then
    echo ""
    echo "Decodificando ogfx1_base.grf con grfcodec..."
    mkdir -p "${SPRITES_DIR}"
    grfcodec -d -p 2 "${DEST}/opengfx-${VERSION}/ogfx1_base.grf" \
      -o "${SPRITES_DIR}/" 2>/dev/null || true
  else
    echo ""
    echo "grfcodec no encontrado; instalalo con: sudo apt install grfcodec"
  fi
fi

if [[ -f "${SPRITES_DIR}/ogfx1_base00.png" ]]; then
  echo ""
  echo "Extrayendo sprites de tesela a ${TILES_DIR}/..."
  export SPRITES_DIR TILES_DIR
  python3 - <<'PYEOF'
import os, re
from pathlib import Path
from PIL import Image

sprites_dir = Path(os.environ["SPRITES_DIR"])
tiles_dir   = Path(os.environ["TILES_DIR"])
tiles_dir.mkdir(parents=True, exist_ok=True)

src = sprites_dir / "ogfx1_base00.png"
img = Image.open(src)

if img.mode == "P":
    pal = img.getpalette()
    transparent_rgb = tuple(pal[0:3])
    img_rgba = img.convert("RGBA")
    data = list(img_rgba.getdata())
    data = [(0, 0, 0, 0) if (r, g, b) == transparent_rgb else (r, g, b, a)
            for r, g, b, a in data]
    img_rgba.putdata(data)
else:
    img_rgba = img.convert("RGBA")

nfo_path = sprites_dir / "ogfx1_base.nfo"
sprite_rect = {}
if nfo_path.is_file():
    pat = re.compile(
        r"^\s*(\d+)\s+sprites/ogfx1_base00\.png\s+8bpp\s+"
        r"(\d+)\s+(\d+)\s+(\d+)\s+(\d+)\s+(-?\d+)\s+(-?\d+)"
    )
    for line in nfo_path.read_text(errors="replace").splitlines():
        m = pat.match(line)
        if m:
            sid = int(m.group(1))
            sprite_rect[sid] = tuple(int(m.group(i)) for i in range(2, 8))


def crop_by_id(sid: int, out_name: str) -> None:
    if sid not in sprite_rect:
        print(f"  (omitido {out_name}: sprite {sid} no en NFO)")
        return
    x, y, w, h, xr, yr = sprite_rect[sid]
    crop = img_rgba.crop((x, y, x + w, y + h))
    out = tiles_dir / out_name
    crop.save(out)
    print(f"  {out_name} ({w}×{h} xrel={xr} yrel={yr}) ← sprite {sid}")


# =============================================================================
# TERRENO BASE (MP_CLEAR)
# =============================================================================
crop_by_id(3924, "terrain_bare.png")           # SPR_FLAT_BARE_LAND
crop_by_id(3943, "terrain_grass_1_3.png")      # SPR_FLAT_1_THIRD_GRASS_TILE
crop_by_id(3962, "terrain_grass_2_3.png")      # SPR_FLAT_2_THIRD_GRASS_TILE
crop_by_id(3981, "terrain_grass.png")          # SPR_FLAT_GRASS_TILE
# Pendientes de grass: SPR_FLAT_GRASS_TILE+1..+14 (tileh 1-14)
for tileh in range(1, 15):
    crop_by_id(3981 + tileh, f"terrain_grass_slope_{tileh:02d}.png")
crop_by_id(4000, "terrain_rough.png")          # SPR_FLAT_ROUGH_LAND
# Pendientes de rough: SPR_FLAT_ROUGH_LAND+1..+14
for tileh in range(1, 15):
    crop_by_id(4000 + tileh, f"terrain_rough_slope_{tileh:02d}.png")
for i, sid in enumerate([4019, 4020, 4021, 4022]):
    crop_by_id(sid, f"terrain_rough_{i+1}.png")
crop_by_id(4023, "terrain_rocky_1.png")        # SPR_FLAT_ROCKY_LAND_1
crop_by_id(4042, "terrain_rocky_2.png")        # SPR_FLAT_ROCKY_LAND_2
# Nieve/desierto
crop_by_id(4493, "terrain_snow_1_4.png")
crop_by_id(4512, "terrain_snow_2_4.png")
crop_by_id(4531, "terrain_snow_3_4.png")
crop_by_id(4550, "terrain_snow_full.png")

# =============================================================================
# AGUA (MP_WATER)
# =============================================================================
crop_by_id(4061, "water_flat.png")             # SPR_FLAT_WATER_TILE
# Costas originales (4062-4069)
for i, sid in enumerate(range(4062, 4070)):
    crop_by_id(sid, f"shore_{i}.png")
# Ship depot
crop_by_id(4070, "ship_depot_se_front.png")
crop_by_id(4071, "ship_depot_sw_front.png")
crop_by_id(4072, "ship_depot_nw.png")
crop_by_id(4073, "ship_depot_ne.png")
crop_by_id(4074, "ship_depot_se_rear.png")
crop_by_id(4075, "ship_depot_sw_rear.png")
crop_by_id(4076, "buoy.png")

# =============================================================================
# CARRETERAS (MP_ROAD)
# =============================================================================
# Carretera plana: SPR_ROAD_Y (1332) + offset → 19 variantes
for sid in range(1332, 1351):
    crop_by_id(sid, f"road_flat_{sid - 1332:02d}.png")
# Carretera con nieve
crop_by_id(1351, "road_y_snow.png")
crop_by_id(1352, "road_x_snow.png")
# Depósito de carretera (4 direcciones)
for i, sid in enumerate(range(1408, 1412)):
    crop_by_id(sid, f"road_depot_{i}.png")

# =============================================================================
# VÍAS FÉRREAS (MP_RAILWAY)
# =============================================================================
# Piezas sueltas para overlays en junctions
for sid in range(1005, 1011):
    crop_by_id(sid, f"rail_single_{sid - 1005}.png")
# Vías combinadas (suelo + raíles)
for sid in range(1011, 1023):
    crop_by_id(sid, f"rail_track_{sid - 1011}.png")
# Vías HORZ/VERT
crop_by_id(1035, "rail_track_ns.png")
crop_by_id(1036, "rail_track_ns_1.png")
# Nieve
crop_by_id(1037, "rail_track_y_snow.png")
crop_by_id(1038, "rail_track_x_snow.png")
# Depósitos de tren
crop_by_id(1063, "rail_depot_se_1.png")
crop_by_id(1064, "rail_depot_se_2.png")
crop_by_id(1065, "rail_depot_sw_1.png")
crop_by_id(1066, "rail_depot_sw_2.png")
crop_by_id(1067, "rail_depot_ne.png")
crop_by_id(1068, "rail_depot_nw.png")
# Estaciones de tren: plataformas
crop_by_id(1069, "rail_platform_y_front.png")
crop_by_id(1070, "rail_platform_x_rear.png")
crop_by_id(1071, "rail_platform_y_rear.png")
crop_by_id(1072, "rail_platform_x_front.png")
crop_by_id(1073, "rail_platform_building_x.png")
crop_by_id(1074, "rail_platform_building_y.png")
crop_by_id(1075, "rail_platform_pillars_y_front.png")
crop_by_id(1076, "rail_platform_pillars_x_rear.png")
crop_by_id(1077, "rail_platform_pillars_y_rear.png")
crop_by_id(1078, "rail_platform_pillars_x_front.png")
# Techos
for sid in range(1079, 1087):
    crop_by_id(sid, f"rail_roof_{sid - 1079}.png")
# Monorraíl
for sid in range(1087, 1093):
    crop_by_id(sid, f"mono_single_{sid - 1087}.png")
crop_by_id(1093, "mono_track_y.png")
crop_by_id(1094, "mono_track_x.png")
for sid in range(1100, 1118):
    crop_by_id(sid, f"mono_track_{sid - 1100}.png")
# Maglev
for sid in range(1169, 1175):
    crop_by_id(sid, f"mglv_single_{sid - 1169}.png")
crop_by_id(1175, "mglv_track_y.png")
crop_by_id(1176, "mglv_track_x.png")
for sid in range(1182, 1200):
    crop_by_id(sid, f"mglv_track_{sid - 1182}.png")
# Vallas de vía
for sid in range(1301, 1309):
    crop_by_id(sid, f"track_fence_{sid - 1301}.png")
# Cruces a nivel
crop_by_id(1370, "crossing_rail_x.png")
crop_by_id(1382, "crossing_mono_x.png")
crop_by_id(1394, "crossing_mglv_x.png")

# =============================================================================
# PARADAS DE CARRETERA (BUS/TRUCK)
# =============================================================================
dirs = ["ne", "se", "sw", "nw"]
# Bus stops
for i, sid in enumerate([2692, 2693, 2694, 2695]):
    crop_by_id(sid, f"bus_stop_{dirs[i]}_ground.png")
for i, sid in enumerate([2696, 2697, 2698, 2699]):
    crop_by_id(sid, f"bus_stop_{dirs[i]}_build_a.png")
for i, sid in enumerate([2700, 2701, 2702, 2703]):
    crop_by_id(sid, f"bus_stop_{dirs[i]}_build_b.png")
for i, sid in enumerate([2704, 2705, 2706, 2707]):
    crop_by_id(sid, f"bus_stop_{dirs[i]}_build_c.png")
# Truck stops
for i, sid in enumerate([2708, 2709, 2710, 2711]):
    crop_by_id(sid, f"truck_stop_{dirs[i]}_ground.png")
for i, sid in enumerate([2712, 2713, 2714, 2715]):
    crop_by_id(sid, f"truck_stop_{dirs[i]}_build_a.png")
for i, sid in enumerate([2716, 2717, 2718, 2719]):
    crop_by_id(sid, f"truck_stop_{dirs[i]}_build_b.png")
for i, sid in enumerate([2720, 2721, 2722, 2723]):
    crop_by_id(sid, f"truck_stop_{dirs[i]}_build_c.png")

# =============================================================================
# CASAS URBANAS (MP_HOUSE)
# =============================================================================
# Tall Office
crop_by_id(1421, "house_talloffice_cnst1.png")
crop_by_id(1422, "house_talloffice_cnst2.png")
crop_by_id(1423, "house_talloffice_cnst3.png")
crop_by_id(1424, "house_talloffice_ground.png")
crop_by_id(1425, "house_talloffice_build.png")
# Office 01
crop_by_id(1426, "house_office01_cnst1.png")
crop_by_id(1427, "house_office01_cnst2.png")
crop_by_id(1428, "house_office01_build.png")
crop_by_id(1429, "house_office01_ground.png")
# Small Block Flats
crop_by_id(1430, "house_smlflats_cnst1.png")
crop_by_id(1431, "house_smlflats_cnst2.png")
crop_by_id(1432, "house_smlflats_build.png")
crop_by_id(1433, "house_smlflats_ground.png")
# Church
crop_by_id(1434, "house_church_cnst1.png")
crop_by_id(1435, "house_church_cnst2.png")
crop_by_id(1436, "house_church_build.png")
crop_by_id(1437, "house_church_ground.png")
# Large Office
crop_by_id(1440, "house_largeoffice_cnst1.png")
crop_by_id(1441, "house_largeoffice_cnst2.png")
crop_by_id(1442, "house_largeoffice_build.png")
# Lift (ascensor animado)
crop_by_id(1443, "house_lift.png")
# Townhouse V1
crop_by_id(1444, "house_townhouse_v1_cnst1.png")
crop_by_id(1445, "house_townhouse_v1_cnst2.png")
crop_by_id(1446, "house_townhouse_v1_build.png")
crop_by_id(1447, "house_townhouse_v1_ground.png")
# Hotel
crop_by_id(1448, "house_hotel_nw_cnst1.png")
crop_by_id(1449, "house_hotel_nw_cnst2.png")
crop_by_id(1450, "house_hotel_nw_build.png")
crop_by_id(1451, "house_hotel_se_cnst1.png")
crop_by_id(1452, "house_hotel_se_cnst2.png")
crop_by_id(1453, "house_hotel_se_build.png")
# Decorativos
crop_by_id(1454, "house_statue_horse.png")
crop_by_id(1455, "house_fountain.png")
crop_by_id(1456, "house_parkstatue.png")
crop_by_id(1457, "house_parkalley.png")
# Shop/Office
crop_by_id(1458, "house_office0d_cnst1.png")
crop_by_id(1459, "house_office0d_cnst2.png")
crop_by_id(1460, "house_office0d_build.png")
crop_by_id(1461, "house_shopoffice0e_cnst1.png")
crop_by_id(1462, "house_shopoffice0e_cnst2.png")
crop_by_id(1463, "house_shopoffice0e_build.png")
crop_by_id(1464, "house_shopoffice0f_cnst1.png")
crop_by_id(1465, "house_shopoffice0f_cnst2.png")
crop_by_id(1466, "house_shopoffice0f_build.png")
# Stadium
crop_by_id(1479, "house_stadium_n.png")
crop_by_id(1480, "house_stadium_e.png")
crop_by_id(1481, "house_stadium_w.png")
crop_by_id(1482, "house_stadium_s.png")
# Townhouse V2
crop_by_id(1501, "house_townhouse_v2_cnst1.png")
crop_by_id(1502, "house_townhouse_v2_pipes.png")
crop_by_id(1503, "house_townhouse_v2_cnst2_g.png")
crop_by_id(1504, "house_townhouse_v2_cnst2.png")
crop_by_id(1505, "house_townhouse_v2_ground.png")
crop_by_id(1506, "house_townhouse_v2_build.png")
# Suelo concreto (SPR_CONCRETE_GROUND) y variante de Large Office
crop_by_id(1311, "house_concrete_ground.png")
crop_by_id(4569, "house_largeoffice_v2.png")

# =============================================================================
# ÁRBOLES (MP_TREES)
# =============================================================================
# Templado (muestras de diferentes tipos y etapas)
tree_ids = [
    1576, 1577, 1578, 1579, 1580, 1581, 1582, 1583,  # Tipo 1
    1584, 1585, 1586, 1587, 1588, 1589,              # Tipo 2
    1590, 1591, 1592, 1593, 1594, 1595, 1596,        # Tipo 3
    1597, 1598, 1599, 1600, 1601, 1602, 1603,        # Tipo 4
    1604, 1605, 1606, 1607, 1608, 1609, 1610,        # Tipo 5
    1611, 1612, 1613, 1614, 1615, 1616, 1617,        # Tipo 6
]
for i, sid in enumerate(tree_ids):
    crop_by_id(sid, f"tree_{i:02d}.png")

# =============================================================================
# INDUSTRIAS — todos los sprites de edificios por sprite_id
# Nombres: industry_{sprite_id}.png
# Nota: se imprimen xrel/yrel del NFO para calibrar INDUSTRY_GFX_DATA en sprites.rs
# =============================================================================
# gfx 0-3: Coal Mine (headframe, torre, aux, pequeño)
for sid in [2013, 2015, 2018, 2021]:
    crop_by_id(sid, f"industry_{sid}.png")
# gfx 7-10: Power Station
for sid in [2047, 2050, 2053, 2054]:
    crop_by_id(sid, f"industry_{sid}.png")
# gfx 11-15: Sawmill
for sid in [2063, 2066, 2069, 2070, 2071]:
    crop_by_id(sid, f"industry_{sid}.png")
# gfx 16-23: Oil Refinery
for sid in [2075, 2076, 2080, 2083, 2086, 2089, 2092, 2095]:
    crop_by_id(sid, f"industry_{sid}.png")
# gfx 25-28: Forest (árboles industriales)
for sid in [2099, 2100, 2101, 2102]:
    crop_by_id(sid, f"industry_{sid}.png")
# gfx 29-32: Printing Works
for sid in [2174, 2177, 2178]:
    crop_by_id(sid, f"industry_{sid}.png")
# gfx 33-38: Oil Rig
for sid in [2108, 2109, 2111, 2113, 2115, 2117]:
    crop_by_id(sid, f"industry_{sid}.png")
# gfx 39-41: Steel Mill
for sid in [2150, 2151, 2152]:
    crop_by_id(sid, f"industry_{sid}.png")
# gfx 43-46: Factory
for sid in [2169, 2170, 2171, 2172]:
    crop_by_id(sid, f"industry_{sid}.png")
# gfx 47-51: Oil Wells
for sid in [2028, 2030, 2033, 2036, 2039]:
    crop_by_id(sid, f"industry_{sid}.png")
# gfx 52-57: Farm (edificios + granero)
for sid in [2119, 2121, 2123, 2126, 2128]:
    crop_by_id(sid, f"industry_{sid}.png")
# gfx 58-59: Bank (templado)
for sid in [2180, 2181]:
    crop_by_id(sid, f"industry_{sid}.png")
# gfx 60-71: Copper Ore Mine
for sid in [2190, 2193, 2196, 2199, 2202, 2205, 2206, 2208, 2209, 2212, 2213, 2214]:
    crop_by_id(sid, f"industry_{sid}.png")
# gfx 72-88: Plantaciones/otros climas (algunos tiles con edificio)
for sid in [2247, 2249, 2250, 2263, 2265]:
    crop_by_id(sid, f"industry_{sid}.png")
# gfx 89-90: Gold Mine
for sid in [2186, 2187]:
    crop_by_id(sid, f"industry_{sid}.png")
# gfx 91-99: Iron Ore Mine
for sid in [2284, 2285, 2286, 2287, 2290]:
    crop_by_id(sid, f"industry_{sid}.png")
# gfx 116-119: Otros climas con edificio
for sid in [2342, 2343, 2349, 2352]:
    crop_by_id(sid, f"industry_{sid}.png")

# =============================================================================
# AEROPUERTOS
# =============================================================================
crop_by_id(2633, "airport_heliport.png")
crop_by_id(2634, "airport_apron.png")
crop_by_id(2635, "airport_stand.png")
for i, sid in enumerate(range(2636, 2645)):
    crop_by_id(sid, f"airport_taxiway_{i}.png")
for i, sid in enumerate(range(2645, 2650)):
    crop_by_id(sid, f"airport_runway_{i}.png")
crop_by_id(2650, "airport_terminal_a.png")
crop_by_id(2651, "airport_tower.png")
crop_by_id(2652, "airport_concourse.png")
crop_by_id(2653, "airport_terminal_b.png")
crop_by_id(2654, "airport_terminal_c.png")
crop_by_id(2655, "airport_hangar_front.png")
crop_by_id(2656, "airport_hangar_rear.png")
crop_by_id(2657, "airfield_hangar_front.png")
crop_by_id(2658, "airfield_hangar_rear.png")
# Radar (animado, 12 frames)
for i, sid in enumerate(range(2680, 2692)):
    crop_by_id(sid, f"airport_radar_{i:02d}.png")

# =============================================================================
# MUELLES
# =============================================================================
crop_by_id(2727, "dock_slope_ne.png")
crop_by_id(2728, "dock_slope_se.png")
crop_by_id(2729, "dock_slope_sw.png")
crop_by_id(2730, "dock_slope_nw.png")
crop_by_id(2731, "dock_flat_x.png")
crop_by_id(2732, "dock_flat_y.png")

# =============================================================================
# TÚNELES Y PUENTES
# =============================================================================
crop_by_id(2365, "tunnel_rail_rear.png")
crop_by_id(2373, "tunnel_mono_rear.png")
crop_by_id(2381, "tunnel_mglv_rear.png")
crop_by_id(2389, "tunnel_road_rear.png")
# Puente de madera
crop_by_id(2545, "bridge_wood_rail_y.png")
crop_by_id(2546, "bridge_wood_rail_x.png")
crop_by_id(2547, "bridge_wood_road_y.png")
crop_by_id(2548, "bridge_wood_road_x.png")
crop_by_id(2549, "bridge_wood_y_front.png")
crop_by_id(2550, "bridge_wood_x_front.png")
crop_by_id(2551, "bridge_wood_y_pillar.png")
crop_by_id(2552, "bridge_wood_x_pillar.png")

# =============================================================================
# OBJETOS ESPECIALES
# =============================================================================
crop_by_id(2601, "object_transmitter.png")
crop_by_id(2602, "object_lighthouse.png")
# HQ Tiny
for i, sid in enumerate(range(2603, 2607)):
    crop_by_id(sid, f"hq_tiny_{i}.png")
# HQ Small
for i, sid in enumerate(range(2607, 2611)):
    crop_by_id(sid, f"hq_small_{i}.png")
# HQ Medium
for i, sid in enumerate(range(2611, 2618)):
    crop_by_id(sid, f"hq_medium_{i}.png")
# HQ Large
for i, sid in enumerate(range(2618, 2625)):
    crop_by_id(sid, f"hq_large_{i}.png")
# HQ Huge
for i, sid in enumerate(range(2625, 2632)):
    crop_by_id(sid, f"hq_huge_{i}.png")
crop_by_id(2632, "object_statue_company.png")
crop_by_id(1420, "object_concrete.png")
crop_by_id(4790, "object_bought_land.png")

# =============================================================================
# VEHÍCULOS (muestras para debug/overlay)
# =============================================================================
# Camiones básicos - usaremos coords fijas o buscar IDs específicos
# Los vehículos están en rangos complejos, por ahora solo algunos ejemplos
crop_by_id(3097, "vehicle_bus_sw.png")
crop_by_id(3098, "vehicle_bus_side.png")

# =============================================================================
# LEGACY (coords fijas para compatibilidad con código existente)
# =============================================================================
legacy = {
    "grass":       (610, 13496, 64, 31),
    "grass_rough": (562, 13640, 64, 31),
    "water":       (402, 14392, 64, 31),
    "truck":       (594, 12408,  8, 16),
}

for name, (x, y, w, h) in legacy.items():
    crop = img_rgba.crop((x, y, x + w, y + h))
    crop.save(tiles_dir / f"{name}.png")
    print(f"  {name}.png ({w}×{h})")

print(f"Sprites listos en {tiles_dir}/")
PYEOF
else
  echo ""
  echo "Hoja de sprites no disponible; asegurate de tener grfcodec instalado."
  echo "Los sprites ya extraídos en ${TILES_DIR}/ se usarán si existen."
fi

echo ""
echo "¡Listo! Para abrir el juego: cargo run -p openttdrs-client"
