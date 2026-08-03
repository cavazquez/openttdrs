#!/usr/bin/env bash
# Ejecuta la matriz de SAV contra el binario oficial OpenTTD fijado.
#
# Es un gate estricto: el binario es obligatorio y cada fixture deja log. El
# modo local opcional sigue disponible mediante los scripts individuales.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ARTIFACT_DIR="${OPENTTDRS_OTTD_ARTIFACT_DIR:-$ROOT/artifacts/openttd-validation}"
LOG_DIR="${OPENTTDRS_OTTD_LOG_DIR:-$ARTIFACT_DIR/logs}"
SUMMARY="$ARTIFACT_DIR/summary.tsv"

mkdir -p "$ARTIFACT_DIR" "$LOG_DIR"
printf 'fixture\tmode\topenttd_version\tresult\tlog\n' >"$SUMMARY"

LOAD_FIXTURES=(
  mvp_openttd_load.sav
  mvp_openttd_stations.sav
  mvp_openttd_train.sav
  mvp_openttd_rich.sav
  mvp_openttd_ship.sav
  demo_openttd.sav
)
ROUNDTRIP_FIXTURES=(mvp_openttd_rich.sav)

export OPENTTDRS_REQUIRE_OPENTTD=1
export OPENTTDRS_OTTD_LOG_DIR="$LOG_DIR"
export OPENTTDRS_OTTD_ARTIFACT_DIR="$ARTIFACT_DIR"
OPENTTD_VERSION="${OPENTTDRS_OTTD_VERSION:-15.3}"

for index in "${!LOAD_FIXTURES[@]}"; do
  fixture="${LOAD_FIXTURES[$index]}"
  sav="$ROOT/crates/openttdrs-core/tests/fixtures/$fixture"
  OPENTTD_SMOKE_PORT="$((3979 + index))" \
    bash "$ROOT/scripts/validate_sav_openttd.sh" "$sav"
  printf '%s\tload\t%s\tpass\tlogs/%s.load.log\n' \
    "$fixture" "$OPENTTD_VERSION" "${fixture%.sav}" >>"$SUMMARY"
done

for fixture in "${ROUNDTRIP_FIXTURES[@]}"; do
  sav="$ROOT/crates/openttdrs-core/tests/fixtures/$fixture"
  OPENTTD_SMOKE_PORT=3990 bash "$ROOT/scripts/roundtrip_sav_openttd.sh" "$sav"
  printf '%s\troundtrip\t%s\tpass\tlogs/%s.roundtrip.log\n' \
    "$fixture" "$OPENTTD_VERSION" "${fixture%.sav}" >>"$SUMMARY"
done

echo "OK: matriz OpenTTD $OPENTTD_VERSION completada; resumen: $SUMMARY"
