#!/usr/bin/env bash
# Exporta una traza JSONL post-timer del scheduler de industrias de OpenTTD.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=lib/openttd_reference.sh
source "${ROOT}/scripts/lib/openttd_reference.sh"

if [[ $# -lt 2 || $# -gt 3 ]]; then
  echo "Uso: $0 <partida.sav> <salida.jsonl> [días]" >&2
  exit 2
fi

SAV="$(realpath "$1")"
OUT="$(realpath -m "$2")"
DAYS="${3:-40}"
BIN="${OPENTTD_BIN:-${ROOT}/reference/openttd-upstream/build/openttd}"
COMMIT="$(openttd_manifest_get "$ROOT" commit)"
BUILD_DIR="$(dirname "$BIN")"
BASESET_SRC="${OPENTTDRS_OPENGFX_DIR:-${ROOT}/.deps/openttd-baseset/opengfx-8.0}"
PREFIX="${OPENTTDRS_DEPS_PREFIX:-${ROOT}/.deps/openttd-prefix}"
TRACE_LOG="/tmp/openttdrs-industry-trace.log"

if [[ ! -f "$SAV" ]]; then
  echo "error: no existe $SAV" >&2
  exit 1
fi
if [[ ! "$DAYS" =~ ^[1-9][0-9]*$ ]]; then
  echo "error: días debe ser entero positivo: $DAYS" >&2
  exit 2
fi
# El dedicated tarda aproximadamente de dos a tres segundos por jornada en
# fixtures cargadas grandes. El límite fijo de 180 s truncaba una comparación
# de 180 días cerca de la muestra 90 y el validador sólo podía informar una
# JSONL incompleta. Conservamos un mínimo acotado para la invocación usual y
# escalamos la ventana larga; un CI o una máquina lenta puede ajustarlo sin
# editar el exportador.
DEFAULT_TIMEOUT_SECONDS=$((DAYS * 3))
if (( DEFAULT_TIMEOUT_SECONDS < 180 )); then
  DEFAULT_TIMEOUT_SECONDS=180
fi
TRACE_TIMEOUT_SECONDS="${OPENTTDRS_INDUSTRY_TRACE_TIMEOUT:-$DEFAULT_TIMEOUT_SECONDS}"
if [[ ! "$TRACE_TIMEOUT_SECONDS" =~ ^[1-9][0-9]*$ ]]; then
  echo "error: OPENTTDRS_INDUSTRY_TRACE_TIMEOUT debe ser entero positivo: $TRACE_TIMEOUT_SECONDS" >&2
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
export OPENTTDRS_INDUSTRY_TRACE_OUT="$OUT"
export OPENTTDRS_INDUSTRY_TRACE_DAYS="$DAYS"
export OPENTTDRS_INDUSTRY_TRACE_SOURCE="${SAV#"$ROOT"/}"
export OPENTTDRS_OPENTTD_COMMIT="$COMMIT"
export OPENTTDRS_SNAPSHOT_MIN_CALL="${OPENTTDRS_SNAPSHOT_MIN_CALL:-2}"

echo "oráculo scheduler industrias OpenTTD: bin=$BIN sav=$SAV días=$DAYS timeout=${TRACE_TIMEOUT_SECONDS}s out=$OUT commit=$COMMIT"
cd "$BUILD_DIR"
timeout "${TRACE_TIMEOUT_SECONDS}s" ./openttd -X -I opengfx -D -g "$SAV" >"$TRACE_LOG" 2>&1 || rc=$?
rc="${rc:-0}"

if [[ "$rc" -eq 124 ]]; then
  echo "error: el oráculo agotó ${TRACE_TIMEOUT_SECONDS}s antes de exportar $DAYS jornadas." >&2
  echo "  aumentá OPENTTDRS_INDUSTRY_TRACE_TIMEOUT para esta máquina o reducí la ventana; no se acepta una traza parcial." >&2
  tail -n 40 "$TRACE_LOG" >&2 || true
  exit 1
fi

if grep -Fq "Could not bind socket" "$TRACE_LOG"; then
  echo "error: OpenTTD dedicated no pudo abrir un socket; la traza puede seguir una ruta de arranque distinta." >&2
  echo "ejecutá el oráculo fuera del sandbox o con sockets permitidos; no se acepta como oracle RNG." >&2
  tail -n 40 "$TRACE_LOG" >&2 || true
  exit 1
fi

if [[ ! -s "$OUT" ]]; then
  echo "error: no se generó la traza de industrias (exit=$rc). Log:" >&2
  tail -n 40 "$TRACE_LOG" >&2 || true
  exit 1
fi

python3 "${ROOT}/scripts/validate_industry_trace.py" "$OUT" "$DAYS" openttd
echo "OK: traza scheduler industrias OpenTTD → $OUT"
