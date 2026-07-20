#!/usr/bin/env bash
# Genera train_consist_2wagon_pbs_15_3.sav y su oráculo JSONL v2 desde train_pbs_15_3.sav.
# Requiere OpenTTD 15.3 parcheado (patches/openttd-15.3-snapshot-export).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=lib/openttd_reference.sh
source "${ROOT}/scripts/lib/openttd_reference.sh"

BIN="${OPENTTD_BIN:-${ROOT}/reference/openttd-upstream/build/openttd}"
BUILD_DIR="$(dirname "$BIN")"
BASESET_SRC="${OPENTTDRS_OPENGFX_DIR:-${ROOT}/.deps/openttd-baseset/opengfx-8.0}"
PREFIX="${OPENTTDRS_DEPS_PREFIX:-${ROOT}/.deps/openttd-prefix}"
COMMIT="$(openttd_manifest_get "$ROOT" commit)"

BASE_SAV="${ROOT}/crates/openttdrs-core/tests/fixtures/train_pbs_15_3.sav"
SAV_OUT="${ROOT}/crates/openttdrs-core/tests/fixtures/train_consist_2wagon_pbs_15_3.sav"
JSONL_OUT="${ROOT}/crates/openttdrs-core/tests/fixtures/parity/train_consist_2wagon_pbs_15_3_openttd.jsonl"
TICKS="${1:-40}"

if [[ ! -x "$BIN" ]]; then
  echo "error: no hay binario OpenTTD parcheado en $BIN" >&2
  exit 1
fi
if [[ ! -f "$BASE_SAV" ]]; then
  echo "error: falta $BASE_SAV" >&2
  exit 1
fi

if [[ -d "$BASESET_SRC" ]]; then
  mkdir -p "${BUILD_DIR}/baseset"
  cp -a "${BASESET_SRC}/." "${BUILD_DIR}/baseset/" 2>/dev/null || true
fi

export LD_LIBRARY_PATH="${PREFIX}/usr/lib/x86_64-linux-gnu:${PREFIX}/usr/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
rm -f "$SAV_OUT" "$JSONL_OUT"

echo "1/2: enganchar 2 vagones y guardar $SAV_OUT"
env -u OPENTTDRS_PBS_TRACE_OUT \
  OPENTTDRS_FIXTURE_ATTACH_WAGONS=2 \
  OPENTTDRS_FIXTURE_SAVE_OUT="$SAV_OUT" \
  OPENTTDRS_SNAPSHOT_MIN_CALL=2 \
  bash -c "cd \"$BUILD_DIR\" && timeout 60s ./openttd -X -I opengfx -D -g \"$BASE_SAV\"" \
  >/tmp/openttdrs-consist-gen-sav.log 2>&1 || true
if [[ ! -s "$SAV_OUT" ]]; then
  echo "error: no se generó el .sav. Log:" >&2
  tail -n 40 /tmp/openttdrs-consist-gen-sav.log >&2 || true
  exit 1
fi

echo "2/2: exportar oráculo PBS v2 ($TICKS ticks)"
# env limpio: no re-enganchar vagones sobre el save ya materializado.
env -u OPENTTDRS_FIXTURE_ATTACH_WAGONS -u OPENTTDRS_FIXTURE_SAVE_OUT \
  OPENTTDRS_PBS_TRACE_OUT="$JSONL_OUT" \
  OPENTTDRS_PBS_TRACE_TICKS="$TICKS" \
  OPENTTDRS_PBS_TRACE_SOURCE="crates/openttdrs-core/tests/fixtures/train_consist_2wagon_pbs_15_3.sav" \
  OPENTTDRS_OPENTTD_COMMIT="$COMMIT" \
  OPENTTDRS_SNAPSHOT_MIN_CALL=2 \
  bash -c "cd \"$BUILD_DIR\" && timeout 60s ./openttd -X -I opengfx -D -g \"$SAV_OUT\"" \
  >/tmp/openttdrs-consist-gen-jsonl.log 2>&1 || true

python3 "${ROOT}/scripts/validate_pbs_trace.py" "$JSONL_OUT" "$TICKS" openttd
echo "OK: fixture multi-vagón → $SAV_OUT + $JSONL_OUT"
