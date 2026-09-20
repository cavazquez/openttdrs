#!/usr/bin/env bash
# Captura y certifica el contrato V1-RAS de Kale (#589).
#
# Este comando exige un directorio de artefactos vacío para que una ejecución
# anterior no pueda mezclarse con la actual. Guarda las tres capturas Normal
# por motor, los logs de cada proceso y los cinco zooms diagnósticos antes de
# llamar al gate combinado de píxeles/cobertura/media.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if [[ $# -lt 1 || $# -gt 2 ]]; then
  echo "Uso: $0 <directorio-artefactos-vacío> [openttd-bin]" >&2
  exit 2
fi

OUT_DIR="$(realpath -m "$1")"
OPENTTD_BIN="${2:-${OPENTTD_BIN:-${ROOT}/reference/openttd-upstream/build/openttd}}"
SAV="${ROOT}/save/Kale_TitleGame.sav"
CENTER="132,2"
RESOLUTION="800x600"
FIXTURE_SHA="584d98c3d1dc389e938ce92aa357cc4a1c179bf9849133f9b85d2e956f3e0a69"
ORACLE_COMMIT="c2661164bcb6cbf5ab97b56ccbee7506a3b26833"
ORACLE_WORKTREE="${OPENTTDRS_V1_RASTER_ORACLE_WORKTREE:-${ROOT}/reference/openttd-upstream}"
REFERENCE_ASSET="${OPENTTDRS_V1_RASTER_REFERENCE_ASSET:-${ROOT}/.deps/openttd-baseset/opengfx-8.0/opengfx.obg}"
CANDIDATE_MODE_FILE="${ROOT}/assets/opengfx/.graphics_mode"
CANDIDATE_ASSET_BASE="${ROOT}/assets/opengfx/opengfx-8.0/ogfx1_base.grf"
CANDIDATE_ASSET_ATLAS="${ROOT}/assets/opengfx/atlas/tiles_atlas_0.png"

if [[ -e "$OUT_DIR" ]] && [[ -n "$(find "$OUT_DIR" -mindepth 1 -maxdepth 1 -print -quit)" ]]; then
  echo "error: el directorio de artefactos ya contiene archivos: $OUT_DIR" >&2
  echo "  Usá un directorio nuevo: mezclar resultados podría certificar PNGs viejos." >&2
  exit 2
fi
if [[ ! -f "$SAV" ]]; then
  echo "error: falta la fixture V1 $SAV" >&2
  exit 1
fi
if [[ "$(sha256sum "$SAV" | awk '{print $1}')" != "$FIXTURE_SHA" ]]; then
  echo "error: la fixture Kale no coincide con el SHA V1" >&2
  exit 1
fi
if [[ ! -x "$OPENTTD_BIN" ]]; then
  echo "error: no hay binario OpenTTD ejecutable en $OPENTTD_BIN" >&2
  exit 1
fi
if [[ "$(git -C "$ORACLE_WORKTREE" rev-parse HEAD)" != "$ORACLE_COMMIT" ]]; then
  echo "error: el oracle OpenTTD no está en el pin V1 $ORACLE_COMMIT" >&2
  exit 1
fi
if ! git -C "$ROOT" diff --quiet || ! git -C "$ROOT" diff --cached --quiet; then
  echo "error: la captura V1 exige cambios rastreados committeados para fijar la SHA candidata" >&2
  exit 1
fi
for input in "$REFERENCE_ASSET" "$CANDIDATE_MODE_FILE" "$CANDIDATE_ASSET_BASE" "$CANDIDATE_ASSET_ATLAS"; do
  if [[ ! -f "$input" ]]; then
    echo "error: falta el input gráfico V1 $input" >&2
    exit 1
  fi
done
if [[ "$(tr -d '[:space:]' <"$CANDIDATE_MODE_FILE")" != "8bpp" ]]; then
  echo "error: el modo gráfico candidato debe ser 8bpp" >&2
  exit 1
fi

mkdir -p "$OUT_DIR/normal" "$OUT_DIR/diagnostics" "$OUT_DIR/logs"
CANDIDATE_SHA="$(git -C "$ROOT" rev-parse HEAD)"

