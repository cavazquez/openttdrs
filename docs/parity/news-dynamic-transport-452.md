# Plantillas dinámicas de noticias de transporte (#452)

Implementado en `main` el 2026-09-07.

## Divergencia auditada

El core genera noticias de transporte con valores dinámicos, pero el catálogo
literal sólo podía traducir cadenas sin parámetros. El ticker y el popup
quedaban en español al cambiar a inglés, o una coincidencia demasiado amplia
podía tocar texto arbitrario.

## Implementación

`localized_text(Locale::En, ...)` reconoce únicamente las formas emitidas por
el core:

- `Entrega de N u. de CARGO` y `¡Primera entrega! N u. de CARGO`;
- `Tu compañía ha cobrado DINERO por transportar CARGO.`;
- `El vehículo ID ha salido a operar.`.

Los números, importes, IDs y nombres de cargo se copian sin traducir. El
parser exige dígitos ASCII, formato monetario válido y fragmentos sin
marcadores/control ni paréntesis; una cadena truncada, con texto adicional o
de GameScript permanece intacta. `LocalizationPlugin` usa ahora el mismo
resolver para registrar y actualizar textos dinámicos tardíos.

Las regresiones `catalog_translates_transport_news_templates_without_mutating_values`
y `localization_plugin_translates_dynamic_transport_news_late` cubren los
cuatro formatos, el cambio de locale y el fallback seguro. Subsidios,
compañías, coordenadas y las demás familias de `NewsItem` siguen pendientes en
#331.
