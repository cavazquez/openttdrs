#!/usr/bin/env bash
# check.sh - Verifica formato, lints y ejecuta tests
#
# Uso:
#   ./scripts/check.sh        # Todo local sin mutar fuentes (fmt-check, lint, test)
#   ./scripts/check.sh fmt    # Solo formatear
#   ./scripts/check.sh fmt-check  # Solo verificar formato (como CI)
#   ./scripts/check.sh lint   # Clippy estricto (como CI)
#   ./scripts/check.sh doc    # Rustdoc del workspace con warnings como errores
#   ./scripts/check.sh test   # Tests del workspace
#   ./scripts/check.sh cov    # Tests + informe LCOV (requiere cargo-llvm-cov + llvm-tools-preview)
#   ./scripts/check.sh ci     # Paridad con .github/workflows/ci.yml (sin instalar APT)
#   ./scripts/check.sh ci-python  # Goldens + py_compile + runs del manifiesto (#120)
#   ./scripts/check.sh generated-tables  # reproducibilidad pilots (#119; hash + regen si hay upstream)
#   ./scripts/check.sh audit  # SP3.0: PNG OpenGFX requeridos vs assets/opengfx/tiles
#   ./scripts/check.sh build  # cargo build --workspace
#   ./scripts/check.sh doctor # deps de entorno (delegado a scripts/doctor.sh)
#   ./scripts/check.sh bench  # smoke Criterion (#116; no forma parte de `ci`)
#   ./scripts/check.sh parity-docs  # frescura docs tick/carga (#125)
#   ./scripts/check.sh openttd-smoke  # load+roundtrip #226 (SKIP si no hay binario)
#
# Excepciones documentadas (solo en GitHub Actions, no en `ci` local):
#   - cargo-audit / cargo-deny — #106
#   - cobertura llvm-cov en push a main (y workflow Coverage manual)
#   - fetch OpenTTD pin + mutación de tablas generadas (`generated-tables` con --fetch-upstream)
# La lista Python compartida vive en scripts/ci_python_manifest.json.

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

