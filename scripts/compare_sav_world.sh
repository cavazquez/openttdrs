#!/usr/bin/env bash
# Orquesta world-raw, world-semantic y world-draw para una misma partida (#304).
#
# La salida queda separada por etapa para que una divergencia tenga evidencia
# estable y no se pierda al avanzar hacia el renderer. El pipeline se detiene
# en la primera frontera que difiere: raw -> semántica -> plan de dibujo.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

usage() {
  cat <<'EOF'
Uso:
  scripts/compare_sav_world.sh <partida.sav> <directorio-salida> [opciones]

Opciones:
  --openttd-bin RUTA  Binario OpenTTD 15.3 parcheado (también OPENTTD_BIN).
  --tile X,Y          Tesela de interés; también se pasa al comparador semántico/draw.
  --radius N          Radio de región alrededor de --tile (default: 0).
  --region A,B,C,D    Región inclusiva x0,y0,x1,y1; incompatible con --tile.
  --kind TIPO         raw, semantic, draw o all (default: all; acepta lista con comas).
  --max-diffs N       Máximo de diferencias detalladas por etapa (default: 20).
  --dry-run           Valida opciones e imprime el plan sin ejecutar exportadores.
  -h, --help          Muestra esta ayuda.

Artefactos:
  <salida>/raw/{openttd,openttdrs}.jsonl + report.json + run.log
  <salida>/semantic/{openttd,openttdrs}.jsonl + report.json + run.log
  <salida>/draw/{openttd,openttdrs}.jsonl + report.json + run.log

El exit code 1 significa una divergencia de paridad ya reportada; 2 o mayor
significa que no se pudo producir alguno de los artefactos.
EOF
}

fail() {
  echo "error: $*" >&2
  exit 2
}

