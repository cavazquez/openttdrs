#!/usr/bin/env bash
# Descarga gráficos base para OpenTTD/OpenTTDRS.
#
# Los archivos se extraen en assets/opengfx/ (carpeta ignorada por git).
# Luego extrae los sprites de tesela a assets/opengfx/tiles/ para el renderer.
# Cada ejecución limpia esa salida en assets (no la caché en .downloads/).
# Modos:
#   - --8bpp  : OpenGFX clásico.
#   - --32bpp : OpenGFX2 High Def.
#
# Uso:
#   ./scripts/descargar_graficos.sh --8bpp
#   ./scripts/descargar_graficos.sh --32bpp
#   OPENGFX_VERSION=7.1 ./scripts/descargar_graficos.sh --8bpp
set -euo pipefail

usage() {
  cat <<'EOF'
Uso:
  ./scripts/descargar_graficos.sh --8bpp
  ./scripts/descargar_graficos.sh --32bpp

Opciones:
  --8bpp     Descarga y procesa OpenGFX clásico (8bpp)
  --32bpp    Descarga y procesa OpenGFX2 High Def (32bpp)
  -h, --help Muestra esta ayuda

Notas:
  - Debés elegir exactamente una opción de modo.
  - OPENGFX_VERSION aplica solo a --8bpp.
  - OPENGFX2_TAG aplica solo a --32bpp.
  - Si --32bpp falla con \"tar: Fin de archivo inesperada\", borrá el .tar en
    .downloads/openttd/ y volvé a ejecutar (el script también detecta tars inválidos).
EOF
}

GRAPHICS_MODE=""
for arg in "$@"; do
  case "$arg" in
    --8bpp)
      if [[ -n "${GRAPHICS_MODE}" ]]; then
        echo "Error: elegí solo un modo (--8bpp o --32bpp)." >&2
        usage
        exit 1
      fi
      GRAPHICS_MODE="8bpp"
      ;;
    --32bpp)
      if [[ -n "${GRAPHICS_MODE}" ]]; then
        echo "Error: elegí solo un modo (--8bpp o --32bpp)." >&2
        usage
        exit 1
      fi
      GRAPHICS_MODE="32bpp"
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Error: opción desconocida '${arg}'." >&2
      usage
      exit 1
      ;;
  esac
done

if [[ -z "${GRAPHICS_MODE}" ]]; then
  echo "Error: debés indicar un modo (--8bpp o --32bpp)." >&2
  usage
  exit 1
fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION="${OPENGFX_VERSION:-8.0}"
DEST="${ROOT}/assets/opengfx"
DOWNLOADS_DIR="${ROOT}/.downloads/openttd"
OPENGFX2_TAG="${OPENGFX2_TAG:-v0.1}"
CDN_8BPP="https://cdn.openttd.org/opengfx-releases/${VERSION}/opengfx-${VERSION}-all.zip"
CDN_32BPP="https://github.com/OpenTTD/OpenGFX2/releases/download/${OPENGFX2_TAG}/opengfx2_32ez.tar"
ZIP_CACHE_8BPP="${DOWNLOADS_DIR}/opengfx-${VERSION}-all.zip"
TAR_CACHE_8BPP="${DOWNLOADS_DIR}/opengfx-${VERSION}.tar"
TAR_CACHE_32BPP="${DOWNLOADS_DIR}/opengfx2-${OPENGFX2_TAG}-32ez.tar"

mkdir -p "${DEST}"
mkdir -p "${DOWNLOADS_DIR}"

echo "Limpiando salida gráfica en ${DEST}/ (tiles PNG y carpetas opengfx-*/opengfx2-*)…"
rm -rf "${DEST}/tiles"
shopt -s nullglob
for d in "${DEST}"/opengfx-* "${DEST}"/opengfx2-*; do
  rm -rf "$d"
done
shopt -u nullglob

