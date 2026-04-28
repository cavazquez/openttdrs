#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

echo "== Entorno =="
echo "OS: $(lsb_release -ds 2>/dev/null || uname -srv)"
echo "Kernel: $(uname -r)"
echo "Python: $(python3 --version 2>&1 || true)"
echo "Pillow: $(python3 - <<'PY'
try:
    import PIL
    print(PIL.__version__)
except Exception:
    print("NO_INSTALADO")
PY
)"
if command -v grfcodec >/dev/null 2>&1; then
  grfcodec_info="$(grfcodec 2>&1 | rg "GRFCodec.*" | head -n 1 || true)"
  echo "grfcodec: ${grfcodec_info:-INSTALADO}"
else
  echo "grfcodec: NO_INSTALADO"
fi
echo

echo "== Verificando assets base =="
required=(
  "assets/opengfx/tiles/grass.png"
  "assets/opengfx/tiles/terrain_rough_slope_12.png"
  "assets/opengfx/tiles/rail_1014.png"
)
for f in "${required[@]}"; do
  if [[ -f "${f}" ]]; then
    echo "OK  ${f}"
  else
    echo "MISS ${f}"
  fi
done
echo

echo "== Análisis de magenta residual =="
python3 - <<'PY'
from pathlib import Path
from PIL import Image

files = [
    Path("assets/opengfx/tiles/grass.png"),
    Path("assets/opengfx/tiles/terrain_rough_slope_12.png"),
    Path("assets/opengfx/tiles/rail_1014.png"),
]

def is_magenta(r, g, b):
    return r >= 220 and b >= 220 and g <= 40 and abs(r - b) <= 24

for p in files:
    if not p.exists():
        print(f"{p}: NO_EXISTE")
        continue
    im = Image.open(p).convert("RGBA")
    total = im.width * im.height
    magenta_opaque = 0
    for r, g, b, a in im.getdata():
        if a > 0 and is_magenta(r, g, b):
            magenta_opaque += 1
    pct = (magenta_opaque / total * 100.0) if total else 0.0
    print(f"{p}: magenta_opaque={magenta_opaque}/{total} ({pct:.4f}%)")
PY
echo

echo "Sugerencia: si hay magenta_opaque > 0, volver a ejecutar ./scripts/descargar_graficos.sh"
