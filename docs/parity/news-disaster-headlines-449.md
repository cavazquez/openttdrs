# Headlines de desastres y locale (#449)

Implementado en `main` el 2026-09-07.

## Divergencia auditada

`push_disaster_news` emite un headline fijo por variante de desastre, pero el
catálogo de UI no conocía esas claves. El headline quedaba en español al
cambiar el cliente a inglés, aunque el popup y el ticker sí se crean después
del cambio de locale.

## Implementación

Se incorporaron las seis claves estáticas de `disaster_copy`: OVNI pequeño,
OVNI enorme, accidente aéreo, accidente de helicóptero, submarino a la deriva
y hundimiento minero. `LocalizationPlugin` las registra tanto para entidades
existentes como para noticias tardías y vuelve a español al cambiar el locale.

Los cuerpos conservan las coordenadas y el resto de datos de la partida; no se
traducen por coincidencia parcial. La regresión
`localization_plugin_translates_disaster_headline_but_not_body` cubre ambos
caminos. Los cuerpos parametrizados, las demás noticias y los catálogos `.lng`
siguen pendientes en #331.