if [[ "${GRAPHICS_MODE}" == "32bpp" ]]; then
  # Descargas interrumpidas dejan un .tar truncado; tar falla al extraer.
  if [[ -f "${TAR_CACHE_32BPP}" ]] && ! tar -tf "${TAR_CACHE_32BPP}" >/dev/null 2>&1; then
    echo "Tar en caché inválido o incompleto, se elimina:" >&2
    echo "  ${TAR_CACHE_32BPP}" >&2
    rm -f "${TAR_CACHE_32BPP}"
  fi
  if [[ ! -f "${TAR_CACHE_32BPP}" ]]; then
    echo "Descargando OpenGFX2 High Def (${OPENGFX2_TAG}) desde ${CDN_32BPP} ..."
    curl -fL "${CDN_32BPP}" -o "${TAR_CACHE_32BPP}.part"
    mv -f "${TAR_CACHE_32BPP}.part" "${TAR_CACHE_32BPP}"
  else
    echo "OpenGFX2 32bpp ya descargado en ${TAR_CACHE_32BPP}"
  fi
  if ! tar -tf "${TAR_CACHE_32BPP}" >/dev/null 2>&1; then
    echo "ERROR: ${TAR_CACHE_32BPP} no es un tar válido tras la descarga." >&2
    echo "Borralo manualmente y reintentá (o probá otra red / OPENGFX2_TAG)." >&2
    exit 1
  fi
else
  if [[ -f "${TAR_CACHE_8BPP}" ]]; then
    echo "OpenGFX ${VERSION} ya descargado en ${TAR_CACHE_8BPP}"
  else
    if [[ ! -f "${ZIP_CACHE_8BPP}" ]]; then
      echo "Descargando OpenGFX ${VERSION} desde ${CDN_8BPP} ..."
      curl -fL "${CDN_8BPP}" -o "${ZIP_CACHE_8BPP}"
    else
      echo "Zip en cache detectado: ${ZIP_CACHE_8BPP}"
    fi

    echo "Preparando ${TAR_CACHE_8BPP} ..."
    TMP="$(mktemp -d)"
    trap 'rm -rf "${TMP}"' EXIT
    unzip -q "${ZIP_CACHE_8BPP}" -d "${TMP}/opengfx"
    CANDIDATE_TAR="$(rg --files "${TMP}/opengfx" | rg "opengfx-${VERSION}\\.tar$" | awk 'NR==1{print; exit}' || true)"
    if [[ -z "${CANDIDATE_TAR}" ]]; then
      echo "No encontré opengfx-${VERSION}.tar dentro del zip."
      exit 1
    fi
    cp "${CANDIDATE_TAR}" "${TAR_CACHE_8BPP}"
  fi
fi

echo ""
echo "Cache de descargas en ${DOWNLOADS_DIR}/:"
ls -1 "${DOWNLOADS_DIR}/"
echo ""
echo "Archivos disponibles en ${DEST}/ (assets finales):"
ls -1 "${DEST}/"

# ── Extracción de sprites de tesela para el renderer isométrico ───────────────
if [[ "${GRAPHICS_MODE}" == "32bpp" ]]; then
  BASE_DIR="${DEST}/opengfx2-32ez"
  BASE_TAR="${TAR_CACHE_32BPP}"
  BASE_GRF="${BASE_DIR}/ogfx21_base_32ez.grf"
  NFO_NAME="ogfx21_base_32ez.nfo"
  SHEET_PREFIX="ogfx21_base_32ez"
else
  BASE_DIR="${DEST}/opengfx-${VERSION}"
  BASE_TAR="${TAR_CACHE_8BPP}"
  BASE_GRF="${BASE_DIR}/ogfx1_base.grf"
  NFO_NAME="ogfx1_base.nfo"
  SHEET_PREFIX="ogfx1_base"
fi
SPRITES_DIR="${BASE_DIR}/sprites"
TILES_DIR="${DEST}/tiles"

# En descarga manual/limpia, OpenGFX suele venir como .tar dentro de DEST.
# Si falta la carpeta base pero existe el tar, extraerla automáticamente.
if [[ ! -d "${BASE_DIR}" && -f "${BASE_TAR}" ]]; then
  echo ""
  echo "Extrayendo ${BASE_TAR} ..."
  if [[ "${GRAPHICS_MODE}" == "32bpp" ]]; then
    mkdir -p "${BASE_DIR}"
    if ! tar -xf "${BASE_TAR}" -C "${BASE_DIR}"; then
      echo "ERROR: extracción del tar falló (¿archivo corrupto?)." >&2
      echo "  rm -rf \"${BASE_DIR}\" \"${BASE_TAR}\"" >&2
      echo "  y ejecutá de nuevo: ./scripts/descargar_graficos.sh --32bpp" >&2
      rm -rf "${BASE_DIR}"
      exit 1
    fi
  else
    tar -xf "${BASE_TAR}" -C "${DEST}"
  fi
