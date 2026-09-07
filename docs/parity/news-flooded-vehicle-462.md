# Body dinámico de vehículo inundado (#462)

Implementado en `main` el 2026-09-07.

## Divergencia auditada

`flood_vehicle_consist` genera el body `Un vehículo quedó bajo el agua en
(x, y).`, que no se localizaba en inglés. El headline incluye el nombre libre
del vehículo y queda fuera de este recorte.

## Implementación

El resolver reconoce sólo el body exacto y valida la posición como
coordenadas `i32` signed:

`Un vehículo quedó bajo el agua en (x, y).` →
`A vehicle was flooded at (x, y).`.

Coordenadas inválidas, truncamientos, texto adicional, paréntesis extra y
marcadores GameScript permanecen intactos. `LocalizationPlugin` cubre
entidades tardías y vuelta al locale español.

Las regresiones `catalog_translates_flooded_vehicle_body_without_mutating_coordinates`
y `localization_plugin_translates_flooded_vehicle_body_late` cubren
coordenadas negativas, fallback seguro y cambio de locale.
