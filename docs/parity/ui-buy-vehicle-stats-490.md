# Paridad UI: estadísticas de compra de vehículos (#490)

Fecha de implementación: 2026-09-07  
Issue: [#490](https://github.com/cavazquez/openttdrs/issues/490) · Parent: [#331](https://github.com/cavazquez/openttdrs/issues/331)

## Divergencia observada

La lista y los filtros de `BuyVehicleWindow` ya seguían el locale, pero el
panel de características de un motor seleccionado mantenía frases españolas.
El placeholder editable `buscar…` tampoco se actualizaba cuando el usuario
cambiaba de idioma o cerraba/limpiaba la ventana.

## Corrección aplicada

- `stats_text` recibe `Locale` y localiza rol, precio, peso, velocidad,
  potencia (`cv`/`hp`), coste anual, capacidad, año y fiabilidad.
- Las etiquetas de cargo vanilla usan el catálogo; los cargos
  `Custom`/NewGRF y nombres de motor permanecen literales.
- Metadatos NewGRF y fallback de locomotora sin carga siguen el idioma activo.
- Un sistema dedicado mantiene el placeholder editable en `search…`/`buscar…`
  durante apertura, limpieza y cambio de locale, sin modificar el filtro.
- Sprites, IDs, precios, cifras, filtros, selección y comandos de compra no
  cambian.

## Regresión

`buy_window_stats_follow_locale_without_translating_engine_or_custom_cargo_names`
cubre salida española/inglesa, unidades `cv`/`hp`, año, cargo vanilla,
preservación de `CargoType::Custom` y ambos placeholders.

## Residual explícito

Esfuerzo tractor (TE), criterios avanzados de orden/filtrado, nombres NewGRF
completos y formato numérico de OpenTTD siguen fuera de este sub-issue.

## Validación

- `cargo fmt --all -- --check`
- `cargo clippy -p openttdrs-core --all-targets -- -D warnings`
- `cargo test -p openttdrs-core --quiet`
- `cargo clippy -p openttdrs-client --all-targets -- -D warnings`
- `cargo test -p openttdrs-client --bin openttdrs-client --quiet`
- `./scripts/check_parity_docs_fresh.sh`
- `git diff --check`
