# Station availability purchase scope (`#404`)

Actualizado: 2026-09-06.

OpenTTD evalúa `CBID_STATION_AVAILABILITY` (`0x13`) antes de crear una
estación. En ese resolver `st == nullptr`, por lo que no corresponde fabricar
una entidad para alimentar el callback: el scope expone los sentinelas nativos
de layout, el estado de compra, la compañía activa, badges de la spec y la
fecha relativa del calendario.

La implementación Rust añade `apply_station_availability_callback_for_build_with_context`:

- `0x40`, `0x41`, `0x46`, `0x47` y `0x49` devuelven `0x02110000`.
- `0x42` devuelve `0` y `0x44` devuelve `2` para PBS en compra.
- `0x43` usa `newgrf_company_info` con la compañía activa, el bit IA y la
  librea por defecto real.
- `0x7A` reutiliza la traducción de badges de la `StationSpec`.
- `0xFA` calcula días desde `STATION_BUILD_DATE_DEFAULT` y satura al WORD
  nativo.
- Los registros `7C` quedan en el contexto efímero: OpenTTD todavía no tiene
  una estación donde persistirlos.

El preflight de `PlaceRailStation` y `PlaceRailStationArea` pasa el pool de
compañías y el calendario del `GameState` antes de mutar mapa o estaciones. La
API anterior sigue disponible con un fallback determinista para integraciones
que no conservan ese mundo.

Regresiones:

- `station_purchase_scope_matches_native_sentinels_and_badges`
- `station_availability_purchase_scope_uses_company_and_calendar_context`

Referencia upstream: `newgrf_station.cpp`, rama de `StationScopeResolver` con
`st == nullptr`.
