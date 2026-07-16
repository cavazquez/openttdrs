#!/usr/bin/env bash
# Ejecuta OpenTTD patched y escribe el snapshot oráculo (#110).
# No usa parse_sav.py ni snapshot_dumper.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=lib/openttd_reference.sh
source "${ROOT}/scripts/lib/openttd_reference.sh"

if [[ $# -lt 2 ]]; then
  echo "Uso: $0 <partida.sav> <salida.json> [openttd-bin]" >&2
  exit 2
fi

SAV="$(realpath "$1")"
OUT="$(realpath -m "$2")"
BIN="${3:-${OPENTTD_BIN:-${ROOT}/reference/openttd-upstream/build/openttd}}"
COMMIT="$(openttd_manifest_get "$ROOT" commit)"
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
  echo "  cmake -B reference/openttd-upstream/build -S reference/openttd-upstream -DOPTION_DEDICATED=ON && cmake --build …" >&2
  exit 1
fi

# Baseset junto al binario (OpenTTD busca ./baseset).
if [[ -d "$BASESET_SRC" ]]; then
  mkdir -p "${BUILD_DIR}/baseset"
  cp -a "${BASESET_SRC}/." "${BUILD_DIR}/baseset/" 2>/dev/null || true
elif [[ ! -f "${BUILD_DIR}/baseset/opengfx.obg" ]]; then
  echo "error: falta OpenGFX en ${BUILD_DIR}/baseset (o $BASESET_SRC)" >&2
  echo "  Extraé .downloads/openttd/opengfx-*.tar en .deps/openttd-baseset/" >&2
  exit 1
fi

mkdir -p "$(dirname "$OUT")"
rm -f "$OUT"

export LD_LIBRARY_PATH="${PREFIX}/usr/lib/x86_64-linux-gnu:${PREFIX}/usr/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
export OPENTTDRS_SNAPSHOT_OUT="$OUT"
export OPENTTDRS_OPENTTD_COMMIT="$COMMIT"
# Saltar AfterLoadGame del new-game dedicado; exportar tras cargar -g.
export OPENTTDRS_SNAPSHOT_MIN_CALL="${OPENTTDRS_SNAPSHOT_MIN_CALL:-2}"

echo "oráculo OpenTTD: bin=$BIN sav=$SAV out=$OUT commit=$COMMIT min_call=$OPENTTDRS_SNAPSHOT_MIN_CALL"
cd "$BUILD_DIR"
set +e
timeout 60s ./openttd -X -I opengfx -D -g "$SAV" >/tmp/openttdrs-oracle-run.log 2>&1
rc=$?
set -e

if [[ ! -f "$OUT" ]]; then
  echo "error: no se generó $OUT (exit=$rc). Log:" >&2
  tail -n 40 /tmp/openttdrs-oracle-run.log >&2 || true
  exit 1
fi

if ! python3 -c "import json,sys; d=json.load(open(sys.argv[1])); sys.exit(0 if d.get('producer')=='openttd' else 1)" "$OUT"; then
  echo "error: $OUT no declara producer=openttd" >&2
  exit 1
fi

echo "OK: oráculo escrito en $OUT (producer=openttd)"
