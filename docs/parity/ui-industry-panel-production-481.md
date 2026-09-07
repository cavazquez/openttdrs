# Paridad UI: ficha y producción de industria (#481)

Fecha de implementación: 2026-09-07  
Issue: [#481](https://github.com/cavazquez/openttdrs/issues/481)  
Parent: [#331](https://github.com/cavazquez/openttdrs/issues/331)

## Divergencia observada

La ficha de industria y su ventana hija de producción mantenían el título,
estados, métricas, historial y cadenas de cargos en español al seleccionar
inglés. La ficha podía quedar mezclada con nombres de GFX/NewGRF y valores de
simulación que sí deben permanecer literales.

## Corrección

- Los prefijos de título, estados de selección, labels de posición/stock,
  producción, cadena e historial se traducen mediante el catálogo activo.
- Los tipos vanilla y los cargos vanilla de la cadena de producción se
  muestran en inglés cuando corresponde; GFX, labels NewGRF y datos sin clave
  se conservan tal cual.
- La ventana hija de producción actualiza su título, estados, cargas y
  métricas con el mismo locale, sin alterar la relación parent/child, foco,
  preview, historial ni acciones.

## Regresión y alcance

Las regresiones cubren títulos vanilla en ambos locales, cadenas de carbón,
tipos con `IndustrySpec`, sufijos de GFX y preservación de labels custom. La
gráfica mensual completa sigue siendo el residual funcional documentado de
#269; catálogos upstream y textos generados sin clave continúan en #331.

Validación publicada: formatter, clippy estricto de core/cliente, tests de
core/cliente, gate de documentación de paridad y `git diff --check`.
