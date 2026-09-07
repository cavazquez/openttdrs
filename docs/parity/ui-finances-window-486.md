# Paridad UI: ventana de finanzas (#486)

Fecha de implementación: 2026-09-07  
Issue: [#486](https://github.com/cavazquez/openttdrs/issues/486)  
Parent: [#331](https://github.com/cavazquez/openttdrs/issues/331)

## Divergencia observada

La ventana de finanzas fijaba en español los botones y todo el resumen de
efectivo, préstamo, beneficios, entregas, infraestructura y compañías cuando
el locale era inglés. La caché de la ventana tampoco distinguía un cambio de
idioma de un snapshot de datos idéntico.

## Corrección

- Botones `Take loan`, `Repay loan`, `Buy rival (bankruptcy)` y `AI…` se
  actualizan de forma reactiva al cambiar el locale.
- El resumen localiza etiquetas financieras, infraestructura, unidades,
  colores, compañías AI y el bloque de compañías; conserva nombres, importes,
  conteos, colores, IDs y comandos.
- El snapshot incluye el locale para volver a pintar el texto sin recalcular
  innecesariamente la infraestructura.

## Regresión y alcance

La regresión cubre las cuatro acciones, etiquetas principales y un nombre de
compañía Unicode sin catálogo. El detalle histórico completo de OpenTTD,
gráficas y edición avanzada de compañías sigue pendiente en #331/#271.

Validación publicada: formatter, Clippy estricto de core/cliente, tests de
core/cliente, gate de documentación de paridad y `git diff --check`.
