#!/usr/bin/env bash
# Smoke de un artefacto de release ya comprimido (#296).
set -euo pipefail

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

workdir="$(mktemp -d)"
server_pid=""
server_log="${workdir}/dedicated.log"

cleanup() {
  set +e
  if [[ -n "$server_pid" ]]; then
    kill "$server_pid" 2>/dev/null
    wait "$server_pid" 2>/dev/null
  fi
  rm -rf "$workdir"
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

"$client" --check-assets

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

if ! "$client" --network-smoke "$address"; then
  echo "Log del dedicated empaquetado:" >&2
  cat "$server_log" >&2
  exit 1
fi

echo "Smoke de paquete OK: assets, fuentes, audio, dedicated y handshake local."
