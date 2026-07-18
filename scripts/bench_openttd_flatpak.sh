#!/usr/bin/env bash
# bench_openttd_flatpak.sh — Game loop (fps console) en OpenTTD Flatpak vs mapas vacíos.
#
# Uso (raíz del repo o cualquier cwd):
#   ./scripts/bench_openttd_flatpak.sh
#   MAP_BITS=10 LANDSCAPE=arctic ./scripts/bench_openttd_flatpak.sh
#
# Requiere: flatpak app org.openttd.OpenTTD
# Métrica: consola `fps` → "Game loop times" (ms/tick). Presupuesto 1× ≈ 27 ms.

set -euo pipefail

APP="${OPENTTD_FLATPAK:-org.openttd.OpenTTD}"
MAP_BITS="${MAP_BITS:-10}"   # 8=256, 10=1024, 12=4096
LANDSCAPE="${LANDSCAPE:-temperate}"  # temperate | arctic
WAIT_GEN="${WAIT_GEN:-}"     # segundos hasta primer fps; auto si vacío
PORT="${PORT:-39790}"
SEED="${SEED:-116}"
CFGDIR="${CFGDIR:-/tmp/openttdrs_ottd_bench}"
mkdir -p "$CFGDIR"

case "$MAP_BITS" in
  8) SIDE=256 ;;
  10) SIDE=1024 ;;
  12) SIDE=4096 ;;
  *) SIDE="2^$MAP_BITS" ;;
esac

if [[ -z "$WAIT_GEN" ]]; then
  case "$MAP_BITS" in
    8) WAIT_GEN=8 ;;
    10) WAIT_GEN=20 ;;
    12) WAIT_GEN=100 ;;
    *) WAIT_GEN=30 ;;
  esac
fi

CFG="$CFGDIR/bench_${SIDE}_${LANDSCAPE}.cfg"
LOG="/tmp/ottd_bench_${SIDE}_${LANDSCAPE}.log"

cat >"$CFG" <<EOF
[misc]
language = english.lng
videodriver = dedicated
sounddriver = null
musicdriver = null
blitter = null

[difficulty]
max_no_competitors = 0
number_towns = 0
industry_density = 0
disasters = false
terrain_type = 1
quantity_sea_lakes = 0

[game_creation]
landscape = ${LANDSCAPE}
map_x = ${MAP_BITS}
map_y = ${MAP_BITS}
starting_year = 1950
town_name = english
industry_density = 0
snow_line_height = 10
snow_coverage = 40
tree_placer = 2

[network]
server_advertise = false
max_clients = 1
max_companies = 1
max_spectators = 0
no_spectator_join = true

[gui]
autosave = off
EOF

echo "[ottd-bench] $APP  map=${SIDE}²  landscape=$LANDSCAPE  wait_gen=${WAIT_GEN}s"
echo "[ottd-bench] log=$LOG"

{
  sleep "$WAIT_GEN"
  echo "fps"
  sleep 4
  echo "fps"
  sleep 2
  echo "quit"
} | flatpak run --filesystem=/tmp "$APP" \
  -D "127.0.0.1:${PORT}" \
  -c "$CFG" \
  -g -G "$SEED" -x -Q \
  -s null -m null \
  2>&1 | tee "$LOG"

echo
echo "[ottd-bench] extract:"
rg -n "Map generated|Game loop rate|Game loop times|GL landscape" "$LOG" || true
