#!/usr/bin/env bash
# Captura OpenTTD + openttdrs en la misma tesela y deja evidencia raster.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if [[ $# -lt 3 || $# -gt 6 ]]; then
  echo "Uso: $0 <partida.sav> <directorio-salida> <x,y> [anchoxalto] [escala] [openttd-bin]" >&2
  exit 2
fi

SAV="$(realpath "$1")"
OUT_DIR="$(realpath -m "$2")"
CENTER="$3"
RESOLUTION="${4:-${OPENTTDRS_WORLD_SCREENSHOT_RES:-1280x720}}"
SCALE="${5:-${OPENTTDRS_WORLD_SCREENSHOT_SCALE:-1}}"
OPENTTD_BIN="${6:-${OPENTTD_BIN:-${ROOT}/reference/openttd-upstream/build/openttd}}"
MODE_FILE="${ROOT}/assets/opengfx/.graphics_mode"
MODE="${OPENTTDRS_WORLD_SCREENSHOT_CANDIDATE_GFX_MODE:-}"
CLEAN="${OPENTTDRS_WORLD_SCREENSHOT_CLEAN:-1}"

if [[ ! -f "$SAV" ]]; then
  echo "error: no existe $SAV" >&2
  exit 1
fi
if [[ -z "$MODE" && -f "$MODE_FILE" ]]; then
  MODE="$(tr -d '[:space:]' <"$MODE_FILE")"
fi
if [[ -z "$MODE" ]]; then
  echo "error: no se pudo determinar el modo gráfico candidato ($MODE_FILE)" >&2
  exit 1
fi
if [[ "$MODE" != "8bpp" && "${OPENTTDRS_WORLD_SCREENSHOT_ALLOW_GFX_MISMATCH:-}" != "1" ]]; then
  echo "error: la referencia raster actual usa OpenGFX 8bpp, pero openttdrs usa '$MODE'." >&2
  echo "  Cambiá a 8bpp o usá OPENTTDRS_WORLD_SCREENSHOT_ALLOW_GFX_MISMATCH=1 sólo para explorar." >&2
  exit 2
fi

mkdir -p "$OUT_DIR"
export OPENTTDRS_WORLD_SCREENSHOT_CLEAN="$CLEAN"
if [[ "$CLEAN" == "0" || "$CLEAN" == "false" || "$CLEAN" == "no" || "$CLEAN" == "off" ]]; then
  CAPTURE_PROFILE="dynamic"
else
  CAPTURE_PROFILE="clean-static"
fi
REFERENCE="$OUT_DIR/reference.png"
CANDIDATE="$OUT_DIR/candidate.png"
DIFF="$OUT_DIR/diff.png"
REPORT="$OUT_DIR/report.json"

OPENTTDRS_WORLD_SCREENSHOT_SCALE="$SCALE" \
  "${ROOT}/scripts/export_openttd_world_screenshot.sh" \
  "$SAV" "$REFERENCE" "$OPENTTD_BIN" "$CENTER" "$RESOLUTION"
"${ROOT}/scripts/export_openttdrs_world_screenshot.sh" \
  "$SAV" "$CANDIDATE" "$CENTER" "$RESOLUTION" "$SCALE"
python3 "${ROOT}/scripts/compare_world_screenshots.py" \
  "$REFERENCE" "$CANDIDATE" \
  --diff "$DIFF" \
  --report "$REPORT" \
  --save "$SAV" \
  --center "$CENTER" \
  --resolution "$RESOLUTION" \
  --openttdrs-scale "$SCALE" \
  --candidate-graphics "OpenGFX · $MODE" \
  --capture-profile "$CAPTURE_PROFILE" \
  --alignment-radius "${OPENTTDRS_WORLD_SCREENSHOT_ALIGNMENT_RADIUS:-8}" \
  --alignment-stride "${OPENTTDRS_WORLD_SCREENSHOT_ALIGNMENT_STRIDE:-8}"

echo "OK: comparación focalizada escrita en $OUT_DIR"
echo "  referencia: $REFERENCE"
echo "  candidata:  $CANDIDATE"
echo "  diff:       $DIFF"
echo "  reporte:    $REPORT"
