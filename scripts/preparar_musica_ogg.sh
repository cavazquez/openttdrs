#!/usr/bin/env bash
# Pre-decodifica OpenMSX → OGG para el cliente Bevy (Camino A: commitear en git).
#
# Mapeo según openmsx.obm (v0.4.2): theme + old_0..9 + new_0..9 + ezy_0..6.
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
echo "Convirtiendo OpenMSX → OGG en ${OUT_DIR} (SoundFont: ${SF2}) ..."

convert_track theme.ogg tttheme2.mid "${SF2}"

convert_track old_00.ogg keep_on_rolling.mid "${SF2}"
convert_track old_01.ogg ttsong_iv_imuh3.mid "${SF2}"
convert_track old_02.ogg modern_motion.mid "${SF2}"
convert_track old_03.ogg busy_schedule.mid "${SF2}"
convert_track old_04.ogg the_fast_route.mid "${SF2}"
convert_track old_05.ogg ttsong_iii_imuh3.mid "${SF2}"
convert_track old_06.ogg train_filled_with_cash.mid "${SF2}"
convert_track old_07.ogg flying_scotsman.mid "${SF2}"
convert_track old_08.ogg chuggachugga.mid "${SF2}"
convert_track old_09.ogg the_hobo_redfarn.mid "${SF2}"

convert_track new_00.ogg ultimate_run.mid "${SF2}"
convert_track new_01.ogg midnight_snow_run.mid "${SF2}"
convert_track new_02.ogg run_for_your_life.mid "${SF2}"
convert_track new_03.ogg coconut_run2.mid "${SF2}"
convert_track new_04.ogg harp_harmony.mid "${SF2}"
convert_track new_05.ogg mighty_giant_run.mid "${SF2}"
convert_track new_06.ogg wood_whistles.mid "${SF2}"
convert_track new_07.ogg linns_basket.mid "${SF2}"
convert_track new_08.ogg relax_song.mid "${SF2}"
convert_track new_09.ogg chemistry_lab.mid "${SF2}"

convert_track ezy_00.ogg boogi_marabi_redfarn.mid "${SF2}"
convert_track ezy_01.ogg 5432gone_redfarn.mid "${SF2}"
convert_track ezy_02.ogg moo_redfarn.mid "${SF2}"
convert_track ezy_03.ogg say_what_redfarn.mid "${SF2}"
convert_track ezy_04.ogg be_sharp_bw_redfarn.mid "${SF2}"
convert_track ezy_05.ogg careless_perc_redfarn.mid "${SF2}"
convert_track ezy_06.ogg mosey_along_redfarn.mid "${SF2}"

echo ""
echo "Música en ${OUT_DIR}/:"
ls -1 "${OUT_DIR}/"*.ogg 2>/dev/null || echo "(ningún OGG generado)"
