#!/usr/bin/env bash
# Regenera los reportes markdown de divergencias conocidas (Fase Rail 4).
set -euo pipefail
cd "$(dirname "$0")/.."

cargo run -q -p openttdrs-core --bin parity_runner -- \
    --scenario truck_bay --ticks 500 \
    --out /tmp/truck_bay.jsonl \
    --divergence-report docs/parity/divergences_found.md

cargo run -q -p openttdrs-core --bin parity_runner -- \
    --scenario train_line --ticks 600 \
    --out /tmp/train_line.jsonl \
    --divergence-report docs/parity/train_line_divergences.md

echo "OK: docs/parity/divergences_found.md y train_line_divergences.md"
