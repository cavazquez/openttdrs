#!/usr/bin/env bash
# Exporta una traza PBS JSONL post-tick desde OpenTTD 15.3 parcheado.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=lib/openttd_reference.sh
source "${ROOT}/scripts/lib/openttd_reference.sh"

if [[ $# -lt 2 || $# -gt 3 ]]; then
  echo "Uso: $0 <partida.sav> <salida.jsonl> [ticks]" >&2
  exit 2
fi

SAV="$(realpath "$1")"
OUT="$(realpath -m "$2")"
TICKS="${3:-40}"
BIN="${OPENTTD_BIN:-${ROOT}/reference/openttd-upstream/build/openttd}"
COMMIT="$(openttd_manifest_get "$ROOT" commit)"
BUILD_DIR="$(dirname "$BIN")"
BASESET_SRC="${OPENTTDRS_OPENGFX_DIR:-${ROOT}/.deps/openttd-baseset/opengfx-8.0}"
PREFIX="${OPENTTDRS_DEPS_PREFIX:-${ROOT}/.deps/openttd-prefix}"

if [[ ! -f "$SAV" ]]; then
  echo "error: no existe $SAV" >&2
  exit 1
fi
if [[ ! "$TICKS" =~ ^[1-9][0-9]*$ ]]; then
  echo "error: ticks debe ser entero positivo: $TICKS" >&2
  exit 2
fi
if [[ ! -x "$BIN" ]]; then
  echo "error: no hay binario OpenTTD parcheado en $BIN" >&2
  echo "  ./patches/openttd-15.3-snapshot-export/integrate.sh" >&2
  echo "  cmake -B reference/openttd-upstream/build -S reference/openttd-upstream -DOPTION_DEDICATED=ON && cmake --build …" >&2
  exit 1
fi

if [[ -d "$BASESET_SRC" ]]; then
  mkdir -p "${BUILD_DIR}/baseset"
  cp -a "${BASESET_SRC}/." "${BUILD_DIR}/baseset/" 2>/dev/null || true
elif [[ ! -f "${BUILD_DIR}/baseset/opengfx.obg" ]]; then
  echo "error: falta OpenGFX en ${BUILD_DIR}/baseset (o $BASESET_SRC)" >&2
  exit 1
fi

mkdir -p "$(dirname "$OUT")"
rm -f "$OUT"

export LD_LIBRARY_PATH="${PREFIX}/usr/lib/x86_64-linux-gnu:${PREFIX}/usr/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
export OPENTTDRS_PBS_TRACE_OUT="$OUT"
export OPENTTDRS_PBS_TRACE_TICKS="$TICKS"
export OPENTTDRS_PBS_TRACE_SOURCE="${SAV#"$ROOT"/}"
export OPENTTDRS_OPENTTD_COMMIT="$COMMIT"
export OPENTTDRS_SNAPSHOT_MIN_CALL="${OPENTTDRS_SNAPSHOT_MIN_CALL:-2}"

echo "oráculo PBS OpenTTD: bin=$BIN sav=$SAV ticks=$TICKS out=$OUT commit=$COMMIT"
cd "$BUILD_DIR"
timeout 60s ./openttd -X -I opengfx -D -g "$SAV" >/tmp/openttdrs-pbs-trace.log 2>&1 || rc=$?
rc="${rc:-0}"

if [[ ! -s "$OUT" ]]; then
  echo "error: no se generó la traza PBS (exit=$rc). Log:" >&2
  tail -n 40 /tmp/openttdrs-pbs-trace.log >&2 || true
  exit 1
fi

python3 "${ROOT}/scripts/validate_pbs_trace.py" "$OUT" "$TICKS" openttd
echo "OK: traza PBS OpenTTD → $OUT"
