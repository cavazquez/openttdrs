#!/usr/bin/env bash
# Genera los artefactos de las familias visuales versionadas (#297, #299 y #300).
#
# Requiere el driver versionado de patches/openttd-15.3-ui-capture y un display
# funcional. No acepta capturas parciales: verifica cada PNG y deja al gate
# crear el diff + sidecar únicamente cuando referencia y candidato son reales.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MANIFEST="${OPENTTDRS_WINDOW_VISUAL_MANIFEST:-${ROOT}/docs/parity/screenshots/window-regression.json}"
OPENTTD_BIN="${OPENTTDRS_UI_CAPTURE_BIN:-/tmp/openttdrs-openttd-15.3-ui/openttd}"
FIXTURE="${OPENTTDRS_UI_CAPTURE_FIXTURE:-${ROOT}/crates/openttdrs-core/tests/fixtures/mvp_openttd_rich.sav}"
DEPOT_INDUSTRY_FIXTURE="${OPENTTDRS_UI_CAPTURE_DEPOT_INDUSTRY_FIXTURE:-${ROOT}/crates/openttdrs-core/tests/fixtures/rail_signals_mixed.sav}"
BASESET="${OPENTTDRS_OPENGFX_DIR:-${ROOT}/.deps/openttd-baseset/opengfx-8.0}"
WINDOW_IDS=(
  Vehicle Orders Timetable Depot Town Industry
  RailStationPicker AirportPicker RoadStopPicker ObjectPicker BridgePicker
  DockPicker BuoyPicker RailWaypointPicker RoadWaypointPicker TreePicker
  TerraformPicker SignPicker DepotBuildPicker SignalPicker
  Finances CompanyView GraphIncome GraphOperatingProfit GraphCompanyValue
  CargoPaymentRates SubsidyList League NewsHistory NewsSettings
)
SELECTED="${OPENTTDRS_WINDOW_CAPTURE_IDS:-Vehicle,Orders,Timetable,Depot,Town,Industry,RailStationPicker,AirportPicker,RoadStopPicker,ObjectPicker,BridgePicker,DockPicker,BuoyPicker,RailWaypointPicker,RoadWaypointPicker,TreePicker,TerraformPicker,SignPicker,DepotBuildPicker,SignalPicker,Finances,CompanyView,GraphIncome,GraphOperatingProfit,GraphCompanyValue,CargoPaymentRates,SubsidyList,League,NewsHistory,NewsSettings}"

if [[ ! -x "$OPENTTD_BIN" ]]; then
  echo "error: no existe OpenTTD UI parcheado: $OPENTTD_BIN" >&2
  echo "  patches/openttd-15.3-ui-capture/integrate.sh && cmake --build ... --target openttd" >&2
  exit 1
fi
if [[ ! -f "$FIXTURE" ]]; then
  echo "error: no existe fixture: $FIXTURE" >&2
  exit 1
fi
if [[ ! -f "$DEPOT_INDUSTRY_FIXTURE" ]]; then
  echo "error: no existe fixture Depot/Industry: $DEPOT_INDUSTRY_FIXTURE" >&2
  exit 1
fi
if [[ ! -f "$BASESET/opengfx.obg" ]]; then
  echo "error: falta OpenGFX 8.0: $BASESET" >&2
  exit 1
fi
if ! command -v xvfb-run >/dev/null || ! command -v weston >/dev/null; then
  echo "error: #297 requiere xvfb-run y weston para capturas de dimensión exacta" >&2
  echo "  instala: sudo apt install xvfb xauth weston" >&2
  exit 1
fi

WORK="$(mktemp -d /tmp/openttdrs-ui-capture.XXXXXX)"
cleanup() { rm -rf "$WORK"; }
trap cleanup EXIT
mkdir -p "$(dirname "$OPENTTD_BIN")/baseset"
cp -a "$BASESET/." "$(dirname "$OPENTTD_BIN")/baseset/"

contains_id() {
  local wanted="$1"
  [[ ",$SELECTED," == *",$wanted,"* ]]
}

artifact_dir() {
  local id="$1" width="$2" height="$3" scale="$4"
  printf '%s/docs/parity/screenshots/window-regression/%s/%sx%s-%sx' "$ROOT" "$id" "$width" "$height" "$scale"
}

fixture_for_id() {
  local id="$1"
  case "$id" in
    Depot|Industry) printf '%s\n' "$DEPOT_INDUSTRY_FIXTURE" ;;
    *) printf '%s\n' "$FIXTURE" ;;
  esac
}

