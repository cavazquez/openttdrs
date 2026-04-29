#!/usr/bin/env bash
# Convierte un .sav con parse_sav.py y valida TNBP con el ejemplo Rust validate_ottdmap_tnbp.
# Uso: scripts/validate_sav_tnbp.sh /ruta/partida.sav [/ruta/salida.ottdmap]
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SAV="${1:?uso: $0 archivo.sav [salida.ottdmap]}"
OUT="${2:-${SAV%.sav}.ottdmap}"
python3 "$ROOT/scripts/parse_sav.py" "$SAV" "$OUT"
cd "$ROOT"
cargo run -q -p openttdrs-core --example validate_ottdmap_tnbp -- "$OUT"
