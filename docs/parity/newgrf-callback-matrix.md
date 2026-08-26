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
`trigger_road_stop_animation`/`_at`, `advance_road_stop_animation`/`_at`,
`trigger_road_stop_randomisation`/`_at`,
`resolve_vehicle_load_amount_callback`, `resolve_vehicle_sound_callback`,
`resolve_vehicle_refit_capacity_callback`,
`resolve_vehicle_length_callback`, `vehicle_unit_length`,
`decode_vehicle_articulated_part`, `resolve_vehicle_articulated_part_callback`,
`resolve_vehicle_visual_effect_callback`, `vehicle_visual_effect_kind`,
`resolve_industry_tile_animation_callback`,
`resolve_industry_tile_random_trigger`
(`crates/openttdrs-core/src/newgrf_callback.rs`).

## Por feature

| Feature | CBID (ejemplos) | Estado | Notas |
|---|---|---|---|
| Stations (`04`) | `0x24` `CBID_STATION_BUILD_TILE_LAYOUT` | **soportado** | Call site: construcción `apply_station_build_tile_layout_callback` |
| Stations | `0x13` `CBID_STATION_AVAILABILITY` | **soportado** | Máscara Action0 `0x0B`, Action3→Action2 y call site query+execute de `PlaceRailStation` / `PlaceRailStationArea`; scope de construcción sin estación/tesela, booleano de 8 bits |
| Stations | `0x14` `CBID_STATION_DRAW_TILE_LAYOUT` | **parcial runtime** | Bit `DrawTileLayout` de Action0 `0x0B`; el renderer lo ejecuta por tesela antes de elegir la vista Action1/3 y conserva el eje. Recibe el contexto Action2 existente de tesela. Faltan scope/regs persistentes de `BaseStation`, layouts 16-bit/invalidación exacta y parents/children de layout NewGRF. |
| Stations | `0x149` `CBID_STATION_LAND_SLOPE_CHECK` | **parcial runtime** | Bit `SlopeCheck` de Action0 `0x0B`; Action3→Action2 por tesela en query+execute de `PlaceRailStation` / `PlaceRailStationArea`, antes de mutar. `param1` conserva slope+orientación; `param2` andenes/longitud/offsets. `FAILED`/`0x400` permite. Faltan scope de estación/vecinos, strings GRF y la inversión del bit 10 para GRF <8. |
| Stations | `0x140`–`0x142` anim | **parcial runtime** | Action0 `0x13`/`0x16`–`0x18`, frame `m7` por tesela y conjunto activo persistido en JSON. CB140 cubre `Built`/`TileLoop` por tesela, `NewCargo`/`CargoTaken` y `AcceptanceTick` (`TA_WHOLE`, cada 250 ticks escalonado por StationID), y `VehicleLoads`/`VehicleArrives`/`VehicleDeparts`/`PathReservation` de tren en la plataforma. `VehicleArrives` se emite al abrir el `BeginLoading` local; `VehicleDeparts` se emite antes de mover tras cierre de carga, descarga o espera de horario; `PathReservation` sólo se emite al pasar una tesela de estación de no reservada a reservada, como `HasStationReservation` upstream. `NewCargo` sale de transferencia, distribución industrial y producción de pueblo; `CargoTaken` sólo se emite al vaciar ese cargo de la estación, como upstream. `param2` usa ordinal en byte bajo y CTT Action0 GlobalVar `0x09` (o fallback de versión) en el alto. CB141/CB142 avanzan con `2^speed`, usan var `4A` también en la caché y hacen writeback `7C`. El contexto de tesela materializa ahora vars `0x44` (estado PBS `2/4/7`), `0x45` (continuación de vía en vecinos), `0x46` (posición centrada), `0x47` (spec centrado homogéneo) y `0x49` (plataforma respetando eje), además de carga `60`–`65`/`69` con CTT/versión; `69` conserva y rota los cuatro flags de estado de `GoodsEntry`, y las reservas se sincronizan al bit `m6` nativo. La aceptación de `65` usa el catchment vivo para los cargos modelados cuando el caller aporta el pool de industrias, y conserva el fallback estático en APIs legacy. Faltan scopes completos y sonidos. |
| Vehicles (`00`–`03`) | `0x12` `CBID_VEHICLE_LOAD_AMOUNT` | **parcial runtime** | `vehicle_load_unload_speed` ejecuta el callback para carga gradual con `param1=0`; un byte no nulo sustituye `EngineInfo::load_amount`, mientras `CALLBACK_FAILED`, cero, resultado fuera de rango, máscara ausente o GRF ausente conservan la propiedad base y escriben `7C` del vehículo |
| Vehicles (`00`–`03`) | `0x11` `CBID_VEHICLE_LENGTH` | **parcial runtime** | `vehicle_unit_length` ejecuta el callback con `param1=param2=0` al comprar/refrescar una unidad; convierte el acortamiento `0..7` a longitud `8−shorten`, conserva `CALLBACK_FAILED`/resultados inválidos y usa la propiedad `shorten_factor` como fallback. La longitud se persiste en la unidad y participa en la geometría de consist; faltan callbacks de longitud dependientes de propiedades `0x36` y refresco diferencial de cadenas importadas |
| Vehicles (`00`–`03`) | `0x15` `CBID_VEHICLE_REFIT_CAPACITY` | **parcial runtime** | `refit_vehicle` evalúa el callback con el cargo objetivo en `Vehicle::cargo_type`; un resultado distinto de `CALLBACK_FAILED` fija la capacidad final (incluido cero), conserva el tipo original durante la consulta y escribe `7C`. Sin callback se aplica la propiedad `capacity` y el multiplicador de cargo; faltan cadenas articuladas, subtipo y la capacidad secundaria de aeronaves |
| Vehicles (`00`–`03`) | `0x16` `CBID_VEHICLE_ARTIC_ENGINE` | **parcial runtime** | `decode_vehicle_articulated_part` implementa la codificación upstream de 8 bits (GRF < 8: `0xFF`/bit 7) y 15 bits (GRF ≥ 8: `0x7FFF`/bit 14); `resolve_vehicle_articulated_part_callback` ejecuta el callback con `index` en `param1`, hace writeback de `7C` y devuelve el id local más el espejo. La compra y el autoreemplazo de trenes, buses, camiones y tranvías materializan la cadena hasta el terminador, enlazan `prev_unit`/`next_unit`, conservan los vagones/unidades del jugador y resuelven el catálogo activo. El movimiento vial procesa sólo la cabeza, persiste su historial de teselas y sincroniza las unidades generadas (incluido el estado oculto de depósito y separaciones de varias teselas); cada unidad conserva el bit de espejo y el renderer consulta la vista invertida, las crea como children de la cabeza y el índice de tráfico no las cuenta como vehículos independientes. Action0 y Action3 de vehículos aceptan ahora `ExtendedByte` (IDs locales hasta 14 bits), y esos IDs se conservan en el catálogo y la resolución runtime. Los grupos Action3 con bit de wagon override conservan la cadena de motores anterior y se resuelven por cargo/default para cada unidad cuyo GRFID coincide. Los Action2 deterministas `0x82/0x86/0x8A` ya consultan `parent_vars`, los random `0x83` usan `parent_random_bits` y `0x84` resuelve offsets relativos en ambos sentidos; el constructor del contexto de consist alimenta el padre inmediato y la cadena firmada. Siguen faltando scopes relativos especiales (primer vehículo con mismo ID), variables `61/62` con consulta recursiva y callbacks avanzados. |
| Vehicles (`00`–`03`) | `0x10` `CBID_VEHICLE_VISUAL_EFFECT` | **parcial runtime** | `resolve_vehicle_visual_effect_callback` ejecuta el callback con `param1=param2=0`, normaliza bits de tipo (vapor/diésel/chispa) y `VE_DISABLE_EFFECT`, y escribe `7C`. La propiedad Action0 (`train 0x22`, `road 0x21`, `ship 0x1C`) se conserva en el catálogo y actúa como fallback cuando el callback falla. El emisor de humo ferroviario usa el catálogo activo (incluidos motores NewGRF) y respeta el resultado; faltan el offset/potencia de vagones y `CBID_VEHICLE_SPAWN_VISUAL_EFFECT` avanzado. |
| Vehicles (`00`–`03`) | `0x31` `CBID_VEHICLE_START_STOP_CHECK` | **soportado** | Call site: `toggle_vehicle_running_checked`; deniega → `NewGrfCallbackDenied` |
| Vehicles | `0x33` `CBID_VEHICLE_SOUND_EFFECT` | **parcial runtime** | Action0 conserva la máscara (bit `SoundEffect`) y el callback recibe `VehicleSoundEvent` en `param1`; los eventos de salida, marcha (`Running`/`Running16`/`Stopped16`), avería, túnel, pago de carga/descarga y despegue/aterrizaje traducen samples globales `0..72` o locales `73+id` a la cola del mixer. `CALLBACK_FAILED` mantiene el SFX vanilla; id local inválido suprime el sonido como `INVALID_SOUND`. El efecto visual (humo/chispas) resuelve también el callback; los motores vanilla conocidos ya seleccionan su muestra específica y queda mapear el `sfx` custom de Action0 sin callback. |
| Vehicles | `0x16`, `0x19`, `0x1D`, `0x23`, `0x2D`, `0x32`, `0x34`–`0x36`, … | **OOS** | Evaluador Action2 listo; sin call sites |
| Houses (`07`) | `0x17` `CBID_HOUSE_ALLOW_CONSTRUCTION` | **soportado** (#266) | Call site: crecimiento físico del pueblo (`try_build_town_house`), antes de reservar el footprint; respeta su máscara y booleano de 8 bits |
| Houses | resto `0x1A`–`0x1C`, `0x1E`–`0x21`, … | **almacenado** | `HouseSpecDef.callback_mask` |
| Industry tiles (`09`) | `0x25` trigger, `0x26` next frame, `0x27` speed | **soportado** (#293) | `phase_tile_animation` ejecuta los tres con coordenada real, `param2=IndustryTick`, máscara Action0 y fallback `CALLBACK_FAILED` |
| Industry tiles | `0x2B`–`0x2C`, … | **almacenado** | `IndustryTileSpecDef.callback_mask` |
| Industries (`0A`) | `0x28` `CBID_INDUSTRY_LOCATION` | **soportado** (#266) | Call site: `place_industry_spec_def_sandbox`; respeta el bit `Location`, carga Action3→Action2 y sólo permite `FAILED`/`0x400` |
| Industries | `0x22`, `0x29`, `0x35`, `0x37`–`0x3B`, `0x3D`, `0x14A`+, … | **almacenado** | `IndustrySpecDef.callback_mask` |
| Airport tiles (`11`) / Airports (`0D`) | anim / FTA-related | **almacenado** / **OOS** | Máscaras; FTA bloqueado (#260) |
| Canals (`05`) | `0x147` sprite offset | **almacenado** | `CanalSpecDef.callback_mask` |
| RoadStops (`14`) | `0x13` `CBID_STATION_AVAILABILITY` | **soportado** | Máscara Action0 `0x11`, Action2/3 y call site query+execute de `PlaceBusStop`/`PlaceTruckStop`; `CALLBACK_FAILED` o booleano 8-bit no nulo permite |
| RoadStops | `0x140`–`0x142` animación + Action2 random | **parcial runtime** | Action0 `0x0D`/`0x0E`/`0x0F`/`0x10`; CB140 cubre `Built`/`TileLoop`, `NewCargo`/`CargoTaken`, `VehicleLoads`/`VehicleArrives`/`VehicleDeparts` vial y `AcceptanceTick`; CB141/CB142 conservan frame/activo y `7C` por parada. Cada tesela custom creada o recuperada desde JSON tiene spec/frame/activo/random propios, preservados al unir paradas; eventos de vehículo usan la tesela exacta y los de carga/aceptación recorren el stop completo. La ruta Action2 conserva la máscara `0x0D`, CTT/versión, bits de estación (16) + tesela (8), triggers pendientes y grupos random `any`/`all` alcanzables; ejecuta NewCargo, CargoTaken, carga, llegada y salida antes de CB140. El render y el scheduler resuelven Action3/Action2 por tesela con random/triggers, vista/tipo/terreno, road/tram (incluidos tipos externos del catálogo), frame, `param[]`, las vars de carga `60`–`65`/`69`, las vars de mundo `45` (zona/distancia a pueblo), `46` (distancia cuadrática) y `47` (propietario/AI/colores), y los offsets `66` (frame misma estación), `67` (land), `68` (info), `6A` (GRFID) y `6B` (id local): nibbles firmados, wrap de mapa y caché por `(var,param)` evitan congelar una variante vecina. `param2` lleva el ordinal del trigger y, para carga, el id CTT (o fallback histórico) en el byte alto. La randomización del scheduler recibe los pools de town/company/industries cuando corre desde `sim_step`; las APIs históricas sin catálogo conservan fallback vanilla, mientras que la ruta explícita `...with_catalog_and_road_types` y el renderer usan tipos externos. `69` conserva y rota los cuatro flags de `GoodsEntry`, mientras `65` usa el catchment vivo con pools de mundo y conserva el fallback estático en APIs legacy. La importación `.sav` ya decodifica `roadstopspeclist`/`roadstoptiledata`, conserva `(GRFID, localidx)`, frame y random por tesela y reata la spec cuando el catálogo NewGRF está disponible; los GRF no instalados siguen siendo no resolubles. |
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
| RoadStops Action0 `0x0D` + Action2 random | **parcial runtime**: CTT/versión, eventos `NewCargo`/`CargoTaken`/llegada/salida/carga vial, `any`/`all`, reseed de bits base/tesela, JSON y selección visual Action2 dinámica con el contexto persistente. El renderer y los CB140–142 resuelven vars de mundo `45`/`46`/`47`, vars de carga `60`–`65`/`69` y `66`/`67`/`68`/`6A`/`6B` por offset; la randomización del scheduler también usa esos scopes y recibe los pools del mundo en `sim_step`. Spec/frame/activo/random son independientes por tesela en stops compuestos creados, JSON o importados desde `.sav` (`roadstopspeclist` + `roadstoptiledata`). El cliente reproduce los samples `NewGRF` de la cola global y los callbacks de sonido de vehículo cubren salida, marcha, avería, túnel, efecto visual, pago carga/descarga y despegue/aterrizaje; quedan las APIs legacy sin catálogo y GRF ausentes.
| `CBID_RANDOM_TRIGGER` genérico | **OOS** |

## Call sites soportados (checklist)

1. Stations CB24 — layout al construir.
2. Vehicles CB31 — start/stop check (+ writeback persistent regs).
3. Vehicles CB11 — longitud efectiva al comprar/refrescar unidades (+ fallback `shorten_factor`).
4. Vehicles CB15 — capacidad de refit en `RefitVehicle` (+ writeback persistent regs).
5. Vehicles CB10 — efecto visual al emitir humo/chispas; el renderer usa el catálogo activo y respeta tipo/desactivación del callback (+ writeback persistent regs).
6. Industries CB28 — location al colocar NewGRF (`place_industry_spec_def_sandbox`).
7. Houses CB17 — allow construction durante crecimiento físico (GRF Action0/2/3 cargado + call site).
8. Stations CB13 — availability en query+execute de construcción; sin storage en el scope nulo de OpenTTD.
9. Industry tiles CB25/CB26/CB27 — trigger, next frame y velocidad en `phase_tile_animation` (FAILED observable).
10. Industry tile trigger → Action2 random group (`resolve_industry_tile_random_trigger`).
11. RoadStops CB13 — disponibilidad al previsualizar y ejecutar `PlaceBusStop`/`PlaceTruckStop`.
12. RoadStops CB140/CB141/CB142 — `Built`/`TileLoop`, carga/retirada de carga, carga/llegada/salida vial y `AcceptanceTick`; scheduler con velocidad/frame, writeback `7C`, CTT en `param2` y JSON round-trip. Cada tesela custom conserva spec/frame/activo/random, incluso tras `JoinStation`; los eventos exactos y de área completa siguen el alcance de `RoadStopTileData`. Action0 `0x0D` + Action2 random reacciona a NewCargo, CargoTaken, carga, llegada y salida con grupos `any`/`all`, bits base/tesela y triggers persistentes. Referencia: `newgrf_roadstop.cpp` / `newgrf_animation_base.h`.
13. Objects CB157 — pendiente por tesela de `BuildObject`, desde Action0 `0x15` y Action3→Action2 cargados; query y execute rechazan antes de mutar.
14. Cargoes CB39 — cálculo de pago por packet durante `unload_vehicles`, desde Action0 `0x1A` y Action3→Action2 cargados; `CALLBACK_FAILED` conserva la fórmula base.
15. Cargoes CB145 — target de rating durante el barrido `update_station_ratings`, desde Action0 `0x1A` y Action3→Action2 cargados; `CALLBACK_FAILED` conserva el algoritmo estándar.
16. Stations CB149 — pendiente por tesela al construir, desde Action0 `0x0B` y Action3→Action2 cargados; query y execute rechazan antes de mutar.
17. Stations CB14 — layout de tesela al dibujar, desde Action0 `0x0B` y Action3→Action2 cargados; el renderer el aplica antes de elegir la vista NewGRF.
18. Stations CB140/CB141/CB142 — `Built`/`TileLoop`, `NewCargo`/`CargoTaken` y `AcceptanceTick` de área completa (250 ticks escalonada por StationID), y `VehicleLoads`/`VehicleArrives`/`VehicleDeparts`/`PathReservation` ferroviarios por plataforma; scheduler persistido por tesela, velocidad/frame, CTT de cargo en `param2` y var Action2 `4A`.

## Residual explícito (no bloquea cierre MVP #266)

- Resto de CBs houses / airports / industries / objects (excepto CB157), cargo (excepto CB39/CB145). Stations aún requieren scopes completos y sonidos propios de tesela; el callback de sonido de vehículo ya cubre salida, marcha, avería, túnel, efecto visual, carga/descarga y despegue/aterrizaje. RoadStops resuelve `45`/`46`/`47`, `60`–`65`/`69` y `66`/`67`/`68`/`6A`/`6B` al renderizar, en CB140–142 y en la randomización con pools de mundo. La importación `.sav` conserva el mapeo nativo `(GRFID, localidx)` y el estado de cada tesela; la API legacy sin catálogo mantiene fallback vanilla y un GRF ausente no puede reatajarse a una vista ejecutable.
- Scopes parent determinista/random y offsets relativos básicos de vehículos ya están cubiertos; siguen pendientes el relativo especial por primer motor, variables `61/62` recursivas y scopes parent de estación/industria/casa/objeto con sus storage propios.
- Storage persistente en industria/casa y callbacks de estación que sí tengan scope de estación; CB140–142 preserva `7C` de la estación pero no los scopes/áreas completos de `BaseStation`; CB14 aún no aporta el scope/regs de `BaseStation` ni layout 16-bit exacto; CB149 aún no aporta scope/vecinos, strings GRF ni la compatibilidad de bit 10 para GRF <8.
- Goldens tick-a-tick vs OpenTTD 15.3 para todos los features.
- Textos GRF de string (`0x40F` / `regs100`) en CB31: denegación genérica `NewGrfCallbackDenied`.
