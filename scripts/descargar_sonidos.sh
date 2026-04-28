#!/usr/bin/env bash
# Descarga OpenSFX — efectos de sonido de reemplazo libre para OpenTTD.
#
# Los archivos se extraen en assets/opensfx/ (carpeta ignorada por git).
# Versión configurable con la variable de entorno OPENSFX_VERSION.
#
# Uso:
#   ./scripts/descargar_sonidos.sh --opensfx
#   OPENSFX_VERSION=1.0.3 ./scripts/descargar_sonidos.sh --opensfx
set -euo pipefail

usage() {
  cat <<'EOF'
Uso:
  ./scripts/descargar_sonidos.sh --opensfx

Opciones:
  --opensfx   Descarga y prepara OpenSFX
  -h, --help  Muestra esta ayuda
EOF
}

if [[ $# -ne 1 ]]; then
  echo "Error: debés indicar --opensfx." >&2
  usage
  exit 1
fi

case "${1}" in
  --opensfx) ;;
  -h|--help)
    usage
    exit 0
    ;;
  *)
    echo "Error: opción desconocida '${1}'." >&2
    usage
    exit 1
    ;;
esac

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION="${OPENSFX_VERSION:-1.0.3}"
DEST="${ROOT}/assets/opensfx"
DOWNLOADS_DIR="${ROOT}/.downloads/openttd"
CDN="https://cdn.openttd.org/opensfx-releases/${VERSION}/opensfx-${VERSION}-all.zip"
ZIP_CACHE="${DOWNLOADS_DIR}/opensfx-${VERSION}-all.zip"
TAR_CACHE="${DOWNLOADS_DIR}/opensfx-${VERSION}.tar"

mkdir -p "${DEST}"
mkdir -p "${DOWNLOADS_DIR}"

if [[ -f "${TAR_CACHE}" ]]; then
  echo "OpenSFX ${VERSION} ya descargado en ${TAR_CACHE}"
else
  if [[ ! -f "${ZIP_CACHE}" ]]; then
    echo "Descargando OpenSFX ${VERSION} desde ${CDN} ..."
    curl -fL "${CDN}" -o "${ZIP_CACHE}"
  else
    echo "Zip en cache detectado: ${ZIP_CACHE}"
  fi

  echo "Preparando ${TAR_CACHE} ..."
  TMP="$(mktemp -d)"
  trap 'rm -rf "${TMP}"' EXIT
  unzip -q "${ZIP_CACHE}" -d "${TMP}/opensfx"
  CANDIDATE_TAR="$(rg --files "${TMP}/opensfx" | rg "opensfx-${VERSION}\\.tar$" | awk 'NR==1{print; exit}' || true)"
  if [[ -z "${CANDIDATE_TAR}" ]]; then
    echo "No encontré opensfx-${VERSION}.tar dentro del zip."
    exit 1
  fi
  cp "${CANDIDATE_TAR}" "${TAR_CACHE}"
fi

BASE_DIR="${DEST}/opensfx-${VERSION}"
if [[ ! -d "${BASE_DIR}" && -f "${TAR_CACHE}" ]]; then
  echo ""
  echo "Extrayendo ${TAR_CACHE} en ${DEST} ..."
  tar -xf "${TAR_CACHE}" -C "${DEST}"
fi

# Limpieza de layout legado: tar dentro de assets/.
rm -f "${DEST}/opensfx-${VERSION}.tar"

echo ""
echo "Cache de descargas en ${DOWNLOADS_DIR}/:"
ls -1 "${DOWNLOADS_DIR}/"
echo ""
echo "Listo. Archivos en ${DEST}/ (assets finales):"
ls -1 "${DEST}/"
echo ""
echo "Los sonidos están en formato .cat (Catálogo de samples)."
echo "Para extraer a .wav podés usar catcodec:"
echo "  https://github.com/OpenTTD/catcodec"
