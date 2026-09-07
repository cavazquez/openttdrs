# Paridad UI: estado dinámico de vehículos (#488)

Fecha de implementación: 2026-09-07  
Issue: [#488](https://github.com/cavazquez/openttdrs/issues/488)  
Parent: [#331](https://github.com/cavazquez/openttdrs/issues/331)

## Divergencia observada

`VehicleView` mostraba en español los estados de avería, detención, ruta,
señales y carga aun con locale inglés. Las frases de velocidad y algunos
destinos fallback también usaban conectores y etiquetas españolas.

## Corrección

- `format_vehicle_status` recibe el locale activo y localiza estados, frases de
  velocidad y el caso sin órdenes.
- Los destinos fallback de depósito y orden condicional consultan el catálogo;
  nombres de estaciones, coordenadas y números de orden permanecen literales.
- `VehicleView` propaga las preferencias sin modificar colores, movimientos,
  comandos, navegación o selección del vehículo.

## Regresión y alcance

Las pruebas cubren estado detenido, velocidad de bus, ausencia de órdenes,
destino de depósito/condicional y preservación de un nombre Unicode. Los tabs de
Details, órdenes y estados avanzados de movimiento siguen pendientes en #331 y
sus bloques funcionales.

Validación publicada: formatter, Clippy estricto de core/cliente, tests de
core/cliente, gate de documentación de paridad y `git diff --check`.
