#!/usr/bin/env bash
# Captura raster reproducible del mundo renderizado por openttdrs.
#
# Es el par de `export_openttd_world_screenshot.sh`: carga el mismo `.sav`,
# centra la cámara en una tesela y guarda sólo el mundo, sin UI ni audio.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if [[ $# -lt 3 || $# -gt 5 ]]; then
  echo "Uso: $0 <partida.sav> <salida.png> <x,y> [anchoxalto] [escala]" >&2
  exit 2
fi

SAV="$(realpath "$1")"
OUT="$(realpath -m "$2")"
CENTER="$3"
RESOLUTION="${4:-${OPENTTDRS_WORLD_SCREENSHOT_RES:-1280x720}}"
SCALE="${5:-${OPENTTDRS_WORLD_SCREENSHOT_SCALE:-1}}"
SETTLE_FRAMES="${OPENTTDRS_WORLD_SCREENSHOT_SETTLE_FRAMES:-180}"
TIMEOUT_SECONDS="${OPENTTDRS_WORLD_SCREENSHOT_TIMEOUT_SECONDS:-120}"
CLEAN="${OPENTTDRS_WORLD_SCREENSHOT_CLEAN:-1}"

if [[ ! -f "$SAV" ]]; then
  echo "error: no existe $SAV" >&2
  exit 1
fi
if ! [[ "$CENTER" =~ ^[[:space:]]*[0-9]+[[:space:]]*,[[:space:]]*[0-9]+[[:space:]]*$ ]]; then
  echo "error: centro inválido '$CENTER' (usar x,y)" >&2
  exit 2
fi
if ! [[ "$RESOLUTION" =~ ^[1-9][0-9]*[xX][1-9][0-9]*$ ]]; then
  echo "error: resolución inválida '$RESOLUTION' (usar anchoxalto)" >&2
  exit 2
fi
if ! [[ "$SCALE" =~ ^([0-9]+([.][0-9]*)?|[.][0-9]+)$ ]] || [[ "$SCALE" == "0" || "$SCALE" == "0." ]]; then
  echo "error: escala inválida '$SCALE' (usar un número positivo)" >&2
  exit 2
fi
if ! [[ "$SETTLE_FRAMES" =~ ^[1-9][0-9]*$ ]] || (( SETTLE_FRAMES < 40 || SETTLE_FRAMES > 900 )); then
  echo "error: OPENTTDRS_WORLD_SCREENSHOT_SETTLE_FRAMES debe estar entre 40 y 900" >&2
  exit 2
fi
if ! [[ "$TIMEOUT_SECONDS" =~ ^[1-9][0-9]*$ ]]; then
  echo "error: OPENTTDRS_WORLD_SCREENSHOT_TIMEOUT_SECONDS debe ser un entero positivo" >&2
  exit 2
fi

mkdir -p "$(dirname "$OUT")"
rm -f "$OUT"

echo "world-screenshot openttdrs: sav=$SAV out=$OUT center=$CENTER res=$RESOLUTION scale=$SCALE settle=${SETTLE_FRAMES}f clean=$CLEAN"
cd "$ROOT"
set +e
OPENTTDRS_SAV_LOAD="$SAV" \
  OPENTTDRS_MAP_SHOT="$OUT" \
  OPENTTDRS_MAP_SHOT_CENTER="$CENTER" \
  OPENTTDRS_MAP_SHOT_SCALE="$SCALE" \
  OPENTTDRS_MAP_SHOT_SETTLE_FRAMES="$SETTLE_FRAMES" \
  OPENTTDRS_MAP_SHOT_CLEAN="$CLEAN" \
  OPENTTDRS_DISABLE_AUDIO=1 \
  OPENTTDRS_SHOT_RES="$RESOLUTION" \
  CARGO_NET_OFFLINE="${CARGO_NET_OFFLINE:-true}" \
  RUSTC_WRAPPER="${RUSTC_WRAPPER:-}" \
  timeout "${TIMEOUT_SECONDS}s" cargo run -q -p openttdrs-client \
  >/tmp/openttdrs-world-screenshot-candidate.log 2>&1
rc=$?
set -e

if [[ ! -s "$OUT" ]]; then
  echo "error: openttdrs no generó $OUT (exit=$rc). Log:" >&2
  tail -n 100 /tmp/openttdrs-world-screenshot-candidate.log >&2 || true
  exit 1
fi

echo "OK: candidata raster openttdrs escrita en $OUT"