fi

# Limpieza de layout legado: tars sueltos dentro de assets/.
rm -f "${DEST}/opengfx-${VERSION}.tar" "${DEST}/opengfx2_32ez.tar"

if [[ ! -f "${SPRITES_DIR}/${SHEET_PREFIX}00.png" && ! -f "${SPRITES_DIR}/${SHEET_PREFIX}00.pcx" && ! -f "${SPRITES_DIR}/${SHEET_PREFIX}00.32.png" ]]; then
  if command -v grfcodec &>/dev/null; then
    echo ""
    echo "Decodificando $(basename "${BASE_GRF}") con grfcodec (salida PNG)..."
    mkdir -p "${SPRITES_DIR}"
    if [[ "${GRAPHICS_MODE}" == "32bpp" ]]; then
      grfcodec -d -o png "${BASE_GRF}" "${SPRITES_DIR}/" 2>/dev/null || true
    else
      grfcodec -d -o png -p 2 "${BASE_GRF}" "${SPRITES_DIR}/" 2>/dev/null || true
    fi
  else
    echo ""
    echo "grfcodec no encontrado."
    echo "Instalación recomendada (Ubuntu/Debian): sudo apt update && sudo apt install -y grfcodec"
    echo "Alternativa: descargar binario/fuentes desde https://github.com/OpenTTD/grfcodec"
  fi
fi

if [[ -f "${SPRITES_DIR}/${SHEET_PREFIX}00.png" || -f "${SPRITES_DIR}/${SHEET_PREFIX}00.pcx" || -f "${SPRITES_DIR}/${SHEET_PREFIX}00.32.png" ]]; then
  echo ""
  echo "Extrayendo sprites de tesela a ${TILES_DIR}/..."
  OPENTTDRS_REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
  export SPRITES_DIR TILES_DIR NFO_NAME SHEET_PREFIX GRAPHICS_MODE OPENTTDRS_REPO_ROOT
  python3 - <<'PYEOF'
import os, re, sys
from collections import Counter
from pathlib import Path
from PIL import Image

sprites_dir = Path(os.environ["SPRITES_DIR"])
tiles_dir   = Path(os.environ["TILES_DIR"])
nfo_name    = os.environ["NFO_NAME"]
sheet_prefix = os.environ["SHEET_PREFIX"]
graphics_mode = os.environ["GRAPHICS_MODE"]
tiles_dir.mkdir(parents=True, exist_ok=True)

def write_rail_placeholder(out_path: Path) -> None:
    """Bevy exige ruta existente; si el NFO no declara el sprite, evitamos error con 1×1 transparente."""
    img = Image.new("RGBA", (1, 1), (0, 0, 0, 0))
    img.save(out_path)

def load_sheet(png_path: Path) -> Image.Image:
    if graphics_mode == "32bpp":
        # En 32bpp no aplicar heurística magenta. Pero si el sheet viene en
        # paleta (8bpp fallback), mantener transparencia por índice 0.
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
        # Detecta variantes de colorkey magenta típicas de conversión 8bpp->RGBA.
        # Mantiene una ventana amplia para capturar ruido de cuantización.
        return (
            r >= 220
            and b >= 220
            and g <= 40
            and abs(r - b) <= 24
        )

    img = Image.open(png_path)
    if img.mode == "P":
        pal = img.getpalette()
        transparent_rgb = tuple(pal[0:3])
        img_rgba = img.convert("RGBA")
        data = []
        for r, g, b, a in img_rgba.getdata():
            # Transparencia por índice de paleta 0 y por colorkey magenta.
            if (r, g, b) == transparent_rgb or is_magenta_key(r, g, b):
                data.append((0, 0, 0, 0))
            else:
                data.append((r, g, b, a))
        img_rgba.putdata(data)
        return img_rgba
    img_rgba = img.convert("RGBA")
    # Fallback: también limpiar colorkey magenta en imágenes no palettizadas.
    data = []
    for r, g, b, a in img_rgba.getdata():
        if is_magenta_key(r, g, b):
            data.append((0, 0, 0, 0))
        else:
            data.append((r, g, b, a))
    img_rgba.putdata(data)
    return img_rgba

