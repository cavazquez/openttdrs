# Selector de puentes localizado (#473)

Implementado en `main` el 2026-09-07.

## Divergencia auditada

El `BridgePicker` tenía título y ayuda inicial en español fijo. Cuando había
un tramo pendiente, el resumen de transporte, vano y rampas también se
reconstruía en español sin consultar el locale.

## Implementación

El sync recibe `ClientPreferences` y genera el hint en inglés o español para
carretera/vía, longitud en teselas y la restricción de rampas. Tipo de puente,
velocidad, disponibilidad, coste, coordenadas y comandos continúan viniendo
del core sin transformación. La regresión cubre ambos transportes y locales,
conservando el valor numérico del vano.

## Alcance pendiente

Los nombres de tipos y velocidades de puente siguen siendo datos vanilla del
core; #331 continúa abierto por los catálogos upstream y otras superficies UI.
