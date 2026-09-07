# Paridad UI: ventana de refit (#487)

Fecha de implementación: 2026-09-07  
Issue: [#487](https://github.com/cavazquez/openttdrs/issues/487)  
Parent: [#331](https://github.com/cavazquez/openttdrs/issues/331)

## Divergencia observada

La ventana de refit dejaba en español el hint, la capacidad/coste y la marca de
selección de las cargas con locale inglés. Los nombres vanilla provenían del
core en español y los nombres definidos por NewGRF no tenían una política
explícita de preservación.

## Corrección

- Hint de depósito, unidades, capacidad resultante, coste y acciones de la
  lista consultan el locale activo en cada slot.
- Los nombres vanilla pasan por el catálogo inglés; un nombre custom/NewGRF no
  catalogado se conserva byte a byte.
- La selección de unidades, capacidad, consist, IDs y `RefitVehicle` no cambia.
- Se añade `ClientPreferences` al sync para que la superficie ya creada pueda
  repintarse tras cambiar idioma.

## Regresión y alcance

Las pruebas cubren coal en ambos locales, capacidad/coste, marca seleccionada y
un nombre custom con acento. `OrderRefit`, filtros NewGRF avanzados y la
semántica completa del depósito permanecen pendientes en #331 y el bloque de
órdenes.

Validación publicada: formatter, Clippy estricto de core/cliente, tests de
core/cliente, gate de documentación de paridad y `git diff --check`.
