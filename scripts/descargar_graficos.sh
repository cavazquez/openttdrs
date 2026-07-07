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
#   OPENGFX2_TAG=0.8.1 ./scripts/descargar_graficos.sh --32bpp
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
  - OPENGFX2_TAG aplica solo a --32bpp (release GitHub, p. ej. 0.8.1).
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
OPENGFX2_TAG="${OPENGFX2_TAG:-0.8.1}"
CDN_8BPP="https://cdn.openttd.org/opengfx-releases/${VERSION}/opengfx-${VERSION}-all.zip"
if [[ "${OPENGFX2_TAG}" == v0.1 ]]; then
  CDN_32BPP="https://github.com/OpenTTD/OpenGFX2/releases/download/${OPENGFX2_TAG}/opengfx2_32ez.tar"
  TAR_CACHE_32BPP="${DOWNLOADS_DIR}/opengfx2-${OPENGFX2_TAG}-32ez.tar"
else
  CDN_32BPP="https://github.com/OpenTTD/OpenGFX2/releases/download/${OPENGFX2_TAG}/OpenGFX2_HighDef-${OPENGFX2_TAG}.tar"
  TAR_CACHE_32BPP="${DOWNLOADS_DIR}/opengfx2-${OPENGFX2_TAG}-highdef.tar"
fi
ZIP_CACHE_8BPP="${DOWNLOADS_DIR}/opengfx-${VERSION}-all.zip"
TAR_CACHE_8BPP="${DOWNLOADS_DIR}/opengfx-${VERSION}.tar"

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
    TMP_EXTRACT="$(mktemp -d)"
    if ! tar -xf "${BASE_TAR}" -C "${TMP_EXTRACT}"; then
      echo "ERROR: extracción del tar falló (¿archivo corrupto?)." >&2
      echo "  rm -rf \"${BASE_DIR}\" \"${BASE_TAR}\"" >&2
      echo "  y ejecutá de nuevo: ./scripts/descargar_graficos.sh --32bpp" >&2
      rm -rf "${BASE_DIR}" "${TMP_EXTRACT}"
      exit 1
    fi
    if [[ -f "${TMP_EXTRACT}/ogfx21_base_32ez.grf" ]]; then
      cp -a "${TMP_EXTRACT}/." "${BASE_DIR}/"
    else
      INNER="$(find "${TMP_EXTRACT}" -name 'ogfx21_base_32ez.grf' -printf '%h\n' 2>/dev/null | head -1)"
      if [[ -z "${INNER}" || ! -f "${INNER}/ogfx21_base_32ez.grf" ]]; then
        echo "ERROR: no encontré ogfx21_base_32ez.grf dentro de ${BASE_TAR}" >&2
        rm -rf "${BASE_DIR}" "${TMP_EXTRACT}"
        exit 1
      fi
      cp -a "${INNER}/." "${BASE_DIR}/"
    fi
    rm -rf "${TMP_EXTRACT}"
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
import os, re, shutil, sys
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

def dematte_cyan_transparency(img: Image.Image) -> Image.Image:
    """Convierte cian opaco (índice de agua en paleta) a alpha 0 en edificios de estación."""
    src = img.convert("RGBA")
    data = []
    for r, g, b, a in src.getdata():
        if a > 0 and b >= 170 and g >= 140 and r <= 140:
            data.append((0, 0, 0, 0))
        else:
            data.append((r, g, b, a))
    src.putdata(data)
    return src

def dematte_cc_blue_mask(img: Image.Image) -> Image.Image:
    """OpenTTD CC recolour (0,0,255) → transparente en vehículos 32bpp sin pre-tintar."""
    src = img.convert("RGBA")
    data = []
    for r, g, b, a in src.getdata():
        if a > 0 and r == 0 and g == 0 and b == 255:
            data.append((0, 0, 0, 0))
        else:
            data.append((r, g, b, a))
    src.putdata(data)
    return src

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
        or out_name.startswith("tram_")
    ):
        crop = cleanup_speckles(crop)
    if out_name.startswith("rail_platform_"):
        crop = dematte_cyan_transparency(crop)
    if out_name.startswith("vehicle_"):
        crop = dematte_cc_blue_mask(crop)
    # Cimientos, industria y UI: el atlas 8bpp a veces deja el índice 0 (0,0,255) opaco.
    if graphics_mode != "32bpp" and (
        out_name.startswith("foundation_")
        or out_name.startswith("industry_")
        or out_name.startswith("ui_")
    ):
        crop = dematte_cc_blue_mask(crop)
    out = tiles_dir / out_name
    crop.save(out)
    print(f"  {out_name} ({w}×{h} xrel={xr} yrel={yr}) ← sprite {sid} [{sheet_key}]")


