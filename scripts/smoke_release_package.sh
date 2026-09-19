#!/usr/bin/env bash
# Smoke de un artefacto de release ya comprimido (#296, #577).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
GRAPHICAL_CHECKER="${ROOT}/scripts/check_release_graphical_smoke.py"
SHOT_WIDTH=1280
SHOT_HEIGHT=720
SHOT_RESOLUTION="${SHOT_WIDTH}x${SHOT_HEIGHT}"

usage() {
  echo "Uso: $0 <archivo .tar.gz|.zip>" >&2
}

if [[ $# -ne 1 ]]; then
  usage
  exit 2
fi

archive="$1"
if [[ ! -f "$archive" ]]; then
  echo "No existe el paquete: $archive" >&2
  exit 2
fi

graphical_smoke="${OPENTTDRS_RELEASE_GRAPHICAL_SMOKE:-0}"
case "$graphical_smoke" in
  0|1) ;;
  *)
    echo "OPENTTDRS_RELEASE_GRAPHICAL_SMOKE debe ser 0 o 1." >&2
    exit 2
    ;;
esac
graphical_timeout="${OPENTTDRS_RELEASE_GRAPHICAL_TIMEOUT_SECONDS:-45}"
if ! [[ "$graphical_timeout" =~ ^[1-9][0-9]*$ ]]; then
  echo "OPENTTDRS_RELEASE_GRAPHICAL_TIMEOUT_SECONDS debe ser un entero positivo." >&2
  exit 2
fi

artifact_dir="${OPENTTDRS_RELEASE_SMOKE_ARTIFACT_DIR:-}"
workdir="$(mktemp -d)"
server_pid=""
server_log="${workdir}/dedicated.log"
asset_log="${workdir}/check-assets.log"
network_log="${workdir}/network.log"
menu_log="${workdir}/menu.log"
first_route_log="${workdir}/first-route.log"
menu_shot="${workdir}/menu.png"
first_route_shot="${workdir}/first-route.png"

collect_artifacts() {
  [[ -n "$artifact_dir" ]] || return 0
  mkdir -p "$artifact_dir"
  sha256sum "$archive" >"${artifact_dir}/package.sha256"
  for source in \
    "$asset_log" \
    "$network_log" \
    "$server_log" \
    "$menu_log" \
    "$first_route_log" \
    "$menu_shot" \
    "$first_route_shot"; do
    [[ -f "$source" ]] || continue
    cp "$source" "${artifact_dir}/$(basename "$source")"
  done
}

cleanup() {
  local status=$?
  set +e
  if [[ -n "$server_pid" ]]; then
    kill "$server_pid" 2>/dev/null
    wait "$server_pid" 2>/dev/null
  fi
  collect_artifacts
  rm -rf "$workdir"
  exit "$status"
}
trap cleanup EXIT

case "$archive" in
  *.tar.gz) tar -xzf "$archive" -C "$workdir" ;;
  *.zip) 7z x -bd -o"$workdir" "$archive" >/dev/null ;;
  *)
    echo "Formato no soportado: $archive" >&2
    exit 2
    ;;
esac

package_dir="$(find "$workdir" -mindepth 1 -maxdepth 1 -type d -name 'openttdrs-*' -print -quit)"
if [[ -z "$package_dir" ]]; then
  echo "El archivo no contiene un directorio openttdrs-* en su raíz." >&2
  exit 1
fi

suffix=""
if [[ "$archive" == *.zip ]]; then
  suffix=".exe"
fi
client="${package_dir}/openttdrs-client${suffix}"
dedicated="${package_dir}/openttdrs-dedicated${suffix}"

for required in "$client" "$dedicated" \
  "${package_dir}/static/fonts/DejaVuSansMono.ttf" \
  "${package_dir}/assets/shaders/rail_glass_post_process.wgsl" \
  "${package_dir}/assets/opengfx/tiles/grass.png" \
  "${package_dir}/assets/opengfx/atlas/tiles_atlas_0.png"; do
  if [[ ! -s "$required" ]]; then
    echo "Falta o está vacío: $required" >&2
    exit 1
  fi
done

