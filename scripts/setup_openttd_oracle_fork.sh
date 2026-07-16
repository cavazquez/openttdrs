#!/usr/bin/env bash
set -euo pipefail

# Bootstrap de "fork mínimo" para usar OpenTTD como oráculo de snapshots.
# No toca este repo: crea un clon externo con scripts auxiliares.
# El commit clonado es el del manifiesto (#109).
#
# Uso:
#   scripts/setup_openttd_oracle_fork.sh /ruta/destino/openttd-oracle
#
# Luego:
#   cd /ruta/destino/openttd-oracle
#   ./tools/export_snapshot.sh /ruta/mapa.sav /tmp/openttd.snapshot.json

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

echo "[1/4] Clonando OpenTTD @ ${TAG} (${EXPECTED}) en $DEST"
mkdir -p "$DEST"
git -C "$DEST" init -q
git -C "$DEST" remote add origin "$URL"
if ! git -C "$DEST" fetch --depth 1 origin "${EXPECTED}"; then
  echo "error: no se pudo fetch ${EXPECTED} desde ${URL}" >&2
  exit 1
fi
git -C "$DEST" checkout --force --detach "${EXPECTED}"
ACTUAL="$(git -C "$DEST" rev-parse HEAD)"
if [[ "${ACTUAL}" != "${EXPECTED}" ]]; then
  echo "error: HEAD=${ACTUAL} != manifiesto ${EXPECTED}" >&2
  exit 1
fi
openttd_manifest_summary "$ROOT_DIR"

echo "[2/4] Creando rama de trabajo openttdrs-snapshot-oracle"
git -C "$DEST" checkout -b openttdrs-snapshot-oracle

echo "[3/4] Instalando script exportador en tools/export_snapshot.sh"
mkdir -p "$DEST/tools"
cat > "$DEST/tools/export_snapshot.sh" <<EOF
#!/usr/bin/env bash
set -euo pipefail

if [[ \$# -ne 2 ]]; then
  echo "Uso: \$0 <save.sav> <snapshot.json>"
  exit 2
fi

SAV="\$1"
OUT="\$2"

# Este export usa los parsers de openttdrs para producir un snapshot canónico.
# Sirve como "oráculo operativo" en un fork mínimo hasta implementar export nativo C++.
OPENTTDRS_ROOT="\${OPENTTDRS_ROOT:-$ROOT_DIR}"

python3 "\$OPENTTDRS_ROOT/scripts/parse_sav.py" "\$SAV" "\$SAV.ottdmap.tmp"
cargo run -q --manifest-path "\$OPENTTDRS_ROOT/Cargo.toml" \
  -p openttdrs-core --bin snapshot_dumper -- "\$SAV.ottdmap.tmp" "\$OUT"
rm -f "\$SAV.ottdmap.tmp"

echo "Snapshot exportado: \$OUT"
EOF
chmod +x "$DEST/tools/export_snapshot.sh"

echo "[4/4] Commit inicial en el fork local"
git -C "$DEST" add tools/export_snapshot.sh
git -C "$DEST" -c user.email=openttdrs@local -c user.name=openttdrs \
  commit -m "chore: add minimal snapshot export helper for openttdrs parity"

echo
echo "Fork mínimo listo en: $DEST"
echo "Referencia OpenTTD: ${TAG} @ ${ACTUAL}"
echo "Siguiente recomendado:"
echo "  cd \"$DEST\""
echo "  ./tools/export_snapshot.sh /ruta/save.sav /tmp/openttd.snapshot.json"
echo
echo "Opcional: configurar remote a tu fork GitHub y pushear:"
echo "  git remote rename origin upstream"
echo "  git remote add origin git@github.com:<tu-usuario>/OpenTTD.git"
echo "  git push -u origin openttdrs-snapshot-oracle"
