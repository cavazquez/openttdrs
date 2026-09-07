# Selector de tipo de depósito localizado (#474)

Implementado en `main` el 2026-09-07.

## Divergencia auditada

El `DepotBuildPicker` tenía chips `Carretera`, `Tren` y `Barco` sin entradas
de catálogo. Su título se escribía de nuevo como `Depósito` cada frame cuando
la herramienta estaba activa, anulando el locale inglés.

## Implementación

Se añadieron las claves `Road`, `Rail`, `Ship` y se pasó
`ClientPreferences` al sync para mantener el título dinámico en el locale
activo. La herramienta elegida, el enum, las IDs y los comandos no cambian.

La regresión cubre el título español/inglés; el plugin de localización cubre
los chips estáticos al registrarlos.

## Alcance pendiente

Las superficies de construcción restantes (muelle, boya, terraformación,
cartel y waypoints) siguen su propio chrome; #331 continúa abierto.