# =============================================================================
# UI toolbar (sprites de cursor OpenTTD)
# =============================================================================
crop_by_id(704, "ui_demolish.png")  # SPR_CURSOR_DEMOLISH_FIRST (dinamita)
crop_by_id(694, "ui_terraform_up.png")    # SPR_IMG_TERRAFORM_UP
crop_by_id(695, "ui_terraform_down.png")  # SPR_IMG_TERRAFORM_DOWN (T1 bajar)
# ui_terraform_level.png (SPR_IMG_LEVEL_LAND = 4964) vive en ogfx2e_extra; ver crop_ui_terraform_icons.py

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
for tileh in range(1, 15):
    crop_by_id(989 + tileh, f"foundation_{tileh:02d}.png")
crop_by_id(4061, "water_flat.png")             # SPR_FLAT_WATER_TILE
# Costas: el set completo (SPR_SHORE_BASE + 0..17) vive en el GRF *extra*
# (Action5 0x0D) y lo extrae scripts/gen_shore_full_set.py, no este NFO base.
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
# Carretera con acera pavimentada: SPR_ROAD_Y - 19 (1313..1331), mismo orden.
for i, sid in enumerate(range(1313, 1332)):
    crop_by_id(sid, f"road_paved_{i:02d}.png")
# Faroles de Roadside::StreetLights (table/road_land.h: 0x57E / 0x57F).
crop_by_id(0x57E, "road_streetlight_0.png")
crop_by_id(0x57F, "road_streetlight_1.png")
# Tranvía sobre asfalto: SPR_TRAMWAY_OVERLAY (OpenTTD table/sprites.h) = 5990;
# mismas 19 piezas en el mismo orden que road_flat_00..18 (GetRoadSpriteOffset).
TRAM_FLAT_BASE = 5990
for i in range(19):
    crop_by_id(TRAM_FLAT_BASE + i, f"tram_flat_{i:02d}.png")
# OpenGFX2 (ogfx21) suele no declarar 5990–6008 en el NFO recortado: sin archivo, el cliente falla.
for i in range(19):
    dst = tiles_dir / f"tram_flat_{i:02d}.png"
    if dst.is_file():
        continue
    src = tiles_dir / f"road_flat_{i:02d}.png"
    if src.is_file():
        shutil.copy2(src, dst)
        print(
            f"  tram_flat_{i:02d}.png (fallback: copia de road_flat; "
            f"sprite {TRAM_FLAT_BASE + i} no en NFO o recorte omitido)"
        )
# Carretera con nieve
crop_by_id(ROAD_SNOW_IDS[0], "road_y_snow.png")
crop_by_id(ROAD_SNOW_IDS[1], "road_x_snow.png")
# Depósito de carretera: 1408–1411 son piezas de boca (12×12) o edificio según orientación;
# 1412/1413 son las otras dos vistas del edificio (NE/NW en el cliente).
for i, sid in enumerate(ROAD_DEPOT_RANGE):
    crop_by_id(sid, f"road_depot_{i}.png")
crop_by_id(1412, "rail_1412.png")
crop_by_id(1413, "rail_1413.png")

# =============================================================================
# VIAS FERREAS (MP_RAILWAY) - alineado con src/sprites/rail.rs
# =============================================================================
RAIL_SINGLE_RANGE = range(1005, 1011)
RAIL_TRACK_RANGE = range(1011, 1023)
RAIL_SLOPE_TRACK_RANGE = range(1023, 1035)
RAIL_WRAPPER_ALIAS_IDS = [
    1005, 1006, 1007, 1008, 1009, 1010,
    1011, 1012, 1013, 1014, 1015, 1016,
    1017, 1018, 1019, 1020, 1021, 1022,
    *range(1023, 1035),
    1035, 1036, 1037, 1038,
    1370, 1371, 1372, 1373,
]
RAIL_SIGNAL_EXPORT_RANGE = range(1275, 1700)

