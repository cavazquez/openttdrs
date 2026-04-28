#!/usr/bin/env bash
# Descarga OpenSFX — efectos de sonido de reemplazo libre para OpenTTD.
#
# Los archivos se extraen en assets/opensfx/ (carpeta ignorada por git).
# Versión configurable con la variable de entorno OPENSFX_VERSION.
#
# Uso:
#   ./scripts/descargar_sonidos.sh
#   OPENSFX_VERSION=1.0.3 ./scripts/descargar_sonidos.sh
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION="${OPENSFX_VERSION:-1.0.3}"
DEST="${ROOT}/assets/opensfx"
CDN="https://cdn.openttd.org/opensfx-releases/${VERSION}/opensfx-${VERSION}-all.zip"

if [[ -d "${DEST}" && -n "$(ls -A "${DEST}" 2>/dev/null)" ]]; then
  echo "OpenSFX ya está en ${DEST}. Borrá la carpeta para re-descargar."
else
  mkdir -p "${DEST}"

  echo "Descargando OpenSFX ${VERSION} desde ${CDN} ..."
  TMP="$(mktemp -d)"
  trap 'rm -rf "${TMP}"' EXIT

  curl -fL "${CDN}" -o "${TMP}/opensfx.zip"
  unzip -q "${TMP}/opensfx.zip" -d "${TMP}/opensfx"

  shopt -s dotglob
  cp -r "${TMP}/opensfx/"*/* "${DEST}/" 2>/dev/null || cp -r "${TMP}/opensfx/"* "${DEST}/"
fi

BASE_DIR="${DEST}/opensfx-${VERSION}"
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
echo "Los sonidos están en formato .cat (Catálogo de samples)."
echo "Para extraer a .wav podés usar catcodec:"
echo "  https://github.com/OpenTTD/catcodec"
