# Matriz de callbacks NewGRF (CBID) — OpenTTD 15.3

Referencia: commit `14ec60f248547d4d062a1160f0fc26d742319888`,
`reference/openttd-upstream/src/newgrf_callbacks.h`.

Issues: [#228](https://github.com/cavazquez/openttdrs/issues/228) (MVP CB24/CB31),
[#266](https://github.com/cavazquez/openttdrs/issues/266) (call sites + storage no-veh + triggers).

Catálogos Action0/3/5: [`newgrf-action0-matrix.md`](newgrf-action0-matrix.md).

Estados:

- **soportado**: se ejecuta en un call site real; fallo → [`CALLBACK_FAILED`](../../crates/openttdrs-core/src/newgrf_sprites/model.rs) observable (no silencioso).
- **parcial runtime**: se ejecuta en el recorte de eventos, estado y/o scopes que se indica; el residual queda explícito en la misma fila.
- **almacenado**: `callback_mask` / props parseadas y guardadas; **no** se ejecuta el CB.
- **OOS**: residual documentado (goldens tick-a-tick vs OpenTTD, resto de CBIDs).

API común: `TrainSpriteGraphics::resolve_callback` / `resolve_callback_ctx`,
`resolve_vehicle_callback`, `writeback_*_persistent_registers`,
`apply_industry_location_callback`, `apply_house_construction_callback`,
`apply_object_slope_callback`,
`resolve_cargo_profit_callback`,
`resolve_cargo_station_rating_callback`,
`apply_station_availability_callback_for_build`,
`apply_station_draw_tile_layout_callback`,
`apply_station_slope_callback_for_build`,
`trigger_newgrf_station_animation`, `step_newgrf_station_tiles`,
`apply_station_availability_callback`, `apply_road_stop_availability_callback`,
`trigger_road_stop_animation`, `advance_road_stop_animation`,
`resolve_industry_tile_animation_callback`, `resolve_industry_tile_random_trigger`
(`crates/openttdrs-core/src/newgrf_callback.rs`).

## Por feature

| Feature | CBID (ejemplos) | Estado | Notas |
|---|---|---|---|
| Stations (`04`) | `0x24` `CBID_STATION_BUILD_TILE_LAYOUT` | **soportado** | Call site: construcción `apply_station_build_tile_layout_callback` |
| Stations | `0x13` `CBID_STATION_AVAILABILITY` | **soportado** | Máscara Action0 `0x0B`, Action3→Action2 y call site query+execute de `PlaceRailStation` / `PlaceRailStationArea`; scope de construcción sin estación/tesela, booleano de 8 bits |
| Stations | `0x14` `CBID_STATION_DRAW_TILE_LAYOUT` | **parcial runtime** | Bit `DrawTileLayout` de Action0 `0x0B`; el renderer lo ejecuta por tesela antes de elegir la vista Action1/3 y conserva el eje. Recibe el contexto Action2 existente de tesela. Faltan scope/regs persistentes de `BaseStation`, layouts 16-bit/invalidación exacta y parents/children de layout NewGRF. |
| Stations | `0x149` `CBID_STATION_LAND_SLOPE_CHECK` | **parcial runtime** | Bit `SlopeCheck` de Action0 `0x0B`; Action3→Action2 por tesela en query+execute de `PlaceRailStation` / `PlaceRailStationArea`, antes de mutar. `param1` conserva slope+orientación; `param2` andenes/longitud/offsets. `FAILED`/`0x400` permite. Faltan scope de estación/vecinos, strings GRF y la inversión del bit 10 para GRF <8. |
| Stations | `0x140`–`0x142` anim | **parcial runtime** | Action0 `0x13`/`0x16`–`0x18`, frame `m7` por tesela y conjunto activo persistido en JSON. CB140 cubre `Built`/`TileLoop` por tesela, `NewCargo`/`CargoTaken` en el área completa y `VehicleLoads` de tren en la plataforma. `NewCargo` sale de transferencia, distribución industrial y producción de pueblo; `CargoTaken` sólo se emite al vaciar ese cargo de la estación, como upstream. `param2` usa ordinal en byte bajo y CTT Action0 GlobalVar `0x09` (o fallback de versión) en el alto. CB141/CB142 avanzan con `2^speed`, usan var `4A` también en la caché y hacen writeback `7C`. Faltan llegadas/salidas, aceptación, reserva de ruta, road stops, sonidos y scopes completos. |
| Vehicles (`00`–`03`) | `0x31` `CBID_VEHICLE_START_STOP_CHECK` | **soportado** | Call site: `toggle_vehicle_running_checked`; deniega → `NewGrfCallbackDenied` |
| Vehicles | `0x10`–`0x12`, `0x15`–`0x16`, `0x19`, `0x1D`, `0x23`, `0x2D`, `0x32`–`0x36`, … | **OOS** | Evaluador Action2 listo; sin call sites |
| Houses (`07`) | `0x17` `CBID_HOUSE_ALLOW_CONSTRUCTION` | **soportado** (#266) | Call site: crecimiento físico del pueblo (`try_build_town_house`), antes de reservar el footprint; respeta su máscara y booleano de 8 bits |
| Houses | resto `0x1A`–`0x1C`, `0x1E`–`0x21`, … | **almacenado** | `HouseSpecDef.callback_mask` |
| Industry tiles (`09`) | `0x25` trigger, `0x26` next frame, `0x27` speed | **soportado** (#293) | `phase_tile_animation` ejecuta los tres con coordenada real, `param2=IndustryTick`, máscara Action0 y fallback `CALLBACK_FAILED` |
| Industry tiles | `0x2B`–`0x2C`, … | **almacenado** | `IndustryTileSpecDef.callback_mask` |
| Industries (`0A`) | `0x28` `CBID_INDUSTRY_LOCATION` | **soportado** (#266) | Call site: `place_industry_spec_def_sandbox`; respeta el bit `Location`, carga Action3→Action2 y sólo permite `FAILED`/`0x400` |
| Industries | `0x22`, `0x29`, `0x35`, `0x37`–`0x3B`, `0x3D`, `0x14A`+, … | **almacenado** | `IndustrySpecDef.callback_mask` |
| Airport tiles (`11`) / Airports (`0D`) | anim / FTA-related | **almacenado** / **OOS** | Máscaras; FTA bloqueado (#260) |
| Canals (`05`) | `0x147` sprite offset | **almacenado** | `CanalSpecDef.callback_mask` |
| RoadStops (`14`) | `0x13` `CBID_STATION_AVAILABILITY` | **soportado** | Máscara Action0 `0x11`, Action2/3 y call site query+execute de `PlaceBusStop`/`PlaceTruckStop`; `CALLBACK_FAILED` o booleano 8-bit no nulo permite |
| RoadStops | `0x140`–`0x142` animación | **parcial runtime** | Action0 `0x0E`/`0x0F`/`0x10`; CB140 en `Built` y `TileLoop`, CB141/CB142 con frame/activo persistidos por parada. Faltan triggers de vehículo/carga, scopes vecinos, sonidos y selección visual dinámica |
| Objects (`0F`) | `0x157` `CBID_OBJECT_LAND_SLOPE_CHECK` | **parcial runtime** | Máscara Action0 `0x15` WORD, Action3→Action2 y call site query+execute de `BuildObject` por tesela. `param1=slope`, `param2=dy<<4\|dx`; faltan scopes completos de objeto/vecinos, string de error GRF y el fallback de pendiente completo de OpenTTD. |
| Cargoes (`0B`) | `0x39` `CBID_CARGO_PROFIT_CALC`; `0x145` `CBID_CARGO_STATION_RATING_CALC` | **parcial runtime** | Máscara Action0 `0x1A`, Action3→Action2: CB39 paga cada packet en `unload_vehicles` (`param1=0`, distancia/cantidad/tránsito, multiplicador signed-15); CB145 sustituye el target durante `update_station_ratings` (`param1` tipo histórico de vehículo; `param2` días/espera/velocidad, resultado signed-15). Faltan scopes avanzados y demás CBs. |
| Cargoes (resto) / Types | varios | **OOS** | Sin ejecución de CB en este corte |
| Generic | `0x01` `CBID_RANDOM_TRIGGER` | **OOS** | Ver triggers abajo |

## Storage

| Tipo | Ciclo de vida | Estado |
|---|---|---|
| Temporal (`7D` / `\2sto`) | Solo durante la evaluación Action2 | Descartado al terminar el ctx |
| Persistente (`7C` / `\2psto`) | Vehículo: `Vehicle.newgrf_persistent_regs` | Writeback tras CB; round-trip JSON save |
| Persistente estación | `Station.newgrf_persistent_regs` | **parcial**: API stateful + JSON round-trip; CB13 de construcción no puede hacer writeback porque OpenTTD lo evalúa sin estación/tesela |
| Persistente industria/casa/… | — | **OOS** (estación cubre el MVP no-vehículo) |

## Triggers / random

| Pieza | Estado |
|---|---|
| Industry tile `m3` random bits + `m6` triggers (reseed) | **Existe** (`industry_random.rs`, P7) |
| `ResolveRerandomisation` / Action2 random sprite groups por trigger | **soportado MVP** (#266): `resolve_industry_tile_random_trigger` reseedea + elige set |
| Vehicle/station random Action2 (`0x80`/`0x83`/`0x84`) en resolve de sprites | **Parcial** (eval con `random_bits`; reseed gameplay vía API #266) |
| `CBID_RANDOM_TRIGGER` genérico | **OOS** |

## Call sites soportados (checklist)

1. Stations CB24 — layout al construir.
2. Vehicles CB31 — start/stop check (+ writeback persistent regs).
3. Industries CB28 — location al colocar NewGRF (`place_industry_spec_def_sandbox`).
4. Houses CB17 — allow construction durante crecimiento físico (GRF Action0/2/3 cargado + call site).
5. Stations CB13 — availability en query+execute de construcción; sin storage en el scope nulo de OpenTTD.
6. Industry tiles CB25/CB26/CB27 — trigger, next frame y velocidad en `phase_tile_animation` (FAILED observable).
7. Industry tile trigger → Action2 random group (`resolve_industry_tile_random_trigger`).
8. RoadStops CB13 — disponibilidad al previsualizar y ejecutar `PlaceBusStop`/`PlaceTruckStop`.
9. RoadStops CB140/CB141/CB142 — `Built`/`TileLoop` + scheduler con velocidad/frame, writeback `7C` y JSON round-trip. Referencia: `newgrf_roadstop.cpp` / `newgrf_animation_base.h`.
10. Objects CB157 — pendiente por tesela de `BuildObject`, desde Action0 `0x15` y Action3→Action2 cargados; query y execute rechazan antes de mutar.
11. Cargoes CB39 — cálculo de pago por packet durante `unload_vehicles`, desde Action0 `0x1A` y Action3→Action2 cargados; `CALLBACK_FAILED` conserva la fórmula base.
12. Cargoes CB145 — target de rating durante el barrido `update_station_ratings`, desde Action0 `0x1A` y Action3→Action2 cargados; `CALLBACK_FAILED` conserva el algoritmo estándar.
13. Stations CB149 — pendiente por tesela al construir, desde Action0 `0x0B` y Action3→Action2 cargados; query y execute rechazan antes de mutar.
14. Stations CB14 — layout de tesela al dibujar, desde Action0 `0x0B` y Action3→Action2 cargados; el renderer el aplica antes de elegir la vista NewGRF.
15. Stations CB140/CB141/CB142 — `Built`/`TileLoop`, `NewCargo`/`CargoTaken` de área completa y `VehicleLoads` ferroviario por plataforma; scheduler persistido por tesela, velocidad/frame, CTT de cargo en `param2` y var Action2 `4A`. Aún no cubre llegadas/salidas, aceptación ni reservas.

## Residual explícito (no bloquea cierre MVP #266)

- Resto de CBs houses / airports / industries / objects (excepto CB157), cargo (excepto CB39/CB145). Stations aún requieren `VehicleArrives`/`VehicleDeparts`, `AcceptanceTick`, `PathReservation`, road stops, scopes completos y sonidos; RoadStops conserva todos los triggers de carga/vehículo/aceptación/reserva y sigue sin selección visual Action2 dinámica.
- Scopes parent/relative completos.
- Storage persistente en industria/casa y callbacks de estación que sí tengan scope de estación; CB140–142 preserva `7C` de la estación pero no los scopes/áreas completos de `BaseStation`; CB14 aún no aporta el scope/regs de `BaseStation` ni layout 16-bit exacto; CB149 aún no aporta scope/vecinos, strings GRF ni la compatibilidad de bit 10 para GRF <8.
- Goldens tick-a-tick vs OpenTTD 15.3 para todos los features.
- Textos GRF de string (`0x40F` / `regs100`) en CB31: denegación genérica `NewGrfCallbackDenied`.