def cleanup_speckles(img: Image.Image) -> Image.Image:
    src = img.convert("RGBA")
    w, h = src.size
    pix = src.load()
    out = src.copy()
    out_pix = out.load()

    def suspicious(r: int, g: int, b: int, a: int) -> bool:
        if a == 0:
            return False
        # Píxeles típicos de artefacto de paleta (cian/blanco muy brillantes).
        cyan_like = (b >= 170 and g >= 140 and r <= 140)
        white_like = (r >= 210 and g >= 210 and b >= 210)
        return cyan_like or white_like

    for y in range(1, h - 1):
        for x in range(1, w - 1):
            r, g, b, a = pix[x, y]
            if not suspicious(r, g, b, a):
                continue

            neigh = []
            for ny in range(y - 1, y + 2):
                for nx in range(x - 1, x + 2):
                    if nx == x and ny == y:
                        continue
                    nr, ng, nb, na = pix[nx, ny]
                    if na > 0:
                        neigh.append((nr, ng, nb, na))
            if len(neigh) < 5:
                continue

            # Si la mayoría de vecinos son mucho más oscuros, el pixel suele ser ruido.
            lum = lambda c: c[0] * 0.2126 + c[1] * 0.7152 + c[2] * 0.0722
            avg_neigh_lum = sum(lum(c) for c in neigh) / len(neigh)
            if avg_neigh_lum < 155:
                rep = Counter(neigh).most_common(1)[0][0]
                out_pix[x, y] = rep
    return out

# Cargar todos los sheets del prefijo en PNG o PCX (incluye .32.png).
# grfcodec a veces deja `*.32.png` de 0 bytes; ignorarlos para no pisar atlas válidos.
sheets: dict[str, Image.Image] = {}
for p in sorted(sprites_dir.glob(f"{sheet_prefix}*.png")):
    try:
        if p.stat().st_size == 0:
            continue
        sheets[p.name] = load_sheet(p)
    except OSError:
        continue
    except Exception as e:
        print(f"  (omitido sheet {p.name}: {e})", file=sys.stderr)
        continue
for p in sorted(sprites_dir.glob(f"{sheet_prefix}*.pcx")):
    try:
        if p.stat().st_size == 0:
            continue
        sheets[p.name] = load_sheet(p)
    except OSError:
        continue
    except Exception as e:
        print(f"  (omitido sheet {p.name}: {e})", file=sys.stderr)
        continue

# Parsear NFO para todos los sheets del set.
nfo_path = sprites_dir / nfo_name
sprite_rect: dict[int, tuple] = {}  # sid -> (x, y, w, h, xr, yr, sheet_name)
if nfo_path.is_file():
    pat = re.compile(
        r"^\s*(\d+)\s+(\S*?((?:ogfx1_base|ogfx21_base_32ez)\d+\.(?:32\.png|png|pcx)))\s+(?:8bpp|32bpp)\s+"
        r"(\d+)\s+(\d+)\s+(\d+)\s+(\d+)\s+(-?\d+)\s+(-?\d+)"
    )
    for line in nfo_path.read_text(errors="replace").splitlines():
        m = pat.match(line)
        if m:
            sid = int(m.group(1))
            sheet = Path(m.group(2)).name
            sprite_rect[sid] = (int(m.group(4)), int(m.group(5)),
                                 int(m.group(6)), int(m.group(7)),
                                 int(m.group(8)), int(m.group(9)), sheet)


