#!/usr/bin/env bash
# Exporta el lado Rust del contrato world-raw con la misma interfaz del oráculo.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if [[ $# -lt 2 || $# -gt 3 ]]; then
  echo "Uso: $0 <partida.sav> <salida.jsonl> [x0,y0,x1,y1]" >&2
  exit 2
fi

SAV="$(realpath "$1")"
OUT="$(realpath -m "$2")"
REGION="${3:-${OPENTTDRS_WORLD_RAW_REGION:-}}"
STAGE="${OPENTTDRS_WORLD_DUMP_STAGE:-sav_map}"

if [[ ! -f "$SAV" ]]; then
  echo "error: no existe $SAV" >&2
  exit 1
fi

mkdir -p "$(dirname "$OUT")"
rm -f "$OUT"

ARGS=("$SAV" "$OUT" --stage "$STAGE")
if [[ -n "$REGION" ]]; then
  ARGS+=(--region "$REGION")
fi

echo "world-raw openttdrs: sav=$SAV out=$OUT region=${REGION:-full} stage=$STAGE"
cd "$ROOT"
CARGO_NET_OFFLINE="${CARGO_NET_OFFLINE:-true}" \
  cargo run -q -p openttdrs-core --bin world_raw_dumper -- "${ARGS[@]}"

if ! python3 -c "import json,sys; d=json.loads(open(sys.argv[1], encoding='utf-8').readline()); sys.exit(0 if d.get('producer') == 'openttdrs' and d.get('contract') == 'world-raw' else 1)" "$OUT"; then
  echo "error: $OUT no declara metadata world-raw de openttdrs" >&2
  exit 1
fi

echo "OK: candidata world-raw escrita en $OUT (producer=openttdrs)"
