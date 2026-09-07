# Tooltips de selectores de vía y roadtype localizados (#477)

Implementado en `main` el 2026-09-07.

## Divergencia auditada

Los tooltips de los botones RailType y RoadType/TramType se definían en
español fijo. La abreviatura estática `Eléc` tampoco tenía variante inglesa.

## Implementación

Se incorporaron al catálogo las descripciones de vía normal, eléctrica,
monorail, maglev, carretera y tranvía, además de `Elec`. El sistema común de
tooltips ya consulta el locale activo, por lo que no se duplicó lógica ni se
alteró el selector. IDs, filtros, nombres de tipos vanilla/NewGRF y comandos
permanecen intactos.

La regresión verifica todas las claves inglesas nuevas.

## Alcance pendiente

Los labels dinámicos de roadtype provistos por NewGRF siguen literales; #331
continúa abierto por los catálogos upstream y otras superficies UI.