# Piezas sueltas para overlays en junctions
for sid in RAIL_SINGLE_RANGE:
    crop_by_id(sid, f"rail_single_{sid - 1005}.png")
# Vías combinadas (suelo + raíles)
for sid in RAIL_TRACK_RANGE:
    crop_by_id(sid, f"rail_track_{sid - 1011}.png")
for sid in RAIL_SLOPE_TRACK_RANGE:
    crop_by_id(sid, f"rail_{sid}.png")
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
for sid, src_name in [(1037, "rail_track_y_snow.png"), (1038, "rail_track_x_snow.png")]:
    dst = tiles_dir / f"rail_{sid}.png"
    if dst.is_file():
        continue
    src = tiles_dir / src_name
    if src.is_file():
        shutil.copy2(src, dst)
        print(f"  rail_{sid}.png (alias de {src_name} para preload Bevy)")
# Pendiente + nieve (offset +26 desde 1023–1034; precarga `rail_sprite_ids_for_preload`)
RAIL_SNOW_OFFSET = 26
for sid in RAIL_SLOPE_TRACK_RANGE:
    snow_sid = sid + RAIL_SNOW_OFFSET
    crop_by_id(snow_sid, f"rail_{snow_sid}.png")
for sid in RAIL_SLOPE_TRACK_RANGE:
    snow_sid = sid + RAIL_SNOW_OFFSET
    dst = tiles_dir / f"rail_{snow_sid}.png"
    if dst.is_file():
        continue
    src = tiles_dir / f"rail_{sid}.png"
    if src.is_file():
        shutil.copy2(src, dst)
        print(
            f"  rail_{snow_sid}.png (fallback: copia de rail_{sid}; "
            f"sprite {snow_sid} no en NFO o recorte omitido)"
        )
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
for sid, src_name in [
    (1069, "rail_platform_y_front.png"),
    (1070, "rail_platform_x_rear.png"),
    (1071, "rail_platform_y_rear.png"),
    (1072, "rail_platform_x_front.png"),
    (1073, "rail_platform_building_x.png"),
    (1074, "rail_platform_building_y.png"),
    (1075, "rail_platform_pillars_y_front.png"),
    (1076, "rail_platform_pillars_x_rear.png"),
    (1077, "rail_platform_pillars_y_rear.png"),
    (1078, "rail_platform_pillars_x_front.png"),
    (1079, "rail_roof_0.png"),
    (1080, "rail_roof_1.png"),
    (1081, "rail_roof_2.png"),
    (1082, "rail_roof_3.png"),
]:
    dst = tiles_dir / f"rail_{sid}.png"
    if dst.is_file():
        continue
    src = tiles_dir / src_name
    if src.is_file():
        shutil.copy2(src, dst)
        print(f"  rail_{sid}.png (alias de {src_name})")
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
# Alias de suelo usados por algunas rutas antiguas del cliente (0..3 = ne,se,sw,nw)
for i, sid in enumerate([2692, 2693, 2694, 2695]):
    crop_by_id(sid, f"bus_stop_ground_{i}.png")
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
# CASAS – house_s{sprite_id}.png para HOUSE_DRAW_DATA (110×16 filas)
# Regenerar tabla: python3 scripts/gen_house_draw_data.py
# =============================================================================
def load_house_sprite_ids() -> list[int]:
    root = Path(os.environ["OPENTTDRS_REPO_ROOT"])
    gen = root / "crates" / "openttdrs-client" / "src" / "sprites" / "house_draw_data_generated.rs"
    text = gen.read_text(encoding="utf-8")
    ids = set(int(x) for x in re.findall(r"s1: (\d+)", text))
    ids |= set(int(x) for x in re.findall(r"s2: (\d+)", text))
    ids.discard(0)
    return sorted(ids)

