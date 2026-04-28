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
import sys, os
from PIL import Image

sprites_dir = os.environ["SPRITES_DIR"]
tiles_dir   = os.environ["TILES_DIR"]
os.makedirs(tiles_dir, exist_ok=True)

src = os.path.join(sprites_dir, "ogfx1_base00.png")
img = Image.open(src)

if img.mode == "P":
    pal = img.getpalette()
    transparent_rgb = tuple(pal[0:3])
    img_rgba = img.convert("RGBA")
    data = list(img_rgba.getdata())
    data = [(0,0,0,0) if (r,g,b)==transparent_rgb else (r,g,b,a)
            for r,g,b,a in data]
    img_rgba.putdata(data)
else:
    img_rgba = img.convert("RGBA")

sprites = {
    "grass":       (610, 13496, 64, 31),
    "grass_rough": (562, 13640, 64, 31),
    "water":       (402, 14392, 64, 31),
    "road_x":      (322,  3912, 64, 31),
    "road_y":      (402,  3912, 64, 31),
    "coal_mine":   (322,  8136, 29, 43),
    "truck":       (594, 12408,  8, 16),
}

for name, (x, y, w, h) in sprites.items():
    crop = img_rgba.crop((x, y, x+w, y+h))
    out  = os.path.join(tiles_dir, f"{name}.png")
    crop.save(out)
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
