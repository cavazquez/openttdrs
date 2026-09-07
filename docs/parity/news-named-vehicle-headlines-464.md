# Headlines de accidente e inundación con nombre de vehículo (#464)

Implementado en `main` el 2026-09-07.

## Divergencia auditada

Los headlines de `crash_airplane` y `flood_vehicle_consist` incorporan el
nombre libre del vehículo (`{name} se estrelló al aterrizar` y `{name}
inundado`). Los nombres normales no se localizaban al usar inglés.

## Implementación

El resolver traduce sólo nombres no vacíos que pasan el contrato seguro de
fragmentos:

- `{name} se estrelló al aterrizar` → `{name} crashed while landing`;
- `{name} inundado` → `{name} flooded`.

El nombre se copia byte a byte. Paréntesis, controles, marcadores GameScript,
truncamientos y formatos ambiguos quedan intactos para no interpretar texto
arbitrario. `LocalizationPlugin` cubre entidades tardías y reversión a
español.

Las regresiones `catalog_translates_named_vehicle_accident_headlines_safely`
y `localization_plugin_translates_named_vehicle_accident_headlines_late`
cubren nombre por defecto, nombre simple, fallback seguro y cambio de locale.