capture_normal() {
  local index="$1"
  local reference="$OUT_DIR/normal/reference-$index.png"
  local candidate="$OUT_DIR/normal/candidate-$index.png"

  {
    printf 'V1-RAS OpenTTD sample %s: center=%s res=%s scale=1 clean-static\n' "$index" "$CENTER" "$RESOLUTION"
    OPENTTDRS_WORLD_SCREENSHOT_CLEAN=1 \
      OPENTTDRS_WORLD_SCREENSHOT_SCALE=1 \
      OPENTTDRS_WORLD_SCREENSHOT_REFERENCE_LOG="$OUT_DIR/logs/reference-$index.engine.log" \
      "$ROOT/scripts/export_openttd_world_screenshot.sh" \
      "$SAV" "$reference" "$OPENTTD_BIN" "$CENTER" "$RESOLUTION"
  } >"$OUT_DIR/logs/reference-$index.runner.log" 2>&1

  {
    printf 'V1-RAS openttdrs sample %s: center=%s res=%s scale=1 clean-static\n' "$index" "$CENTER" "$RESOLUTION"
    OPENTTDRS_WORLD_SCREENSHOT_CLEAN=1 \
      OPENTTDRS_WORLD_SCREENSHOT_CANDIDATE_LOG="$OUT_DIR/logs/candidate-$index.engine.log" \
      "$ROOT/scripts/export_openttdrs_world_screenshot.sh" \
      "$SAV" "$candidate" "$CENTER" "$RESOLUTION" 1
  } >"$OUT_DIR/logs/candidate-$index.runner.log" 2>&1
}

capture_diagnostic() {
  local scale="$1"
  local label="$2"
  local diagnostic_dir="$OUT_DIR/diagnostics/$label"

  mkdir -p "$diagnostic_dir"
  {
    printf 'V1-RAS diagnostic %s: center=%s res=%s scale=%s clean-static\n' "$label" "$CENTER" "$RESOLUTION" "$scale"
    OPENTTDRS_WORLD_SCREENSHOT_CLEAN=1 \
      OPENTTDRS_WORLD_SCREENSHOT_ALIGNMENT_RADIUS=0 \
      OPENTTDRS_WORLD_SCREENSHOT_ALIGNMENT_STRIDE=1 \
      OPENTTDRS_WORLD_SCREENSHOT_REFERENCE_LOG="$OUT_DIR/logs/$label.reference.engine.log" \
      OPENTTDRS_WORLD_SCREENSHOT_CANDIDATE_LOG="$OUT_DIR/logs/$label.candidate.engine.log" \
      "$ROOT/scripts/compare_focused_world_screenshot.sh" \
      "$SAV" "$diagnostic_dir" "$CENTER" "$RESOLUTION" "$scale" "$OPENTTD_BIN"
  } >"$OUT_DIR/logs/$label.runner.log" 2>&1
}

for index in 1 2 3; do
  capture_normal "$index"
done

# Las escalas fuera de Normal se conservan para diagnóstico de regresiones. No
# alimentan el presupuesto V1, pero su ausencia vuelve inválida la evidencia.
capture_diagnostic 0.25 in4x
capture_diagnostic 0.5 in2x
capture_diagnostic 2 out2x
capture_diagnostic 4 out4x
capture_diagnostic 8 out8x

python3 "$ROOT/scripts/v1_raster_gate.py" \
  --artifact-dir "$OUT_DIR" \
  --save "$SAV" \
  --candidate-sha "$CANDIDATE_SHA" \
  --candidate-mode-file "$CANDIDATE_MODE_FILE" \
  --reference-asset "$REFERENCE_ASSET" \
  --candidate-asset "$CANDIDATE_ASSET_BASE" \
  --candidate-asset "$CANDIDATE_ASSET_ATLAS" \
  --diagnostic-report "$OUT_DIR/diagnostics/in4x/report.json" \
  --diagnostic-report "$OUT_DIR/diagnostics/in2x/report.json" \
  --diagnostic-report "$OUT_DIR/diagnostics/out2x/report.json" \
  --diagnostic-report "$OUT_DIR/diagnostics/out4x/report.json" \
  --diagnostic-report "$OUT_DIR/diagnostics/out8x/report.json"

echo "OK: V1-RAS certificado; evidencia preservada en $OUT_DIR"
