# Headlines dinámicos de cierre de industria (#456)

Implementado en `main` el 2026-09-07.

## Divergencia auditada

El core emite dos headlines con la posición de la industria:
`Industria en (x, y) anuncia su cierre` y `Industria cerrada en (x, y)`.
El body fijo del primer aviso ya tenía traducción inglesa, pero ambos
headlines quedaban en español.

## Implementación

El resolver dinámico reconoce sólo esas dos formas y valida el par como
coordenadas `i32` signed antes de traducir:

- `Industria en (x, y) anuncia su cierre` →
  `Industry at (x, y) announces its closure`;
- `Industria cerrada en (x, y)` → `Industry closed at (x, y)`.

Paréntesis extra, coordenadas inválidas, cadenas truncadas y marcadores
GameScript permanecen intactos. `LocalizationPlugin` registra entidades
tardías y permite volver al locale español sin cambiar el dato de la noticia.

Las regresiones `catalog_translates_industry_closure_headlines_with_valid_coordinates`
y `localization_plugin_translates_industry_closure_headlines_late` cubren
coordenadas negativas, fallback seguro y cambio de locale.
