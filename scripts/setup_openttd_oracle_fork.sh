#!/usr/bin/env bash
set -euo pipefail

# Bootstrap de "fork mínimo" para usar OpenTTD como oráculo de snapshots.
# No toca este repo: crea un clon externo con scripts auxiliares.
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

if [[ -e "$DEST" ]]; then
  echo "El destino ya existe: $DEST"
  exit 1
fi

echo "[1/4] Clonando OpenTTD upstream en $DEST"
git clone --depth=1 https://github.com/OpenTTD/OpenTTD.git "$DEST"

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
git -C "$DEST" commit -m "chore: add minimal snapshot export helper for openttdrs parity"

echo
echo "Fork mínimo listo en: $DEST"
echo "Siguiente recomendado:"
echo "  cd \"$DEST\""
echo "  ./tools/export_snapshot.sh /ruta/save.sav /tmp/openttd.snapshot.json"
echo
echo "Opcional: configurar remote a tu fork GitHub y pushear:"
echo "  git remote rename origin upstream"
echo "  git remote add origin git@github.com:<tu-usuario>/OpenTTD.git"
echo "  git push -u origin openttdrs-snapshot-oracle"
