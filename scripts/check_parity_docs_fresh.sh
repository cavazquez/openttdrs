#!/usr/bin/env bash
# check_parity_docs_fresh.sh — niega afirmaciones de paridad que ya contradicen
# capacidades cubiertas por código/tests (#125).
#
# No escanea docs/adr/ (ADR 0002 conserva texto histórico a propósito)
# ni docs/archive/.

set -euo pipefail
cd "$(dirname "$0")/.."

# Narrativa consolidada. No incluye docs/archive/: conserva snapshots históricos.
SCAN_PATHS=(
  docs/PARIDAD.md
  docs/PLANIFICACION.md
  docs/MAPA_Y_FERROCARRIL.md
  docs/ARCHITECTURE.md
  docs/README.md
  docs/parity/sav-compatibility.md
  docs/parity/newgrf-action0-matrix.md
  docs/parity/newgrf-callback-matrix.md
  docs/parity/METODOLOGIA_RENDER_SAV.md
  docs/parity/WORLD_DRAW_SCHEMA.md
  docs/parity/WORLD_SCREENSHOT_SCHEMA.md
  README.md
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

require_pat() {
  local pat="$1"
  local path="$2"
  if ! rg -q -e "$pat" "$path"; then
    err "falta patrón canónico /$pat/ en $path"
  fi
}

require_pat '^## Estado canónico actual$' docs/PARIDAD.md
require_pat 'OpenTTD 15\.3, commit' docs/PARIDAD.md
require_pat '^# Compatibilidad `.sav` OpenTTD ↔ openttdrs$' docs/parity/sav-compatibility.md
python3 scripts/check_active_parity_backlog.py

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
check_pat 'Economía \+ 6 cargos'
check_pat '\| Barcos / aviones \| 🔮'
check_pat 'RailTypeInfo.*\*\*No implementado\*\*'
check_pat 'Ownership de tile de vía.*\*\*No implementado\*\*'
check_pat 'Consist:.*\*\*No implementado\*\*'
check_pat 'PBS:.*\*\*No implementado\*\*'
check_pat 'no hay intervalos de servicio'
check_pat 'no hay servicio en la sim'
check_pat 'transfer necesita feeder share'
check_pat '\*\*Non-stop / go via\*\* \| No existe'
check_pat '\*\*Stop location de trenes.*\| No existe'
check_pat '\| Barcos / aviones \| ❌ omitidos'
check_pat '\| Barcos, aviones, efectos \| ❌ omitidos'
check_pat 'La sim actual no considera tráfico en carretera'
check_pat 'ENTRY ignorado al bloquear'
check_pat 'EXIT y COMBO se tratan como BLOCK'
check_pat 'lógica de segmento upstream no replica'
check_pat 'sin ejecutar callbacks'
check_pat 'compatibilidad con `.sav` OpenTTD \(sigue siendo `parse_sav`'
check_pat 'Paridad visual OpenGFX vanilla \| 🟡 ~85–90 %'
check_pat 'Paridad visual SP3 ≥ 90 %'

if [[ "$FAIL" -ne 0 ]]; then
  err "docs de paridad desactualizadas (#125)"
  exit 1
fi

info "OK — matriz canónica presente y sin afirmaciones obsoletas conocidas"
