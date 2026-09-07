#!/usr/bin/env bash
# Configura sccache sólo cuando el wrapper local puede ejecutar el rustc activo.
#
# Este archivo se sourcea desde check.sh. No reemplaza una elección explícita
# del usuario/CI en RUSTC_WRAPPER: sólo evita que la mera presencia del binario
# sccache vuelva rojo un gate que usa cargo indirectamente.

if [[ -z "${RUSTC_WRAPPER+x}" ]] && command -v sccache >/dev/null 2>&1; then
    rustc_bin="$(command -v rustc)"
    if sccache "$rustc_bin" -vV >/dev/null 2>&1; then
        export RUSTC_WRAPPER=sccache
        info "sccache local activado ($(sccache --version | head -1))"
    else
        warn "sccache local no puede ejecutar rustc; usando cargo directo"
    fi
fi
