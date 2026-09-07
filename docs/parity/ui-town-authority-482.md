# Paridad UI: autoridad local del pueblo (#482)

Fecha de implementación: 2026-09-07  
Issue: [#482](https://github.com/cavazquez/openttdrs/issues/482)  
Parent: [#331](https://github.com/cavazquez/openttdrs/issues/331)

## Divergencia observada

La ventana `TownAuthority` mostraba en español el título, estado vacío,
resumen, ratings por compañía, acciones y estados de disponibilidad aunque el
locale activo fuera inglés.

## Corrección

- El título y el estado sin pueblo consultan el catálogo `es`/`en`.
- Las ocho acciones de autoridad y sus estados (`Available`, `Insufficient
  funds`, `Unavailable`) se traducen sin alterar sus costes ni máscaras.
- El resumen y la lista de ratings usan labels localizados, preservando nombre
  del pueblo, importes, ratings, IDs y valores negativos.
- Cascada Town→Authority, efectos diferidos, comandos y logs de diagnóstico no
  cambian.

## Regresión y alcance

La regresión cubre acciones, estados, ratings en ambos locales y nombres con
Unicode. La semántica de autoridad y el resto de textos generados sin clave
vanilla continúan bajo el parent #331.

Validación publicada: formatter, clippy estricto de core/cliente, tests de
core/cliente, gate de documentación de paridad y `git diff --check`.
