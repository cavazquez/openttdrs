#!/usr/bin/env bash
# Smoke de un artefacto Snap estricto ya construido (#582, #583).
#
# No instala el Snap en el host: extrae su SquashFS, vuelve la raíz de assets
# de sólo lectura y ejecuta el launcher con un perfil temporal. Es el contrato
# que el mount `$SNAP` tendrá en producción.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
GRAPHICAL_CHECKER="${ROOT}/scripts/check_release_graphical_smoke.py"
SHOT_WIDTH=1280
SHOT_HEIGHT=720
SHOT_RESOLUTION="${SHOT_WIDTH}x${SHOT_HEIGHT}"

usage() {
  echo "Uso: $0 <archivo .snap>" >&2
}

if [[ $# -ne 1 ]]; then
  usage
  exit 2
fi

artifact="$1"
if [[ ! -f "$artifact" ]]; then
  echo "No existe el Snap: $artifact" >&2
  exit 2
fi

timeout_seconds="${OPENTTDRS_SNAP_SMOKE_TIMEOUT_SECONDS:-45}"
if ! [[ "$timeout_seconds" =~ ^[1-9][0-9]*$ ]]; then
  echo "OPENTTDRS_SNAP_SMOKE_TIMEOUT_SECONDS debe ser un entero positivo." >&2
  exit 2
fi

for command in unsquashfs xvfb-run xauth timeout python3; do
  if ! command -v "$command" >/dev/null 2>&1; then
    echo "Falta $command para el smoke del Snap." >&2
    exit 2
  fi
done
if [[ ! -f "$GRAPHICAL_CHECKER" ]]; then
  echo "No existe el validador gráfico: $GRAPHICAL_CHECKER" >&2
  exit 2
fi

lavapipe_icd="$(find /usr/share/vulkan/icd.d -maxdepth 1 -type f -name 'lvp_icd*.json' -print -quit)"
if [[ -z "$lavapipe_icd" ]]; then
  echo "No se encontró el ICD Lavapipe para el renderer de software." >&2
  exit 2
fi

workdir="$(mktemp -d)"
snap_root="${workdir}/squashfs-root"
profile="${workdir}/isolated-profile"
asset_log="${workdir}/check-assets.log"
menu_log="${workdir}/menu.log"
menu_shot="${workdir}/menu.png"

cleanup() {
  local status=$?
  set +e
  # `chmod` sólo revierte el permiso temporal bajo nuestro mktemp; hace que
  # la limpieza funcione también en un host donde rm requiere escribir dentro
  # de cada directorio extraído.
  chmod -R u+w "$snap_root" 2>/dev/null
  rm -rf "$workdir"
  exit "$status"
}
trap cleanup EXIT

unsquashfs -q -d "$snap_root" "$artifact"

launcher="${snap_root}/bin/openttdrs-launch"
client="${snap_root}/bin/openttdrs-client"
for required in \
  "$launcher" \
  "$client" \
  "${snap_root}/static/fonts/DejaVuSansMono.ttf" \
  "${snap_root}/assets/shaders/rail_glass_post_process.wgsl" \
  "${snap_root}/assets/opengfx/tiles/grass.png" \
  "${snap_root}/assets/opengfx/atlas/tiles_atlas_0.png"; do
  if [[ ! -s "$required" ]]; then
    echo "Falta o está vacío: $required" >&2
    exit 1
  fi
done

shopt -s nullglob
music=("${snap_root}"/assets/music/*.ogg)
sounds=("${snap_root}"/assets/sounds/*.wav)
if (( ${#music[@]} == 0 || ${#sounds[@]} == 0 )); then
  echo "El Snap no contiene música OGG y sonidos WAV reales." >&2
  exit 1
fi

# Emula el mount SquashFS de producción. Si faltan tiles derivados, el
# `--check-assets` siguiente debe fallar en vez de generarlos silenciosamente.
chmod -R a-w "$snap_root"
# Symlinks report mode 0777 by definition even when their targets are
# read-only; inspect only filesystem objects whose write bits are meaningful.
if find "$snap_root" \( -type f -o -type d \) -perm /0222 -print -quit | grep -q .; then
  echo "La raíz extraída del Snap no quedó de sólo lectura." >&2
  exit 1
fi

mkdir -p \
  "${profile}/common" \
  "${profile}/data" \
  "${profile}/home" \
  "${profile}/config" \
  "${profile}/cache" \
  "${profile}/runtime"
chmod 700 "${profile}/runtime"

run_in_snap_env() {
  env -u WAYLAND_DISPLAY -u WAYLAND_SOCKET \
    SNAP="$snap_root" \
    SNAP_USER_COMMON="${profile}/common" \
    SNAP_USER_DATA="${profile}/data" \
    HOME="${profile}/home" \
    XDG_CONFIG_HOME="${profile}/config" \
    XDG_CACHE_HOME="${profile}/cache" \
    XDG_RUNTIME_DIR="${profile}/runtime" \
    XDG_SESSION_TYPE=x11 \
    WINIT_UNIX_BACKEND=x11 \
    LIBGL_ALWAYS_SOFTWARE=1 \
    VK_ICD_FILENAMES="$lavapipe_icd" \
    WGPU_BACKEND=vulkan \
    RUST_BACKTRACE=1 \
    RUST_LOG=info \
    "$@"
}

if ! run_in_snap_env "$launcher" --check-assets >"$asset_log" 2>&1; then
  echo "El launcher del Snap no validó sus assets con raíz read-only:" >&2
  cat "$asset_log" >&2
  exit 1
fi
if ! grep -Fq "Assets OK: $snap_root" "$asset_log"; then
  echo "El launcher no validó exactamente los assets incluidos en el Snap:" >&2
  cat "$asset_log" >&2
  exit 1
fi

if ! run_in_snap_env \
  OPENTTDRS_LANGUAGE=es \
  OPENTTDRS_SHOT_RES="$SHOT_RESOLUTION" \
  OPENTTDRS_MAIN_MENU_SHOT="$menu_shot" \
  xvfb-run -a -s "-screen 0 ${SHOT_RESOLUTION}x24 -nolisten tcp" \
  timeout "${timeout_seconds}s" "$launcher" >"$menu_log" 2>&1; then
  echo "El Snap no pudo iniciar el menú gráfico con assets read-only:" >&2
  tail -n 100 "$menu_log" >&2 || true
  exit 1
fi
if ! grep -Fq "main_menu_shot: menú localizado sin escenario guiado listo" "$menu_log"; then
  echo "El Snap no confirmó la navegación actual del menú:" >&2
  tail -n 100 "$menu_log" >&2 || true
  exit 1
fi

python3 "$GRAPHICAL_CHECKER" \
  --menu "$menu_shot" \
  --width "$SHOT_WIDTH" \
  --height "$SHOT_HEIGHT"

echo "Smoke Snap OK: assets incluidos, launcher, raíz read-only, perfil limpio y menú gráfico."
