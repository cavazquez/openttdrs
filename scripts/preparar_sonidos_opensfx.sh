#!/usr/bin/env bash
# Copia el subset de OpenSFX usado por SoundId / SimEvent (además del HUD).
#
# Índices osfx_NN vía _sound_idx[] de sound.cpp (SoundFx enum → slot en .cat).
# Requiere WAV decodificados (catcodec); ver preparar_sonidos_hud.sh.
#
# Uso:
#   ./scripts/preparar_sonidos_opensfx.sh
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
  if [[ -f "${WAV_DIR}/osfx_02.wav" && -f "${WAV_DIR}/osfx_20.wav" ]]; then
    return
  fi
  if [[ ! -f "${CAT_FILE}" ]]; then
    echo "Error: no está ${CAT_FILE}. Ejecutá: ./scripts/descargar_sonidos.sh --opensfx" >&2
    exit 1
  fi
  local codec
  codec="$(resolve_catcodec)"
  if [[ -z "${codec}" ]]; then
    echo "Error: catcodec no encontrado (ver preparar_sonidos_hud.sh)." >&2
    exit 1
  fi
  echo "Decodificando ${CAT_FILE} con ${codec} ..."
  mkdir -p "${WAV_DIR}"
  (cd "${OPENSFX_BASE}" && "${codec}" -d opensfx.cat)
}

copy_osfx() {
  local osfx_idx="$1"
  local dest_name="$2"
  local src="${WAV_DIR}/osfx_${osfx_idx}.wav"
  if [[ ! -f "${src}" ]]; then
    echo "Aviso: falta ${src}; omito ${dest_name}" >&2
    return
  fi
  cp "${src}" "${OUT_DIR}/${dest_name}"
}

mkdir -p "${OUT_DIR}"
ensure_wav_extracted

# HUD (idempotente con preparar_sonidos_hud.sh)
"${ROOT}/scripts/preparar_sonidos_hud.sh"

# SoundId / SimEvent (SoundFx → osfx según _sound_idx[])
copy_osfx 0  good_year.wav           # SND_00_GOOD_YEAR (39)
copy_osfx 1  bad_year.wav            # SND_01_BAD_YEAR (40)
copy_osfx 2  construction_water.wav  # SND_BEGIN / agua (0)
copy_osfx 4  departure_steam.wav     # SND_04_DEPARTURE_STEAM (2)
copy_osfx 5  train_tunnel.wav        # SND_05_TRAIN_THROUGH_TUNNEL (3)
copy_osfx 10 departure_train.wav     # SND_0A_DEPARTURE_TRAIN (8)
copy_osfx 14 level_crossing.wav      # SND_0E_LEVEL_CROSSING (12)
copy_osfx 18 explosion.wav           # SND_12_EXPLOSION (16)
copy_osfx 19 train_collision.wav     # SND_13_TRAIN_COLLISION (17)
copy_osfx 23 departure_road.wav      # SND_19_DEPARTURE_OLD_RV_1 (23)
copy_osfx 24 takeoff_heli.wav        # SND_18_TAKEOFF_HELICOPTER (22)
copy_osfx 23 skid_plane.wav          # SND_17_SKID_PLANE (21)
copy_osfx 32 construction_rail.wav     # SND_20_CONSTRUCTION_RAIL (30)
copy_osfx 33 road_works.wav          # SND_21_ROAD_WORKS (31)
copy_osfx 39 construction_bridge.wav # SND_27_CONSTRUCTION_BRIDGE (37)

echo ""
echo "Sonidos OpenSFX en ${OUT_DIR}/:"
ls -1 "${OUT_DIR}/"*.wav
