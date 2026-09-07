# Noticias dinámicas de compra de compañía (#463)

Implementado en `main` el 2026-09-07.

## Divergencia auditada

`buy_company` emitía un headline con el nombre de la compañía adquirida y un
body con ese nombre y el precio en libras. Ninguno se localizaba al elegir
inglés.

## Implementación

El resolver reconoce únicamente las formas emitidas por el core:

- `Comprada EMPRESA` → `Company bought EMPRESA`;
- `La compañía «EMPRESA» fue adquirida por £PRECIO.` →
  `Company «EMPRESA» was acquired for £PRECIO.`.

Los nombres se validan como fragmentos no vacíos sin controles, paréntesis ni
marcadores, y el precio exige dígitos ASCII. Los valores se copian sin
traducir; truncamientos, precios inválidos, texto adicional y GameScript
permanecen intactos. `LocalizationPlugin` cubre entidades tardías y vuelta a
español.

Las regresiones `catalog_translates_company_news_templates_without_mutating_values`
y `localization_plugin_translates_company_news_late` cubren compra, quiebra,
logro, precio, fallback seguro y cambio de locale.