capture_reference() {
  local id="$1" width="$2" height="$3" scale="$4" out="$5"
  local run="$WORK/reference-${id}-${width}x${height}-${scale}x"
  local name="openttdrs-${id}-${width}x${height}-${scale}x"
  local fixture
  fixture="$(fixture_for_id "$id")"
  local config_template="${ROOT}/patches/openttd-15.3-ui-capture/configs/gui-scale-${scale}.cfg"
  if [[ ! -f "$config_template" ]]; then
    echo "error: no existe plantilla de escala UI: $config_template" >&2
    return 1
  fi
  mkdir -p "$run/data" "$run/config"
  cp "$config_template" "$run/openttd.cfg"
  # OpenTTD escribe screenshot/ relativo al directorio de trabajo, no al XDG
  # aislado. Por eso el proceso debe arrancar dentro de $run.
  (
    cd "$run"
    xvfb-run -a -s "-screen 0 ${width}x${height}x24" env -u WAYLAND_DISPLAY -u WAYLAND_SOCKET \
      XDG_SESSION_TYPE=x11 \
      XDG_DATA_HOME="$run/data" \
      XDG_CONFIG_HOME="$run/config" \
      OPENTTDRS_UI_CAPTURE_ID="$id" \
      OPENTTDRS_UI_CAPTURE_NAME="$name" \
      timeout 45s "$OPENTTD_BIN" -X -x -I OpenGFX -v sdl -s null -m null \
        -r "${width}x${height}" -c "$run/openttd.cfg" -g "$fixture" \
        >"$run/openttd.log" 2>&1
  ) || true
  local generated
  generated="$(find "$run" -type f -name "${name}.png" -print -quit)"
  if [[ -z "$generated" ]]; then
    echo "error: OpenTTD no produjo referencia $id ${width}x${height}@${scale}x" >&2
    tail -n 60 "$run/openttd.log" >&2 || true
    return 1
  fi
  mkdir -p "$(dirname "$out")"
  cp "$generated" "$out"
}

capture_candidate() {
  local id="$1" width="$2" height="$3" scale="$4" out="$5"
  local runtime="$WORK/candidate-${id}-${width}x${height}-${scale}x-runtime"
  # `sun_path` admite como máximo 108 bytes. El runtime temporal ya consume
  # buena parte de ese presupuesto, y los IDs de construction son largos.
  # Cada captura tiene su propio runtime, por lo que no hace falta incluir ID.
  local socket="shot-${width}x${height}-${scale}"
  local fixture
  fixture="$(fixture_for_id "$id")"
  local weston_pid status=0
  mkdir -p "$(dirname "$out")"
  mkdir -p "$runtime"
  chmod 700 "$runtime"
  XDG_RUNTIME_DIR="$runtime" weston --backend=headless --socket="$socket" \
    --width="$width" --height="$height" --renderer=gl --log="$runtime/weston.log" \
    >"$runtime/weston.stderr" 2>&1 &
  weston_pid=$!
  for _ in $(seq 1 100); do
    [[ -S "$runtime/$socket" ]] && break
    sleep 0.1
  done
  if [[ ! -S "$runtime/$socket" ]]; then
    echo "error: Weston no inició para $id ${width}x${height}@${scale}x" >&2
    tail -n 60 "$runtime/weston.log" "$runtime/weston.stderr" >&2 || true
    kill "$weston_pid" 2>/dev/null || true
    wait "$weston_pid" 2>/dev/null || true
    return 1
  fi
  # El enlace dinámico de Bevy es el modo local recomendado por el proyecto.
  # Weston GL (llvmpipe) expone una superficie Wayland presentable a wgpu.
  if ! env -u DISPLAY \
    XDG_RUNTIME_DIR="$runtime" \
    WAYLAND_DISPLAY="$socket" \
    XDG_SESSION_TYPE=wayland \
    OPENTTDRS_SAV_LOAD="$fixture" \
    OPENTTDRS_WINDOW_SHOT_ID="$id" \
    OPENTTDRS_SHOT_RES="${width}x${height}" \
    OPENTTDRS_SHOT_UI_SCALE="$scale" \
    OPENTTDRS_WINDOWS_SHOT="$out" \
    cargo run -p openttdrs-client --features dynamic_linking; then
    status=1
  fi
  kill "$weston_pid" 2>/dev/null || true
  wait "$weston_pid" 2>/dev/null || true
  if [[ $status -ne 0 || ! -s "$out" ]]; then
    echo "error: openttdrs no produjo candidato $id ${width}x${height}@${scale}x" >&2
    return 1
  fi
}

gate_args=()
for id in "${WINDOW_IDS[@]}"; do
  contains_id "$id" || continue
  gate_args+=(--window "$id")
  for profile in '1280 720 1' '1280 720 2' '1920 1080 1' '1920 1080 2'; do
    read -r width height scale <<<"$profile"
    directory="$(artifact_dir "$id" "$width" "$height" "$scale")"
    echo "→ $id ${width}x${height}@${scale}x"
    capture_reference "$id" "$width" "$height" "$scale" "$directory/reference.png"
    capture_candidate "$id" "$width" "$height" "$scale" "$directory/candidate.png"
  done
done

if [[ ${#gate_args[@]} -eq 0 ]]; then
  echo "error: OPENTTDRS_WINDOW_CAPTURE_IDS no selecciona una ventana versionada (#297/#299/#300)" >&2
  exit 1
fi
python3 "$ROOT/scripts/window_visual_regression.py" --manifest "$MANIFEST" --write-sidecars "${gate_args[@]}"
python3 "$ROOT/scripts/window_visual_regression.py" --manifest "$MANIFEST" "${gate_args[@]}"
