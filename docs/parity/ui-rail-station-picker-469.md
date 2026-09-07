# Selector de estación ferroviaria localizado (#469)

Implementado en `main` el 2026-09-07.

## Divergencia auditada

El chrome de `RailStationPicker` no tenía catálogo para el título, las
secciones de clase/orientación/andenes/cobertura ni los botones de cobertura.
Además, el resumen bajo el cursor se reescribía cada frame como `Acepta:` y
`Suministra:` en español, incluso con locale inglés.

## Implementación

Se catalogaron título, secciones, `Off`/`On` y los prefijos dinámicos
`Accepts:`/`Supplies:`. El sentinel `Nada` se traduce a `Nothing`; los cargos
que siguen al prefijo y los labels de clase/spec se conservan literalmente,
porque provienen del estado vanilla/NewGRF.

El sync recibe `ClientPreferences` y sólo transforma el chrome. Huella,
orientación, cobertura, filtros, IDs de catálogo y comandos no cambian.

La regresión `coverage_prefixes_localize_without_interpreting_cargo_labels`
verifica los dos prefijos, el sentinel, cargos con tildes y una línea
desconocida intacta.

## Alcance pendiente

La traducción de nombres de cargos, clases y specs suministrados por catálogos
queda fuera de alcance; #331 continúa abierto por catálogos upstream y otras
superficies UI.
