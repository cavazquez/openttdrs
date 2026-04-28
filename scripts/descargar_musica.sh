#!/usr/bin/env bash
# Descarga OpenMSX — set de música libre para OpenTTD.
#
# Los archivos se extraen en assets/openmsx/ (carpeta ignorada por git).
# Versión configurable con la variable de entorno OPENMSX_VERSION.
#
# Uso:
#   ./scripts/descargar_musica.sh
#   OPENMSX_VERSION=0.4.2 ./scripts/descargar_musica.sh
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION="${OPENMSX_VERSION:-0.4.2}"
DEST="${ROOT}/assets/openmsx"
CDN="https://cdn.openttd.org/openmsx-releases/${VERSION}/openmsx-${VERSION}-all.zip"

if [[ -d "${DEST}" && -n "$(ls -A "${DEST}" 2>/dev/null)" ]]; then
  echo "OpenMSX ya está en ${DEST}. Borrá la carpeta para re-descargar."
else
  mkdir -p "${DEST}"

  echo "Descargando OpenMSX ${VERSION} desde ${CDN} ..."
  TMP="$(mktemp -d)"
  trap 'rm -rf "${TMP}"' EXIT

  curl -fL "${CDN}" -o "${TMP}/openmsx.zip"
  unzip -q "${TMP}/openmsx.zip" -d "${TMP}/openmsx"

  shopt -s dotglob
  cp -r "${TMP}/openmsx/"*/* "${DEST}/" 2>/dev/null || cp -r "${TMP}/openmsx/"* "${DEST}/"
fi

BASE_DIR="${DEST}/openmsx-${VERSION}"
BASE_TAR="${BASE_DIR}.tar"
if [[ ! -d "${BASE_DIR}" && -f "${BASE_TAR}" ]]; then
  echo ""
  echo "Extrayendo ${BASE_TAR} ..."
  tar -xf "${BASE_TAR}" -C "${DEST}"
fi

echo ""
echo "Listo. Archivos en ${DEST}/:"
ls -1 "${DEST}/"
echo ""
echo "OpenMSX se instala en OpenTTD como base music set."
echo "En OpenTTD: Game Options -> Base music set -> OpenMSX."
