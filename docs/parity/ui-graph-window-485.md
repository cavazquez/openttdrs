# Paridad UI: ventanas de gráficos económicos (#485)

Fecha de implementación: 2026-09-07  
Issue: [#485](https://github.com/cavazquez/openttdrs/issues/485)  
Parent: [#331](https://github.com/cavazquez/openttdrs/issues/331)

## Divergencia observada

Las ventanas `GraphIncome`, `GraphOperatingProfit` y `GraphCompanyValue`
mostraban títulos y ayudas en español con locale inglés. El tipo de serie
(`mensuales` o `trimestrales`) se calculaba, pero no se reflejaba en la ayuda;
además, todas las ventanas podían terminar escribiendo sobre el primer hint.

## Corrección

- Los títulos de Ingresos, Beneficio operativo, Valor de compañía y Rendimiento
  consultan el catálogo inglés al abrirse y al cambiar el locale.
- Los hints localizan el estado sin datos, el avance del tiempo, `Último` y el
  período de la serie; cada ventana actualiza su propio hint.
- Series, valores monetarios, escalado/color de barras, nombre de compañía,
  IDs y selección de compañía permanecen sin mutaciones.

## Regresión y alcance

La regresión cubre las cuatro etiquetas de gráfico, los dos períodos, el estado
vacío y la preservación de un nombre Unicode no catalogado. El contenido
gráfico avanzado (ejes, tooltips por barra y filtro manual de compañía) sigue
siendo residual del parent #331/#271.

Validación publicada: formatter, Clippy estricto de core/cliente, tests de
core/cliente, gate de documentación de paridad y `git diff --check`.