for sid in load_house_sprite_ids():
    crop_by_id(sid, f"house_s{sid}.png")

# Alias / respaldo cuando el PNG tiene otro nombre histórico en el script.
for sid, src_name in (
    (1479, "house_stadium_n.png"),
    (1480, "house_stadium_e.png"),
    (1481, "house_stadium_w.png"),
    (1482, "house_stadium_s.png"),
    (1420, "object_concrete.png"),
):
    dst = tiles_dir / f"house_s{sid}.png"
    src = tiles_dir / src_name
    if src.is_file() and (not dst.is_file() or dst.stat().st_size == 0):
        shutil.copy2(src, dst)
        print(f"  house_s{sid}.png (alias de {src_name})")

# =============================================================================
# ÁRBOLES (MP_TREES)
# =============================================================================
# Templado completo: 19 especies × 7 etapas (SPR_TREES_BASE=1576..1708).
# `_tree_layout_sprite` (tree_land.h) referencia las 19 especies.
tree_ids = list(range(1576, 1709))
for i, sid in enumerate(tree_ids):
    crop_by_id(sid, f"tree_{i:02d}.png")

# =============================================================================
# INDUSTRIAS — IDs desde industry_gfx_data_generated.rs (suelo + edificio, 4 estadios).
# Regenerar tablas de offsets
# scripts/gen_industry_gfx_data.py
# scripts/gen_road_stop_gfx_data.py
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
crop_by_id(2634, "road_depot_ground.png")
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
# Portales por dirección (`SPR_TUNNEL_ENTRY_REAR_* + DiagDirection * 2`).
crop_by_id(2365, "tunnel_rail_rear_ne.png")
crop_by_id(2367, "tunnel_rail_rear_se.png")
crop_by_id(2369, "tunnel_rail_rear_sw.png")
crop_by_id(2371, "tunnel_rail_rear_nw.png")
# Alias histórico (= NE)
crop_by_id(2365, "tunnel_rail_rear.png")
crop_by_id(2373, "tunnel_mono_rear.png")
crop_by_id(2381, "tunnel_mglv_rear.png")
crop_by_id(2389, "tunnel_road_rear_ne.png")
crop_by_id(2391, "tunnel_road_rear_se.png")
crop_by_id(2393, "tunnel_road_rear_sw.png")
crop_by_id(2395, "tunnel_road_rear_nw.png")
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
# Humo mina de cobre (SPR_SMOKE_0..4)
for i, sid in enumerate(range(2040, 2045)):
    crop_by_id(sid, f"mine_smoke_{i}.png")
# Humo de chimenea de la central eléctrica (SPR_CHIMNEY_SMOKE_0..7)
for i, sid in enumerate(range(3701, 3709)):
    crop_by_id(sid, f"chimney_smoke_{i}.png")
# EffectVehicle: humo tren, chispas, explosión, avería (ver gen_effect_vehicle_sprites.py)
for i, sid in enumerate(range(3073, 3079)):
    crop_by_id(sid, f"diesel_smoke_{i - 3073}.png")
for i, sid in enumerate(range(3079, 3084)):
    crop_by_id(sid, f"steam_smoke_{i - 3079}.png")
for i, sid in enumerate(range(3084, 3090)):
    crop_by_id(sid, f"electric_spark_{i - 3084}.png")
for i, sid in enumerate(range(3709, 3725)):
    crop_by_id(sid, f"explosion_large_{i - 3709}.png")
for i, sid in enumerate(range(3737, 3741)):
    crop_by_id(sid, f"breakdown_smoke_{i - 3737}.png")
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
# Bus MPS (engine 0): 8 direcciones OpenTTD (sprites 3092..3099).
for sid, name in [
    (3092, "vehicle_bus_n.png"),
    (3093, "vehicle_bus_ne.png"),
    (3094, "vehicle_bus_e.png"),
    (3095, "vehicle_bus_se.png"),
    (3096, "vehicle_bus_s.png"),
    (3097, "vehicle_bus_sw.png"),
    (3098, "vehicle_bus_w.png"),
    (3099, "vehicle_bus_nw.png"),
]:
    crop_by_id(sid, name)
