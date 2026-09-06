# RoadStop `CBID_STATION_AVAILABILITY` — scope de compra (#406)

Actualizado: **2026-09-06**. Implementación: `openttdrs-core`.

## Referencia nativa

`OpenTTD` construye `RoadStopScopeResolver` para el selector de paradas con
`st == nullptr` y `tile == INVALID_TILE`. Por eso el callback no debe leer una
parada ya colocada ni inventar una tesela temporal. La fuente de verdad es
`src/newgrf_roadstop.cpp` (`RoadStopScopeResolver::GetVariable`) y el call site
`CmdBuildRoadStop`/`RoadStopChangeInfo`.

## Implementación

`apply_road_stop_availability_callback_with_context` recibe el propietario,
el color de la compañía activa, el pool de compañías y el catálogo de tipos de
carretera. Antes de resolver `CBID_STATION_AVAILABILITY` (`0x13`) publica:

| Variable | Valor de compra |
|---:|---|
| `0x40` | Vista `0` |
| `0x41` | `0` bus, `1` camión, `2` waypoint |
| `0x42` | Terreno plano/sentinel `0` |
| `0x43` / `0x44` | Road/tram type traducido por la tabla GlobalVar del GRF |
| `0x45` | `HouseZone::TownEdge << 16` |
| `0x46` | Distancia cuadrática `0` sin tesela |
| `0x47` | `GetCompanyInfo` exacto: id, bit IA y colores de librea |
| `0x49` | Frame `0` sin tesela |
| `0x50` | Bit de scope de compra `1 << 4` |

La API histórica `apply_road_stop_availability_callback` conserva un wrapper
determinista para callers sin `GameState`: propietario jugador, color de
fallback y pool vacío. El comando de construcción (`PlaceBusStop` y
`PlaceTruckStop`) usa la variante contextual antes de cambiar el mapa, igual
que la ruta ferroviaria de estaciones.

Como todavía no existe una entidad `RoadStop` en el picker, no se escribe PSA
`7C` en este scope. Las consultas de vecinos, terreno real, cargas y estado de
una parada ya colocada permanecen en los resolvers de render y animación.

## Regresiones

`road_stop_availability_purchase_scope_uses_native_sentinels_and_company_context`
comprueba:

- `0x47 = 0x9201_0001` para una compañía IA con librea primaria/secundaria
  distinta;
- bit `0x50 = 0x10` del picker;
- sentinels de zona, distancia y frame (`0x45`, `0x46`, `0x49`);
- ejecución con `CompanyId`, pool y road type del contexto real.

## Límites restantes

El callback de disponibilidad ya comparte el contexto de compañía y los
sentinels del selector, pero no convierte el picker en una tesela: no hay
vecinos ni variables de carga hasta que la parada es creada. Strings/mensajes
de error, sonidos, scopes completos de `BaseStation` y GRF ausentes continúan
en el issue padre [#329](https://github.com/cavazquez/openttdrs/issues/329).
