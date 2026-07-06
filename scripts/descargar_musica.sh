#!/usr/bin/env bash
# Descarga OpenMSX — set de música libre para OpenTTD.
#
# Los archivos se extraen en assets/openmsx/ (carpeta ignorada por git).
# Versión configurable con la variable de entorno OPENMSX_VERSION.
#
# Uso:
#   ./scripts/descargar_musica.sh --openmsx
#   OPENMSX_VERSION=0.4.2 ./scripts/descargar_musica.sh --openmsx
set -euo pipefail

usage() {
  cat <<'EOF'
Uso:
  ./scripts/descargar_musica.sh --openmsx

Opciones:
  --openmsx   Descarga y prepara OpenMSX
  -h, --help  Muestra esta ayuda
EOF
}

if [[ $# -ne 1 ]]; then
  echo "Error: debés indicar --openmsx." >&2
  usage
  exit 1
fi

case "${1}" in
  --openmsx) ;;
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
VERSION="${OPENMSX_VERSION:-0.4.2}"
DEST="${ROOT}/assets/openmsx"
DOWNLOADS_DIR="${ROOT}/.downloads/openttd"
CDN="https://cdn.openttd.org/openmsx-releases/${VERSION}/openmsx-${VERSION}-all.zip"
ZIP_CACHE="${DOWNLOADS_DIR}/openmsx-${VERSION}-all.zip"
TAR_CACHE="${DOWNLOADS_DIR}/openmsx-${VERSION}.tar"

mkdir -p "${DEST}"
mkdir -p "${DOWNLOADS_DIR}"

if [[ -f "${TAR_CACHE}" ]]; then
  echo "OpenMSX ${VERSION} ya descargado en ${TAR_CACHE}"
else
  if [[ ! -f "${ZIP_CACHE}" ]]; then
    echo "Descargando OpenMSX ${VERSION} desde ${CDN} ..."
    curl -fL "${CDN}" -o "${ZIP_CACHE}"
  else
    echo "Zip en cache detectado: ${ZIP_CACHE}"
  fi

  echo "Preparando ${TAR_CACHE} ..."
  TMP="$(mktemp -d)"
  trap 'rm -rf "${TMP}"' EXIT
  unzip -q "${ZIP_CACHE}" -d "${TMP}/openmsx"
  CANDIDATE_TAR="$(rg --files "${TMP}/openmsx" | rg "openmsx-${VERSION}\\.tar$" | awk 'NR==1{print; exit}' || true)"
  if [[ -z "${CANDIDATE_TAR}" ]]; then
    echo "No encontré openmsx-${VERSION}.tar dentro del zip."
    exit 1
  fi
  cp "${CANDIDATE_TAR}" "${TAR_CACHE}"
fi

BASE_DIR="${DEST}/openmsx-${VERSION}"
if [[ ! -d "${BASE_DIR}" && -f "${TAR_CACHE}" ]]; then
  echo ""
  echo "Extrayendo ${TAR_CACHE} en ${DEST} ..."
  tar -xf "${TAR_CACHE}" -C "${DEST}"
fi

# Limpieza de layout legado: tar dentro de assets/.
rm -f "${DEST}/openmsx-${VERSION}.tar"

echo ""
echo "Cache de descargas en ${DOWNLOADS_DIR}/:"
ls -1 "${DOWNLOADS_DIR}/"
echo ""
echo "Listo. Archivos en ${DEST}/ (assets finales):"
ls -1 "${DEST}/"
echo ""
echo "OpenMSX se instala en OpenTTD como base music set."
echo "En OpenTTD: Game Options -> Base music set -> OpenMSX."

# Pre-decodificar subset a OGG para el cliente Bevy (requiere fluidsynth o timidity).
MUSIC_OGG_DIR="${ROOT}/assets/music"
mkdir -p "${MUSIC_OGG_DIR}"
if command -v fluidsynth >/dev/null 2>&1; then
  echo ""
  echo "Convirtiendo subset MIDI → OGG en ${MUSIC_OGG_DIR} ..."
  for midi in theme old_01 old_02 new_01 ezy_01; do
    src="$(find "${BASE_DIR}" -name "${midi}.mid" -o -name "${midi}.MID" 2>/dev/null | head -1 || true)"
    if [[ -n "${src}" && -f "${src}" ]]; then
      fluidsynth -ni -F /tmp/openttdrs_${midi}.wav -r 44100 /usr/share/sounds/sf2/FluidR3_GM.sf2 "${src}" 2>/dev/null || true
      if [[ -f "/tmp/openttdrs_${midi}.wav" ]] && command -v ffmpeg >/dev/null 2>&1; then
        ffmpeg -y -i "/tmp/openttdrs_${midi}.wav" -c:a libvorbis "${MUSIC_OGG_DIR}/${midi}.ogg" 2>/dev/null || true
        rm -f "/tmp/openttdrs_${midi}.wav"
      fi
    fi
  done
  echo "OGG listos (si fluidsynth/ffmpeg disponibles):"
  ls -1 "${MUSIC_OGG_DIR}/" 2>/dev/null || true
else
  echo ""
  echo "Nota: instala fluidsynth + ffmpeg para generar assets/music/*.ogg automáticamente."
fi