shopt -s nullglob
music=("${package_dir}"/assets/music/*.ogg)
sounds=("${package_dir}"/assets/sounds/*.wav)
if (( ${#music[@]} == 0 || ${#sounds[@]} == 0 )); then
  echo "El paquete no contiene música OGG y sonidos WAV reales." >&2
  exit 1
fi

if ! "$client" --check-assets >"$asset_log" 2>&1; then
  echo "El cliente empaquetado no validó sus assets:" >&2
  cat "$asset_log" >&2
  exit 1
fi
cat "$asset_log"

capture_graphical_frame() {
  local capture_env="$1"
  local output="$2"
  local log="$3"
  local lavapipe_icd="$4"
  local run_cwd="$workdir/outside-checkout"
  local profile="$workdir/isolated-profile"

  (
    cd "$run_cwd"
    env -u WAYLAND_DISPLAY -u WAYLAND_SOCKET \
      HOME="${profile}/home" \
      XDG_CONFIG_HOME="${profile}/config" \
      XDG_DATA_HOME="${profile}/data" \
      XDG_STATE_HOME="${profile}/state" \
      XDG_CACHE_HOME="${profile}/cache" \
      XDG_RUNTIME_DIR="${profile}/runtime" \
      XDG_SESSION_TYPE=x11 \
      WINIT_UNIX_BACKEND=x11 \
      LIBGL_ALWAYS_SOFTWARE=1 \
      VK_ICD_FILENAMES="$lavapipe_icd" \
      WGPU_BACKEND=vulkan \
      RUST_BACKTRACE=1 \
      RUST_LOG=info \
      OPENTTDRS_ASSET_ROOT="$package_dir" \
      OPENTTDRS_DISABLE_AUDIO=1 \
      OPENTTDRS_LANGUAGE=es \
      OPENTTDRS_SHOT_RES="$SHOT_RESOLUTION" \
      "$capture_env=$output" \
      xvfb-run -a -s "-screen 0 ${SHOT_RESOLUTION}x24 -nolisten tcp" \
      timeout "${graphical_timeout}s" "$client"
  ) >"$log" 2>&1
}

run_graphical_smoke() {
  [[ "$graphical_smoke" == "1" ]] || return 0
  if [[ "$suffix" == ".exe" ]]; then
    echo "El smoke gráfico sólo admite un paquete Linux extraído." >&2
    return 1
  fi
  for command in xvfb-run xauth timeout python3; do
    if ! command -v "$command" >/dev/null 2>&1; then
      echo "Falta $command para el smoke gráfico Linux." >&2
      return 1
    fi
  done
  if [[ ! -f "$GRAPHICAL_CHECKER" ]]; then
    echo "No existe el validador de capturas: $GRAPHICAL_CHECKER" >&2
    return 1
  fi
  local lavapipe_icd
  lavapipe_icd="$(find /usr/share/vulkan/icd.d -maxdepth 1 -type f -name 'lvp_icd*.json' -print -quit)"
  if [[ -z "$lavapipe_icd" ]]; then
    echo "No se encontró el ICD Lavapipe para el renderer de software." >&2
    return 1
  fi

  local profile="$workdir/isolated-profile"
  mkdir -p \
    "$workdir/outside-checkout" \
    "${profile}/home" \
    "${profile}/config" \
    "${profile}/data" \
    "${profile}/state" \
    "${profile}/cache" \
    "${profile}/runtime"
  chmod 700 "${profile}/runtime"
  if [[ -e "$workdir/outside-checkout/assets" || -e "$workdir/outside-checkout/reference" ]]; then
    echo "El cwd gráfico no quedó aislado del checkout." >&2
    return 1
  fi

  if ! capture_graphical_frame OPENTTDRS_MAIN_MENU_SHOT "$menu_shot" "$menu_log" "$lavapipe_icd"; then
    echo "El paquete no pudo capturar el menú gráfico:" >&2
    tail -n 100 "$menu_log" >&2 || true
    return 1
  fi
  if ! grep -Fq "main_menu_shot: acción localizada de Primera ruta lista" "$menu_log"; then
    echo "El menú empaquetado no confirmó la acción Primera ruta." >&2
    tail -n 100 "$menu_log" >&2 || true
    return 1
  fi

  if ! capture_graphical_frame OPENTTDRS_FIRST_ROUTE_SHOT "$first_route_shot" "$first_route_log" "$lavapipe_icd"; then
    echo "El paquete no pudo capturar el escenario Primera ruta:" >&2
    tail -n 100 "$first_route_log" >&2 || true
    return 1
  fi
  if ! grep -Fq "first_route_shot: escenario activado por la acción Primera ruta" "$first_route_log"; then
    echo "El escenario empaquetado no confirmó la transición Primera ruta." >&2
    tail -n 100 "$first_route_log" >&2 || true
    return 1
  fi

  python3 "$GRAPHICAL_CHECKER" \
    --menu "$menu_shot" \
    --scenario "$first_route_shot" \
    --width "$SHOT_WIDTH" \
    --height "$SHOT_HEIGHT"
  echo "Smoke gráfico Linux OK: menú y Primera ruta desde cwd/configuración aislados."
}

run_graphical_smoke

if [[ -n "${OPENTTDRS_RELEASE_SMOKE_PORT:-}" ]]; then
  port="$OPENTTDRS_RELEASE_SMOKE_PORT"
else
  python_cmd="python3"
  if ! command -v "$python_cmd" >/dev/null 2>&1; then
    python_cmd="python"
  fi
  # Los runners Windows reservan puertos efímeros de forma dinámica; un puerto
  # fijo puede devolver WSAEACCES aun cuando no haya otro proceso escuchando.
  # Pedimos uno al SO y lo usamos inmediatamente para el smoke de loopback.
  port="$($python_cmd -c 'import socket; listener = socket.socket(); listener.bind(("127.0.0.1", 0)); print(listener.getsockname()[1]); listener.close()')"
fi
address="127.0.0.1:${port}"
"$dedicated" --bind "$address" >"$server_log" 2>&1 &
server_pid=$!

if ! "$client" --network-smoke "$address" >"$network_log" 2>&1; then
  echo "Log del dedicated empaquetado:" >&2
  cat "$server_log" >&2
  echo "Log del cliente empaquetado:" >&2
  cat "$network_log" >&2
  exit 1
fi
cat "$network_log"

echo "Smoke de paquete OK: assets, fuentes, audio, dedicated y handshake local."
