# Headline del primer vehículo y locale (#451)

Implementado en `main` el 2026-09-07.

`push_first_vehicle_running_news` sólo varía el headline por seis valores
cerrados de `VehicleKind`. El catálogo inglés ahora cubre autobús, camión,
tranvía, tren, barco y avión. El body conserva el `vehicle_id` y no se traduce
por coincidencia literal, de modo que los datos de la partida no se alteran al
cambiar de locale.

Las regresiones
`catalog_translates_first_vehicle_headlines_without_vehicle_ids` y
`localization_plugin_translates_first_vehicle_headline_only` cubren las seis
claves, el cambio Español↔English y el fallback intacto del body. La
parametrización general de `NewsItem` y el resto de catálogos siguen pendientes
en #331.
