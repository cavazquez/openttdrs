# Headlines dinámicos de avisos de vehículo (#458)

Implementado en `main` el 2026-09-07.

## Divergencia auditada

`push_vehicle_advice_news` genera cinco headlines con el ID del vehículo y,
para la falta de ruta, el índice de la orden. Sin resolver esos parámetros, el
locale inglés dejaba los avisos operativos en español.

## Implementación

El resolver reconoce únicamente las plantillas de `vehicle_advice_headline`:

- `Sin ruta por red: vehículo ID (orden N)` →
  `No network route: vehicle ID (order N)`;
- `Sin órdenes: vehículo ID` → `No orders: vehicle ID`;
- `Parada incompatible: vehículo ID` → `Incompatible stop: vehicle ID`;
- `Sin carga disponible: vehículo ID` → `No cargo available: vehicle ID`;
- `Sin camino reservado: vehículo ID` → `No reserved path: vehicle ID`.

Los IDs e índices exigen dígitos ASCII y se copian sin mutación. Cadenas
truncadas, texto adicional, paréntesis y marcadores GameScript permanecen
intactos. `LocalizationPlugin` cubre entidades tardías y reversión a español.

Las regresiones `catalog_translates_vehicle_advice_headlines_without_mutating_ids`
y `localization_plugin_translates_vehicle_advice_late` cubren las cinco
variantes, validación de parámetros y cambio de locale.
