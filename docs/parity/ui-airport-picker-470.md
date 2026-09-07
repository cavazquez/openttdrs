# Selector de aeropuerto localizado (#470)

Implementado en `main` el 2026-09-07.

## Divergencia auditada

`AirportPicker` tenía chrome español fijo: título, ejes, cobertura, botones
`Off`/`On` y los resúmenes de tamaño/cobertura no seguían el locale inglés.
Además, el resumen de cobertura se generaba cada frame, por lo que el cambio
de idioma no podía resolverlo mediante el catálogo de textos estáticos.

## Implementación

Se catalogaron el título, `Axis X`/`Axis Y`, `Coverage`, `Off`/`On`, el
prefijo de tamaño y los estados de cobertura. El sync recibe
`ClientPreferences` y localiza sólo el chrome dinámico:

- `Airport · <label>` conserva literalmente el nombre del spec, incluido un
  nombre NewGRF.
- `Size: <w×h>` conserva huella y orientación calculadas por el core.
- `Coverage r=<n>` conserva radio y contadores de casas/stock.
- `Coverage: hidden` y `Coverage r=<n>: point to the map` reflejan los dos
  estados de interacción sin cambiar comandos ni datos de simulación.

La regresión cubre ambos locales, valores dinámicos y un label de aeropuerto
personalizado sin reinterpretarlo.

## Alcance pendiente

Las clases, specs y nombres NewGRF siguen siendo datos literales, igual que en
OpenTTD; #331 continúa abierto por los catálogos upstream y otras superficies
UI todavía no migradas.
