#!/usr/bin/env bash
# Extrae los 3 WAV del HUD desde OpenSFX (requiere opensfx.cat decodificado).
#
# Mapeo (SoundIDs OpenTTD → archivo cliente):
#   hud_soft.wav  ← osfx_21 "Beep" (error UI; si ya existe no se sobrescribe)
#   build_ok.wav  ← osfx_31 "Splat (terraform/non-rail builds)"
#   income.wav    ← osfx_20 "Cash till"
#
# Uso:
#   ./scripts/descargar_sonidos.sh --opensfx   # descarga + prepara
#   ./scripts/preparar_sonidos_hud.sh            # solo copia (opensfx ya en disco)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION="${OPENSFX_VERSION:-1.0.3}"
OPENSFX_BASE="${ROOT}/assets/opensfx/opensfx-${VERSION}"
CAT_FILE="${OPENSFX_BASE}/opensfx.cat"
WAV_DIR="${OPENSFX_BASE}/src/wav"
OUT_DIR="${ROOT}/assets/sounds"

resolve_catcodec() {
  if [[ -n "${CATCODEC:-}" && -x "${CATCODEC}" ]]; then
    echo "${CATCODEC}"
    return
  fi
  if command -v catcodec &>/dev/null; then
    command -v catcodec
    return
  fi
  local cached="${ROOT}/.downloads/catcodec/build/catcodec"
  if [[ -x "${cached}" ]]; then
    echo "${cached}"
    return
  fi
  echo ""
}

ensure_wav_extracted() {
  if [[ -f "${WAV_DIR}/osfx_20.wav" && -f "${WAV_DIR}/osfx_31.wav" ]]; then
    return
  fi
  if [[ ! -f "${CAT_FILE}" ]]; then
    echo "Error: no está ${CAT_FILE}. Ejecutá: ./scripts/descargar_sonidos.sh --opensfx" >&2
    exit 1
  fi
  local codec
  codec="$(resolve_catcodec)"
  if [[ -z "${codec}" ]]; then
    echo "Error: catcodec no encontrado. Instalalo o compilalo:" >&2
    echo "  git clone https://github.com/OpenTTD/catcodec.git ${ROOT}/.downloads/catcodec" >&2
    echo "  cmake -S ${ROOT}/.downloads/catcodec -B ${ROOT}/.downloads/catcodec/build" >&2
    echo "  cmake --build ${ROOT}/.downloads/catcodec/build" >&2
    echo "  export CATCODEC=${ROOT}/.downloads/catcodec/build/catcodec" >&2
    exit 1
  fi
  echo "Decodificando ${CAT_FILE} con ${codec} ..."
  mkdir -p "${WAV_DIR}"
  (cd "${OPENSFX_BASE}" && "${codec}" -d opensfx.cat)
}

mkdir -p "${OUT_DIR}"
ensure_wav_extracted

cp "${WAV_DIR}/osfx_31.wav" "${OUT_DIR}/build_ok.wav"
cp "${WAV_DIR}/osfx_20.wav" "${OUT_DIR}/income.wav"
if [[ ! -f "${OUT_DIR}/hud_soft.wav" ]]; then
  cp "${WAV_DIR}/osfx_21.wav" "${OUT_DIR}/hud_soft.wav"
  echo "hud_soft.wav creado desde osfx_21 (Beep)"
else
  echo "hud_soft.wav ya existe; no se sobrescribe"
fi

echo ""
echo "Sonidos HUD en ${OUT_DIR}/:"
ls -1 "${OUT_DIR}/"*.wav
