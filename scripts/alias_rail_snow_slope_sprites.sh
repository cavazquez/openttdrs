#!/usr/bin/env bash
# Crea rail_{1049..1060}.png desde rail_{1023..1034}.png (pendiente + nieve, preload Bevy).
# Si el NFO exporta el sprite real, descargar_graficos.sh lo recorta; este script es fallback rápido.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TILES="$ROOT/assets/opengfx/tiles"
RAIL_SNOW_OFFSET=26
for sid in $(seq 1023 1034); do
  snow_sid=$((sid + RAIL_SNOW_OFFSET))
  src_path="$TILES/rail_${sid}.png"
  dst_path="$TILES/rail_${snow_sid}.png"
  if [[ ! -f "$src_path" ]]; then
    echo "Falta $src_path — ejecutá: bash scripts/descargar_graficos.sh" >&2
    exit 1
  fi
  cp -f "$src_path" "$dst_path"
  echo "  rail_${snow_sid}.png <- rail_${sid}.png"
done
echo "Listo."