# Bus MPS cargado (+88 → sprites 3180..3187).
for sid, name in [
    (3180, "vehicle_bus_n_loaded.png"),
    (3181, "vehicle_bus_ne_loaded.png"),
    (3182, "vehicle_bus_e_loaded.png"),
    (3183, "vehicle_bus_se_loaded.png"),
    (3184, "vehicle_bus_s_loaded.png"),
    (3185, "vehicle_bus_sw_loaded.png"),
    (3186, "vehicle_bus_w_loaded.png"),
    (3187, "vehicle_bus_nw_loaded.png"),
]:
    crop_by_id(sid, name)
# Camión MPS (spritenum 1): vacío 3100..3107, cargado +88 → 3188..3195.
for sid, name in [
    (3100, "vehicle_truck_n.png"),
    (3101, "vehicle_truck_ne.png"),
    (3102, "vehicle_truck_e.png"),
    (3103, "vehicle_truck_se.png"),
    (3104, "vehicle_truck_s.png"),
    (3105, "vehicle_truck_sw.png"),
    (3106, "vehicle_truck_w.png"),
    (3107, "vehicle_truck_nw.png"),
    (3188, "vehicle_truck_n_loaded.png"),
    (3189, "vehicle_truck_ne_loaded.png"),
    (3190, "vehicle_truck_e_loaded.png"),
    (3191, "vehicle_truck_se_loaded.png"),
    (3192, "vehicle_truck_s_loaded.png"),
    (3193, "vehicle_truck_sw_loaded.png"),
    (3194, "vehicle_truck_w_loaded.png"),
    (3195, "vehicle_truck_nw_loaded.png"),
]:
    crop_by_id(sid, name)
# Kirby Paul Tank (image_index 2): sprites 2921..2928.
for sid, name in [
    (2921, "vehicle_train_n.png"),
    (2922, "vehicle_train_ne.png"),
    (2923, "vehicle_train_e.png"),
    (2924, "vehicle_train_se.png"),
    (2925, "vehicle_train_s.png"),
    (2926, "vehicle_train_sw.png"),
    (2927, "vehicle_train_w.png"),
    (2928, "vehicle_train_nw.png"),
]:
    crop_by_id(sid, name)
# Chaney Jubilee (image_index 0): sprites 2905..2912.
for sid, name in [
    (2905, "vehicle_train_t0_n.png"),
    (2906, "vehicle_train_t0_ne.png"),
    (2907, "vehicle_train_t0_e.png"),
    (2908, "vehicle_train_t0_se.png"),
    (2909, "vehicle_train_t0_s.png"),
    (2910, "vehicle_train_t0_sw.png"),
    (2911, "vehicle_train_t0_w.png"),
    (2912, "vehicle_train_t0_nw.png"),
]:
    crop_by_id(sid, name)
# Ginzu A4 (image_index 1): sprites 2913..2920.
for sid, name in [
    (2913, "vehicle_train_t1_n.png"),
    (2914, "vehicle_train_t1_ne.png"),
    (2915, "vehicle_train_t1_e.png"),
    (2916, "vehicle_train_t1_se.png"),
    (2917, "vehicle_train_t1_s.png"),
    (2918, "vehicle_train_t1_sw.png"),
    (2919, "vehicle_train_t1_w.png"),
    (2920, "vehicle_train_t1_nw.png"),
]:
    crop_by_id(sid, name)
# Diésel representativo (image_index 8): sprites 2949..2956.
for sid, name in [
    (2949, "vehicle_train_td_n.png"),
    (2950, "vehicle_train_td_ne.png"),
    (2951, "vehicle_train_td_e.png"),
    (2952, "vehicle_train_td_se.png"),
    (2953, "vehicle_train_td_s.png"),
    (2954, "vehicle_train_td_sw.png"),
    (2955, "vehicle_train_td_w.png"),
    (2956, "vehicle_train_td_nw.png"),
]:
    crop_by_id(sid, name)
