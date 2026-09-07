# Body dinámico de accidente aéreo (#460)

Implementado en `main` el 2026-09-07.

## Divergencia auditada

`crash_airplane` genera el body `Un jet intentó aterrizar en pista corta en
(x, y).`. El texto no pasaba por el catálogo inglés, mientras que el
headline conserva un nombre de vehículo potencialmente arbitrario.

## Implementación

El resolver reconoce sólo el body exacto y valida la posición como
coordenadas `i32` signed:

`Un jet intentó aterrizar en pista corta en (x, y).` →
`A jet attempted to land on a short runway at (x, y).`.

El nombre del vehículo y el headline no se interpretan en este bloque. Las
coordenadas inválidas, truncamientos, texto adicional, paréntesis extra y
marcadores GameScript permanecen intactos. `LocalizationPlugin` cubre
entidades tardías y vuelta a español.

Las regresiones `catalog_translates_aircraft_crash_body_without_mutating_coordinates`
y `localization_plugin_translates_aircraft_crash_body_late` cubren
coordenadas negativas, fallback seguro y cambio de locale.
