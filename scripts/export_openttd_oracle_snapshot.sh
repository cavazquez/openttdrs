#!/usr/bin/env bash
# Ejecuta OpenTTD (binario patched) y escribe el snapshot oráculo (#110).
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

if [[ ! -f "$SAV" ]]; then
  echo "error: no existe $SAV" >&2
  exit 1
fi
if [[ ! -x "$BIN" ]]; then
  echo "error: no hay binario OpenTTD en $BIN" >&2
  echo "  Integrá el export y compilá dedicated:" >&2
  echo "    ./patches/openttd-15.3-snapshot-export/integrate.sh" >&2
  echo "    cmake -B reference/openttd-upstream/build -S reference/openttd-upstream -DOPTION_DEDICATED=ON" >&2
  echo "    cmake --build reference/openttd-upstream/build -j" >&2
  exit 1
fi

mkdir -p "$(dirname "$OUT")"
rm -f "$OUT"

echo "oráculo OpenTTD: bin=$BIN sav=$SAV out=$OUT commit=$COMMIT"
# Dedicated carga el save y AfterLoadGame escribe el JSON; matamos el proceso tras un tiempo.
set +e
OPENTTDRS_SNAPSHOT_OUT="$OUT" OPENTTDRS_OPENTTD_COMMIT="$COMMIT" \
  timeout 20s "$BIN" -D -g "$SAV" >/tmp/openttdrs-oracle-run.log 2>&1
rc=$?
set -e
if [[ ! -f "$OUT" ]]; then
  echo "error: no se generó $OUT (exit=$rc). Log:" >&2
  tail -n 40 /tmp/openttdrs-oracle-run.log >&2 || true
  exit 1
fi

# Validar que no sea un artefacto circular.
if python3 -c "import json,sys; d=json.load(open(sys.argv[1])); sys.exit(0 if d.get('producer')=='openttd' else 1)" "$OUT"; then
  echo "OK: oráculo escrito en $OUT (producer=openttd)"
else
  echo "error: $OUT no declara producer=openttd" >&2
  exit 1
fi
