#!/usr/bin/env bash
# Clona o actualiza la referencia OpenTTD al commit fijado en
# docs/parity/openttd-reference.json (#109).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=lib/openttd_reference.sh
source "${ROOT}/scripts/lib/openttd_reference.sh"

DEST="${ROOT}/reference/openttd-upstream"
URL="${OPENTTD_UPSTREAM_URL:-$(openttd_manifest_get "$ROOT" url)}"
EXPECTED="${OPENTTD_UPSTREAM_COMMIT:-$(openttd_manifest_get "$ROOT" commit)}"
TAG="$(openttd_manifest_get "$ROOT" tag)"

if [[ ! "$EXPECTED" =~ ^[0-9a-fA-F]{40}$ ]]; then
  echo "error: commit del manifiesto no es un SHA-1 de 40 hex: ${EXPECTED}" >&2
  exit 1
fi

mkdir -p "${ROOT}/reference"

checkout_pinned() {
  local dest="$1"
  echo "Fetch explícito del commit ${EXPECTED} (tag ${TAG})..."
  # GitHub permite fetch por SHA con depth 1; evita clonar todo el historial.
  if ! git -C "${dest}" fetch --depth 1 origin "${EXPECTED}"; then
    echo "error: no se pudo fetch ${EXPECTED} desde ${URL}" >&2
    echo "  Probá sin shallow: git -C ${dest} fetch origin ${EXPECTED}" >&2
    exit 1
  fi
  git -C "${dest}" checkout --force --detach "${EXPECTED}"
}

if [[ -d "${DEST}/.git" ]]; then
  echo "Actualizando ${DEST} al commit fijado..."
  git -C "${DEST}" remote set-url origin "${URL}" 2>/dev/null \
    || git -C "${DEST}" remote add origin "${URL}"
  checkout_pinned "${DEST}"
else
  echo "Clonando OpenTTD en ${DEST} (solo commit fijado)..."
  mkdir -p "${DEST}"
  git -C "${DEST}" init -q
  git -C "${DEST}" remote add origin "${URL}"
  checkout_pinned "${DEST}"
fi

ACTUAL="$(git -C "${DEST}" rev-parse HEAD)"
if [[ "${ACTUAL}" != "${EXPECTED}" ]]; then
  echo "error: HEAD=${ACTUAL} no coincide con el manifiesto ${EXPECTED}" >&2
  exit 1
fi

openttd_manifest_summary "${ROOT}"
echo "Listo en ${DEST} (detached HEAD @ ${ACTUAL})."
echo "Para cambiar la referencia: editá docs/parity/openttd-reference.json y abrí un PR (ver docs/parity/OPENTTD_REFERENCE.md)."
