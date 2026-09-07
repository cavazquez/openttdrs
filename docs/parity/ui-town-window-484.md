# Paridad UI: ficha de pueblo (#484)

Fecha de implementación: 2026-09-07  
Issue: [#484](https://github.com/cavazquez/openttdrs/issues/484)  
Parent: [#331](https://github.com/cavazquez/openttdrs/issues/331)

## Divergencia observada

La ficha de pueblo mostraba en español sus acciones, métricas de autoridad y
crecimiento, metas de carga e historial aun cuando el locale activo era inglés.
El cambio de idioma tampoco podía refrescar los botones porque sus textos eran
literales del árbol inicial.

## Corrección

- Las cuatro acciones (`Center`, `Advertise`, `Fund` y `Auth.`) se crean con un
  marcador propio y se actualizan al cambiar el locale; los importes siguen
  siendo los mismos.
- El cuerpo localiza métricas, rating, financiación, demanda, metas y series
  mensuales, incluyendo los nombres de clima y el estado de crecimiento.
- El título conserva el nombre del pueblo y su población; posiciones, casas,
  valores, IDs, comandos, cámara y vínculo con la ventana de autoridad no se
  traducen ni mutan.

## Regresión y alcance

Las pruebas cubren etiquetas y acciones en inglés, valores de metas de invierno
y desierto, y preservación de nombres Unicode. La fórmula de crecimiento y la
semántica de los datos siguen siendo las del core; la equivalencia completa de
crecimiento urbano continúa pendiente en los issues de simulación.

Validación publicada: formatter, Clippy estricto de core/cliente, tests de
core/cliente, gate de documentación de paridad y `git diff --check`.
