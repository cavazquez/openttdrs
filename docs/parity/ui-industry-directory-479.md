# Paridad UI: directorio de industrias (#479)

Fecha de implementación: 2026-09-07  
Issue: [#479](https://github.com/cavazquez/openttdrs/issues/479)  
Parent: [#331](https://github.com/cavazquez/openttdrs/issues/331)

## Divergencia observada

`IndustryDirectory` dejaba en español el título, el placeholder del buscador,
los botones de orden, la ayuda de fundación, los nombres de industrias y los
estados vacíos cuando se seleccionaba inglés. Las cadenas de entrada/salida de
las filas y los botones vanilla de fundación también mezclaban el locale con
los datos del juego.

## Corrección

- El título, buscador, orden, fundación, filtros y estados vacío/filtrado se
  actualizan con `ClientPreferences::locale()` y cambian en vivo.
- Los nombres vanilla de `IndustryKind`/`IndustrySpec`, botones de fundación y
  cargos vanilla de las cadenas I/O usan el catálogo inglés cuando corresponde.
- La caché incluye el locale, por lo que las filas y los botones se regeneran
  al cambiar idioma sin cambiar posiciones, stock, capacidad, IDs, orden o
  acciones de construcción.
- Se conservan los labels sin clave del catálogo (incluidos valores custom) y
  se admite la búsqueda tanto por el label traducido como por su fuente
  española.

## Regresión y alcance

La regresión cubre nombres de industria, cadenas de cargo, botones vanilla y
preservación de valores custom en ambos locales. Los nombres de industrias,
cargos y reportes NewGRF que no tienen una clave vanilla siguen siendo datos
literales; la paridad completa de catálogos y textos generados continúa en el
parent #331.

Validación publicada: formatter, clippy estricto de core/cliente, tests de
core/cliente, gate de documentación de paridad y `git diff --check`.
