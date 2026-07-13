#!/usr/bin/env bash
# Copia el catálogo completo OpenSFX (73 SFX) usado por SoundId.
#
# Índices osfx_NN vía _sound_idx[] de sound.cpp (SoundFx enum → slot en .cat).
# Salida canónica: assets/sounds/snd_XX.wav (XX = índice SoundFx 0..72).
# También mantiene alias legibles del HUD / subset histórico.
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

# _sound_idx[] de OpenTTD/src/sound.cpp
SOUND_IDX=(
  2 3 4 5 6 7 8 9
  10 11 12 13 14 15 16 17
  18 19 20 21 22 23 24 25
  26 27 28 29 30 31 32 33
  34 35 36 37 38 39 40 0
  1 41 42 43 44 45 46 47
  48 49 50 51 52 53 54 55
  56 57 58 59 60 61 62 63
  64 65 66 67 68 69 70 71
  72
)

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
  local src="${WAV_DIR}/osfx_$(printf '%02d' "${osfx_idx}").wav"
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

# Catálogo completo SoundFx → snd_XX.wav
for i in $(seq 0 72); do
  osfx="${SOUND_IDX[$i]}"
  copy_osfx "${osfx}" "snd_$(printf '%02d' "${i}").wav"
done

# Alias legibles (compat scripts/docs antiguos)
copy_osfx 0  good_year.wav
copy_osfx 1  bad_year.wav
copy_osfx 2  construction_water.wav
copy_osfx 4  departure_steam.wav
copy_osfx 5  train_tunnel.wav
copy_osfx 10 departure_train.wav
copy_osfx 14 level_crossing.wav
copy_osfx 18 explosion.wav
copy_osfx 19 train_collision.wav
copy_osfx 23 skid_plane.wav
copy_osfx 25 departure_road.wav
copy_osfx 24 takeoff_heli.wav
copy_osfx 31 construction_other.wav
copy_osfx 32 construction_rail.wav
copy_osfx 33 road_works.wav
copy_osfx 39 construction_bridge.wav

echo ""
echo "Catálogo OpenSFX (snd_XX + alias) en ${OUT_DIR}/:"
ls -1 "${OUT_DIR}/"snd_*.wav 2>/dev/null | wc -l | xargs -I{} echo "  snd_XX.wav: {} archivos"
ls -1 "${OUT_DIR}/"*.wav | head -n 20
echo "  ..."
