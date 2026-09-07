# Noticia de choque en paso a nivel (#461)

Implementado en `main` el 2026-09-07.

## Divergencia auditada

Las dos rutas de `maybe_road_train_crash` generaban un headline con el ID del
vehículo de carretera y un body con la posición del choque. Ambos textos
quedaban en español en el locale inglés.

## Implementación

El resolver reconoce las formas exactas del core:

- `Choque en paso a nivel (vehículo #ID)` →
  `Level crossing crash (vehicle #ID)`;
- `Un vehículo de carretera chocó con un tren en (x, y).` →
  `A road vehicle collided with a train at (x, y).`.

El ID exige dígitos ASCII y la posición exige coordenadas `i32` signed. Los
valores se copian sin cambios; truncamientos, formatos inválidos, paréntesis
extra y marcadores GameScript permanecen intactos. `LocalizationPlugin` cubre
entidades tardías y reversión a español.

Las regresiones `catalog_translates_level_crossing_crash_news_without_mutating_values`
y `localization_plugin_translates_level_crossing_crash_news_late` cubren
headline, body, coordenadas negativas, fallback seguro y cambio de locale.
