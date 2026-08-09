#!/usr/bin/env bash
# Ejecuta OpenTTD parcheado y exporta el oráculo JSONL world-raw (#305).
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
BIN="${3:-${OPENTTD_BIN:-${ROOT}/reference/openttd-upstream/build/openttd}}"
REGION="${4:-${OPENTTDRS_WORLD_RAW_REGION:-}}"
COMMIT="${OPENTTDRS_OPENTTD_COMMIT:-$(openttd_manifest_get "$ROOT" commit)}"
BUILD_DIR="$(dirname "$BIN")"
BASESET_SRC="${OPENTTDRS_OPENGFX_DIR:-${ROOT}/.deps/openttd-baseset/opengfx-8.0}"
PREFIX="${OPENTTDRS_DEPS_PREFIX:-${ROOT}/.deps/openttd-prefix}"

if [[ ! -f "$SAV" ]]; then
  echo "error: no existe $SAV" >&2
  exit 1
fi
if [[ ! -x "$BIN" ]]; then
  echo "error: no hay binario OpenTTD en $BIN" >&2
  echo "  ./patches/openttd-15.3-snapshot-export/integrate.sh" >&2
  exit 1
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

export LD_LIBRARY_PATH="${PREFIX}/usr/lib/x86_64-linux-gnu:${PREFIX}/usr/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
export OPENTTDRS_WORLD_RAW_OUT="$OUT"
export OPENTTDRS_WORLD_RAW_SOURCE="$SAV"
export OPENTTDRS_WORLD_RAW_SAVE_SHA256="$SAVE_SHA256"
export OPENTTDRS_OPENTTD_COMMIT="$COMMIT"
# Dedicated + -g carga antes una partida nueva; exportar en el segundo AfterLoadGame.
export OPENTTDRS_WORLD_RAW_MIN_CALL="${OPENTTDRS_WORLD_RAW_MIN_CALL:-${OPENTTDRS_SNAPSHOT_MIN_CALL:-2}}"
if [[ -n "$REGION" ]]; then
  export OPENTTDRS_WORLD_RAW_REGION="$REGION"
fi

echo "world-raw OpenTTD: bin=$BIN sav=$SAV out=$OUT region=${REGION:-full} commit=$COMMIT"
cd "$BUILD_DIR"
set +e
timeout 120s "$BIN" -X -I opengfx -D -g "$SAV" >/tmp/openttdrs-world-raw-run.log 2>&1
rc=$?
set -e

if [[ ! -f "$OUT" ]]; then
  echo "error: no se generó $OUT (exit=$rc). Log:" >&2
  tail -n 40 /tmp/openttdrs-world-raw-run.log >&2 || true
  exit 1
fi

if ! python3 -c "import json,sys; d=json.loads(open(sys.argv[1], encoding='utf-8').readline()); sys.exit(0 if d.get('producer') == 'openttd' and d.get('contract') == 'world-raw' else 1)" "$OUT"; then
  echo "error: $OUT no declara metadata world-raw de OpenTTD" >&2
  exit 1
fi

echo "OK: oráculo world-raw escrito en $OUT (producer=openttd)"
