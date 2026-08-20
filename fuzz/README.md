# Fuzzing

`corpus/` y `artifacts/` son áreas locales, mutables e ignoradas por Git. El
corpus que protege regresiones es `regression-corpus/`: es el mínimo de
cobertura extraído con el nightly fijado y su manifiesto controla cantidad,
tamaño y hash agregado. `regression-anchors/` conserva inputs reales que no
deben ser descartados por esa minimización.

El gate corto que corre en cada PR se puede reproducir con:

```bash
FUZZ_TOOLCHAIN=nightly-2026-07-31 ./scripts/replay_fuzz_regressions.sh
```

El workflow semanal `fuzz.yml` parte del mismo corpus y sigue buscando casos
nuevos. Al minimizar un crash nuevo, agregalo al corpus de regresión y
actualizá `manifest.json`; no reemplaces ni borres el corpus local de trabajo.
