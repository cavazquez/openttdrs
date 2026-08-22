#!/usr/bin/env bash
# Ejecuta OpenTTD parcheado y exporta los draw calls reales `world-draw` (#307).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=lib/openttd_reference.sh
source "${ROOT}/scripts/lib/openttd_reference.sh"

if [[ $# -lt 2 || $# -gt 4 ]]; then
  echo "Uso: $0 <partida.sav> <salida.jsonl> [openttd-bin] [x0,y0,x1,y1]" >&2
  exit 2
fi

SAV="$(realpath "$1")"
OUT="$(realpath -m "$2")"
SORT_OUT="${OPENTTDRS_WORLD_SORT_OUT:-}"
if [[ -n "$SORT_OUT" ]]; then
  SORT_OUT="$(realpath -m "$SORT_OUT")"
fi
BIN="$(realpath -m "${3:-${OPENTTD_BIN:-${ROOT}/reference/openttd-upstream/build/openttd}}")"
REGION="${4:-${OPENTTDRS_WORLD_DRAW_REGION:-}}"
COMMIT="${OPENTTDRS_OPENTTD_COMMIT:-$(openttd_manifest_get "$ROOT" commit)}"
BUILD_DIR="$(dirname "$BIN")"
BASESET_SRC="${OPENTTDRS_OPENGFX_DIR:-${ROOT}/.deps/openttd-baseset/opengfx-8.0}"
PREFIX="${OPENTTDRS_DEPS_PREFIX:-${ROOT}/.deps/openttd-prefix}"
# Las regiones focalizadas terminan holgadamente en dos minutos, pero una
# auditoría de Kale recorre 65.536 teselas y necesita más margen después de
# cargar el save. Se mantiene el valor conservador para el uso diario y se
# permite ampliar sólo la corrida que lo requiere.
TIMEOUT_SECONDS="${OPENTTDRS_WORLD_DRAW_TIMEOUT_SECONDS:-120}"

if [[ ! -f "$SAV" ]]; then
  echo "error: no existe $SAV" >&2
  exit 1
fi
if [[ ! -x "$BIN" ]]; then
  echo "error: no hay binario OpenTTD en $BIN" >&2
  echo "  ./patches/openttd-15.3-snapshot-export/integrate.sh" >&2
  exit 1
fi
if ! [[ "$TIMEOUT_SECONDS" =~ ^[1-9][0-9]*$ ]]; then
  echo "error: OPENTTDRS_WORLD_DRAW_TIMEOUT_SECONDS debe ser un entero positivo" >&2
  exit 2
fi

if [[ -d "$BASESET_SRC" ]]; then
  mkdir -p "${BUILD_DIR}/baseset"
  cp -a "${BASESET_SRC}/." "${BUILD_DIR}/baseset/" 2>/dev/null || true
elif [[ ! -f "${BUILD_DIR}/baseset/opengfx.obg" ]]; then
  echo "error: falta OpenGFX en ${BUILD_DIR}/baseset (o $BASESET_SRC)" >&2
  exit 1
fi

if command -v sha256sum >/dev/null; then
  SAVE_SHA256="$(sha256sum "$SAV" | awk '{print $1}')"
else
  SAVE_SHA256="$(shasum -a 256 "$SAV" | awk '{print $1}')"
fi

mkdir -p "$(dirname "$OUT")"
rm -f "$OUT"
if [[ -n "$SORT_OUT" ]]; then
  mkdir -p "$(dirname "$SORT_OUT")"
  rm -f "$SORT_OUT"
fi

export LD_LIBRARY_PATH="${PREFIX}/usr/lib/x86_64-linux-gnu:${PREFIX}/usr/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
export OPENTTDRS_WORLD_DRAW_OUT="$OUT"
export OPENTTDRS_WORLD_DRAW_SOURCE="$SAV"
export OPENTTDRS_WORLD_DRAW_SAVE_SHA256="$SAVE_SHA256"
export OPENTTDRS_OPENTTD_COMMIT="$COMMIT"
if [[ -n "$SORT_OUT" ]]; then
  export OPENTTDRS_WORLD_SORT_OUT="$SORT_OUT"
else
  unset OPENTTDRS_WORLD_SORT_OUT
fi
# Dedicated + -g carga antes una partida nueva; capturar al completar el save pedido.
export OPENTTDRS_WORLD_DRAW_MIN_CALL="${OPENTTDRS_WORLD_DRAW_MIN_CALL:-${OPENTTDRS_WORLD_SEMANTIC_MIN_CALL:-${OPENTTDRS_WORLD_RAW_MIN_CALL:-2}}}"
if [[ -n "$REGION" ]]; then
  export OPENTTDRS_WORLD_DRAW_REGION="$REGION"
fi

echo "world-draw OpenTTD: bin=$BIN sav=$SAV out=$OUT sort=${SORT_OUT:-off} region=${REGION:-full} timeout=${TIMEOUT_SECONDS}s commit=$COMMIT"
cd "$BUILD_DIR"
set +e
timeout "${TIMEOUT_SECONDS}s" "$BIN" -X -I opengfx -D -g "$SAV" >/tmp/openttdrs-world-draw-run.log 2>&1
rc=$?
set -e

if [[ ! -f "$OUT" ]]; then
  echo "error: no se generó $OUT (exit=$rc). Log:" >&2
  tail -n 60 /tmp/openttdrs-world-draw-run.log >&2 || true
  exit 1
fi

if ! python3 -c "import json,sys; d=json.loads(open(sys.argv[1], encoding='utf-8').readline()); sys.exit(0 if d.get('producer') == 'openttd' and d.get('contract') == 'world-draw' else 1)" "$OUT" \
  || ! tail -n 1 "$OUT" | python3 -c "import json,sys; d=json.loads(sys.stdin.readline()); sys.exit(0 if d.get('kind') == 'complete' else 1)"; then
  echo "error: $OUT no contiene una traza world-draw completa de OpenTTD" >&2
  exit 1
fi

echo "OK: oráculo world-draw escrito en $OUT (producer=openttd)"

if [[ -n "$SORT_OUT" ]]; then
  if [[ ! -f "$SORT_OUT" ]]; then
    echo "error: no se generó $SORT_OUT con el orden final de sprites" >&2
    exit 1
  fi
  if ! python3 -c "import json,sys; d=json.loads(open(sys.argv[1], encoding='utf-8').readline()); sys.exit(0 if d.get('producer') == 'openttd' and d.get('contract') == 'world-sort' and d.get('stage') == 'post_viewport_sprite_sorter' else 1)" "$SORT_OUT"; then
    echo "error: $SORT_OUT no contiene el contrato world-sort de OpenTTD" >&2
    exit 1
  fi
  if ! tail -n 1 "$SORT_OUT" | python3 -c "import json,sys; d=json.loads(sys.stdin.readline()); sys.exit(0 if d.get('kind') == 'complete' else 1)"; then
    echo "error: $SORT_OUT no contiene un orden final completo" >&2
    exit 1
  fi
  echo "OK: oráculo world-sort escrito en $SORT_OUT (post_viewport_sprite_sorter)"
fi
