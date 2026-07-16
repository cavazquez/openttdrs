#!/usr/bin/env bash
# bench_baseline.sh — cinco corridas Criterion + resumen de variabilidad (#116).
#
# Uso (raíz del repo):
#   ./scripts/bench_baseline.sh
#   BENCH_FILTER=sim_tick ./scripts/bench_baseline.sh   # solo un [[bench]]
#
# Escribe benches/baselines/latest.md (informativo; no es golden de CI).

set -euo pipefail

cd "$(dirname "$0")/.."

OUT_DIR="crates/openttdrs-core/benches/baselines"
mkdir -p "$OUT_DIR"
STAMP="$(date -u +%Y%m%dT%H%M%SZ)"
RAW_DIR="$OUT_DIR/raw-$STAMP"
mkdir -p "$RAW_DIR"

RUNS="${BENCH_RUNS:-5}"
FILTER="${BENCH_FILTER:-}"
WARM="${BENCH_WARM_UP:-0.5}"
MEAS="${BENCH_MEASUREMENT:-1.5}"
SAMPLES="${BENCH_SAMPLE_SIZE:-40}"

COMMIT="$(git rev-parse --short HEAD 2>/dev/null || echo unknown)"
HOST="$(uname -srmo 2>/dev/null || uname -a)"
CPU="$(grep -m1 'model name' /proc/cpuinfo 2>/dev/null | cut -d: -f2- | sed 's/^ //' || echo unknown)"
NPROC="$(nproc 2>/dev/null || echo '?')"

info() { echo "[bench] $*"; }

CRITERION_ARGS=(--warm-up-time "$WARM" --measurement-time "$MEAS" --sample-size "$SAMPLES")

# Targets Criterion explícitos: `cargo bench` también intenta bins/tests con el harness default.
BENCH_TARGETS=(sim_tick pathfinding)
if [[ -n "$FILTER" ]]; then
    BENCH_TARGETS=("$FILTER")
fi

for i in $(seq 1 "$RUNS"); do
    info "corrida $i/$RUNS"
    log="$RAW_DIR/run_$i.log"
    : >"$log"
    for bench in "${BENCH_TARGETS[@]}"; do
        cargo bench -p openttdrs-core --bench "$bench" -- "${CRITERION_ARGS[@]}" >>"$log" 2>&1
    done
done

REPORT="$OUT_DIR/latest.md"
{
    echo "# Baseline headless (#116)"
    echo
    echo "- Generado (UTC): \`$STAMP\`"
    echo "- Commit: \`$COMMIT\`"
    echo "- Host: \`$HOST\`"
    echo "- CPU: \`$CPU\` (${NPROC} hilos)"
    echo "- Corridas: $RUNS"
    echo "- Criterion: warm-up=${WARM}s measurement=${MEAS}s sample-size=${SAMPLES}"
    if [[ -n "$FILTER" ]]; then
        echo "- Filtro bench: \`$FILTER\`"
    fi
    echo
    echo "## Tiempos medios por benchmark (ns/iter o según Criterion)"
    echo
    echo "| Benchmark | mean (ns) | CV % | n |"
    echo "|-----------|----------:|-----:|--:|"
} >"$REPORT"

# Extrae líneas tipo:  sim_tick/truck_bay/100
#                      time:   [1.2345 ms 1.2356 ms 1.2367 ms]
python3 - "$RAW_DIR" "$REPORT" "$RUNS" <<'PY'
import re, sys, statistics
from pathlib import Path

raw_dir = Path(sys.argv[1])
report = Path(sys.argv[2])
runs = int(sys.argv[3])

# Criterion: "id  time: [lo mid hi]" o id en línea previa + "time: [...]".
bench_only_re = re.compile(r"^([A-Za-z0-9_./-]+)\s*$")
inline_re = re.compile(
    r"^([A-Za-z0-9_./-]+)\s+time:\s*\[\s*([0-9.]+)\s*([a-zµμ]+)\s+"
    r"([0-9.]+)\s*([a-zµμ]+)\s+([0-9.]+)\s*([a-zµμ]+)\s*\]"
)
time_re = re.compile(
    r"time:\s*\[\s*([0-9.]+)\s*([a-zµμ]+)\s+([0-9.]+)\s*([a-zµμ]+)\s+([0-9.]+)\s*([a-zµμ]+)\s*\]"
)

unit_to_ns = {
    "ps": 1e-3,
    "ns": 1.0,
    "µs": 1e3,
    "μs": 1e3,
    "us": 1e3,
    "ms": 1e6,
    "s": 1e9,
}

def to_ns(value: float, unit: str) -> float:
    u = unit.replace("μ", "µ")
    if u not in unit_to_ns:
        raise SystemExit(f"unidad Criterion desconocida: {unit!r}")
    return value * unit_to_ns[u]

samples: dict[str, list[float]] = {}
current = None
for i in range(1, runs + 1):
    text = (raw_dir / f"run_{i}.log").read_text(encoding="utf-8", errors="replace")
    for line in text.splitlines():
        stripped = line.strip()
        im = inline_re.match(stripped)
        if im and "/" in im.group(1):
            mid = to_ns(float(im.group(4)), im.group(5))
            samples.setdefault(im.group(1), []).append(mid)
            current = None
            continue
        m = bench_only_re.match(stripped)
        if m and "/" in m.group(1) and not m.group(1).startswith("Benchmarking"):
            current = m.group(1)
            continue
        tm = time_re.search(stripped)
        if tm and current:
            mid = to_ns(float(tm.group(3)), tm.group(4))
            samples.setdefault(current, []).append(mid)
            current = None

rows = []
for name in sorted(samples):
    xs = samples[name]
    mean = statistics.fmean(xs)
    if len(xs) > 1 and mean > 0:
        cv = 100.0 * statistics.pstdev(xs) / mean
    else:
        cv = 0.0
    rows.append((name, mean, cv, len(xs)))

with report.open("a", encoding="utf-8") as f:
    for name, mean, cv, n in rows:
        f.write(f"| `{name}` | {mean:.0f} | {cv:.2f} | {n} |\n")
    f.write("\n## Notas\n\n")
    f.write("- CV% = desviación típica poblacional / media × 100 (sobre las medias Criterion de cada corrida).\n")
    f.write("- Umbrales CI agresivos: fuera de alcance; usar este informe para regresiones manuales.\n")
    f.write(f"- Logs crudos: `{raw_dir}`\n")

if not rows:
    print("No se parsearon tiempos Criterion; revisá los logs en", raw_dir, file=sys.stderr)
    sys.exit(1)
print(f"Escrito {report} ({len(rows)} benchmarks)")
PY

info "listo → $REPORT"
