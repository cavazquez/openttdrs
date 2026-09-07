#!/usr/bin/env bash
# Exporta la misma traza JSONL desde el scheduler de openttdrs.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if [[ $# -lt 2 || $# -gt 3 ]]; then
  echo "Uso: $0 <partida.sav> <salida.jsonl> [días]" >&2
  exit 2
fi

SAV="$(realpath "$1")"
OUT="$(realpath -m "$2")"
DAYS="${3:-40}"

if [[ ! -f "$SAV" ]]; then
  echo "error: no existe $SAV" >&2
  exit 1
fi
if [[ ! "$DAYS" =~ ^[1-9][0-9]*$ ]]; then
  echo "error: días debe ser entero positivo: $DAYS" >&2
  exit 2
fi

mkdir -p "$(dirname "$OUT")"
rm -f "$OUT"

cd "$ROOT"
cargo run --quiet -p openttdrs-core --bin sav_industry_scheduler_runner -- \
  "$SAV" --days "$DAYS" --out "$OUT"
python3 "$ROOT/scripts/validate_industry_trace.py" "$OUT" "$DAYS" openttdrs
echo "OK: traza scheduler industrias openttdrs → $OUT"