# Eléctrico AsiaStar (image_index 23): sprites 2965..2972.
for sid, name in [
    (2965, "vehicle_train_te_n.png"),
    (2966, "vehicle_train_te_ne.png"),
    (2967, "vehicle_train_te_e.png"),
    (2968, "vehicle_train_te_se.png"),
    (2969, "vehicle_train_te_s.png"),
    (2970, "vehicle_train_te_sw.png"),
    (2971, "vehicle_train_te_w.png"),
    (2972, "vehicle_train_te_nw.png"),
]:
    crop_by_id(sid, name)
# Regenerar metadatos: python3 scripts/gen_vehicle_gfx_data.py
# Solo locomotoras (sin borrar tiles): python3 scripts/extract_train_vehicle_sprites.py

# =============================================================================
# LEGACY (alias del cliente ← sprites NFO; evita recortes fijos con artefactos cian)
# =============================================================================
legacy_coords = {
    "truck": (594, 12408, 8, 16),
    # Fallback para el cliente actual (usa vehicle_bus_sw.png).
    "vehicle_bus_sw": (594, 12408, 8, 16),
}

if graphics_mode != "32bpp":
    sheet00 = sheets.get("ogfx1_base00.png") or sheets.get("ogfx1_base00.pcx")
    for name, (x, y, w, h) in legacy_coords.items():
        if sheet00 is None:
            print(f"  (omitido {name}: sheet ogfx1_base00.(png|pcx) no encontrado)")
            continue
        crop = sheet00.crop((x, y, x + w, y + h))
        crop.save(tiles_dir / f"{name}.png")
        print(f"  {name}.png ({w}×{h})")

# grass / grass_rough / water: siempre desde crop_by_id (terrain_* / water_flat) + cleanup_speckles.
terrain_aliases = {
    "terrain_grass.png": "grass.png",
    "terrain_rough.png": "grass_rough.png",
    "water_flat.png": "water.png",
}
for src, dst in terrain_aliases.items():
    src_p = tiles_dir / src
    dst_p = tiles_dir / dst
    if not src_p.is_file():
        print(f"  (omitido alias {dst}: no existe {src})")
        continue
    img = Image.open(src_p).convert("RGBA")
    img.save(dst_p)
    print(f"  {dst} ← {src}")

print(f"Sprites listos en {tiles_dir}/")
PYEOF

echo "${GRAPHICS_MODE}" > "${DEST}/.graphics_mode"
echo "Modo gráfico registrado en ${DEST}/.graphics_mode (${GRAPHICS_MODE})"

# Rombo blanco de selección de teselas (fantasma de estaciones).
python3 "$(dirname "$0")/gen_tile_select.py"

# Orillas completas, animación de agua, campos/cercas e iconos de toolbar (GRF extra).
python3 "$(dirname "$0")/gen_shore_full_set.py"
python3 "$(dirname "$0")/gen_water_anim_frames.py"
python3 "$(dirname "$0")/gen_field_draw_data.py"
python3 "$(dirname "$0")/gen_toolbar_rail_icons.py"
python3 "$(dirname "$0")/crop_ui_terraform_icons.py"

# Waypoints ferroviarios (SPR_WAYPOINT_* en GRF extra).
python3 "$(dirname "$0")/gen_rail_waypoint_sprites.py" || true
# Alias rail_{1069..1082}.png para preload Bevy (desde rail_platform_* / rail_roof_*).
bash "$(dirname "$0")/alias_rail_station_sprites.sh" || true
# Señales: reexporta 1275–1699 eligiendo el mejor recorte entre NFO base y extra.
python3 "$(dirname "$0")/gen_rail_signal_sprites.py" || true
python3 "$(dirname "$0")/gen_rail_station_draw_data.py" || true
# Sprites de puentes por tipo (tablero + pilares; ver gen_bridge_sprites.py).
python3 "$(dirname "$0")/gen_bridge_sprites.py" || true
python3 "$(dirname "$0")/gen_bridge_structure_palette.py" || true
python3 "$(dirname "$0")/gen_effect_vehicle_sprites.py" || true

# Texture atlas: empaqueta tiles/*.png en páginas + metadata Rust (batching).
python3 "$(dirname "$0")/gen_tile_atlas.py"
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