def crop_by_id(sid: int, out_name: str) -> None:
    if sid not in sprite_rect:
        print(f"  (omitido {out_name}: sprite {sid} no en NFO)")
        if out_name.startswith("rail_") and out_name.endswith(".png"):
            p = tiles_dir / out_name
            write_rail_placeholder(p)
            print(f"  → placeholder {out_name}")
        return
    x, y, w, h, xr, yr, sheet = sprite_rect[sid]
    sheet_key = sheet
    # Nota: no forzar ".32.png" aquí. Las coordenadas del NFO generado por
    # grfcodec en este flujo refieren al atlas base; usar .32 directo produce
    # recortes fuera de lugar (huecos celestes masivos en el mapa).
    if sheet_key not in sheets:
        # grfcodec suele generar PCX aunque el NFO refiera PNG.
        alt = Path(sheet).with_suffix(".pcx").name
        if alt in sheets:
            sheet_key = alt
        else:
            print(f"  (omitido {out_name}: sheet {sheet} no encontrado)")
            if out_name.startswith("rail_") and out_name.endswith(".png"):
                write_rail_placeholder(tiles_dir / out_name)
                print(f"  → placeholder {out_name}")
            return
    if sheet_key not in sheets:
        if out_name.startswith("rail_") and out_name.endswith(".png"):
            write_rail_placeholder(tiles_dir / out_name)
            print(f"  → placeholder {out_name} (sheet ausente)")
        return
    crop = sheets[sheet_key].crop((x, y, x + w, y + h))
    # Limpieza de artefactos en sprites de terreno/árboles/vías.
    if graphics_mode != "32bpp" and (
        out_name.startswith("terrain_")
        or out_name.startswith("grass")
        or out_name.startswith("tree_")
        or out_name.startswith("rail_")
        or out_name.startswith("road_")
    ):
        crop = cleanup_speckles(crop)
    out = tiles_dir / out_name
    crop.save(out)
    print(f"  {out_name} ({w}×{h} xrel={xr} yrel={yr}) ← sprite {sid} [{sheet_key}]")


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
# CARRETERAS (MP_ROAD) - alineado con src/sprites/road.rs
# =============================================================================
ROAD_FLAT_RANGE = range(1332, 1351)
ROAD_SNOW_IDS = [1351, 1352]
ROAD_DEPOT_RANGE = range(1408, 1412)

# Carretera plana: SPR_ROAD_Y (1332) + offset -> 19 variantes
for sid in ROAD_FLAT_RANGE:
    crop_by_id(sid, f"road_flat_{sid - 1332:02d}.png")
# Carretera con nieve
crop_by_id(ROAD_SNOW_IDS[0], "road_y_snow.png")
crop_by_id(ROAD_SNOW_IDS[1], "road_x_snow.png")
# Depósito de carretera (4 direcciones)
for i, sid in enumerate(ROAD_DEPOT_RANGE):
    crop_by_id(sid, f"road_depot_{i}.png")

# =============================================================================
# VIAS FERREAS (MP_RAILWAY) - alineado con src/sprites/rail.rs
# =============================================================================
RAIL_SINGLE_RANGE = range(1005, 1011)
RAIL_TRACK_RANGE = range(1011, 1023)
RAIL_WRAPPER_ALIAS_IDS = [
    1005, 1006, 1007, 1008, 1009, 1010,
    1011, 1012, 1013, 1014, 1015, 1016,
    1017, 1018, 1019, 1020, 1021, 1022,
    1035, 1036,
    1370, 1371, 1372, 1373,
]
RAIL_SIGNAL_EXPORT_RANGE = range(1275, 1700)

# Piezas sueltas para overlays en junctions
for sid in RAIL_SINGLE_RANGE:
    crop_by_id(sid, f"rail_single_{sid - 1005}.png")
# Vías combinadas (suelo + raíles)
for sid in RAIL_TRACK_RANGE:
    crop_by_id(sid, f"rail_track_{sid - 1011}.png")
# Alias usados por el cliente Bevy actual (rail_<sprite_id>.png)
for sid in RAIL_WRAPPER_ALIAS_IDS:
    crop_by_id(sid, f"rail_{sid}.png")