if [[ $# -eq 1 && ( "$1" == "-h" || "$1" == "--help" ) ]]; then
  usage
  exit 0
fi
if [[ $# -lt 2 ]]; then
  usage >&2
  exit 2
fi

SAV="$(realpath "$1")"
OUT_DIR="$(realpath -m "$2")"
shift 2

[[ -f "$SAV" ]] || fail "no existe la partida $SAV"

OPENTTD_BIN="${OPENTTD_BIN:-${ROOT}/reference/openttd-upstream/build/openttd}"
TILE=""
RADIUS=0
REGION=""
MAX_DIFFS=20
DRY_RUN=0
declare -A REQUESTED=([raw]=1 [semantic]=1 [draw]=1)

request_kind() {
  local group="$1" kind
  IFS=',' read -r -a kinds <<<"$group"
  for kind in "${kinds[@]}"; do
    case "$kind" in
      all)
        REQUESTED=([raw]=1 [semantic]=1 [draw]=1)
        ;;
      raw|semantic|draw)
        REQUESTED["$kind"]=1
        ;;
      *)
        fail "--kind admite raw, semantic, draw o all; se recibió '$kind'"
        ;;
    esac
  done
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --openttd-bin)
      [[ $# -ge 2 ]] || fail "--openttd-bin requiere una ruta"
      OPENTTD_BIN="$2"
      shift 2
      ;;
    --tile)
      [[ $# -ge 2 ]] || fail "--tile requiere X,Y"
      TILE="$2"
      shift 2
      ;;
    --radius)
      [[ $# -ge 2 ]] || fail "--radius requiere N"
      RADIUS="$2"
      shift 2
      ;;
    --region)
      [[ $# -ge 2 ]] || fail "--region requiere A,B,C,D"
      REGION="$2"
      shift 2
      ;;
    --kind)
      [[ $# -ge 2 ]] || fail "--kind requiere un tipo"
      REQUESTED=()
      request_kind "$2"
      shift 2
      ;;
    --max-diffs)
      [[ $# -ge 2 ]] || fail "--max-diffs requiere N"
      MAX_DIFFS="$2"
      shift 2
      ;;
    --dry-run)
      DRY_RUN=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      fail "opción desconocida '$1'"
      ;;
  esac
done

[[ "$RADIUS" =~ ^[0-9]+$ ]] || fail "--radius debe ser un entero no negativo"
[[ "$MAX_DIFFS" =~ ^[1-9][0-9]*$ ]] || fail "--max-diffs debe ser un entero positivo"
if [[ -n "$TILE" && -n "$REGION" ]]; then
  fail "--tile y --region son incompatibles"
fi

if [[ -n "$TILE" ]]; then
  if [[ ! "$TILE" =~ ^([0-9]+),([0-9]+)$ ]]; then
    fail "--tile debe tener formato X,Y con coordenadas no negativas"
  fi
  TILE_X="${BASH_REMATCH[1]}"
  TILE_Y="${BASH_REMATCH[2]}"
  MIN_X=$(( TILE_X > RADIUS ? TILE_X - RADIUS : 0 ))
  MIN_Y=$(( TILE_Y > RADIUS ? TILE_Y - RADIUS : 0 ))
  REGION="${MIN_X},${MIN_Y},$((TILE_X + RADIUS)),$((TILE_Y + RADIUS))"
elif [[ -n "$REGION" ]]; then
  if [[ ! "$REGION" =~ ^([0-9]+),([0-9]+),([0-9]+),([0-9]+)$ ]]; then
    fail "--region debe tener formato A,B,C,D con coordenadas no negativas"
  fi
  if (( BASH_REMATCH[1] > BASH_REMATCH[3] || BASH_REMATCH[2] > BASH_REMATCH[4] )); then
    fail "--region debe cumplir A<=C y B<=D"
  fi
fi

STAGES=()
for stage in raw semantic draw; do
  [[ -n "${REQUESTED[$stage]+x}" ]] && STAGES+=("$stage")
done
[[ ${#STAGES[@]} -gt 0 ]] || fail "--kind no seleccionó ninguna etapa"

if (( DRY_RUN )); then
  printf 'save=%s\noutput=%s\nopenttd_bin=%s\nregion=%s\ntile=%s\nstages=%s\nmax_diffs=%s\n' \
    "$SAV" "$OUT_DIR" "$OPENTTD_BIN" "${REGION:-full}" "${TILE:-none}" \
    "$(IFS=,; echo "${STAGES[*]}")" "$MAX_DIFFS"
  exit 0
fi

RAW_REFERENCE_EXPORT="${OPENTTDRS_WORLD_ORACLE_RAW_REFERENCE_EXPORT:-${ROOT}/scripts/export_openttd_world_raw.sh}"
RAW_CANDIDATE_EXPORT="${OPENTTDRS_WORLD_ORACLE_RAW_CANDIDATE_EXPORT:-${ROOT}/scripts/export_openttdrs_world_raw.sh}"
SEMANTIC_REFERENCE_EXPORT="${OPENTTDRS_WORLD_ORACLE_SEMANTIC_REFERENCE_EXPORT:-${ROOT}/scripts/export_openttd_world_semantic.sh}"
SEMANTIC_CANDIDATE_EXPORT="${OPENTTDRS_WORLD_ORACLE_SEMANTIC_CANDIDATE_EXPORT:-${ROOT}/scripts/export_openttdrs_world_semantic.sh}"
DRAW_REFERENCE_EXPORT="${OPENTTDRS_WORLD_ORACLE_DRAW_REFERENCE_EXPORT:-${ROOT}/scripts/export_openttd_world_draw.sh}"
DRAW_CANDIDATE_EXPORT="${OPENTTDRS_WORLD_ORACLE_DRAW_CANDIDATE_EXPORT:-${ROOT}/scripts/export_openttdrs_world_draw.sh}"

run_logged() {
  local log="$1"
  shift
  local rc
  if "$@" 2>&1 | tee "$log"; then
    return 0
  else
    rc="${PIPESTATUS[0]}"
    return "$rc"
  fi
}

export_reference() {
  local exporter="$1" reference="$2" log="$3"
  if [[ -n "$REGION" ]]; then
    run_logged "$log" "$exporter" "$SAV" "$reference" "$OPENTTD_BIN" "$REGION"
  else
    run_logged "$log" "$exporter" "$SAV" "$reference" "$OPENTTD_BIN"
  fi
}

export_candidate() {
  local exporter="$1" candidate="$2" log="$3"
  if [[ -n "$REGION" ]]; then
    run_logged "$log" "$exporter" "$SAV" "$candidate" "$REGION"
  else
    run_logged "$log" "$exporter" "$SAV" "$candidate"
  fi
}

run_stage() {
  local stage="$1" reference_export="$2" candidate_export="$3" comparator="$4"
  local dir="${OUT_DIR}/${stage}"
  local reference="${dir}/openttd.jsonl"
  local candidate="${dir}/openttdrs.jsonl"
  local report="${dir}/report.json"
  mkdir -p "$dir"

  echo "== ${stage}: exportando OpenTTD =="
  export_reference "$reference_export" "$reference" "${dir}/openttd-export.log" || return $?
  echo "== ${stage}: exportando openttdrs =="
  export_candidate "$candidate_export" "$candidate" "${dir}/openttdrs-export.log" || return $?
  echo "== ${stage}: comparando =="

  local -a compare_args=(python3 "$comparator" "$reference" "$candidate" --max-diffs "$MAX_DIFFS" --json-report "$report")
  case "$stage" in
    semantic)
      compare_args+=(--show-inventory)
      [[ -n "$TILE" ]] && compare_args+=(--where "$TILE")
      ;;
    draw)
      compare_args+=(--geometry --foundations --order --strict-reference --by-role)
      [[ -n "$TILE" ]] && compare_args+=(--where "$TILE")
      ;;
  esac
  run_logged "${dir}/compare.log" "${compare_args[@]}"
}

for stage in "${STAGES[@]}"; do
  case "$stage" in
    raw)
      reference_export="$RAW_REFERENCE_EXPORT"
      candidate_export="$RAW_CANDIDATE_EXPORT"
      comparator="${ROOT}/scripts/compare_world_raw.py"
      ;;
    semantic)
      reference_export="$SEMANTIC_REFERENCE_EXPORT"
      candidate_export="$SEMANTIC_CANDIDATE_EXPORT"
      comparator="${ROOT}/scripts/compare_world_semantic.py"
      ;;
    draw)
      reference_export="$DRAW_REFERENCE_EXPORT"
      candidate_export="$DRAW_CANDIDATE_EXPORT"
      comparator="${ROOT}/scripts/compare_world_draw.py"
      ;;
  esac
  if run_stage "$stage" "$reference_export" "$candidate_export" "$comparator"; then
    continue
  else
    rc=$?
    if (( rc == 1 )); then
      echo "PARITY DIFF: primera frontera divergente=${stage}; ver ${OUT_DIR}/${stage}/report.json" >&2
    else
      echo "ERROR: no se pudo completar frontera=${stage}; ver ${OUT_DIR}/${stage}/*.log" >&2
    fi
    exit "$rc"
  fi
done

echo "OK: world-raw, world-semantic y world-draw equivalentes; artefactos en ${OUT_DIR}"
