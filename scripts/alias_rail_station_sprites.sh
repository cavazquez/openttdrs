#!/usr/bin/env bash
# Crea rail_{1069..1074}.png desde rail_platform_* (preload Bevy SP3.3).
# No descarga OpenGFX; solo copia si ya tenés los recortes.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TILES="$ROOT/assets/opengfx/tiles"
shopt -s nullglob
pairs=(
  "rail_platform_y_front.png:1069"
  "rail_platform_x_rear.png:1070"
  "rail_platform_y_rear.png:1071"
  "rail_platform_x_front.png:1072"
  "rail_platform_building_x.png:1073"
  "rail_platform_building_y.png:1074"
  "rail_platform_pillars_y_front.png:1075"
  "rail_platform_pillars_x_rear.png:1076"
  "rail_platform_pillars_y_rear.png:1077"
  "rail_platform_pillars_x_front.png:1078"
  "rail_roof_0.png:1079"
  "rail_roof_1.png:1080"
  "rail_roof_2.png:1081"
  "rail_roof_3.png:1082"
  "rail_roof_4.png:1083"
  "rail_roof_5.png:1084"
  "rail_roof_6.png:1085"
  "rail_roof_7.png:1086"
)
for pair in "${pairs[@]}"; do
  src="${pair%%:*}"
  id="${pair##*:}"
  src_path="$TILES/$src"
  dst_path="$TILES/rail_${id}.png"
  if [[ ! -f "$src_path" ]]; then
    echo "Falta $src_path — ejecutá: bash scripts/descargar_graficos.sh --8bpp" >&2
    exit 1
  fi
  cp -f "$src_path" "$dst_path"
  echo "  rail_${id}.png <- $src"
done
echo "Listo."
