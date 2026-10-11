#!/usr/bin/env bash
# Materializa los tiles derivados sin cambiar la pareja atlas/tablas compilada.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
if [[ $# -ne 1 ]]; then
  echo "Uso: $0 <cliente-compilado>" >&2
  exit 2
fi

# Este control debe preceder a --check-assets: un atlas de otra generación
# puede ser una imagen válida y aun así asignar cada rect a un sprite distinto.
git diff --exit-code -- assets/opengfx/atlas crates/openttdrs-client/src/sprites
OPENTTDRS_ASSET_ROOT="$ROOT" "$1" --check-assets
for required in grass.png vehicle_train_e.png; do
  if [[ ! -s "$ROOT/assets/opengfx/tiles/$required" ]]; then
    echo "Falta tile materializado: $required" >&2
    exit 1
  fi
done
echo "Assets de release OK: atlas versionado y tiles del cliente compilado."
