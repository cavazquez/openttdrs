# Plantillas dinámicas de subsidios y locale (#453)

Implementado en `main` el 2026-09-07.

## Divergencia auditada

Las noticias de oferta y adjudicación de subsidios se construían con cargo,
empresa y coordenadas dentro del texto. El catálogo literal no podía
traducirlas sin perder esos valores.

## Implementación

El resolver dinámico reconoce las cuatro formas del core:

- `Subvención: CARGO`;
- `Subvención adjudicada: CARGO`;
- `Transportar CARGO desde (x, y) hacia la estación (x, y).`;
- `«EMPRESA» se adjudica el transporte de CARGO (pago ×2).`.

Los fragmentos de cargo/empresa se conservan y los pares de coordenadas se
validan como enteros signed antes de traducir. Truncamientos, paréntesis,
marcadores y texto de GameScript quedan intactos. `LocalizationPlugin` registra
las entidades tardías y revierte el texto al español sin modificar el estado
de la partida.

Las regresiones `catalog_translates_subsidy_templates_without_mutating_values`
y `localization_plugin_translates_subsidy_news_late` cubren oferta,
adjudicación, coordenadas negativas y fallback seguro. Otras noticias de
compañía y el catálogo completo continúan pendientes en #331.
