# Paridad UI: vista de compañía (#483)

Fecha de implementación: 2026-09-07  
Issue: [#483](https://github.com/cavazquez/openttdrs/issues/483)  
Parent: [#331](https://github.com/cavazquez/openttdrs/issues/331)

## Divergencia observada

`CompanyView` mantenía en español el título, el botón de Finanzas y el resumen
de dinero, préstamo y flota cuando el locale activo era inglés. El texto
residual de Livery/ManagerFace/Infrastructure también quedaba sin adaptar.

## Corrección

- El título, botón y labels económicos consultan el locale activo y se
  actualizan en vivo.
- El nombre de compañía, importes, conteos, IDs y navegación a Finanzas se
  preservan como datos del juego.
- El residual funcional permanece explícito y sólo se traduce su chrome, sin
  afirmar paridad de Livery/ManagerFace/Infrastructure.

## Regresión y alcance

La regresión cubre labels ingleses y preservación de nombres con Unicode. La
implementación de Livery/ManagerFace/Infrastructure y otros datos de compañía
continúa siendo residual bajo el parent #331.

Validación publicada: formatter, clippy estricto de core/cliente, tests de
core/cliente, gate de documentación de paridad y `git diff --check`.
