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
    info "Golden parse_sav OK ✓"
}

do_py_compile() {
    info "Sintaxis Python (scripts)..."
    python3 -m py_compile scripts/parse_sav.py
    python3 -m py_compile scripts/verify_parse_sav_reference.py
    python3 -m py_compile scripts/emit_parse_sav_golden.py
    python3 -m py_compile scripts/gen_tnbp_fixture_ottdmap.py
    info "Python OK ✓"
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
    do_fmt_check
    do_lint
    do_test
    do_tnbp
    do_golden_parse_sav
    do_py_compile
    echo
    info "=== CI OK (paridad con .github/workflows/ci.yml) ==="
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
    ci)          do_ci ;;
    all)         do_all ;;
    *)
        echo "Uso: $0 {fmt|fmt-check|lint|test|tnbp|golden|py|cov|build|ci|all}"
        exit 1
        ;;
esac
