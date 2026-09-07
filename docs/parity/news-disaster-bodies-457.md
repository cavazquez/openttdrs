# Bodies dinámicos de desastres y locale (#457)

Implementado en `main` el 2026-09-07.

## Divergencia auditada

`push_disaster_news` ya traducía los seis headlines de accidente, pero los
bodies con la posición del evento seguían en español al seleccionar inglés:
OVNI pequeño/enorme, avión, helicóptero, submarino y hundimiento minero.

## Implementación

El resolver dinámico reconoce las seis plantillas exactas emitidas por
`disaster_copy` y valida el par como coordenadas `i32` signed:

- `Un OVNI pequeño se aproxima a (x, y).` →
  `A small UFO is approaching (x, y).`;
- `Un OVNI enorme se aproxima a (x, y).` →
  `A large UFO is approaching (x, y).`;
- `Un avión se estrella cerca de (x, y).` →
  `An aircraft crashes near (x, y).`;
- `Un helicóptero se estrella en (x, y).` →
  `A helicopter crashes at (x, y).`;
- `Un submarino provoca daños en (x, y).` →
  `A submarine causes damage at (x, y).`;
- `Un hundimiento en mina afecta (x, y).` →
  `A mine subsidence affects (x, y).`.

Coordenadas inválidas, truncamientos, texto adicional, paréntesis extra y
marcadores GameScript permanecen intactos. `LocalizationPlugin` cubre las
entidades tardías y la reversión al locale español sin mutar el evento.

Las regresiones `catalog_translates_all_disaster_bodies_without_mutating_coordinates`,
`localization_plugin_translates_disaster_headline_and_body` y
`localization_plugin_translates_all_disaster_bodies_late` cubren las seis
variantes, coordenadas negativas, fallback seguro y cambio de locale.
