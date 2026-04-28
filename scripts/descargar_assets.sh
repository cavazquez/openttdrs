#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

usage() {
  cat <<'EOF'
Uso:
  ./scripts/descargar_assets.sh <componente> [opciones]

Componentes:
  graficos   Descarga/procesa gráficos (requiere --8bpp o --32bpp)
  sonidos    Descarga/procesa OpenSFX
  musica     Descarga/procesa OpenMSX
  todo       Ejecuta graficos + sonidos + musica

Ejemplos:
  ./scripts/descargar_assets.sh graficos --32bpp
  ./scripts/descargar_assets.sh graficos --8bpp
  ./scripts/descargar_assets.sh sonidos
  ./scripts/descargar_assets.sh musica
  ./scripts/descargar_assets.sh todo --32bpp

Notas:
  - Para 'graficos', el modo es obligatorio: --8bpp o --32bpp.
  - En 'todo', el modo de gráficos también es obligatorio.
EOF
}

if [[ $# -lt 1 ]]; then
  usage
  exit 1
fi

component="${1}"
shift

run_graficos() {
  if [[ $# -ne 1 ]]; then
    echo "Error: para 'graficos' debés indicar --8bpp o --32bpp." >&2
    exit 1
  fi
  case "${1}" in
    --8bpp|--32bpp)
      "${ROOT}/scripts/descargar_graficos.sh" "${1}"
      ;;
    *)
      echo "Error: modo inválido '${1}'. Usá --8bpp o --32bpp." >&2
      exit 1
      ;;
  esac
}

case "${component}" in
  graficos)
    run_graficos "$@"
    ;;
  sonidos)
    if [[ $# -ne 0 ]]; then
      echo "Error: 'sonidos' no recibe opciones extra." >&2
      exit 1
    fi
    "${ROOT}/scripts/descargar_sonidos.sh" --opensfx
    ;;
  musica)
    if [[ $# -ne 0 ]]; then
      echo "Error: 'musica' no recibe opciones extra." >&2
      exit 1
    fi
    "${ROOT}/scripts/descargar_musica.sh" --openmsx
    ;;
  todo)
    run_graficos "$@"
    "${ROOT}/scripts/descargar_sonidos.sh" --opensfx
    "${ROOT}/scripts/descargar_musica.sh" --openmsx
    ;;
  -h|--help)
    usage
    ;;
  *)
    echo "Error: componente desconocido '${component}'." >&2
    usage
    exit 1
    ;;
esac
