# Paridad UI: directorio de pueblos (#480)

Fecha de implementación: 2026-09-07  
Issue: [#480](https://github.com/cavazquez/openttdrs/issues/480)  
Parent: [#331](https://github.com/cavazquez/openttdrs/issues/331)

## Divergencia observada

`TownDirectory` dejaba en español el título, el buscador, los botones de
orden, la acción de fundación y los estados vacío/filtrado al usar inglés.
Además, cada fila conservaba los sufijos españoles de población y autoridad,
por lo que la ventana quedaba mezclada aunque la selección y el centrado
funcionaran.

## Corrección

- El título, placeholder, orden, fundación y estados vacíos consultan el
  locale activo y se actualizan en vivo.
- Las métricas de población/autoridad se formatean por locale; el cambio de
  idioma invalida la caché para reconstruir las filas.
- Nombres personalizados de pueblos, valores numéricos, selección, IDs,
  orden, navegación y la restricción de fundar sólo en el editor permanecen
  intactos.

## Regresión y alcance

La regresión cubre ambos formatos de métricas, valores negativos de autoridad y
la preservación de nombres con caracteres no ASCII. Los nombres generados por
la partida y otros textos dinámicos no catalogados siguen siendo datos
literales; la paridad completa de catálogos continúa en el parent #331.

Validación publicada: formatter, clippy estricto de core/cliente, tests de
core/cliente, gate de documentación de paridad y `git diff --check`.