# Señales ferroviarias (bloque clásico + PBS: la fórmula del cliente puede superar 1519)
for sid in RAIL_SIGNAL_EXPORT_RANGE:
    crop_by_id(sid, f"rail_{sid}.png")
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
# Alias de suelo usados por el cliente Bevy actual (0..3 = ne,se,sw,nw)
for i, sid in enumerate([2708, 2709, 2710, 2711]):
    crop_by_id(sid, f"truck_stop_ground_{i}.png")
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
# CASAS – sprites por ID numérico para HOUSE_DRAW_DATA (HouseIDs 0-127)
# Nombrados house_s{sprite_id}.png para lookup directo.
# Cubre ground (s1) y building (s2) de los 128 tipos de casa temperate.
# =============================================================================
for sid in [
    # Ground sprites (s1)
    1311, 1424, 1429, 1433, 1437, 1447, 1487, 1489, 1491, 1493,
    1495, 1499, 1505, 1511, 1517, 1522, 1528, 1534, 1536, 1538,
    1544, 1550, 1552, 1574,
    # Building sprites (s2)
    1423, 1425, 1428, 1432, 1436, 1442, 1446, 1450, 1453, 1454,
    1455, 1456, 1457, 1460, 1463, 1466, 1469, 1472, 1475, 1478,
    1483, 1484, 1485, 1486, 1488, 1490, 1492, 1494, 1496, 1500,
    1506, 1512, 1518, 1523, 1529, 1535, 1537, 1539, 1545, 1551,
    1553, 1575, 4569,
]:
    crop_by_id(sid, f"house_s{sid}.png")

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
# INDUSTRIAS — IDs desde industry_gfx_data_generated.rs (suelo + edificio, estadio 3).
# Regenerar tabla: scripts/gen_industry_gfx_data.py
# Listar IDs: scripts/list_industry_sprite_ids.py
# =============================================================================
def load_industry_sprite_ids() -> list[int]:
    root = Path(os.environ["OPENTTDRS_REPO_ROOT"])
    gen = root / "crates" / "openttdrs-client" / "src" / "sprites" / "industry_gfx_data_generated.rs"
    text = gen.read_text(encoding="utf-8")
    ids = set(int(x) for x in re.findall(r"ground_sprite_id:\s*(\d+)", text))
    ids |= set(int(x) for x in re.findall(r"sprite_id:\s*(\d+)", text))
    ids.discard(0)
    return sorted(ids)


INDUSTRY_SPRITE_IDS = load_industry_sprite_ids()

# Nombres: industry_{sprite_id}.png
# Nota: se imprimen xrel/yrel del NFO para calibrar INDUSTRY_GFX_DATA en sprites.rs
for sid in INDUSTRY_SPRITE_IDS:
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
    # Fallback para el cliente actual (usa vehicle_bus_sw.png).
    "vehicle_bus_sw": (594, 12408,  8, 16),
}

if graphics_mode != "32bpp":
    sheet00 = sheets.get("ogfx1_base00.png") or sheets.get("ogfx1_base00.pcx")
    for name, (x, y, w, h) in legacy.items():
        if sheet00 is None:
            print(f"  (omitido {name}: sheet ogfx1_base00.(png|pcx) no encontrado)")
            continue
        crop = sheet00.crop((x, y, x + w, y + h))
        crop.save(tiles_dir / f"{name}.png")
        print(f"  {name}.png ({w}×{h})")
else:
    # Alias requeridos por el cliente actual también en modo 32bpp.
    aliases = {
        "terrain_grass.png": "grass.png",
        "terrain_rough.png": "grass_rough.png",
        "water_flat.png": "water.png",
    }
    for src, dst in aliases.items():
        src_p = tiles_dir / src
        dst_p = tiles_dir / dst
        if not src_p.exists():
            print(f"  (omitido alias {dst}: no existe {src})")
            continue
        img = Image.open(src_p).convert("RGBA")
        img.save(dst_p)
        print(f"  {dst} (alias de {src})")

print(f"Sprites listos en {tiles_dir}/")
PYEOF
else
  echo ""
  echo "Hoja de sprites no disponible (faltan ${SPRITES_DIR}/${SHEET_PREFIX}00.(png|pcx|32.png) y/o hojas relacionadas)."
  echo "Para generarlas, instalá grfcodec y volvé a ejecutar este script:"
  echo "  Ubuntu/Debian: sudo apt update && sudo apt install -y grfcodec"
  echo "  Alternativa: https://github.com/OpenTTD/grfcodec"
  echo "Sin hoja decodificada no se generaron tiles (esta ejecución ya vació ${TILES_DIR}/ al inicio)."
fi

echo ""
echo "¡Listo! Para abrir el juego: cargo run -p openttdrs-client"
