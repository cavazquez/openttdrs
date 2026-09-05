#!/usr/bin/env bash
# Captura raster reproducible de OpenTTD para contrastar el renderer de openttdrs.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=lib/openttd_reference.sh
source "${ROOT}/scripts/lib/openttd_reference.sh"

if [[ $# -lt 2 || $# -gt 5 ]]; then
  echo "Uso: $0 <partida.sav> <salida.png> [openttd-bin] [x,y] [anchoxalto]" >&2
  exit 2
fi

SAV="$(realpath "$1")"
OUT="$(realpath -m "$2")"
BIN="${3:-${OPENTTD_BIN:-${ROOT}/reference/openttd-upstream/build/openttd}}"
CENTER="${4:-${OPENTTDRS_WORLD_SCREENSHOT_CENTER:-}}"
RESOLUTION="${5:-${OPENTTDRS_WORLD_SCREENSHOT_RES:-1280x720}}"
SCALE="${OPENTTDRS_WORLD_SCREENSHOT_SCALE:-1}"
CLEAN="${OPENTTDRS_WORLD_SCREENSHOT_CLEAN:-1}"
BUILD_DIR="$(dirname "$BIN")"
BASESET_SRC="${OPENTTDRS_OPENGFX_DIR:-${ROOT}/.deps/openttd-baseset/opengfx-8.0}"
GRAPHICS_SET="${OPENTTDRS_GRAPHICS_SET:-opengfx}"
BLITTER="${OPENTTDRS_SCREENSHOT_BLITTER:-8bpp-simple}"
PREFIX="${OPENTTDRS_DEPS_PREFIX:-${ROOT}/.deps/openttd-prefix}"

if [[ ! -f "$SAV" ]]; then
  echo "error: no existe $SAV" >&2
  exit 1
fi
if [[ ! -x "$BIN" ]]; then
  echo "error: no hay binario OpenTTD en $BIN" >&2
  echo "  ./patches/openttd-15.3-snapshot-export/integrate.sh" >&2
  exit 1
fi
# El exportador cambia al directorio del build para que OpenTTD encuentre su
# baseset. Convertir antes el argumento evita que un binario relativo termine
# buscándose como `build/reference/...` desde ese directorio.
BIN="$(realpath "$BIN")"
if [[ -z "$CENTER" ]]; then
  echo "error: indicá el centro x,y (cuarto argumento o OPENTTDRS_WORLD_SCREENSHOT_CENTER)" >&2
  exit 2
fi

if [[ -d "$BASESET_SRC" ]]; then
  mkdir -p "${BUILD_DIR}/baseset"
  cp -a "${BASESET_SRC}/." "${BUILD_DIR}/baseset/" 2>/dev/null || true
elif ! compgen -G "${BUILD_DIR}/baseset/*.obg" >/dev/null; then
	echo "error: falta un baseset OpenGFX en ${BUILD_DIR}/baseset (o $BASESET_SRC)" >&2
  exit 1
fi

mkdir -p "$(dirname "$OUT")"
rm -f "$OUT"

export LD_LIBRARY_PATH="${PREFIX}/usr/lib/x86_64-linux-gnu:${PREFIX}/usr/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
export OPENTTDRS_WORLD_SCREENSHOT_OUT="$OUT"
export OPENTTDRS_WORLD_SCREENSHOT_CENTER="$CENTER"
export OPENTTDRS_WORLD_SCREENSHOT_RES="$RESOLUTION"
export OPENTTDRS_WORLD_SCREENSHOT_SCALE="$SCALE"
export OPENTTDRS_WORLD_SCREENSHOT_CLEAN="$CLEAN"

echo "world-screenshot OpenTTD: bin=$BIN sav=$SAV out=$OUT center=$CENTER res=$RESOLUTION scale=$SCALE clean=$CLEAN gfx=$GRAPHICS_SET blitter=$BLITTER"
cd "$BUILD_DIR"
set +e
# El exportador se ejecuta contra un build dedicado con el blitter 8bpp simple
# habilitado por `integrate.sh` + `-DOPENTTDRS_HEADLESS_RASTER=ON`. Así usa el
# framebuffer en memoria del driver dedicado, pero no requiere SDL ni una
# sesión gráfica para componer la referencia raster.
timeout 120s "$BIN" -X -x -I "$GRAPHICS_SET" -v dedicated -b "$BLITTER" -s null -m null -r "$RESOLUTION" -g "$SAV" >/tmp/openttdrs-world-screenshot-run.log 2>&1
rc=$?
set -e

if [[ ! -s "$OUT" ]]; then
  echo "error: no se generó $OUT (exit=$rc). Log:" >&2
  tail -n 80 /tmp/openttdrs-world-screenshot-run.log >&2 || true
  exit 1
fi

echo "OK: referencia raster OpenTTD escrita en $OUT"
