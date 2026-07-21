#!/usr/bin/env bash
# Exporta una traza airport FTA JSONL post-tick desde OpenTTD 15.3 parcheado.
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
TICKS="${3:-80}"
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
export OPENTTDRS_AIRPORT_FTA_TRACE_OUT="$OUT"
export OPENTTDRS_AIRPORT_FTA_TRACE_TICKS="$TICKS"
export OPENTTDRS_AIRPORT_FTA_TRACE_SOURCE="${SAV#"$ROOT"/}"
export OPENTTDRS_OPENTTD_COMMIT="$COMMIT"
export OPENTTDRS_SNAPSHOT_MIN_CALL="${OPENTTDRS_SNAPSHOT_MIN_CALL:-2}"

echo "oráculo airport FTA OpenTTD: bin=$BIN sav=$SAV ticks=$TICKS out=$OUT commit=$COMMIT"
cd "$BUILD_DIR"
timeout 90s ./openttd -X -I opengfx -D -g "$SAV" >/tmp/openttdrs-airport-fta-trace.log 2>&1 || rc=$?
rc="${rc:-0}"

if [[ ! -s "$OUT" ]]; then
  echo "error: no se generó la traza FTA (exit=$rc). Log:" >&2
  tail -n 40 /tmp/openttdrs-airport-fta-trace.log >&2 || true
  exit 1
fi

python3 - "$OUT" "$TICKS" <<'PY'
import json, sys
path, ticks = sys.argv[1], int(sys.argv[2])
rows = [json.loads(l) for l in open(path) if l.strip()]
assert rows and rows[0]["kind"] == "metadata" and rows[0].get("trace") == "airport_fta"
body = [r for r in rows if r["kind"] in ("initial", "tick")]
assert body and body[0]["kind"] == "initial"
assert sum(1 for r in body if r["kind"] == "tick") == ticks
assert any(r.get("aircraft") for r in body), "sin aircraft en la traza"
print(f"OK: validate {path} ticks={ticks} aircraft0={len(body[0].get('aircraft',[]))} airports0={len(body[0].get('airports',[]))}")
PY
echo "OK: traza airport FTA OpenTTD → $OUT"
