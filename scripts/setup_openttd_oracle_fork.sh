#!/usr/bin/env bash
set -euo pipefail

# Prepara un clon OpenTTD @ pin #109 con el export de snapshots nativo (#110).
# Ya no envuelve parse_sav.py (eso era circular).
#
# Uso:
#   scripts/setup_openttd_oracle_fork.sh /ruta/destino/openttd-oracle

if [[ $# -ne 1 ]]; then
  echo "Uso: $0 <directorio-destino-openttd>"
  exit 2
fi

DEST="$1"
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=lib/openttd_reference.sh
source "${ROOT_DIR}/scripts/lib/openttd_reference.sh"

URL="${OPENTTD_UPSTREAM_URL:-$(openttd_manifest_get "$ROOT_DIR" url)}"
EXPECTED="${OPENTTD_UPSTREAM_COMMIT:-$(openttd_manifest_get "$ROOT_DIR" commit)}"
TAG="$(openttd_manifest_get "$ROOT_DIR" tag)"

if [[ -e "$DEST" ]]; then
  echo "El destino ya existe: $DEST"
  exit 1
fi

if [[ ! "$EXPECTED" =~ ^[0-9a-fA-F]{40}$ ]]; then
  echo "error: commit del manifiesto no es un SHA-1 de 40 hex: ${EXPECTED}" >&2
  exit 1
fi

echo "[1/3] Clonando OpenTTD @ ${TAG} (${EXPECTED}) en $DEST"
mkdir -p "$DEST"
git -C "$DEST" init -q
git -C "$DEST" remote add origin "$URL"
git -C "$DEST" fetch --depth 1 origin "${EXPECTED}"
git -C "$DEST" checkout --force --detach "${EXPECTED}"
ACTUAL="$(git -C "$DEST" rev-parse HEAD)"
if [[ "${ACTUAL}" != "${EXPECTED}" ]]; then
  echo "error: HEAD=${ACTUAL} != manifiesto ${EXPECTED}" >&2
  exit 1
fi
openttd_manifest_summary "$ROOT_DIR"

echo "[2/3] Integrando export nativo openttdrs (#110)"
"${ROOT_DIR}/patches/openttd-15.3-snapshot-export/integrate.sh" "$DEST"

echo "[3/3] Rama de trabajo"
git -C "$DEST" checkout -b openttdrs-snapshot-oracle

echo
echo "Fork listo en: $DEST"
echo "Compilá dedicated y exportá (sin parse_sav):"
echo "  cmake -B \"$DEST/build\" -S \"$DEST\" -DOPTION_DEDICATED=ON"
echo "  cmake --build \"$DEST/build\" -j"
echo "  OPENTTD_BIN=\"$DEST/build/openttd\" \\"
echo "    $ROOT_DIR/scripts/export_openttd_oracle_snapshot.sh partida.sav /tmp/openttd.oracle.json"
echo
echo "Docs: docs/SNAPSHOT_ORACLE_WORKFLOW.md · docs/parity/SNAPSHOT_SCHEMA.md"
