# Noticia dinámica de choque de trenes (#459)

Implementado en `main` el 2026-09-07.

## Divergencia auditada

`resolve_train_collisions` emite un headline con víctimas y un body con los
dos IDs de tren y la posición del choque. Ninguno de los dos textos se
localizaba al cambiar al inglés.

## Implementación

El resolver reconoce las formas exactas del core:

- `Choque de trenes (N víctimas)` → `Train collision (N victims)`;
- `Los trenes #A y #B colisionaron en (x, y).` →
  `Trains #A and #B collided at (x, y).`.

Víctimas e IDs exigen dígitos ASCII; el par de posición exige coordenadas
`i32` signed. Los valores se copian sin cambios y se rechazan truncamientos,
coordenadas inválidas, paréntesis extra y marcadores GameScript. El plugin
cubre entidades tardías y reversión a español.

Las regresiones `catalog_translates_train_collision_news_without_mutating_values`
y `localization_plugin_translates_train_collision_news_late` cubren headline,
body, coordenadas negativas, fallback seguro y cambio de locale.
