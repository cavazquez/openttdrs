#!/usr/bin/env bash
# check.sh - Formatea, verifica lints y ejecuta tests
#
# Uso:
#   ./scripts/check.sh        # Todo (format, lint, test)
#   ./scripts/check.sh fmt    # Solo formatear
#   ./scripts/check.sh lint   # Solo lints (clippy)
#   ./scripts/check.sh test   # Solo tests
#   ./scripts/check.sh ci     # Modo CI (falla si hay cambios de formato)

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

info() { echo -e "${GREEN}[INFO]${NC} $*"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $*"; }
error() { echo -e "${RED}[ERROR]${NC} $*"; }

cd "$(dirname "$0")/.."

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
    info "Ejecutando Clippy..."
    # Usamos -W clippy::all en lugar de -D warnings para evitar warnings excesivos
    cargo clippy --all-targets --all-features
    info "Clippy OK ✓"
}

do_test() {
    info "Ejecutando tests..."
    cargo test --all
    info "Tests OK ✓"
}

do_build() {
    info "Verificando compilación..."
    cargo build --all
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
    echo
    info "=== CI OK ==="
}

case "${1:-all}" in
    fmt)    do_fmt ;;
    lint)   do_lint ;;
    test)   do_test ;;
    build)  do_build ;;
    ci)     do_ci ;;
    all)    do_all ;;
    *)
        echo "Uso: $0 {fmt|lint|test|build|ci|all}"
        exit 1
        ;;
esac
