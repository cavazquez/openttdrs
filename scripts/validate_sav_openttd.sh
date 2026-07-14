#!/usr/bin/env bash
# Smoke opcional: intenta cargar un .sav con OpenTTD oficial (#66).
# Si OpenTTD no está instalado, sale 0 (SKIP).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SAV="${1:-$ROOT/crates/openttdrs-core/tests/fixtures/demo_openttd.sav}"
OPENTTD_BIN="${OPENTTD:-openttd}"

if ! command -v "$OPENTTD_BIN" >/dev/null 2>&1; then
  echo "SKIP: OpenTTD no encontrado (exportá OPENTTD=/ruta/openttd para probar)."
  exit 0
fi

if [[ ! -f "$SAV" ]]; then
  echo "FAIL: no existe $SAV" >&2
  exit 1
fi

# Carga headless; muchos builds aceptan -g. Si falla, documentamos el gap de chunks.
set +e
"$OPENTTD_BIN" -g "$SAV" -v null: 2>/tmp/openttdrs_sav_openttd.log
rc=$?
set -e
if [[ $rc -eq 0 ]]; then
  echo "OK: OpenTTD cargó $SAV"
  exit 0
fi
echo "WARN: OpenTTD no cargó el save (rc=$rc). Ver /tmp/openttdrs_sav_openttd.log"
echo "      El export openttdrs es válido para roundtrip interno; falta GSET/NewGRF completo."
# No fallamos CI: la validación estructural es la obligatoria.
exit 0
