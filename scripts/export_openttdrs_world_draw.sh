#!/usr/bin/env bash
# Exporta la selección lógica de sprites del renderer Rust (`world-draw`, #307).
# Se ejecuta en modo headless mediante un test ignorado para no requerir ventana
# ni GPU y poder compararlo directamente con el exportador C++.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if [[ $# -lt 2 || $# -gt 3 ]]; then
  echo "Uso: $0 <partida.sav> <salida.jsonl> [x0,y0,x1,y1]" >&2
  exit 2
fi

SAV="$(realpath "$1")"
OUT="$(realpath -m "$2")"
REGION="${3:-${OPENTTDRS_WORLD_DRAW_REGION:-}}"

if [[ ! -f "$SAV" ]]; then
  echo "error: no existe $SAV" >&2
  exit 1
fi

if command -v sha256sum >/dev/null; then
  SAVE_SHA256="$(sha256sum "$SAV" | awk '{print $1}')"
else
  SAVE_SHA256="$(shasum -a 256 "$SAV" | awk '{print $1}')"
fi

mkdir -p "$(dirname "$OUT")"
rm -f "$OUT"

export OPENTTDRS_WORLD_DRAW_SAV="$SAV"
export OPENTTDRS_WORLD_DRAW_OUT="$OUT"
export OPENTTDRS_WORLD_DRAW_SOURCE="$SAV"
export OPENTTDRS_WORLD_DRAW_SAVE_SHA256="$SAVE_SHA256"
if [[ -n "$REGION" ]]; then
  export OPENTTDRS_WORLD_DRAW_REGION="$REGION"
else
  unset OPENTTDRS_WORLD_DRAW_REGION
fi

echo "world-draw openttdrs: sav=$SAV out=$OUT region=${REGION:-full}"
cd "$ROOT"
CARGO_NET_OFFLINE="${CARGO_NET_OFFLINE:-true}" \
  cargo test -q -p openttdrs-client --bin openttdrs-client \
  world_draw_trace_exports_requested_sav -- --ignored --nocapture --test-threads=1

if ! python3 -c "import json,sys; d=json.loads(open(sys.argv[1], encoding='utf-8').readline()); sys.exit(0 if d.get('producer') == 'openttdrs' and d.get('contract') == 'world-draw' else 1)" "$OUT" \
  || ! tail -n 1 "$OUT" | python3 -c "import json,sys; d=json.loads(sys.stdin.readline()); sys.exit(0 if d.get('kind') == 'complete' else 1)"; then
  echo "error: $OUT no contiene una traza world-draw completa de openttdrs" >&2
  exit 1
fi

echo "OK: candidata world-draw escrita en $OUT (producer=openttdrs)"
