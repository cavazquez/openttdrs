#!/usr/bin/env bash
# Smoke de un artefacto de release ya comprimido (#296, #577).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
GRAPHICAL_CHECKER="${ROOT}/scripts/check_release_graphical_smoke.py"
NATIVE_GRAPHICAL_SMOKE="${ROOT}/scripts/smoke_release_graphical.py"
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
graphical_timeout="${OPENTTDRS_RELEASE_GRAPHICAL_TIMEOUT_SECONDS:-60}"
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
menu_shot="${workdir}/menu.png"
native_graphical_dir="${workdir}/native-graphical"
asset_cwd="${workdir}/outside-checkout"
asset_profile="${workdir}/isolated-profile"

python_command() {
  if command -v python3 >/dev/null 2>&1; then
    command -v python3
  elif command -v python >/dev/null 2>&1; then
    command -v python
  else
    echo "Hace falta Python para el smoke de paquete." >&2
    return 1
  fi
}

write_package_sha() {
  local output="$1"
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$archive" >"$output"
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$archive" >"$output"
  else
    local python_cmd
    python_cmd="$(python_command)" || return 1
    "$python_cmd" -c 'import hashlib, pathlib, sys; path = pathlib.Path(sys.argv[1]); print(f"{hashlib.sha256(path.read_bytes()).hexdigest()}  {path}")' "$archive" >"$output"
  fi
}

collect_artifacts() {
  [[ -n "$artifact_dir" ]] || return 0
  mkdir -p "$artifact_dir"
  write_package_sha "${artifact_dir}/package.sha256" || true
  for source in \
    "$asset_log" \
    "$network_log" \
    "$server_log" \
    "$menu_log" \
    "$menu_shot"; do
    [[ -f "$source" ]] || continue
    cp "$source" "${artifact_dir}/$(basename "$source")"
  done
  if [[ -d "$native_graphical_dir" ]]; then
    cp -a "$native_graphical_dir" "${artifact_dir}/graphical"
  fi
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

mkdir -p \
  "$asset_cwd" \
  "${asset_profile}/home" \
  "${asset_profile}/config" \
  "${asset_profile}/data" \
  "${asset_profile}/state" \
  "${asset_profile}/cache" \
  "${asset_profile}/runtime"
chmod 700 "${asset_profile}/runtime"
if [[ -e "$asset_cwd/assets" || -e "$asset_cwd/reference" ]]; then
  echo "El cwd del smoke no quedó aislado del checkout." >&2
  exit 1
fi
if ! (
  cd "$asset_cwd"
  env \
    HOME="${asset_profile}/home" \
    XDG_CONFIG_HOME="${asset_profile}/config" \
    XDG_DATA_HOME="${asset_profile}/data" \
    XDG_STATE_HOME="${asset_profile}/state" \
    XDG_CACHE_HOME="${asset_profile}/cache" \
    XDG_RUNTIME_DIR="${asset_profile}/runtime" \
    OPENTTDRS_ASSET_ROOT="$package_dir" \
    "$client" --check-assets
) >"$asset_log" 2>&1; then
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
  local run_cwd="$asset_cwd"
  local profile="$asset_profile"

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

run_native_graphical_smoke() {
  local python_cmd
  python_cmd="$(python_command)" || return 1
  if [[ ! -f "$NATIVE_GRAPHICAL_SMOKE" ]]; then
    echo "No existe el smoke gráfico nativo: $NATIVE_GRAPHICAL_SMOKE" >&2
    return 1
  fi
  local candidate_sha="${OPENTTDRS_RELEASE_CANDIDATE_SHA:-}"
  if ! [[ "$candidate_sha" =~ ^[0-9a-fA-F]{40}$ ]]; then
    echo "El smoke gráfico nativo requiere OPENTTDRS_RELEASE_CANDIDATE_SHA de 40 hexadecimales." >&2
    return 1
  fi
  mkdir -p "$native_graphical_dir"
  "$python_cmd" "$NATIVE_GRAPHICAL_SMOKE" \
    --client "$client" \
    --package-root "$package_dir" \
    --archive "$archive" \
    --candidate-sha "$candidate_sha" \
    --artifact-dir "$native_graphical_dir" \
    --timeout-seconds "$graphical_timeout"
  echo "Smoke gráfico nativo OK: menú ES/EN desde el paquete y sesión del runner."
}

run_graphical_smoke() {
  [[ "$graphical_smoke" == "1" ]] || return 0
  case "$(uname -s)" in
    MINGW*|MSYS*|CYGWIN*|Darwin*) run_native_graphical_smoke; return $? ;;
    Linux*) ;;
    *)
      echo "No hay driver gráfico nativo declarado para $(uname -s)." >&2
      return 1
      ;;
  esac
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

  if ! capture_graphical_frame OPENTTDRS_MAIN_MENU_SHOT "$menu_shot" "$menu_log" "$lavapipe_icd"; then
    echo "El paquete no pudo capturar el menú gráfico:" >&2
    tail -n 100 "$menu_log" >&2 || true
    return 1
  fi
  if ! grep -Fq "main_menu_shot: menú localizado sin escenario guiado listo" "$menu_log"; then
    echo "El menú empaquetado no confirmó la navegación actual." >&2
    tail -n 100 "$menu_log" >&2 || true
    return 1
  fi

  python3 "$GRAPHICAL_CHECKER" \
    --menu "$menu_shot" \
    --width "$SHOT_WIDTH" \
    --height "$SHOT_HEIGHT"
  echo "Smoke gráfico Linux OK: menú desde cwd/configuración aislados."
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

if ! (
  cd "$asset_cwd"
  env OPENTTDRS_ASSET_ROOT="$package_dir" "$client" --network-smoke "$address"
) >"$network_log" 2>&1; then
  echo "Log del dedicated empaquetado:" >&2
  cat "$server_log" >&2
  echo "Log del cliente empaquetado:" >&2
  cat "$network_log" >&2
  exit 1
fi
cat "$network_log"

echo "Smoke de paquete OK: assets, fuentes, audio, dedicated y handshake local."
