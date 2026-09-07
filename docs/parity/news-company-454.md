# Noticias dinámicas de compañía y locale (#454)

Implementado en `main` el 2026-09-07.

## Divergencia auditada

El core genera noticias de logro rival y quiebra con nombre de compañía,
objetivo y contadores dentro del texto. El catálogo literal no podía
traducirlas sin perder esos valores, por lo que el ticker y el periódico
quedaban en español al seleccionar inglés.

## Implementación

`localized_text(Locale::En, ...)` reconoce únicamente las cuatro formas
emitidas por `push_rival_achievement_news` y `push_bankruptcy_news`:

- `Logro rival: EMPRESA`;
- `«EMPRESA» cumplió el objetivo: OBJETIVO`;
- `Quiebra: EMPRESA`;
- `La compañía «EMPRESA» está en quiebra (mes MES/LÍMITE).`.

Los nombres, objetivos y contadores se copian sin traducir. El parser exige
dígitos ASCII para mes/límite y fragmentos no vacíos sin marcadores, controles
ni paréntesis; cadenas truncadas, texto de GameScript o formatos ambiguos
permanecen intactos. `LocalizationPlugin` registra entidades tardías y
restaura las cadenas españolas al volver al locale `es`.

Las regresiones `catalog_translates_company_news_templates_without_mutating_values`
y `localization_plugin_translates_company_news_late` cubren ambas familias,
los valores dinámicos, el cambio de locale y el fallback seguro. Otras
noticias dinámicas y la migración completa de `NewsItem` continúan en #331.
