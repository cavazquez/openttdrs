#!/usr/bin/env bash
# Captura los ocho perfiles runtime de certificación V1 para Órdenes (#590).
#
# Uso:
#   bash scripts/capture_orders_v1_certification.sh baseline
#   bash scripts/capture_orders_v1_certification.sh candidate
#
# Cada invocación arranca la demo vial integrada (sin cargar saves personales),
# abre el camión demo mediante el driver de ventana y registra ES/EN × las
# cuatro combinaciones resolución/escala. `baseline` escribe reference.png y
# `candidate` candidate.png; el gate separado genera diff/sidecar.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ARTIFACT_ROOT="${OPENTTDRS_ORDERS_V1_ARTIFACT_ROOT:-${ROOT}/docs/parity/screenshots/orders-v1}"

case "${1:-}" in
  baseline) ARTIFACT_NAME="reference.png" ;;
  candidate) ARTIFACT_NAME="candidate.png" ;;
  *)
    echo "uso: $0 {baseline|candidate}" >&2
    exit 2
    ;;
esac

for dependency in cargo weston; do
  if ! command -v "$dependency" >/dev/null 2>&1; then
    echo "error: falta $dependency para capturar Órdenes V1" >&2
    exit 1
  fi
done

capture_profile() {
  local language="$1" width="$2" height="$3" scale="$4"
  local window_id runtime_dir weston_pid output
  case "$language" in
    es) window_id="OrdersEs" ;;
    en) window_id="OrdersEn" ;;
    *)
      echo "error: idioma no soportado: $language" >&2
      return 2
      ;;
  esac
  output="${ARTIFACT_ROOT}/${window_id}/${width}x${height}-${scale}x/${ARTIFACT_NAME}"
  runtime_dir="$(mktemp -d /tmp/openttdrs-orders-v1.XXXXXX)"
  weston_pid=""
  cleanup_capture_profile() {
    if [[ -n "$weston_pid" ]]; then
      kill "$weston_pid" 2>/dev/null || true
      wait "$weston_pid" 2>/dev/null || true
    fi
    rm -rf "$runtime_dir"
  }
  trap cleanup_capture_profile RETURN

  mkdir -p "$(dirname "$output")" "$runtime_dir/config" "$runtime_dir/data"
  chmod 700 "$runtime_dir"
  XDG_RUNTIME_DIR="$runtime_dir" weston \
    --backend=headless \
    --socket=orders-v1 \
    --width="$width" \
    --height="$height" \
    --renderer=gl \
    --log="$runtime_dir/weston.log" \
    >"$runtime_dir/weston.stderr" 2>&1 &
  weston_pid=$!
  for _ in $(seq 1 100); do
    [[ -S "$runtime_dir/orders-v1" ]] && break
    sleep 0.1
  done
  if [[ ! -S "$runtime_dir/orders-v1" ]]; then
    tail -n 60 "$runtime_dir/weston.log" "$runtime_dir/weston.stderr" >&2 || true
    return 1
  fi

  echo "→ Órdenes V1 ${language} ${width}x${height}@${scale}x (${ARTIFACT_NAME})"
  env -u DISPLAY \
    XDG_RUNTIME_DIR="$runtime_dir" \
    XDG_CONFIG_HOME="$runtime_dir/config" \
    XDG_DATA_HOME="$runtime_dir/data" \
    WAYLAND_DISPLAY=orders-v1 \
    XDG_SESSION_TYPE=wayland \
    OPENTTDRS_LANGUAGE="$language" \
    OPENTTDRS_WINDOW_SHOT_ID=Orders \
    OPENTTDRS_SHOT_RES="${width}x${height}" \
    OPENTTDRS_SHOT_UI_SCALE="$scale" \
    OPENTTDRS_WINDOWS_SHOT="$output" \
    cargo run -p openttdrs-client --features dynamic_linking
  if [[ ! -s "$output" ]]; then
    echo "error: captura ausente: $output" >&2
    return 1
  fi
}

for language in es en; do
  for profile in "1280 720 1" "1280 720 2" "1920 1080 1" "1920 1080 2"; do
    read -r width height scale <<<"$profile"
    capture_profile "$language" "$width" "$height" "$scale"
  done
done
