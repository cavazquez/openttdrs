#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEST="${ROOT}/reference/openttd-upstream"
URL="${OPENTTD_UPSTREAM_URL:-https://github.com/OpenTTD/OpenTTD.git}"

mkdir -p "${ROOT}/reference"
if [[ -d "${DEST}/.git" ]]; then
  echo "Ya existe ${DEST}; intentando fast-forward..."
  git -C "${DEST}" pull --ff-only || echo "No se pudo ff-only; revisa el remoto manualmente."
else
  echo "Clonando OpenTTD en ${DEST} (shallow)..."
  git clone --depth 1 "${URL}" "${DEST}"
fi
echo "Listo. Revisa docs/INFORME_ARQUITECTURA_OPENTTD.md para el mapa de módulos."
