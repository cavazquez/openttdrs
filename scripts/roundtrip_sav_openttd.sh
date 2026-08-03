#!/usr/bin/env bash
# Round-trip acotado #226: OpenTTD dedicated carga → console `save` → openttdrs importa.
#
# Uso:
#   bash scripts/roundtrip_sav_openttd.sh [entrada.sav]
#
# Default: mvp_openttd_rich.sav (estaciones + tren + bus + industria).
# Requiere binario OpenTTD (reference/.../openttd o $OPENTTD).
# Si no hay binario: SKIP (exit 0), salvo OPENTTDRS_REQUIRE_OPENTTD=1.
# Fallos de load/save/import → exit 1.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SAV="${1:-$ROOT/crates/openttdrs-core/tests/fixtures/mvp_openttd_rich.sav}"
TMP="${TMPDIR:-/tmp}/openttdrs_roundtrip_$$"
CFGDIR="$TMP/cfg"
if [[ -n "${OPENTTDRS_OTTD_LOG_DIR:-}" ]]; then
  mkdir -p "$OPENTTDRS_OTTD_LOG_DIR"
  LOG="$OPENTTDRS_OTTD_LOG_DIR/$(basename "${SAV%.sav}").roundtrip.log"
else
  LOG="$TMP/openttd.log"
fi
OUT_NAME="openttdrs_resaved"
OUT_SAV=""
DEDICATED_ARGS=(-D)
if [[ -n "${OPENTTD_SMOKE_PORT:-}" ]]; then
  DEDICATED_ARGS=(-D ":${OPENTTD_SMOKE_PORT}")
fi

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
  if [[ "${OPENTTDRS_REQUIRE_OPENTTD:-0}" == "1" ]]; then
    echo "FAIL: OpenTTD requerido pero no encontrado (OPENTTD=$OPENTTD_BIN)." >&2
    exit 1
  fi
  echo "SKIP: OpenTTD no encontrado (exportá OPENTTD=/ruta/openttd)."
  exit 0
fi

if [[ ! -f "$SAV" ]]; then
  echo "FAIL: no existe $SAV" >&2
  echo "      Regenerá: OPENTTDRS_DUMP_MVP_RICH_SAV=... cargo test -p openttdrs-core --lib sav::write::tests::export_mvp_rich_emits_indy_road_vehs_and_stations -- --exact" >&2
  exit 1
fi

# 1) Smoke load (mismo gate que CI).
bash "$ROOT/scripts/validate_sav_openttd.sh" "$SAV"

mkdir -p "$CFGDIR/save"
cat >"$CFGDIR/openttd.cfg" <<'EOF'
[misc]
fullscreen = false
[network]
max_companies = 1
EOF

# 2) Dedicated: cargar y forzar `save` por consola (stdin retardado; el parser
#    de consola no acepta comandos hasta estar online).
set +e
(
  sleep 2
  echo "save ${OUT_NAME}"
  sleep 1
  echo quit
) | timeout --signal=KILL 25 \
  "$OPENTTD_BIN" "${DEDICATED_ARGS[@]}" -g "$SAV" -c "$CFGDIR/openttd.cfg" -x \
  >"$LOG" 2>&1
rc=$?
set -e

if ! grep -Eqi 'Map successfully saved|partida.*guardad' "$LOG"; then
  echo "FAIL: OpenTTD no re-guardó (rc=$rc). Ver $LOG" >&2
  tail -n 40 "$LOG" >&2 || true
  echo "RESIDUAL: dedicated console save no confirmado." >&2
  exit 1
fi

# Buscar el .sav (OpenTTD escribe bajo el personal dir del -c).
OUT_SAV="$(find "$CFGDIR" -name "${OUT_NAME}.sav" -type f 2>/dev/null | head -n1 || true)"
if [[ -z "$OUT_SAV" || ! -f "$OUT_SAV" ]]; then
  echo "FAIL: no se encontró ${OUT_NAME}.sav tras save. Ver $LOG" >&2
  find "$CFGDIR" -type f 2>/dev/null | head -n 20 >&2 || true
  exit 1
fi

echo "OK: OpenTTD re-guardó → $OUT_SAV ($(wc -c <"$OUT_SAV") bytes)"

if [[ -n "${OPENTTDRS_OTTD_ARTIFACT_DIR:-}" ]]; then
  mkdir -p "$OPENTTDRS_OTTD_ARTIFACT_DIR"
  cp "$OUT_SAV" "$OPENTTDRS_OTTD_ARTIFACT_DIR/$(basename "${SAV%.sav}").resaved.sav"
fi

# 3) Import openttdrs + assert subconjunto (strict si el input es el fixture rico).
export OPENTTDRS_ROUNDTRIP_SAV="$OUT_SAV"
if [[ "$(basename "$SAV")" == "mvp_openttd_rich.sav" ]]; then
  export OPENTTDRS_ROUNDTRIP_STRICT=1
fi

cd "$ROOT"
cargo test -p openttdrs-core --test sav_openttd_roundtrip_subset \
  openttd_resaved_preserves_declared_subset -- --exact --nocapture

echo "OK: round-trip OpenTTD→openttdrs del subconjunto declarado"
