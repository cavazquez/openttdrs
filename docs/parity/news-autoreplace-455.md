# Headline dinámico de fallo de autoreemplazo (#455)

Implementado en `main` el 2026-09-07.

## Divergencia auditada

`push_autoreplace_failed_news` emite `Autoreemplazo falló (vehículo ID)`.
Al cambiar al locale inglés, el headline quedaba en español aunque el
catálogo ya cubría el chrome y los errores estáticos de autoreemplazo.

## Implementación

El resolver dinámico reconoce sólo el formato emitido por el core y exige un
ID compuesto por dígitos ASCII:

`Autoreemplazo falló (vehículo ID)` → `Autoreplace failed (vehicle ID)`.

El body sigue pasando por el catálogo estático cuando corresponde, pero un
`CommandError` generado por la partida no se interpreta ni se reescribe. Los
headlines truncados, con texto adicional o con marcadores GameScript quedan
intactos. `LocalizationPlugin` registra también entidades creadas después del
arranque y permite volver a `es` sin mutar la noticia.

Las regresiones `catalog_translates_autoreplace_failure_headline_without_touching_body`
y `localization_plugin_translates_autoreplace_failure_late` cubren IDs,
fallback seguro, cambio de locale y materialización tardía.
