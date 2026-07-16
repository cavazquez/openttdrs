#!/usr/bin/env bash
# Regenera los reportes markdown de divergencias conocidas (Fase Rail 4 / #125).
set -euo pipefail
cd "$(dirname "$0")/.."

COMMIT="$(git rev-parse --short HEAD 2>/dev/null || echo unknown)"
PIN_TAG="$(python3 -c "import json; print(json.load(open('docs/parity/openttd-reference.json'))['tag'])" 2>/dev/null || echo '?')"
PIN_SHA="$(python3 -c "import json; print(json.load(open('docs/parity/openttd-reference.json'))['commit'][:12])" 2>/dev/null || echo '?')"

cargo run -q -p openttdrs-core --bin parity_runner -- \
    --scenario truck_bay --ticks 500 \
    --out /tmp/truck_bay.jsonl \
    --divergence-report docs/parity/divergences_found.md

cargo run -q -p openttdrs-core --bin parity_runner -- \
    --scenario train_line --ticks 600 \
    --out /tmp/train_line.jsonl \
    --divergence-report docs/parity/train_line_divergences.md

# Anexa commit/pin al bloque de metadatos (el runner ya escribe constantes).
for f in docs/parity/divergences_found.md docs/parity/train_line_divergences.md; do
  if ! grep -q 'openttdrs commit:' "$f"; then
    # Inserta tras la línea del pin OpenTTD del bloque de metadatos.
    python3 - "$f" "$COMMIT" "$PIN_TAG" "$PIN_SHA" <<'PY'
import pathlib, sys
path = pathlib.Path(sys.argv[1])
commit, tag, sha = sys.argv[2], sys.argv[3], sys.argv[4]
text = path.read_text(encoding="utf-8")
needle = "- Pin OpenTTD:"
extra = (
    f"- Pin OpenTTD: [`openttd-reference.json`](openttd-reference.json)"
    f" (tag **{tag}**, `{sha}`)\n"
    f"- openttdrs commit: `{commit}`\n"
)
# Sustituye la línea de pin genérica del runner por una con tag/SHA + commit.
import re
text2, n = re.subn(
    r"- Pin OpenTTD:.*\n",
    extra,
    text,
    count=1,
)
if n == 0:
    text2 = text.replace(
        "Estas divergencias son conocidas",
        f"openttdrs `{commit}` · OpenTTD {tag} (`{sha}`).\n\nEstas divergencias son conocidas",
        1,
    )
path.write_text(text2, encoding="utf-8")
PY
  fi
done

./scripts/check_parity_docs_fresh.sh

echo "OK: docs/parity/divergences_found.md y train_line_divergences.md (commit $COMMIT, OpenTTD $PIN_TAG)"
