#!/usr/bin/env bash
# Capturas de referencia UI (#33). Requiere display (Wayland/X11).
#
# Uso:
#   bash scripts/capture_ui_reference.sh
#   bash scripts/capture_ui_reference.sh save/partida.json
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "${ROOT}"

SAVE="${1:-}"
OUT_ROOT="${ROOT}/docs/parity/screenshots"
mkdir -p "${OUT_ROOT}/1280x720" "${OUT_ROOT}/1920x1080"

if [[ -n "${SAVE}" ]]; then
  export OTTDJSON_LOAD="${SAVE}"
fi

capture_one() {
  local res="$1"
  local out="${OUT_ROOT}/${res}/windows_reference.png"
  echo "→ ${res} → ${out}"
  if ! OPENTTDRS_SHOT_RES="${res}" OPENTTDRS_WINDOWS_SHOT="${out}" \
    cargo run -p openttdrs-client --release; then
    echo "Aviso: captura ${res} falló (¿hay display?). Harness listo para regenerar." >&2
  fi
}

capture_one 1280x720
capture_one 1920x1080

echo "Listo. Ver ${OUT_ROOT}/README.md"
