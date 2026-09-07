# Paridad UI: ventana de detalles de vehículos (#489)

Fecha de implementación: 2026-09-07  
Issue: [#489](https://github.com/cavazquez/openttdrs/issues/489) · Parent: [#331](https://github.com/cavazquez/openttdrs/issues/331)

## Divergencia observada

`VehicleDetailsWindow` mantenía literales en español en sus cuatro pestañas
cuando el locale del cliente era inglés: título, resumen de totales, unidades
de potencia/edad, fiabilidad, depósito, renovación, paquetes, velocidad,
órdenes y año de coste.

## Corrección aplicada

- `Locale` se propaga a resumen, filas y cuerpo de detalles.
- El catálogo traduce chrome (`Details`, `Capacity`, `Totals`) y métricas
  vanilla (`Weight`, `Power`, `Profit this year`, `rel.`, `depot`, `renew`,
  `year`, `Speed`, `max.`, `Active`).
- Cargas vanilla usan `cargo_spec_display_name` y catálogo; nombres definidos
  por NewGRF/custom se conservan literalmente.
- El título dinámico sigue el locale sin traducir el nombre personalizado del
  vehículo; IDs, coordenadas, cifras, consist, sprites, selección y comandos
  no se mutan.

## Regresión

`english_details_localize_chrome_and_preserve_engine_custom_name_and_values`
cubre locale inglés, nombre Unicode, cargo vanilla, velocidad, capacidad,
beneficio, coste anual y ausencia de literales españoles; las pruebas previas
mantienen la salida española y la composición de consist.

## Residual explícito

El editor de intervalo de servicio, esfuerzo tractor (TE), formato numérico
completo y editor avanzado de órdenes siguen fuera de este sub-issue.

## Validación

- `cargo fmt --all -- --check`
- `cargo clippy -p openttdrs-core --all-targets -- -D warnings`
- `cargo test -p openttdrs-core --quiet`
- `cargo clippy -p openttdrs-client --all-targets -- -D warnings`
- `cargo test -p openttdrs-client --bin openttdrs-client --quiet`
- `./scripts/check_parity_docs_fresh.sh`
- `git diff --check`
