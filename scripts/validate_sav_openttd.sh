#!/usr/bin/env bash
# Smoke real: carga un .sav con OpenTTD dedicated (#66 / #226).
# Si OpenTTD no está instalado, sale 0 (SKIP).
# Falla si el log indica corrupción / load fallido — aunque el proceso salga 0
# (dedicated cierra el server tras un load fallido con rc=0).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SAV="${1:-$ROOT/crates/openttdrs-core/tests/fixtures/mvp_openttd_load.sav}"
LOG="${TMPDIR:-/tmp}/openttdrs_sav_openttd.log"
CFGDIR="${TMPDIR:-/tmp}/openttdrs_sav_openttd_cfg"
TIMEOUT_SECS="${OPENTTD_SMOKE_TIMEOUT:-12}"

if [[ -z "${OPENTTD:-}" ]]; then
  if [[ -x "$ROOT/reference/openttd-upstream/build/openttd" ]]; then
    OPENTTD_BIN="$ROOT/reference/openttd-upstream/build/openttd"
  else
    OPENTTD_BIN="openttd"
  fi
else
  OPENTTD_BIN="$OPENTTD"
fi

if ! command -v "$OPENTTD_BIN" >/dev/null 2>&1 && [[ ! -x "$OPENTTD_BIN" ]]; then
  echo "SKIP: OpenTTD no encontrado (exportá OPENTTD=/ruta/openttd para probar)."
  exit 0
fi

if [[ ! -f "$SAV" ]]; then
  echo "FAIL: no existe $SAV" >&2
  echo "      Regenerá con: python3 scripts/gen_mvp_openttd_load_sav.py" >&2
  exit 1
fi

mkdir -p "$CFGDIR"
if [[ ! -f "$CFGDIR/openttd.cfg" ]]; then
  cat >"$CFGDIR/openttd.cfg" <<'EOF'
[misc]
fullscreen = false
[network]
max_companies = 1
EOF
fi

# Dedicated + -g: carga completa (no solo LoadCheck de -q).
# Si el load OK, el server sigue vivo → timeout (124) = éxito.
set +e
timeout --signal=KILL "$TIMEOUT_SECS" \
  "$OPENTTD_BIN" -D -g "$SAV" -c "$CFGDIR/openttd.cfg" -x \
  >"$LOG" 2>&1
rc=$?
set -e

fail_patterns='Failed to open savegame|Partida guardada corrupta|Invalid chunk|SlErrorCorrupt|Savegame is corrupt|Unknown chunk|Broken savegame|GameLoadFailed|Error reading savegame|Loading requested map failed|no hay municipios|NO_TOWN_IN_SCENARIO|Error al cargar la partida'

if grep -Eiq "$fail_patterns" "$LOG"; then
  echo "FAIL: OpenTTD reportó error de saveload al cargar $SAV (rc=$rc)." >&2
  echo "----- $LOG -----" >&2
  tail -n 50 "$LOG" >&2 || true
  exit 1
fi

# 124 = timeout: dedicated siguió corriendo tras load OK.
if [[ $rc -eq 124 ]]; then
  echo "OK: OpenTTD dedicated cargó $SAV (server vivo hasta timeout ${TIMEOUT_SECS}s)"
  exit 0
fi

# Otros rc ≠ 0 sin patrón de error: fallo genérico.
if [[ $rc -ne 0 ]]; then
  echo "FAIL: OpenTTD salió con rc=$rc al cargar $SAV. Ver $LOG" >&2
  tail -n 50 "$LOG" >&2 || true
  exit 1
fi

# rc=0 sin mensajes de error: load-check path raro; exigir evidencia positiva.
if grep -Eqi 'Network online|Listening on|Loading savegame version' "$LOG"; then
  echo "OK: OpenTTD dedicated aceptó $SAV (rc=0, sin error de saveload)"
  exit 0
fi

echo "FAIL: sin evidencia de load OK. Ver $LOG" >&2
tail -n 50 "$LOG" >&2 || true
exit 1