info() { echo -e "${GREEN}[INFO]${NC} $*"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $*"; }
error() { echo -e "${RED}[ERROR]${NC} $*"; }

cd "$(dirname "$0")/.."

# sccache es opcional en la máquina local: acelera los comandos de este script
# cuando está disponible, pero no vuelve al repo dependiente de él (en Windows
# y macOS `cargo` directo sigue siendo portable). CI lo activa siempre con el
# composite `.github/composite/sccache`.
if [[ -z "${RUSTC_WRAPPER+x}" ]] && command -v sccache >/dev/null 2>&1; then
    export RUSTC_WRAPPER=sccache
    info "sccache local activado ($(sccache --version | head -1))"
fi

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

do_rustdoc() {
    info "Validando rustdoc (workspace, -D warnings)..."
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps
    info "Rustdoc OK ✓"
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
    local -a args=(-q -p openttdrs-core)
    if [[ -n "${CARGO_PROFILE:-}" ]]; then
        args+=(--profile "$CARGO_PROFILE")
    fi
    args+=(--example validate_ottdmap_tnbp -- "$TNBP_FIXTURE")
    cargo run "${args[@]}"
    info "TNBP OK ✓"
}

do_golden_parse_sav() {
    info "Golden parse_sav (manifiesto CI)..."
    python3 scripts/run_ci_python.py golden
    info "Golden parse_sav OK ✓"
}

do_openttd_reference_manifest() {
    info "Manifiesto referencia OpenTTD (#109)..."
    python3 scripts/test_openttd_reference_manifest.py
    info "Manifiesto OpenTTD OK ✓"
}

do_snapshot_oracle_tools() {
    info "Herramientas oráculo de snapshots (#110)..."
    python3 -m py_compile scripts/compare_snapshots.py
    python3 -m py_compile scripts/test_compare_snapshots_mutation.py
    python3 scripts/test_compare_snapshots_mutation.py
    info "Oráculo snapshots tooling OK ✓"
}

do_py_compile() {
    info "Sintaxis Python (manifiesto CI)..."
    python3 scripts/run_ci_python.py py_compile
    info "Python OK ✓"
}

do_ci_python() {
    info "Checks Python compartidos con GHA (#120)..."
    python3 scripts/run_ci_python.py all
    info "ci-python OK ✓"
}

do_generated_tables() {
    info "Tablas generadas (#119)..."
    python3 scripts/check_generated_tables.py --check
    info "Tablas generadas OK ✓"
}

do_generated_tables_ci() {
    info "Tablas generadas CI (#119, fetch pin OpenTTD)..."
    python3 scripts/check_generated_tables.py --check --fetch-upstream
    python3 scripts/test_check_generated_tables_mutation.py
    info "Tablas generadas CI OK ✓"
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
    cargo llvm-cov --workspace --all-targets --lcov --output-path lcov.info --fail-under-lines 68
    info "Listo: lcov.info (subilo a Codecov o abrí con un visor LCOV) ✓"
}

do_build() {
    info "Verificando compilación..."
    cargo build --workspace
    info "Build OK ✓"
}

do_doctor() {
    info "Chequeando dependencias de entorno..."
    ./scripts/doctor.sh
}

do_bench() {
    info "Smoke benchmarks headless (#116)..."
    local args=(--warm-up-time 0.3 --measurement-time 0.8 --sample-size 20)
    cargo bench -p openttdrs-core --bench sim_tick -- "${args[@]}"
    cargo bench -p openttdrs-core --bench pathfinding -- "${args[@]}"
    info "Bench smoke OK ✓ (baseline completo: ./scripts/bench_baseline.sh)"
}

do_parity_docs() {
    info "Frescura docs de paridad (#125)..."
    ./scripts/check_parity_docs_fresh.sh
    info "parity-docs OK ✓"
}

do_openttd_smoke() {
    info "Smoke OpenTTD dedicated (#226)..."
    # Gate real cuando hay binario (reference/ o $OPENTTD); SKIP limpio si no.
    ./scripts/validate_sav_openttd.sh \
        crates/openttdrs-core/tests/fixtures/mvp_openttd_rich.sav
    ./scripts/roundtrip_sav_openttd.sh \
        crates/openttdrs-core/tests/fixtures/mvp_openttd_rich.sav
    info "openttd-smoke OK ✓"
}

do_all() {
    do_fmt_check
    do_lint
    do_test
    echo
    info "=== Todo OK ==="
}

do_ci() {
    export CARGO_PROFILE="${CARGO_PROFILE:-ci}"
    do_fmt_check
    # Paridad con CI: clippy + nextest con perfil Cargo `ci`.
    if command -v cargo-nextest &>/dev/null; then
        info "Ejecutando Clippy (profile ci)..."
        cargo clippy --workspace --all-targets --profile ci -- -D warnings
        info "Clippy OK ✓"
        info "Ejecutando tests (nextest, --cargo-profile ci)..."
        cargo nextest run --workspace --cargo-profile ci
        info "Tests OK ✓"
    else
        do_lint
        do_test
    fi
    do_rustdoc
    do_tnbp
    do_ci_python
    do_generated_tables
    do_parity_docs
    # Smoke OpenTTD: obligatorio en local si hay binario; en GHA suele SKIP.
    do_openttd_smoke
    echo
    info "=== CI OK (núcleo compartido con ci.yml; ver excepciones GHA en cabecera) ==="
}

case "${1:-all}" in
    fmt)         do_fmt ;;
    fmt-check)   do_fmt_check ;;
    lint)        do_lint ;;
    doc)         do_rustdoc ;;
    test)        do_test ;;
    tnbp)        do_tnbp ;;
    golden)      do_golden_parse_sav ;;
    py)          do_py_compile ;;
    ci-python)   do_ci_python ;;
    generated-tables) do_generated_tables ;;
    generated-tables-ci) do_generated_tables_ci ;;
    openttd-ref) do_openttd_reference_manifest ;;
    snapshot-oracle) do_snapshot_oracle_tools ;;
    cov|coverage) do_coverage ;;
    build)       do_build ;;
    audit)       do_audit ;;
    doctor)      do_doctor ;;
    bench)       do_bench ;;
    parity-docs) do_parity_docs ;;
    openttd-smoke) do_openttd_smoke ;;
    ci)          do_ci ;;
    all)         do_all ;;
    *)
        echo "Uso: $0 {fmt|fmt-check|lint|doc|test|tnbp|golden|py|ci-python|generated-tables|generated-tables-ci|openttd-ref|snapshot-oracle|audit|cov|build|doctor|bench|parity-docs|openttd-smoke|ci|all}"
        exit 1
        ;;
esac
