#!/usr/bin/env bash
# check_parity_docs_fresh.sh — niega afirmaciones obsoletas de tick 5 Hz / carga
# instantánea como estado vigente en docs de paridad (#125).
#
# No escanea docs/adr/ (ADR 0002 conserva texto histórico a propósito)
# ni docs/archive/.

set -euo pipefail
cd "$(dirname "$0")/.."

# Narrativa consolidada + artefactos regenerables en parity/
SCAN_PATHS=(
  docs/PARIDAD.md
  docs/PLANIFICACION.md
  docs/parity/divergences_found.md
  docs/parity/train_line_divergences.md
)

FAIL=0

info() { echo "[parity-docs] $*"; }
err() { echo "[parity-docs] ERROR: $*" >&2; FAIL=1; }

info "Buscando afirmaciones obsoletas en docs de paridad ..."

check_pat() {
  local pat="$1"
  local hits
  hits="$(rg -n -e "$pat" "${SCAN_PATHS[@]}" || true)"
  if [[ -n "$hits" ]]; then
    err "patrón /$pat/:"
    printf '%s\n' "$hits" >&2
  fi
}

check_pat 'SIM_TICK_HZ = 5\.0'
check_pat 'REFERENCE_PROGRESS_STEP = 51'
check_pat 'cliente a 5 Hz'
check_pat 'sim a 5 Hz'
check_pat 'tick de 5 Hz'
check_pat 'Tick rate 5 Hz'
check_pat 'se mantiene 5 Hz'
check_pat 'carga la capacidad completa en un tick'
check_pat 'load_vehicles` \(instantánea\)'
check_pat 'Tick de simulación a 5 Hz'

if [[ "$FAIL" -ne 0 ]]; then
  err "docs de paridad desactualizadas (#125)"
  exit 1
fi

info "OK — sin afirmaciones vigentes de tick 5 Hz / carga instantánea obsoleta"
