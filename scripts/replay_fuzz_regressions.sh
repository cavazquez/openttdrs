#!/usr/bin/env bash
# Reproduce el corpus de regresión de libFuzzer sin mutarlo.
#
# El fuzz aleatorio de larga duración vive en `.github/workflows/fuzz.yml`.
# Este script es el gate corto: ejecuta cada input versionado una vez y falla
# ante un crash, sanitizer o cambio que invalide el lockfile independiente.

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
FUZZ_TOOLCHAIN="${FUZZ_TOOLCHAIN:-nightly-2026-07-31}"

cd "$ROOT/fuzz"

cargo +"$FUZZ_TOOLCHAIN" metadata --manifest-path Cargo.toml --locked --format-version 1 >/dev/null

for target in sav_load newgrf_parse net_message; do
    mkdir -p "artifacts/$target"
    cargo +"$FUZZ_TOOLCHAIN" fuzz run "$target" "regression-corpus/$target" -- \
        -runs=0 \
        -timeout=10 \
        -rss_limit_mb=1024 \
        -artifact_prefix="artifacts/$target/" \
        -print_final_stats=1
done

# Este SAV real no forma parte del corpus reducido por cobertura: lo fijamos
# para conservar la ruta de carga completa que usamos como ancla semántica.
mkdir -p artifacts/sav_load
cargo +"$FUZZ_TOOLCHAIN" fuzz run sav_load regression-anchors/sav_load -- \
    -runs=0 \
    -timeout=10 \
    -rss_limit_mb=1024 \
    -artifact_prefix=artifacts/sav_load/ \
    -print_final_stats=1
