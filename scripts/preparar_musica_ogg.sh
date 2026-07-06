#!/usr/bin/env bash
# Pre-decodifica subset OpenMSX → OGG para el cliente Bevy (Camino A: commitear en git).
#
# Mapeo de slots del cliente → MIDIs reales en openmsx.obm (v0.4.2):
#   theme.ogg   ← tttheme2.mid
#   old_01.ogg  ← ttsong_iv_imuh3.mid
#   old_02.ogg  ← modern_motion.mid
#   new_01.ogg  ← midnight_snow_run.mid
#   ezy_01.ogg  ← 5432gone_redfarn.mid
#
# Requiere: OpenMSX extraído, fluidsynth, ffmpeg y un SoundFont GM (.sf2).
# Uso:
#   ./scripts/preparar_musica_ogg.sh
#   ./scripts/descargar_musica.sh --openmsx   # descarga + llama a este script
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION="${OPENMSX_VERSION:-0.4.2}"
BASE_DIR="${ROOT}/assets/openmsx/openmsx-${VERSION}"
OUT_DIR="${ROOT}/assets/music"

resolve_soundfont() {
  if [[ -n "${SOUNDFONT:-}" && -f "${SOUNDFONT}" ]]; then
    echo "${SOUNDFONT}"
    return
  fi
  for candidate in \
    /usr/share/sounds/sf2/FluidR3_GM.sf2 \
    /usr/share/sounds/sf2/default-GM.sf2 \
    /usr/share/soundfonts/FluidR3_GM.sf2; do
    if [[ -f "${candidate}" ]]; then
      echo "${candidate}"
      return
    fi
  done
  echo ""
}

convert_track() {
  local ogg_name="$1"
  local midi_name="$2"
  local sf2="$3"
  local src="${BASE_DIR}/${midi_name}"
  local wav_tmp
  wav_tmp="$(mktemp /tmp/openttdrs_XXXXXX.wav)"

  if [[ ! -f "${src}" ]]; then
    echo "Aviso: falta ${src}; omito ${ogg_name}" >&2
    return
  fi
  if ! fluidsynth -ni -F "${wav_tmp}" -r 44100 -g 0.4 "${sf2}" "${src}" >/dev/null 2>&1; then
    echo "Aviso: fluidsynth falló en ${midi_name}" >&2
    rm -f "${wav_tmp}"
    return
  fi
  if ! ffmpeg -y -loglevel error -i "${wav_tmp}" -c:a libvorbis -q:a 4 "${OUT_DIR}/${ogg_name}"; then
    echo "Aviso: ffmpeg falló en ${ogg_name}" >&2
    rm -f "${wav_tmp}"
    return
  fi
  rm -f "${wav_tmp}"
  echo "  ${ogg_name} ← ${midi_name}"
}

if [[ ! -d "${BASE_DIR}" ]]; then
  echo "Error: no está ${BASE_DIR}. Ejecutá: ./scripts/descargar_musica.sh --openmsx" >&2
  exit 1
fi
if ! command -v fluidsynth >/dev/null 2>&1; then
  echo "Error: fluidsynth no encontrado." >&2
  echo "  Ubuntu/Debian: sudo apt install fluidsynth fluid-soundfont-gm" >&2
  exit 1
fi
if ! command -v ffmpeg >/dev/null 2>&1; then
  echo "Error: ffmpeg no encontrado." >&2
  echo "  Ubuntu/Debian: sudo apt install ffmpeg" >&2
  exit 1
fi
SF2="$(resolve_soundfont)"
if [[ -z "${SF2}" ]]; then
  echo "Error: no encontré SoundFont GM (.sf2)." >&2
  echo "  Ubuntu/Debian: sudo apt install fluid-soundfont-gm" >&2
  echo "  O exportá SOUNDFONT=/ruta/a/tu.sf2" >&2
  exit 1
fi

mkdir -p "${OUT_DIR}"
echo "Convirtiendo subset MIDI → OGG en ${OUT_DIR} (SoundFont: ${SF2}) ..."
convert_track theme.ogg tttheme2.mid "${SF2}"
convert_track old_01.ogg ttsong_iv_imuh3.mid "${SF2}"
convert_track old_02.ogg modern_motion.mid "${SF2}"
convert_track new_01.ogg midnight_snow_run.mid "${SF2}"
convert_track ezy_01.ogg 5432gone_redfarn.mid "${SF2}"

echo ""
echo "Música en ${OUT_DIR}/:"
ls -1 "${OUT_DIR}/"*.ogg 2>/dev/null || echo "(ningún OGG generado)"
