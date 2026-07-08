#!/usr/bin/env bash
# check.sh - Formatea, verifica lints y ejecuta tests
#
# Uso:
#   ./scripts/check.sh        # Todo local (fmt, lint, test)
#   ./scripts/check.sh fmt    # Solo formatear
#   ./scripts/check.sh fmt-check  # Solo verificar formato (como CI)
#   ./scripts/check.sh lint   # Clippy estricto (como CI)
#   ./scripts/check.sh test   # Tests del workspace
#   ./scripts/check.sh cov    # Tests + informe LCOV (requiere cargo-llvm-cov + llvm-tools-preview)
#   ./scripts/check.sh ci     # Paridad con .github/workflows/ci.yml (sin instalar APT)
#   ./scripts/check.sh audit  # SP3.0: PNG OpenGFX requeridos vs assets/opengfx/tiles
#   ./scripts/check.sh build  # cargo build --workspace

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

info() { echo -e "${GREEN}[INFO]${NC} $*"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $*"; }
error() { echo -e "${RED}[ERROR]${NC} $*"; }

cd "$(dirname "$0")/.."

TNBP_FIXTURE="crates/openttdrs-core/tests/fixtures/v5p12_tnbp.ottdmap"

do_fmt() {
    info "Formateando código..."
    cargo fmt --all
    info "Formato aplicado ✓"
}

do_fmt_check() {
    info "Verificando formato..."
    if ! cargo fmt --all -- --check; then
        error "Código no formateado. Ejecuta: cargo fmt --all"
        return 1
    fi
    info "Formato OK ✓"
}

do_lint() {
    info "Ejecutando Clippy (workspace, -D warnings)..."
    cargo clippy --workspace --all-targets -- -D warnings
    info "Clippy OK ✓"
}

do_test() {
    info "Ejecutando tests (workspace)..."
    if command -v cargo-nextest &>/dev/null; then
        cargo nextest run --workspace
    else
        warn "cargo-nextest no instalado; usando cargo test --workspace"
        cargo test --workspace
    fi
    info "Tests OK ✓"
}

do_tnbp() {
    info "Validando TNBP ($TNBP_FIXTURE)..."
    cargo run -q -p openttdrs-core --example validate_ottdmap_tnbp -- "$TNBP_FIXTURE"
    info "TNBP OK ✓"
}

do_golden_parse_sav() {
    info "Golden parse_sav..."
    python3 scripts/verify_parse_sav_reference.py
    python3 scripts/verify_parse_sav_water_m5.py
    python3 scripts/verify_parse_sav_rail_m5.py
    info "Golden parse_sav OK ✓"
}

do_py_compile() {
    info "Sintaxis Python (scripts)..."
    python3 -m py_compile scripts/parse_sav.py
    python3 -m py_compile scripts/verify_parse_sav_reference.py
    python3 -m py_compile scripts/emit_parse_sav_golden.py
    python3 -m py_compile scripts/verify_parse_sav_water_m5.py
    python3 -m py_compile scripts/verify_parse_sav_rail_m5.py
    python3 -m py_compile scripts/gen_tnbp_fixture_ottdmap.py
    python3 -m py_compile scripts/audit_sp3_assets.py
    python3 -m py_compile scripts/gen_house_draw_data.py
    python3 -m py_compile scripts/gen_vehicle_gfx_data.py
    python3 -m py_compile scripts/gen_rail_signals_sav.py
    python3 -m py_compile scripts/extract_roadveh_movement.py
    info "Python OK ✓"
}

do_audit() {
    info "Auditoría SP3.0 (assets OpenGFX)..."
    python3 scripts/audit_sp3_assets.py
    info "Auditoría SP3 OK ✓"
}

do_coverage() {
    if ! command -v cargo-llvm-cov &>/dev/null; then
        error "Instalá cargo-llvm-cov: cargo install cargo-llvm-cov"
        error "Y el componente: rustup component add llvm-tools-preview"
        return 1
    fi
    info "Generando cobertura (workspace, LCOV en lcov.info)..."
    cargo llvm-cov --workspace --all-targets --lcov --output-path lcov.info
    info "Listo: lcov.info (subilo a Codecov o abrí con un visor LCOV) ✓"
}

do_build() {
    info "Verificando compilación..."
    cargo build --workspace
    info "Build OK ✓"
}

do_all() {
    do_fmt
    do_lint
    do_test
    echo
    info "=== Todo OK ==="
}

do_ci() {
    export CARGO_PROFILE="${CARGO_PROFILE:-ci}"
    do_fmt_check
    # Paridad con CI: clippy compila todo; nextest reutiliza binarios (--no-build).
    if command -v cargo-nextest &>/dev/null; then
        info "Ejecutando Clippy (profile ci)..."
        cargo clippy --workspace --all-targets --profile ci -- -D warnings
        info "Clippy OK ✓"
        info "Ejecutando tests (nextest, --no-build)..."
        cargo nextest run --workspace --profile ci --no-build
        info "Tests OK ✓"
    else
        do_lint
        do_test
    fi
    do_tnbp
    do_golden_parse_sav
    do_py_compile
    echo
    info "=== CI OK (paridad con .github/workflows/ci.yml, profile=${CARGO_PROFILE}) ==="
}

case "${1:-all}" in
    fmt)         do_fmt ;;
    fmt-check)   do_fmt_check ;;
    lint)        do_lint ;;
    test)        do_test ;;
    tnbp)        do_tnbp ;;
    golden)      do_golden_parse_sav ;;
    py)          do_py_compile ;;
    cov|coverage) do_coverage ;;
    build)       do_build ;;
    audit)       do_audit ;;
    ci)          do_ci ;;
    all)         do_all ;;
    *)
        echo "Uso: $0 {fmt|fmt-check|lint|test|tnbp|golden|py|audit|cov|build|ci|all}"
        exit 1
        ;;
esac
