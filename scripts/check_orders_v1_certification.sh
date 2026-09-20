#!/usr/bin/env bash
# Verifica la evidencia versionada de Órdenes V1 sin requerir compositor (#590).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MANIFEST="${OPENTTDRS_ORDERS_V1_MANIFEST:-${ROOT}/docs/parity/screenshots/orders-v1-regression.json}"
CANDIDATE_SHA="${OPENTTDRS_ORDERS_V1_CANDIDATE_SHA:-$(python3 - "$MANIFEST" <<'PY'
import json
import re
import sys

data = json.load(open(sys.argv[1], encoding="utf-8"))
sha = data.get("certified_candidate_sha")
if not isinstance(sha, str) or not re.fullmatch(r"[0-9a-f]{40}", sha):
    raise SystemExit("certified_candidate_sha inválido en manifiesto Órdenes V1")
print(sha)
PY
)}"

python3 "$ROOT/scripts/window_visual_regression.py" \
  --manifest "$MANIFEST" \
  --mode certification \
  --candidate-sha "$CANDIDATE_SHA"
