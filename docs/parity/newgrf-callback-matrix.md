# Matriz de callbacks NewGRF (CBID) — OpenTTD 15.3

Actualizada: **2026-09-04** (commit `7782568d`, CTT de cargos custom en scopes
de estación/parada,
shape-check, foundations,
autoslope, color, rechazo temporal, cargos dinámicos, efectos especiales,
`PlantOnBuild`, rehidratación SAV legacy, historiales aceptados runtime,
reatachación de industrias al
catálogo NewGRF; triggers, PSA parent, aceptación exacta de carga de teselas y
`CargoTypesUnlimited` y el transporte runtime de hasta 32 cargos custom ya
publicados; IDs globales SAV `SLV≥55` y slots climáticos legacy `SLV<55`
también se distinguen al hidratar `INDY`/`STNN`/`VEHS`/`LGRP`).

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
`apply_object_slope_callback`/`apply_object_slope_callback_for_build`,
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
`resolve_vehicle_modify_property_callback`,
`trigger_vehicle_randomisation_chain`,
`decode_vehicle_articulated_part`, `resolve_vehicle_articulated_part_callback`,
`resolve_vehicle_visual_effect_callback`, `vehicle_visual_effect_kind`,
`resolve_industry_tile_animation_callback`,
`resolve_industry_tile_animation_callback_with_world`,
`trigger_newgrf_industry_animation_with_world_and_extra`,
`resolve_industry_tile_cargo_acceptance_callback_with_world`,
`apply_industry_tile_shape_callback_for_build`,
`advance_industry_tile_randomisation_from_visits_with_catalog_and_world`,
`resolve_industry_tile_random_trigger`
(`crates/openttdrs-core/src/newgrf_callback.rs`).

AirportTile animation uses `trigger_newgrf_airport_tile_animation`,
`trigger_newgrf_airport_animation_for_station` and
`step_newgrf_airport_tiles` (`crates/openttdrs-core/src/map/station_tile_anim.rs`).

## Por feature

Corrección de la fila `Industry tiles (09)` en este corte (`bd613e2a`, sobre
`67ef8101`): además de
los tres eventos ya descritos, `CargoDistributed` se dispara tras una
transferencia efectiva y `ConstructionStageChanged` se dispara al crear una
industria (con `var 18 |= 0x100`) y después de cada cambio de etapa. Ambos
call sites usan el contexto parent/PSA de la industria; la pasada visual CB26/
CB27 sigue separada. La aceptación exacta de carga de teselas también se evalúa
en runtime: CB2C selecciona los tres slots locales y CB2B sus cantidades, y la
tabla resultante alimenta la cobertura de estación y `unload_vehicles`. La API
legacy sin mundo mantiene su fallback explícito.

| Feature | CBID (ejemplos) | Estado | Notas |
|---|---|---|---|
| Stations (`04`) | `0x24` `CBID_STATION_BUILD_TILE_LAYOUT` | **soportado** | Call site: construcción `apply_station_build_tile_layout_callback` |
| Stations | `0x13` `CBID_STATION_AVAILABILITY` | **soportado** | Máscara Action0 `0x0B`, Action3→Action2 y call site query+execute de `PlaceRailStation` / `PlaceRailStationArea`; scope de construcción sin estación/tesela, booleano de 8 bits |
| Stations | `0x14` `CBID_STATION_DRAW_TILE_LAYOUT` | **parcial runtime** | Bit `DrawTileLayout` de Action0 `0x0B`; el renderer lo ejecuta por tesela antes de elegir la vista Action1/3 y conserva el eje. Los layouts `TileSeq` completos se resuelven por Action3/2→Action1, reemplazan el suelo, emiten parents `M(...)` y children relativos después de la catenaria y comparten fingerprint de registros `7D`/`0x100`; sprites base, paletas custom y layouts incompletos usan fallback vanilla atómico. Faltan scope/regs persistentes de `BaseStation`, layouts 16-bit/invalidación exacta y callbacks/sonidos de estación. |
| Stations | `0x149` `CBID_STATION_LAND_SLOPE_CHECK` | **parcial runtime** | Bit `SlopeCheck` de Action0 `0x0B`; Action3→Action2 por tesela en query+execute de `PlaceRailStation` / `PlaceRailStationArea`, antes de mutar. `param1` conserva slope+orientación; `param2` andenes/longitud/offsets. `FAILED`/`0x400` permite en GRF ≥8 y se aplica la inversión de bit 10 para GRF <8. Faltan scope de estación/vecinos y strings GRF. |
| Stations | `0x140`–`0x142` anim | **parcial runtime** | Action0 `0x13`/`0x16`–`0x18`, frame `m7` por tesela y conjunto activo persistido en JSON. CB140 cubre `Built`/`TileLoop` por tesela, `NewCargo`/`CargoTaken` y `AcceptanceTick` (`TA_WHOLE`, cada 250 ticks escalonado por StationID), y `VehicleLoads`/`VehicleArrives`/`VehicleDeparts`/`PathReservation` de tren en la plataforma. `VehicleArrives` se emite al abrir el `BeginLoading` local; `VehicleDeparts` se emite antes de mover tras cierre de carga, descarga o espera de horario; `PathReservation` sólo se emite al pasar una tesela de estación de no reservada a reservada, como `HasStationReservation` upstream. `NewCargo` sale de transferencia, distribución industrial y producción de pueblo; `CargoTaken` sólo se emite al vaciar ese cargo de la estación, como upstream. `param2` usa ordinal en byte bajo y CTT Action0 GlobalVar `0x09` (o fallback de versión) en el alto. CB141/CB142 avanzan con `2^speed`, usan var `4A` también en la caché y hacen writeback `7C`. El contexto de tesela materializa ahora vars `0x44` (estado PBS `2/4/7`), `0x45` (continuación de vía en vecinos), `0x46` (posición centrada), `0x47` (spec centrado homogéneo) y `0x49` (plataforma respetando eje), además de carga `60`–`65`/`69` con CTT/versión; `69` conserva y rota los cuatro flags de estado de `GoodsEntry`, y las reservas se sincronizan al bit `m6` nativo. La aceptación de `65` usa el catchment vivo para los cargos modelados cuando el caller aporta el pool de industrias, y conserva el fallback estático en APIs legacy. Faltan scopes completos y sonidos. |
| Vehicles (`00`–`03`) | `0x12` `CBID_VEHICLE_LOAD_AMOUNT` | **parcial runtime** | `vehicle_load_unload_speed` ejecuta el callback para carga gradual con `param1=0`; un byte no nulo sustituye `EngineInfo::load_amount`, mientras `CALLBACK_FAILED`, cero, resultado fuera de rango, máscara ausente o GRF ausente conservan la propiedad base y escriben `7C` del vehículo |
| Vehicles (`00`–`03`) | `0x11` `CBID_VEHICLE_LENGTH` | **parcial runtime** | `vehicle_unit_length` ejecuta el callback con `param1=param2=0` al comprar/refrescar una unidad; convierte el acortamiento `0..7` a longitud `8−shorten`, conserva `CALLBACK_FAILED`/resultados inválidos y usa la propiedad `shorten_factor` como fallback. La longitud se persiste en la unidad y participa en la geometría de consist; faltan callbacks de longitud dependientes de propiedades `0x36` y refresco diferencial de cadenas importadas |
| Vehicles (`00`–`03`) | `0x15` `CBID_VEHICLE_REFIT_CAPACITY` | **parcial runtime** | `refit_vehicle` evalúa el callback con el cargo objetivo en `Vehicle::cargo_type`; un resultado distinto de `CALLBACK_FAILED` fija la capacidad final (incluido cero), conserva el tipo original durante la consulta y escribe `7C`. Sin callback se aplica la propiedad `capacity` y el multiplicador de cargo; faltan cadenas articuladas, subtipo y la capacidad secundaria de aeronaves |
| Vehicles (`00`–`03`) | `0x16` `CBID_VEHICLE_ARTIC_ENGINE` | **parcial runtime** | `decode_vehicle_articulated_part` implementa la codificación upstream de 8 bits (GRF < 8: `0xFF`/bit 7) y 15 bits (GRF ≥ 8: `0x7FFF`/bit 14); `resolve_vehicle_articulated_part_callback` ejecuta el callback con `index` en `param1`, hace writeback de `7C` y devuelve el id local más el espejo. La compra y el autoreemplazo de trenes, buses, camiones y tranvías materializan la cadena hasta el terminador, enlazan `prev_unit`/`next_unit`, conservan los vagones/unidades del jugador y resuelven el catálogo activo. El movimiento vial procesa sólo la cabeza, persiste su historial de teselas y sincroniza las unidades generadas (incluido el estado oculto de depósito y separaciones de varias teselas); cada unidad conserva el bit de espejo y el renderer consulta la vista invertida, las crea como children de la cabeza y el índice de tráfico no las cuenta como vehículos independientes. Action0 y Action3 de vehículos aceptan ahora `ExtendedByte` (IDs locales hasta 14 bits), y esos IDs se conservan en el catálogo y la resolución runtime. Los grupos Action3 con bit de wagon override conservan la cadena de motores anterior y se resuelven por cargo/default para cada unidad cuyo GRFID coincide. Los Action2 deterministas `0x82/0x86/0x8A` ya consultan `parent_vars`, los random `0x83` usan `parent_random_bits` y `0x84` resuelve offsets relativos en ambos sentidos, incluido el tramo especial que empieza en el primer vehículo contiguo con el mismo motor; el constructor del contexto de consist alimenta el padre inmediato, la cadena firmada y ese tramo. Variable `61` puede consultar var `62` con un segundo offset relativo del vehículo seleccionado y var `0x60` cuenta los IDs locales presentes desde esa unidad; las variables de badges `0x64`/`0x65`/`0x7A` se traducen mediante GlobalVar `0x18`, y `0x65` consulta el tipo de vía de la tesela; siguen pendientes los callbacks avanzados. |
| Vehicles (`00`–`03`) | `0x10` `CBID_VEHICLE_VISUAL_EFFECT` | **parcial runtime** | `resolve_vehicle_visual_effect_callback` ejecuta el callback con `param1=param2=0`, normaliza bits de tipo (vapor/diésel/chispa) y `VE_DISABLE_EFFECT`, y escribe `7C`. La propiedad Action0 (`train 0x22`, `road 0x21`, `ship 0x1C`) se conserva en el catálogo y actúa como fallback cuando el callback falla. El emisor de humo ferroviario usa el catálogo activo (incluidos motores NewGRF) y respeta el resultado; faltan el offset/potencia de vagones y `CBID_VEHICLE_SPAWN_VISUAL_EFFECT` avanzado. |
| Vehicles (`00`–`03`) | `0x31` `CBID_VEHICLE_START_STOP_CHECK` | **soportado** | Call site: `toggle_vehicle_running_checked`; deniega → `NewGrfCallbackDenied` |
| Vehicles (`00`–`03`) | `0x32` `CBID_VEHICLE_32DAY_CALLBACK` | **parcial runtime** | El barrido económico escalonado conserva `day_counter` y ejecuta el callback al inicio y cada 32 días por unidad; decodifica los bits de randomización/paleta y expone los bits desconocidos para diagnóstico. El trigger `Callback32` persiste, consume la máscara del grupo Action2 activo, conserva los 16 bits nativos de `random_bits` y reseedea los bits declarados de forma determinista. `trigger_vehicle_randomisation_chain` propaga `NewCargo`/`AnyNewCargo`, `Depot` y `Empty` por `prev_unit`/`next_unit` con la palabra base correcta. La simulación conecta `NewCargo` al primer lote cargado desde industria/estación, `Empty` al vaciado completo del consist y `Depot` al borde de entrada de trenes, barcos, carretera y aeronaves (hangar); la invalidación de paleta incrementa una generación visual que forma parte de la huella de caché. |
| Vehicles | `0x33` `CBID_VEHICLE_SOUND_EFFECT` | **parcial runtime** | Action0 conserva la máscara (bit `SoundEffect`) y el callback recibe `VehicleSoundEvent` en `param1`; los eventos de salida, marcha (`Running`/`Running16`/`Stopped16`), avería, túnel, pago de carga/descarga y despegue/aterrizaje traducen samples globales `0..72` o locales `73+id` a la cola del mixer. `CALLBACK_FAILED` mantiene el SFX vanilla o el `sound_effect` de Action0; id local inválido suprime el sonido como `INVALID_SOUND`. El efecto visual (humo/chispas) resuelve también el callback; los motores vanilla conocidos ya seleccionan su muestra específica. Faltan sólo los eventos ambientales y las tablas de sonido fuera del alcance de vehículos. |
| Vehicles (`00`–`03`) | `0x2D` `CBID_VEHICLE_COLOUR_MAPPING` | **parcial runtime** | El callback se consulta cuando Action0 declara la máscara `ColourRemap` (bit 6), conserva el `PaletteID` y el bit de colores de compañía sin mutar el vehículo, y la caché de sprites runtime aplica las paletas `775..790`, el rango 2CC (`SPR_2CCMAP_BASE + primary + secondary * 16`) y crash (`804`). Los mapas Action5 `0x0A` se cargan como tablas de 256 índices cuando están instalados; IDs no materializables permanecen raw. Sigue pendiente la invalidación global en consumidores fuera del renderer de vehículos y la cobertura de scopes/call sites restantes. |
| Vehicles (`00`–`03`) | `0x36` `CBID_VEHICLE_MODIFY_PROPERTY` | **parcial runtime** | `resolve_vehicle_modify_property_callback` ejecuta el resultado signed/unsigned de 15 bits y hace visible `CALLBACK_FAILED`. `vehicle_unit_length` consume las propiedades `0x21` (tren) y `0x23` (carretera) como acortamiento cuando falta CB11. La velocidad efectiva consulta ahora `0x09` (tren), `0x15` (carretera), `0x0B` (barco) y `0x0C` (aeronave) en el controlador de movimiento, pendientes, crucero, adelantamiento y rating de estación, con writeback de `7C`; la ruta de movimiento común y el controlador naval resuelven el motor desde el catálogo activo. En barcos, `Ship::UpdateCache` aplica la fracción de mar/canal después de `PROP_SHIP_SPEED`, igual que `OpenTTD`; la regresión `ship_cb36_speed_is_fractioned_after_callback` evita volver a la velocidad Action0 cuando CB36 es dinámico. La predicción ferroviaria de salida de tesela y la comprobación de señales/depot también resuelven `PROP_TRAIN_SPEED` (`0x09`) desde ese catálogo sobre una copia del vehículo, por lo que un callback reductivo afecta el tick especulativo sin duplicar writeback; `train_tile_prediction_uses_newgrf_speed_property` fija esa diferencia. Las operaciones de depósito y la importación SAV refrescan `ConsistChanged` con el catálogo activo después de acoplar, desacoplar, mover o vender unidades; `attach_newgrf_wagon_refreshes_callback_consist_cache` verifica que capacidad/velocidad/potencia de una cadena NewGRF no quede en la caché vanilla. `ReplaceChain` vuelve a evaluar capacidad CB36 y el multiplicador de cargo inmediatamente al autoreemplazar, incluidas traseras dual-head y unidades articuladas; `autoreplace_applies_newgrf_capacity_before_leaving_depot` fija que no se conserve la capacidad vieja durante el tick de depósito. La capacidad consulta `0x14`/`0x0F`/`0x0D`/`0x0F` (o `0x11` para correo aéreo) al comprar, articular, refitar, autoreemplazar y en cada fase `LoadUnloadStation`, refrescando la suma del consist y las unidades articuladas y conservando resultados válidos iguales a cero. Potencia (`0x0B`/`0x13`), peso (`0x16`/`0x14`) y esfuerzo tractor (`0x1F`/`0x18`) se convierten desde sus unidades nativas y participan del recálculo físico ferroviario y del tick vial con catálogo activo, incluida la comparación de velocidad del adelantamiento; los factores `0x17`/`0x11`/`0x0A`/`0x0B` de compra y `0x0D`/`0x09`/`0x0F`/`0x0E` de explotación se aplican al construir, vender, autoreemplazar, valorar activos y cobrar el coste anual. Las piezas articuladas viales no aportan motor. `CALLBACK_FAILED` conserva la propiedad Action0. Faltan APIs legacy sin catálogo, otras propiedades Action0 y la semántica completa de cadenas articuladas. |
| RailTypes (`06`) | Action3 `RailSpriteType::Depot` (`RTSG_DEPOT`, selector `8`) | **parcial runtime** | `apply_newgrf_rail_signals` conserva un grupo por `RailType`; el renderer consume el bloque relocatable desde `SPR_RAIL_DEPOT_SE_1` en el orden nativo `SE_1`, `SE_2`, `SW_1`, `SW_2`, `NE`, `NW`, resuelve Action2 con variables de tesela/fecha/random y conserva offsets NFO, profundidad y children de `FOUNDATION_LEVELED`. Una vista ausente cae sólo esa capa al sprite OpenGFX. |
| RailTypes (`06`) | Action3 `RailSpriteType::Tunnel` / `TunnelPortal` (`RTSG_TUNNEL`, selector `3`; `RTSG_TUNNEL_PORTAL`, selector `10`) | **parcial runtime** | El parser conserva ambos grupos por `RailType`; en bocas con `UsesOverlay()` el renderer resuelve `RTSG_TUNNEL` por dirección y dibuja la superficie custom con ancla NFO como `DrawGroundSprite`, manteniendo PBS, catenaria y separadores, aunque el grupo de superficie es independiente del portal. La base de césped Action5 `0x17` (`SPR_RAILTYPE_TUNNEL_BASE`) se extrae por `LandscapeType` y slots normal/nieve, y la fachada `RTSG_TUNNEL_PORTAL` se resuelve por dirección con centro NFO como child sortable de esa base; una vista/asset ausente cae atómicamente a OpenGFX sólo para esa capa. Aún faltan pendientes/rotaciones y paletas especiales. |
| Vehicles | `0x16`, `0x19`, `0x1D`, `0x23`, `0x34`–`0x35`, … | **OOS** | Evaluador Action2 listo; sin call sites |
| Houses (`07`) | `0x17` `CBID_HOUSE_ALLOW_CONSTRUCTION` | **soportado** (#266) | Call site: crecimiento físico del pueblo (`try_build_town_house`), antes de reservar el footprint; respeta su máscara y booleano de 8 bits. El resolver recibe la tesela candidata y el `TownScopeResolver` parent real; los grupos Action2 parent escriben `\2psto` al PSA del pueblo por GRFID y el writer puede reemitir `CITY.psa_list`/`PSAC`. |
| Houses (`07`) | `0x150` `CBID_HOUSE_DRAW_FOUNDATIONS` | **parcial runtime** | El renderer evalúa el callback con el scope de casa (etapa, edad, random, pueblo y parámetros GRF) antes de `FOUNDATION_LEVELED`; `CALLBACK_FAILED` conserva la fundación y un resultado cero la suprime, como `ConvertBooleanCallback` upstream. La regresión `newgrf_house_draw_foundations_callback_can_suppress_default` cubre una casa inclinada custom sin parent vanilla. Falta el resto de callbacks de casa y layouts/rotaciones avanzados. |
| Houses | resto `0x1A`–`0x1C`, `0x1E`–`0x21`, … | **almacenado** | `HouseSpecDef.callback_mask` |
| Houses | Action2 `TileLayoutSpriteGroup` (`TileSeq`) | **parcial runtime** | El catálogo conserva el grafo aunque la casa no tenga callbacks variables. El renderer resuelve la etapa/edad/terreno/random de la tesela y `0x42` consulta la zona del pueblo cuyo `TownID` está persistido en `MAP2` (con fallback al más cercano en mapas legacy); `0x44`, `0x60`/`0x61` calculan conteos por `HouseID` global y por pueblo desde una instantánea del mapa, y `0x62`/`0x63` consultan información/frame de teselas vecinas con wrap. Cuando el pueblo tiene una fila `CITY.psa_list`, el scope parent copia sus registros `7C` por GRFID para que `0x7C` lea el mismo PSA que OpenTTD. Sustituye `s1`/`s2` cuando el layout es completo y materializa ground, parents y children con cajas `M(...)`, incluyendo la superficie de una fundación nivelada; layouts incompletos, sprites base y paletas especiales mantienen fallback vanilla atómico. El writeback de `7C` está cubierto en CB17 de construcción; faltan el callback de dibujo/tesela, conteos por clase, aceptación de estaciones y layouts 16-bit completos.
| Industry tiles (`09`) | `0x25` trigger, `0x26` next frame, `0x27` speed | **parcial runtime** | `phase_tile_animation` separa el disparador de la pasada visual: `TileLoop` se ejecuta sólo para teselas visitadas, `IndustryTick` al intervalo de producción y `CargoReceived` después de procesar la entrega; CB25 recibe el ordinal correspondiente y la ruta con pools de mundo construye el scope completo de tesela/industria y escribe `\\2psto` parent en `Industry.newgrf_persistent_regs`. CB26/CB27 avanzan sólo teselas activas y respetan `animation_speed`/`animation_frames`. `CargoDistributed` se dispara después de una transferencia efectiva y `ConstructionStageChanged` al construir y cambiar de etapa; ambos usan el mismo contexto parent/PSA. La API legacy conserva el trigger IndustryTick para herramientas antiguas y no puede hacer writeback de una industria ausente |
| Industry tiles (`09`) | `0x2F` `CBID_INDTILE_SHAPE_CHECK` | **parcial runtime** | `prop 0x0E` activa el callback durante `place_industry_spec_def_layout_sandbox`, una vez por tesela del layout y antes de escribir el mapa; `param1=0` y `param2=(creation_type << 8) | layout_index`, con parent temporal que conserva huella, tipo, random y fundador. `CALLBACK_FAILED` cae a `prop 0x0D` (`slopes_refused`) mediante `IsSlopeRefused`; GRF <7 usa el booleano invertido y GRF ≥7 acepta sólo `0x400`. Las regresiones de máscaras, parser y semántica cubren el contrato. Faltan otros tipos de creación, mensajes de error y el call site de generación automática. |
| Industry tiles (`09`) | `0x30` `CBID_INDTILE_DRAW_FOUNDATIONS` | **parcial runtime** | El renderer evalúa el callback con el scope de tesela/industria padre (etapa, random, terreno, pueblo, posición y parámetros GRF) antes de `FOUNDATION_LEVELED`; `CALLBACK_FAILED` conserva la fundación y cero la suprime, igual que `DrawNewIndustryTile` upstream. La regresión `newgrf_industry_draw_foundations_callback_can_suppress_default` cubre una tesela inclinada custom sin cimiento vanilla. Faltan callbacks de sonido, autoslope y scopes avanzados. |
| Industry tiles (`09`) | `0x3C` `CBID_INDTILE_AUTOSLOPE` | **parcial runtime** | `IndustryTileSpecDef` conserva la máscara `IndustryTileCallbackMask::Autoslope`; `raise_land`, `lower_land` y `level_land` ejecutan ahora el callback sólo cuando la pendiente vieja/nueva no es empinada y el máximo absoluto se conserva, siguiendo `TerraformTile_Industry`. `CALLBACK_FAILED` o cero permiten el autoslope; cualquier resultado no nulo deja que la limpieza normal rechace la tesela. El contexto usa la industria viva, asocia por `m2`/huella y escribe de vuelta el PSA `7C`; las regresiones cubren semántica y rechazo. La generación automática aún no invoca este call site. |
| Industry tiles | Action2 vars `0x40`–`0x44`, `0x60`–`0x62`, `0x7A` | **parcial runtime** | El renderer, shape-check, autoslope y los callbacks de animación construyen el scope por tesela con `m3` como random (incluidos triggers), etapa de obra, terreno, zona del pueblo más cercano, posición relativa, frame completo `m3hi`, información/frame/id de teselas vecinas y presencia de badges mediante GlobalVar `0x18`; el scope parent comparte stock/producción/historial de la `Industry` y sus registros PSA. El mismo contexto alimenta vistas planas y layouts `TileSeq`, con caché por fingerprint. `0x62` conserva los sentinelas `0xFFFF`/`0xFFFE` y traduce el local del mismo GRF. Faltan el resto de variables específicas del tile, callbacks de sonido y el call site de generación automática. |
| Industry tiles (`09`) | `0x2B` `CBID_INDTILE_CARGO_ACCEPTANCE`; `0x2C` `CBID_INDTILE_ACCEPT_CARGO` | **parcial runtime** | `IndustryTileSpecDef` conserva ambas máscaras y `AcceptsAllCargo`. CB2C se ejecuta primero y desempaqueta tres cargos locales de 5 bits; CB2B desempaqueta tres cantidades de 4 bits. El contexto incluye tesela, industria padre, CTT y PSA con writeback. La cobertura exacta por tesela llega a `station_coverage_at_with_newgrf` y la descarga la consulta; `CALLBACK_FAILED`/máscara ausente conserva el fallback estático. Cargos custom no resolubles y scopes restantes siguen pendientes. |
| Industries (`0A`) | `0x28` `CBID_INDUSTRY_LOCATION` | **parcial runtime** (#266) | Call site: `place_industry_spec_def_layout_sandbox` (la variante histórica usa layout 0); respeta el bit `Location`, valida y materializa el layout elegido, carga Action3→Action2, pasa `IACT_USERCREATION` (`param2=2`) y expone el scope de construcción con `0x7A` (badges), `0x80`/`0x81` (TileIndex), `0x82` (pueblo), `0x86` (layout cero-based), `0x87` (terreno), `0x88` (zona), `0x89`/`0x8D` (distancia), `0x8A` (altura), `0x8B` (distancia a agua) y `0x8F` (random). La instancia conserva `selected_layout` uno-based, `random`, fundador, fecha/tipo de construcción, año de última producción y flags para el scope padre y `INDY`; se permiten sólo `FAILED`/`0x400`. Siguen pendientes otros tipos de creación, strings de error GRF y la semántica de callbacks de GameScript |
| Industries | `0x29` `CBID_INDUSTRY_PRODUCTION_CHANGE`; `0x35` `CBID_INDUSTRY_MONTHLYPROD_CHANGE`; `0x14A` `CBID_INDUSTRY_DECIDE_COLOUR`; `0x15F` `CBID_INDUSTRY_PROD_CHANGE_BUILD` | **parcial runtime** | CB29 se ejecuta en el cambio diario de una industria NewGRF y CB35 después de actualizar las estadísticas mensuales; se decodifican `no-op`, halve/double, divide/multiply, increment/decrement, cierre, cambio estándar y set de `prod_level` vía registro `0x100`. CB15F fija el nivel inicial al fundar si el resultado está en `PRODLEVEL_MINIMUM..MAXIMUM`. CB14A se ejecuta al fundar, acepta sólo un resultado con bits 4..14 en cero y aplica el nibble de color; `CALLBACK_FAILED`/resultados inválidos conservan el color sorteado. Los Action2 `IndustryProductionSpriteGroup` v0/v1/v2 se parsean, se conservan y se resuelven a través de Action3→Action2 (incluidos grupos random/variational). Textos y la escala `ProdMultiHandling` siguen pendientes. |
| Industries | `0x3D` `CBID_INDUSTRY_REFUSE_CARGO` | **parcial runtime** | El callback se ejecuta para cada entrada de una procesadora NewGRF después de comprobar que el lote requerido está disponible y antes de retirarlo de las estaciones. `param1=0` y `param2` recibe el índice de cargo local traducido por el label CTT; un resultado no nulo acepta y cero rechaza (OpenTTD invierte `ConvertBooleanCallback`); `CALLBACK_FAILED`, runtime ausente o cargo no traducible conserva la aceptación. La ruta normal de descarga ya materializa `DeliverGoodsToIndustry`: ordena por `DistanceMax`, excluye la industria de origen, recorre destinos hasta agotar la carga o `u16::MAX`, y registra `last_accepted`/`was_cargo_delivered`; los destinos se disparan después de `load_vehicles`, con producción vanilla, CB1 de llegada o diferimiento CB2 exclusivo. La negativa deja intacto el stock de estación y no dispara CB1/CB2 ni `CargoReceived`. Los historiales aceptados y producidos por salida y el monitor runtime `AddCargoDelivery` se actualizan y se reemiten para cargos representables; faltan `exclusive_supplier`/neutral stations, bindings de GameScript, aceptación exacta de estaciones y cargos custom. |
| Industries | `0x14B` `CBID_INDUSTRY_INPUT_CARGO_TYPES`; `0x14C` `CBID_INDUSTRY_OUTPUT_CARGO_TYPES` | **parcial runtime** | Al fundar una industria con runtime Action2, los callbacks se consultan por slot (`param1`, `param2=0`) y reemplazan las listas estáticas de entradas/salidas. `CALLBACK_FAILED`, `0xFF`, valores fuera de 8 bits, cargos no declarados o duplicados terminan la secuencia. En el modo legacy no-unlimited, un `INVALID_CARGO` conserva un hueco `None` en el slot nativo y la consulta continúa, igual que `industry_cmd.cpp`; `newgrf_input_cargo_slots`/`newgrf_output_cargo_slots` mantienen la posición y el procesamiento económico compacta sólo los cargos válidos, preservando el índice estático para tasas y multiplicadores. `prop 0x1A` ya se parsea como `IndustryBehaviour`: con `CargoTypesUnlimited` se recorren hasta 16 entradas/salidas y un cargo inválido termina la secuencia sin crear hueco; las salidas desde el tercer slot se conservan en `newgrf_extra_output_cargos`/`newgrf_extra_produced_cargo` para transporte y SAV. Sin runtime se mantiene el fallback estático. Al reabrir un SAV, las listas efectivas, tasas, multiplicadores y stocks se reatachan al `IndustrySpecDef` activo sin volver a ejecutar callbacks de fundación. La economía procesa todas las salidas declaradas; siguen pendientes cargos custom no representables y rehidratación completa cuando falta su catálogo. |
| Industries | `0x3B` `CBID_INDUSTRY_SPECIAL_EFFECT` | **parcial runtime** | El ciclo nativo de 256 ticks evalúa `PlantFields` (`param2=0`) y `CutTrees` (`param2=1`) con `Random()` en `param1`, escribe de vuelta `7C` y convierte el resultado booleano; `CALLBACK_FAILED` conserva el fallback OpenTTD. PlantFields reutiliza la geometría/RNG de `PlantFarmField` y CutTrees recorre la espiral 40×40, exige la huella terminada, corta el primer árbol adulto y añade 45 unidades al primer output. Las industrias vanilla Farm/FarmTropic/LumberMill respetan los flags equivalentes. `PlantOnBuild` ejecuta 50 intentos en la colocación manual vanilla y NewGRF. Al cargar SAV `<SLV_32`, el afterload limpia los campos huérfanos con `MakeClear(CLEAR_GRASS, 3)`, replanta 50 intentos por industria y marca el remapeo una sola vez; si la definición custom aún no está catalogada, el trabajo queda diferido hasta aplicar la pila NewGRF. Siguen faltando la reatachación económica completa de tipos custom ausentes, escalas/sonidos y goldens tick-a-tick. |
| Industries | `0x01` `CBID_INDUSTRY_PRODUCTION_CARGO_ARRIVAL`; `0x02` `CBID_INDUSTRY_PRODUCTION_256_TICKS` | **parcial runtime** | Los grupos v0/v1/v2 se ejecutan al procesar entradas de estación y en el ciclo de producción; consumen las colas de cargos aceptados, agregan todas las salidas declaradas y transportan salidas adicionales v2 a estaciones. Se respeta `again` con límite de 65536 iteraciones y los valores indirectos `7D` de v1/v2. Faltan el scope completo de industria, `ProdMultiHandling`/escala de cargo, traducción exacta de cargos custom, mensajes y storage SAV nativo de las colas. |
| Industries (cambio de nivel) | `0x29` `CBID_INDUSTRY_PRODUCTION_CHANGE`; `0x35` `CBID_INDUSTRY_MONTHLYPROD_CHANGE`; `0x15F` `CBID_INDUSTRY_PROD_CHANGE_BUILD` | **parcial runtime** | CB29 se ejecuta en el cambio diario de una industria NewGRF y CB35 después de actualizar las estadísticas mensuales; se decodifican `no-op`, halve/double, divide/multiply, increment/decrement, cierre, cambio estándar y set de `prod_level` vía registro `0x100`. CB15F fija el nivel inicial al fundar si el resultado está en `PRODLEVEL_MINIMUM..MAXIMUM`. `CALLBACK_FAILED` conserva no-op y no cae silenciosamente al algoritmo vanilla. Los registros `7C` se escriben en la entidad y su referencia `INDY.psa`/fila `PSAC` se persiste; faltan scopes completos, mensajes y callbacks de tesela. |
| Industries | Scope `0x40`–`0xB4` y callbacks restantes | **parcial runtime** | El scope padre de las teselas expone esperas, distancia al agua, layout seleccionado/posición/dimensiones, cargos/salida, producción mensual, nivel, color aleatorio, contador y random persistente; los valores WORD conservan sus bits altos (`0x8A`/`0x8C`, `0x94`…`0xA5`, `0xAA`). También expone fundador (`0x45`/`0xA7`), fecha absoluta (`0x46`), flags (`0x47`), último año de producción (`0xA9`), entrega (`0xAC`), fecha relativa (`0xB0`), tipo de construcción (`0xB3`) y la última aceptación global (`0xB4`); las variables de cargo `0x6E` devuelven la fecha absoluta del slot. Los callbacks de producción/cambio de nivel conservan registros `7C` en la entidad y el contexto de tesela lee el storage del padre; `INDY.psa`, `STNN.normal.airport.psa`, `CITY.psa_list` y el pool `PSAC` ya se importan, se hidratan sus referencias y se reemiten con índices/256 registros. `INDY.accepted[].history`, `accepted[].accumulated_waiting`, `INDY.produced[].history` y `valid_history` se hidratan, actualizan en entrega/transferencia/barrido/rollover y se reemiten para cargos representables. Siguen pendientes el writeback de callbacks de tesela, la asociación runtime completa GRFID/feature, callbacks PSA de pueblo/casa/objeto y la granularidad completa del scope. `INVALID_OWNER` y cero quedan como fallbacks de saves antiguos. Siguen fuera del modelo los cargos custom no representables; `IndustrySpecDef.callback_mask` conserva callbacks aún no conectados. |
| Industries | `0x22`, `0x37`–`0x3A`, `0x14D`+, … | **almacenado** | `IndustrySpecDef.callback_mask` |
| Airport tiles (`11`) / Airports (`0D`) | Action2 vars `0x41`/`0x42`/`0x43`/`0x44`/`0x60`–`0x62`/`0x7A` + `0x150`/`0x152`–`0x154` | **parcial runtime** / **almacenado** / **OOS** | El renderer resuelve por tesela la posición relativa, terreno, frame, layout padre, random y vecinos con caché por fingerprint; Action1/3 puede seleccionar una vista distinta por esos valores y conserva fallback vanilla. Action0 conserva frames/status/speed/triggers y las listas de badges traducidas por GlobalVar `0x18`; `0x42` consulta la zona del pueblo más cercano y `0x7A` devuelve presencia o `UINT_MAX` para un índice local fuera de tabla. `CB0x150` decide si se conserva la fundación nivelada en pendientes (`CALLBACK_FAILED`/no nulo = conservar, cero = suprimir); `CB0x152` se ejecuta para `Built`/`TileLoop`/`NewCargo`/`CargoTaken`/`AcceptanceTick`, recibe el cargo local en `var18` usando la CTT del GRF, `CB0x153` selecciona el próximo frame y `CB0x154` regula la cadencia `2^speed`, con `AnimatedTileList` persistida en JSON y secuencia vanilla como fallback. `AirplaneTouchdown` se conecta al cruce de fases de aterrizaje (y al nodo FTA cuando existe). Siguen pendientes foundations de compositor (`0x150` Action5/rotación de cimiento), rotaciones runtime y sonidos (#260) |
| Canals (`05`) | `0x147` sprite offset | **almacenado** | `CanalSpecDef.callback_mask` |
| RoadStops (`14`) | `0x13` `CBID_STATION_AVAILABILITY` | **soportado** | Máscara Action0 `0x11`, Action2/3 y call site query+execute de `PlaceBusStop`/`PlaceTruckStop`; `CALLBACK_FAILED` o booleano 8-bit no nulo permite |
| RoadStops | `0x140`–`0x142` animación + Action2 random | **parcial runtime** | Action0 `0x0D`/`0x0E`/`0x0F`/`0x10`; CB140 cubre `Built`/`TileLoop`, `NewCargo`/`CargoTaken`, `VehicleLoads`/`VehicleArrives`/`VehicleDeparts` vial y `AcceptanceTick`; CB141/CB142 conservan frame/activo y `7C` por parada. Cada tesela custom creada o recuperada desde JSON tiene spec/frame/activo/random propios, preservados al unir paradas; eventos de vehículo usan la tesela exacta y los de carga/aceptación recorren el stop completo. La ruta Action2 conserva la máscara `0x0D`, CTT/versión, bits de estación (16) + tesela (8), triggers pendientes y grupos random `any`/`all` alcanzables; ejecuta NewCargo, CargoTaken, carga, llegada y salida antes de CB140. El render y el scheduler resuelven Action3/Action2 por tesela con random/triggers, vista/tipo/terreno, road/tram (incluidos tipos externos del catálogo), frame, `param[]`, las vars de carga `60`–`65`/`69`, las vars de mundo `45` (zona/distancia a pueblo), `46` (distancia cuadrática) y `47` (propietario/AI/colores), y los offsets `66` (frame misma estación), `67` (land), `68` (info), `6A` (GRFID) y `6B` (id local): nibbles firmados, wrap de mapa y caché por `(var,param)` evitan congelar una variante vecina. `param2` lleva el ordinal del trigger y, para carga, el id CTT (o fallback histórico) en el byte alto. La randomización del scheduler recibe los pools de town/company/industries cuando corre desde `sim_step`; las APIs históricas sin catálogo conservan fallback vanilla, mientras que la ruta explícita `...with_catalog_and_road_types` y el renderer usan tipos externos. `69` conserva y rota los cuatro flags de `GoodsEntry`, mientras `65` usa el catchment vivo con pools de mundo y conserva el fallback estático en APIs legacy. La importación `.sav` ya decodifica `roadstopspeclist`/`roadstoptiledata`, conserva `(GRFID, localidx)`, frame y random por tesela y reata la spec cuando el catálogo NewGRF está disponible; los GRF no instalados siguen siendo no resolubles. |
| RoadStops / Stations | Action2 `TileLayoutSpriteGroup` (`TileSeq`) | **parcial runtime** | Se parsean `type`, ground, origen/extents, children, referencias custom Action1 y los índices de registros. Action3/2 se resuelve por vista, random y variables de la tesela. El renderer materializa layouts de road stops, waypoints y estaciones rail con suelo propio, cajas parent `M(...)`, children relativos y caché por fingerprint, incluyendo pendientes niveladas; el procesador aplica `DODRAW`, offsets de sprite, offsets de cajas/children, `var10` y draw mode `0x100`. Sprites base y paletas custom siguen en fallback vanilla atómico; callbacks avanzados y paletas fuera de la rampa de compañía continúan pendientes. |
| Objects | Action2 `TileLayoutSpriteGroup` (`TileSeq`) | **parcial runtime** | El catálogo conserva el grafo cuando el objeto publica layouts aunque no tenga callbacks variables. El renderer resuelve la tesela del footprint, materializa suelo y secuencia de parents/children con cajas `M(...)`, consulta `0x42` (fecha), `0x45` (zona/distancia Manhattan) y `0x46` (distancia euclídea al cuadrado), y expone `0x47` (color) y `0x48` (vista) desde la instancia `OBJS`; `0x60`/`0x61`/`0x62`/`0x63` consultan id, random, información y frame de teselas vecinas del mismo footprint, y `0x64` devuelve cantidad/distancia de instancias por tipo. La asociación objeto↔pueblo usa `Object::town` importado o el pueblo más cercano como fallback; cuando existe `CITY.psa_list`, el scope parent copia los registros `7C` del pueblo por GRFID. Los offsets solicitados se cachean por fingerprint de registros. Sprites base, paletas custom y layouts incompletos usan fallback vanilla atómico. Faltan writeback `7C`, callbacks de objeto (salvo el slope check), conteos por clase/catchment y layouts 16-bit completos. |
| Industry tiles | Action2 `TileLayoutSpriteGroup` (`TileSeq`) | **parcial runtime** | El catálogo conserva layouts aunque la tesela no tenga callbacks variables. El renderer resuelve Action2 con etapa, random y parámetros GRF, reemplaza el suelo/agua cuando el layout es completo, emite parents/children con cajas `M(...)` sobre la superficie que decide `CB0x30` y cachea cada pieza por fingerprint y color de compañía. Sprites base, paletas especiales y layouts incompletos mantienen fallback vanilla atómico; faltan scopes avanzados, callbacks de sonido, generación automática y variantes de animación/layout por etapa. |
| Objects (`0F`) | `0x157` `CBID_OBJECT_LAND_SLOPE_CHECK` | **parcial runtime** | Máscara Action0 `0x15` WORD, Action3→Action2 y call site query+execute de `BuildObject` por tesela. `param1=slope`, `param2=dy<<4\|dx`; el resultado aplica la inversión de bit 10 para GRF <8. Durante construcción el resolver recibe el pueblo más cercano como parent, carga sus registros `7C` por GRFID y persiste `\\2psto` en una copia de `Town` durante el preflight; el execute la conserva sólo después de comprobar fondos. El objeto conserva además town/build date/color/view y las variables de alcance `0x40`–`0x48`, `0x60`–`0x64` durante el render. Faltan callbacks de objeto adicionales, writeback de `7C` propio de tesela/instancia, scopes/vecinos completos, string de error GRF y el fallback de pendiente completo de OpenTTD. |
| Cargoes (`0B`) | `0x39` `CBID_CARGO_PROFIT_CALC`; `0x145` `CBID_CARGO_STATION_RATING_CALC` | **parcial runtime** | Máscara Action0 `0x1A`, Action3→Action2: CB39 paga cada packet en `unload_vehicles` (`param1=0`, distancia/cantidad/tránsito, multiplicador signed-15); CB145 sustituye el target durante `update_station_ratings` (`param1` tipo histórico de vehículo; `param2` días/espera/velocidad, resultado signed-15). Faltan scopes avanzados y demás CBs. |
| Cargoes (resto) / Types | varios | **OOS** | Sin ejecución de CB en este corte |
| Generic | `0x01` `CBID_RANDOM_TRIGGER` | **OOS** | Ver triggers abajo |

## Storage

| Tipo | Ciclo de vida | Estado |
|---|---|---|
| Temporal (`7D` / `\2sto`) | Solo durante la evaluación Action2 | Descartado al terminar el ctx |
| Persistente (`7C` / `\2psto`) | Vehículo: `Vehicle.newgrf_persistent_regs` | Writeback tras CB; round-trip JSON save |
| Persistente estación | `Station.newgrf_persistent_regs` | **parcial**: API stateful + JSON round-trip; CB13 de construcción no puede hacer writeback porque OpenTTD lo evalúa sin estación/tesela |
| Persistente industria | `Industry.newgrf_persistent_regs` | Writeback tras CB de producción, CB25/26/27 y `ResolveRerandomisation` de tesela para `TileLoop`, `IndustryTick` y `CargoReceived` cuando el scheduler recibe pools de mundo; CB25 se dispara en sus eventos reales y CB26/CB27 quedan en la pasada visual de frames activos; reseed parent agregado una vez por footprint; round-trip SAV/JSON, con `INDY.psa`/`PSAC` conservados |
| Persistente pueblo (scope parent de casas/objetos) | `Town.newgrf_persistent_regs` | **parcial**: lectura `7C` por GRFID en Action2, `CITY`/`PSAC` y round-trip SAV/JSON; CB17 de casas y CB157 de objetos durante construcción hacen writeback parent por GRFID (con preflight aislado); callbacks de teselas de industria y su re-randomización (`TileLoop`, `IndustryTick`, `CargoReceived`) ya escriben el parent `Industry`, mientras scopes de otras entidades siguen pendientes |
| Persistente casa/objeto | — | **OOS** como entidad propia; consumen el PSA del pueblo asociado cuando existe |

## Triggers / random

| Pieza | Estado |
|---|---|
| Industry tile `m3` random bits + `m6` triggers (reseed) | **Parcial runtime**: la ruta con catálogo conserva `m3`/triggers por tesela y deja vanilla en no-op; `TileLoop`, `IndustryTick` y `CargoReceived` hidratan/escriben el PSA parent de la industria alrededor de `ResolveRerandomisation`; el fallback histórico sin catálogo sigue disponible para herramientas legacy |
| `ResolveRerandomisation` / Action2 random sprite groups por trigger | **Parcial runtime** (#266): los grupos alcanzables consumen sólo sus triggers y reseedean la máscara declarada, con writeback de `\2psto` del parent `Industry` y reseed parent agregado por footprint en `TileLoop`/`IndustryTick`/`CargoReceived`; layouts/variables no random y callbacks de sonido/slope/autoslope siguen pendientes |
| Vehicle/station random Action2 (`0x80`/`0x83`/`0x84`) en resolve de sprites | **Parcial** (eval con `random_bits` de 16 bits; reseed gameplay vía `trigger_vehicle_randomisation_chain`; vehículos consumen `Callback32`, `NewCargo` y `Empty`, y conservan triggers pendientes) |
| RoadStops Action0 `0x0D` + Action2 random | **parcial runtime**: CTT/versión, eventos `NewCargo`/`CargoTaken`/llegada/salida/carga vial, `any`/`all`, reseed de bits base/tesela, JSON y selección visual Action2 dinámica con el contexto persistente. El renderer y los CB140–142 resuelven vars de mundo `45`/`46`/`47`, vars de carga `60`–`65`/`69` y `66`/`67`/`68`/`6A`/`6B` por offset; la randomización del scheduler también usa esos scopes y recibe los pools del mundo en `sim_step`. Spec/frame/activo/random son independientes por tesela en stops compuestos creados, JSON o importados desde `.sav` (`roadstopspeclist` + `roadstoptiledata`). El cliente reproduce los samples `NewGRF` de la cola global y los callbacks de sonido de vehículo cubren salida, marcha, avería, túnel, efecto visual, pago carga/descarga y despegue/aterrizaje; quedan las APIs legacy sin catálogo y GRF ausentes.
| `CBID_RANDOM_TRIGGER` genérico | **Parcial runtime**: la re-randomización de `IndustryTile` con catálogo/world ya ejecuta el contexto parent durante `TileLoop`, `IndustryTick` y `CargoReceived`; otros features siguen OOS |

## Call sites soportados (checklist)

1. Stations CB24 — layout al construir.
2. Vehicles CB31 — start/stop check (+ writeback persistent regs).
3. Vehicles CB11 — longitud efectiva al comprar/refrescar unidades (+ fallback `shorten_factor`).
4. Vehicles CB15 — capacidad de refit en `RefitVehicle` (+ writeback persistent regs).
5. Vehicles CB10 — efecto visual al emitir humo/chispas; el renderer usa el catálogo activo y respeta tipo/desactivación del callback (+ writeback persistent regs).
6. Industries CB28 — location al colocar NewGRF (`place_industry_spec_def_sandbox`),
   con `IACT_USERCREATION`, badges, TileIndex/layout, terreno, pueblo,
   distancias, altura, agua y random; permanecen scopes nativos y errores GRF.
7. Houses CB17 — allow construction durante crecimiento físico (GRF Action0/2/3 cargado + call site).
8. Stations CB13 — availability en query+execute de construcción; sin storage en el scope nulo de OpenTTD.
9. Industry tiles CB25/CB26/CB27 — trigger por evento (`TileLoop`, `IndustryTick`,
   `CargoReceived`) y next-frame/velocidad en la pasada visual de activos
   (FAILED observable).
10. Industry tile trigger → Action2 random group (`resolve_industry_tile_random_trigger`).
11. RoadStops CB13 — disponibilidad al previsualizar y ejecutar `PlaceBusStop`/`PlaceTruckStop`.
12. RoadStops CB140/CB141/CB142 — `Built`/`TileLoop`, carga/retirada de carga, carga/llegada/salida vial y `AcceptanceTick`; scheduler con velocidad/frame, writeback `7C`, CTT en `param2` y JSON round-trip. Cada tesela custom conserva spec/frame/activo/random, incluso tras `JoinStation`; los eventos exactos y de área completa siguen el alcance de `RoadStopTileData`. Action0 `0x0D` + Action2 random reacciona a NewCargo, CargoTaken, carga, llegada y salida con grupos `any`/`all`, bits base/tesela y triggers persistentes. Referencia: `newgrf_roadstop.cpp` / `newgrf_animation_base.h`.
13. Vehicles CB32 — barrido económico escalonado, `day_counter` persistente, palabra de randomización de 16 bits, bits de paleta y trigger `Callback32` con reseed del grupo Action2 activo; `trigger_vehicle_randomisation_chain` cubre la propagación de `NewCargo`/`AnyNewCargo`, `Depot` y `Empty`. Los call sites de carga, vaciado y entrada a depósito de los cuatro tipos ya están conectados; la invalidación de paleta queda representada en la huella de Action2 que consume el renderer.
14. Vehicles CB36 — resolver signed/unsigned, aplicar acortamiento `0x21`/`0x23`, consultar velocidad por clase y capacidad al crear/refitar/articular y durante `LoadUnloadStation`, resolver potencia/peso/TE para los recálculos ferroviario y vial con catálogo, y aplicar factores de compra/explotación en economía, activos y autoreplace; quedan APIs legacy sin catálogo, otras propiedades Action0 y scopes avanzados.
15. Objects CB157 — parcial por tesela de `BuildObject`, desde Action0 `0x15` y Action3→Action2 cargados; query y execute rechazan antes de mutar y aplican la inversión de bit 10 para GRF <8. Faltan scopes/vecinos, strings de error y fallback completo.
16. Cargoes CB39 — cálculo de pago por packet durante `unload_vehicles`, desde Action0 `0x1A` y Action3→Action2 cargados; `CALLBACK_FAILED` conserva la fórmula base.
17. Cargoes CB145 — target de rating durante el barrido `update_station_ratings`, desde Action0 `0x1A` y Action3→Action2 cargados; `CALLBACK_FAILED` conserva el algoritmo estándar.
18. Stations CB149 — parcial por tesela al construir, desde Action0 `0x0B` y Action3→Action2 cargados; query y execute rechazan antes de mutar y aplican la inversión de bit 10 para GRF <8. Siguen pendientes scopes/vecinos y mensajes de error GRF.
19. Stations CB14 — layout de tesela al dibujar, desde Action0 `0x0B` y Action3→Action2 cargados; el renderer el aplica antes de elegir la vista NewGRF.
20. Stations CB140/CB141/CB142 — `Built`/`TileLoop`, `NewCargo`/`CargoTaken` y `AcceptanceTick` de área completa (250 ticks escalonada por StationID), y `VehicleLoads`/`VehicleArrives`/`VehicleDeparts`/`PathReservation` ferroviarios por plataforma; scheduler persistido por tesela, velocidad/frame, CTT de cargo en `param2` y var Action2 `4A`.
21. Industries CB29/CB35 — cambios diario/mensual de `prod_level` con decodificación de acciones estándar y callback `0x15F` — nivel inicial al fundar.
22. Industries Action2 `IndustryProductionSpriteGroup` — parseo y resolución Action3→Action2 de versiones v0/v1/v2, incluidos grupos random/variational.
23. Industries CB1/CB2 — consumo iterativo de entradas/salidas y `again` para cargos vanilla representables. CB14B/CB14C conserva ahora los huecos `INVALID_CARGO` del modo legacy y el índice de origen usado por multiplicadores; siguen faltando scopes/escala, cargos custom y persistencia SAV nativa.
24. Industries CB3B — efectos especiales periódicos de campos/árboles y `PlantOnBuild` manual/NewGRF; call sites, fallback y afterload SAV `<SLV_32` implementados. La reatachación económica de tipos custom ausentes, escalas/sonidos y goldens siguen pendientes.

## Residual explícito (no bloquea cierre MVP #266)

Actualización #329-VEHICLE-AIRPORT-005 (2026-09-02): el controlador de la FTA
de aeropuertos recibe ahora el catálogo activo desde
`tick_aircraft_phase_with_catalog`; `finish_takeoff` consulta
`PROP_AIRCRAFT_SPEED` (`0x0C`) mediante CB36 antes de volver a crucero y
reinicia `subspeed`. La API legacy conserva el fallback vanilla pasando un
catálogo vacío. La regresión `finish_takeoff_uses_active_catalog_speed_callback`
cubre la transición de salida desde pista. El resto de propiedades/call sites
de vehículos y los scopes avanzados siguen en estado parcial.

Actualización #329-VEHICLE-ROAD-SLOPE-006 (2026-09-02): el cierre del tick de
carretera propaga el catálogo activo a `sync_road_slope_speed`; la bajada de
`RoadZPosAffectSpeed` consulta `PROP_ROAD_SPEED` (`0x15`) mediante CB36 antes
de aplicar el empuje, manteniendo el límite dinámico del motor. La variante
legacy sigue pasando un catálogo vacío y usa el fallback vanilla. La regresión
`slope_sync_uses_active_catalog_speed_callback` fija el techo NewGRF durante
la corrección de pendiente. El resto de propiedades/call sites viales y los
scopes avanzados siguen parciales.

Actualización #329-VEHICLE-AIRCRAFT-SUBTYPE-007 (2026-09-02): la clasificación
de helicópteros en la FTA y en `AirplaneTouchdown` usa ahora
`EngineDef::is_helicopter` del catálogo activo, con fallback para los IDs
vanilla. Un aeropuerto mixto ya conserva headings heli para motores Action0 con
IDs propios y evita emitir touchdown de ala fija. La regresión
`fta_approach_uses_active_catalog_helicopter_flag` cubre esa entrada; el resto
del runtime de aeronaves y los scopes avanzados permanece parcial.

Actualización #329-VEHICLE-SAV-AIRCRAFT-008 (2026-09-02): el escritor `VEHS`
consulta el catálogo activo para clasificar aeronaves y emite la cadena
helicóptero+sombra+rotor cuando `EngineDef::is_helicopter` procede de Action0 o
NewGRF. La regresión `vehs_uses_newgrf_catalog_for_aircraft_subtype` cubre un
ID local fuera de la tabla vanilla. La serialización aún reconstruye un
encabezado mínimo para cambios estructurales, variables o anidados; los
cambios escalares fijos compatibles se fusionan sobre el cuerpo importado.
Ese residual sigue abierto.

Actualización #329-VEHICLE-SAV-VEHS-009 (2026-09-02): `VEHS` importa también
el cuerpo crudo y una huella de las filas semánticas. Si la huella no cambia,
el ciclo SAV reemite el chunk original byte a byte y mantiene columnas futuras;
una mutación semántica invalida ese cuerpo para evitar exportar posiciones o
velocidades obsoletas. La regresión
`imported_vehs_body_is_reused_until_vehicle_semantics_change` cubre la
reutilización y el fallback canónico. La fusión común conserva ahora columnas
desconocidas cuando sólo cambia un escalar fijo y no cambian filas ni índices;
mutaciones variables/anidadas o estructurales siguen pendientes.

Actualización #329-VEHICLE-SAV-ORDL-010 (2026-09-02): `ORDL` comparte ahora
el snapshot semántico de vehículos. Cuando no cambia ninguna fila, el
exportador reemite la lista de órdenes original y conserva sus columnas
futuras; una mutación de orden o de topología activa el encoder canónico.
La regresión `imported_vehs_body_is_reused_until_vehicle_semantics_change`
cubre la pareja `ORDL`/`VEHS`. La fusión común cubre ahora escalares fijos
compatibles; mutaciones de strings/listas/structs o cambios de filas e índices
siguen pendientes.

Actualización #329-VEHICLE-SAV-TABLES-011 (2026-09-02): `STNN`, `CITY` e
`INDY` también guardan el cuerpo original y su huella semántica. Los ciclos
SAV sin cambios de estaciones, ciudades o industrias conservan columnas
futuras; cualquier diferencia de filas activa la serialización canónica para
evitar datos obsoletos. Los escalares fijos compatibles se fusionan ahora sin
perder columnas futuras; mutaciones variables/anidadas y los demás pools
nativos siguen pendientes.

La tabla de variables relativas conserva el parámetro `ExtendedByte` completo
(WORD, hasta 14 bits) para `61 → 0x60`; los IDs locales superiores a `0xFF` no
se aliasan con el motor vanilla de id cero.

Las listas de badges de vehículos (`0x33`/`0x2A`/`0x26`/`0x24`) y de tipos de
vía (`0x1E`) se traducen por GlobalVar `0x18`. Las variables `0x64`/`0x65`/`0x7A`
ya están disponibles en el contexto Action2: `0x65` selecciona los badges del
RailType/RoadType de la tesela actual y conserva los offsets relativos (`61`).

Actualización #329-VEHICLE-SAV-META-012 (2026-09-02): `PATS`, `ECMY` y `CAPY`
reutilizan ahora sus cuerpos originales cuando las filas conocidas no cambian.
Esto preserva ajustes, contadores económicos y referencias/pagos de carga de
versiones nuevas durante un round-trip; una mutación semántica invalida sólo
ese chunk y activa el writer canónico salvo que sea un escalar fijo compatible,
que se fusiona in-place. Las mutaciones variables/anidadas y los otros pools
SAV todavía requieren trabajo diferencial.

Actualización #329-VEHICLE-SAV-FLEET-014 (2026-09-02): `GRPS` y `ERNW` usan
ahora una huella densa de filas semánticas para reemitir sus cuerpos originales
sin cambios. Se conservan así columnas futuras, huecos de pool y enlaces de
reglas; una mutación de grupo o autorrenovación activa el writer actual y
recalcula las referencias. Los escalares fijos compatibles se fusionan ahora;
mutaciones variables/anidadas y los pools restantes siguen abiertos.

Actualización #329-VEHICLE-SAV-LINKGRAPH-015 (2026-09-02): `LGRP` usa ahora
una huella de registros reconstruidos para conservar headers y columnas
futuras sin cambios; `LGRJ`/`LGRS` siguen reutilizando sus cuerpos runtime
opacos hasta que se registra una arista nueva. La mutación semántica activa el
writer actual y mantiene el grafo válido; los escalares fijos compatibles
pueden fusionarse sin perder columnas futuras, pero jobs/cargodist completos
aún no se ejecutan en Rust.

Actualización #329-VEHICLE-SAV-NGRF-016 (2026-09-02): `NGRF` reemite su cuerpo
original mientras las filas del stack activo no cambian, conservando digest,
paleta y columnas futuras; un cambio de orden, GRFID, versión o parámetro usa
el encoder semántico. El runtime aún necesita resolver archivos no instalados y
fusionar mutaciones variables/anidadas después de cambios parciales.

Actualización #329-VEHICLE-SAV-DATE-017 (2026-09-02): `DATE` reutiliza ahora el
cuerpo original cuando sus cuatro valores conocidos (fecha, tick y dos
palabras RNG) coinciden. Cambiar cualquiera de ellos fuerza la serialización
canónica, manteniendo el reloj válido sin copiar estado obsoleto.

Actualización #329-VEHICLE-SAV-CAPA-018 (2026-09-02): `CAPA` compara ahora la
huella densa de paquetes físicos y reemite el cuerpo original sin cambios,
conservando columnas futuras. Cualquier mutación de paquetes o referencias
activa la serialización canónica y mantiene los enlaces `STNN`/`VEHS` válidos;
las mutaciones variables/anidadas siguen pendientes.

Actualización #329-VEHICLE-SAV-PLYR-013 (2026-09-02): `PLYR` conserva ahora su
cuerpo original cuando las filas semánticas de compañías coinciden. El
exportador mantiene así columnas nuevas de ajustes, economía, libreas y
retrato durante un round-trip sin cambios, y cae al writer canónico si cambia
una compañía o una referencia de autorrenovación. Los escalares fijos
compatibles se fusionan ahora; mutaciones variables/anidadas y headers
estructurales tras una mutación siguen pendientes.

Actualización #329-TOWN-CITY-027 (2026-09-02): el importador `CITY` conserva
ahora la metadata nativa que antes se descartaba (`townnamegrfid`,
`townnametype`, `townnameparts`, flags, ratings, `have_ratings`, unwanted,
metas de crecimiento, exclusividad, layout, estatuas, `valid_history` y texto
de GameScript). `TownScopeResolver` usa los flags importados y expone también
`0x40` (pueblo grande), `0x92`/`0x93` (flags), y `0xAE` (`have_ratings`) con
fallbacks explícitos para saves antiguos. La prueba sintética reproduce la
forma `CITY` de OpenTTD y verifica los escalares y listas decodificados. El
writer canónico pasó a reemitir esta metadata y las listas/structs modelados en
la actualización 029; el passthrough sin mutaciones conserva el cuerpo
original. Persisten las columnas desconocidas, el writeback de PSA y los
consumidores de crecimiento/economía, por lo que #328/#329 permanecen abiertos.

Actualización #329-TOWN-CITY-028 (2026-09-02): `CITY.supplied` y
`CITY.received` ya se decodifican en estructuras tipadas (`cargo` + historial
mensual de producción/transporte y los cuatro contadores old/new max/act por
efecto). Esto evita que el parser descarte los historiales nativos presentes
en SAV reales; una regresión cubre cargos con y sin historial y más de un slot
recibido. El writer canónico los reemite con el formato nativo; antes no se
conectaban esos contadores al cálculo de crecimiento/economía y la
interoperabilidad de ciudades continúa siendo parcial.

Actualización #329-TOWN-CITY-029 (2026-09-02): el writer canónico de `CITY`
reemite ahora la metadata nativa modelada (generador de nombres, flags,
ratings, `have_ratings`, unwanted, metas, contadores, exclusividad, layout,
estatuas, `valid_history` y texto), las listas `supplied`/`received` y
`psa_list`. Las arrays fijas se escriben con los tamaños que exige OpenTTD
(`MAX_COMPANIES` y `NUM_TAE`, con `TAE_NONE` en el slot 0), y una regresión
valida el round-trip de escalares e historiales; un fixture generado se acepta
con OpenTTD 15.3. La caché `cache.population` continúa fuera del writer porque
OpenTTD la reconstruye desde las teselas. Mutaciones estructurales aún caen al
header canónico y descartan columnas anidadas desconocidas; el writeback de PSA
y los consumidores de cargos custom siguen pendientes.

Actualización #329-TOWN-CITY-030 (2026-09-02, commit `b7429397`): `CITY.received.old_act/new_act`
se hidrata ahora en las ventanas runtime que consulta `UpdateTownGrowth`, y
el rollover mensual mantiene sincronizados los arrays semánticos y el vector
nativo. La producción de casas registra `CITY.supplied` por cargo y actualiza
sus muestras `THIS_MONTH`; los scopes parent de pueblo leen producción,
transporte y porcentaje (`0xBA`–`0xCB`) desde ese historial. La cobertura sigue
limitada a cargos representados y no resuelve todavía mutaciones de PSA ni
columnas anidadas desconocidas.

Actualización #329-TOWN-SCOPE-026 (2026-09-02): casas y objetos materializan
ahora el scope parent de `TownScopeResolver` con las variables conservadas por
el modelo (`0x41`, posición `0x80`/`0x81`, población `0x82`/`0x83`, crecimiento,
radios, ratings, estatuas, cantidad de casas, historial de pasajeros/correo,
entregas de comida/agua y meses de financiación), además de seleccionar `7C`
por GRFID desde `CITY.psa_list`. Los flags/cache de cargos no representados,
writeback de town PSA de callbacks de tesela y scopes completos de estación/aeropuerto siguen
pendientes.

Actualización #329-TOWN-PSA-031 (2026-09-03, commit `bd3ea9c1`): CB17 de
construcción evalúa el `TownScopeResolver` parent real antes de reservar el
footprint. El operador `\2psto` de un grupo Action2 parent escribe ahora el
PSA del pueblo por GRFID y una regresión verifica que no contamine el storage
propio de la casa; el writer puede asignar/reemitir la fila `PSAC` y su
referencia `CITY.psa_list`. El writeback de callbacks de teselas, además de los
callbacks de pueblo restantes, sigue pendiente.

Actualización #329-OBJECT-PSA-032 (2026-09-03, commit `9303cf65`): CB157 de
construcción recibe el pueblo más cercano como `TownScopeResolver` parent y
persiste `\\2psto` por GRFID. Query/preview evalúan sobre copias de pueblos y
el execute sólo conserva la copia después de comprobar fondos; la regresión
cubre writeback, aislamiento por GRFID y rechazo sin fondos. El writeback de
callbacks de teselas y los demás callbacks/scope de objetos siguen pendientes.

Actualización #329-INDTILE-PSA-033 (2026-09-03, commit `47afecd7`): los
callbacks de animación `CB25`/`CB26`/`CB27` de `IndustryTile` ya reciben el
contexto completo de tesela y `IndustryTileResolverObject` parent en la ruta
normal de simulación. `\\2psto` se hidrata desde la industria viva y vuelve a
`Industry.newgrf_persistent_regs` tras cada evaluación; el scheduler conserva
la asociación por `m2`/footprint y la API legacy queda explícitamente sin
writeback. `CBID_RANDOM_TRIGGER` y los callbacks de foundations, sonido,
slope/autoslope siguen pendientes; #329 no se cierra.

Actualización #329-INDTILE-RANDOM-034 (2026-09-03, commit `601e7685`): la
re-randomización `Action2` de `IndustryTile` ya se ejecuta con el parent
`Industry` vivo en la ruta `TileLoop`. El scheduler hidrata el PSA antes de
`ResolveRerandomisation`, persiste `\\2psto` después de evaluar el grupo y
mantiene la asociación por `m2`/footprint incluso cuando varias teselas
comparten una industria. La API histórica sin catálogo/world continúa como
fallback explícito. Siguen pendientes los triggers `IndustryTick` y
`CargoReceived`, además de foundations/sonido/slope/autoslope; #329 no se
cierra por este subconjunto.

Actualización #329-INDTILE-TRIGGERS-035 (2026-09-03, commit `916247a2`): los
call sites económicos de `CargoReceived` y `IndustryTick` dejaron de usar el
fallback vanilla. Cada trigger recorre la huella viva de la industria, hidrata
el PSA parent antes de `ResolveRerandomisation`, conserva los triggers no
consumidos y persiste `\\2psto`; las máscaras `0x83` se agregan y reseedean una
sola vez en `Industry.newgrf_random` después de evaluar toda la huella. La
regresión cubre ambos triggers, el writeback PSA y una huella de dos teselas.
Siguen pendientes foundations/sonido/slope/autoslope, historiales mutables y
cargos custom; #329 continúa abierto.

Actualización #329-INDTILE-SLOPE-036 (2026-09-03, commit `9e01c1a9`): el
parser de `IndustryTiles` conserva `prop 0x0D` (`slopes_refused`) y los bits
de máscara upstream para shape-check (`0x2F`), foundations (`0x30`) y
autoslope (`0x3C`). La colocación NewGRF ejecuta `CBID_INDTILE_SHAPE_CHECK`
por tesela con `IACT_*`/layout en `param2`, parent temporal de industria y la
inversión de booleano anterior a GRF v7; ante `CALLBACK_FAILED` aplica
`IsSlopeRefused` con la pendiente complementaria. `CBID_INDTILE_DRAW_FOUNDATIONS`
ya usa el ID correcto `0x30` en el renderer. El call site de terraformación
para autoslope, generación automática y los callbacks de sonido siguen
pendientes; #329 no se cierra.

Actualización #329-INDTILE-AUTOSLOPE-037 (2026-09-03, commit `fe70a433`): el
callback `CBID_INDTILE_AUTOSLOPE` (`0x3C`) ya participa en el preflight de
`raise_land`, `lower_land` y `level_land`. Se replica la guarda de
`TerraformTile_Industry`: sólo se consulta para teselas vieja/nueva no
empinadas cuyo máximo absoluto permanece igual; `CALLBACK_FAILED` o cero
permiten conservar la industria y cualquier valor no nulo deja continuar la
limpieza genérica. El contexto Action2 usa la instancia viva de `Industry`,
asocia por `m2`/huella y persiste los registros `7C`; las regresiones cubren la
semántica opt-out, el scope parent y la conservación de una industria al subir
una esquina. La generación automática y callbacks de sonido siguen pendientes;
`#329` continúa abierto.

Actualización #329-INDUSTRY-COLOUR-038 (2026-09-03, commit `63d37f04`): el
callback `CBID_INDUSTRY_DECIDE_COLOUR` (`0x14A`) ya se ejecuta después de
inicializar la industria NewGRF y antes de publicarla en `IndustryPool`.
OpenTTD sólo acepta el nibble bajo cuando los bits 4..14 del resultado son
cero; `CALLBACK_FAILED` o un valor inválido conservan el color sorteado. La
evaluación usa el random/variables del parent ya inicializado y persiste sus
registros `7C`; las regresiones cubren resultados válidos, inválidos y la ruta
de colocación. Efectos especiales, cargos dinámicos y generación automática
siguen pendientes; `#329` continúa abierto.

Actualización #329-INDUSTRY-REFUSE-039 (2026-09-03, commit `65682a42`):
`CBID_INDUSTRY_REFUSE_CARGO` (`0x3D`) ya consulta el runtime de cada entrada
de una procesadora NewGRF antes de consumir el lote de estación. El índice
local se obtiene desde la traducción CTT del catálogo; un resultado no nulo
acepta, cero rechaza y `CALLBACK_FAILED` conserva el fallback. La regresión
comprueba `param2`, la inversión booleana y que una negativa deje intacto el
stock y no produzca. La ruta actual sigue acoplada al ciclo de procesamiento:
la entrega directa a la entidad `Industry` y su temporización/monitor de
`DeliverGoodsToIndustry` aún deben modelarse, junto con efectos especiales,
cargos dinámicos, sonido y generación automática; `#329` permanece abierto.

Actualización #329-INDUSTRY-CARGO-TYPES-040 (2026-09-03, commit `389109c1`):
`CBID_INDUSTRY_INPUT_CARGO_TYPES` (`0x14B`) y
`CBID_INDUSTRY_OUTPUT_CARGO_TYPES` (`0x14C`) ya se ejecutan durante la
fundación NewGRF cuando el GRF declara sus máscaras. Cada slot recibe su
`param1`, la tabla CTT valida el índice local y la instancia reemplaza sus
listas estáticas; `0xFF`/`CALLBACK_FAILED` terminan la lista, y el runtime
ausente conserva el fallback estático. Las regresiones cubren los tres slots
de entrada, multiplicadores por índice y la salida vacía. La cobertura sigue
limitada a 3 entradas/2 salidas: `CargoTypesUnlimited`, cargos custom, listas
persistidas en SAV y el procesamiento de efectos/temporización todavía no
están modelados; `#329` permanece abierto.

Actualización #329-INDUSTRY-CARGO-TYPES-041 (2026-09-03, commit `36662249`):
`prop 0x1A` de `Industries` ya se conserva como `IndustrySpecDef.behaviour` y
expone el bit `CargoTypesUnlimited` (`1 << 18`). Los callbacks `0x14B`/`0x14C`
consultan ahora hasta `INDUSTRY_NUM_INPUTS`/`INDUSTRY_NUM_OUTPUTS` (16 slots)
cuando ese bit está activo; las salidas a partir de la tercera quedan en
`newgrf_extra_output_cargos`, y su stock se transporta/exporta mediante el
buffer adicional. Se agregaron regresiones de parseo Action0 y de cuatro
entradas/salidas dinámicas. Esto no cierra el issue: el procesamiento normal
multi-output (rates/matriz por cada salida), slots vacíos legacy, cargos custom
y rehidratación runtime desde SAV siguen pendientes.

Actualización #329-INDUSTRY-CARGO-TYPES-042 (2026-09-03, commit `0fddd2f4`):
la economía ya calcula las tasas de producción y la matriz de multiplicadores
para todas las salidas declaradas. Las procesadoras consumen y depositan cada
slot (incluidos los extras en `newgrf_extra_produced_cargo`), la comprobación
de capacidad considera todos los stocks y `INDY` exporta la tasa/espera de los
slots desde el tercero. La regresión de cuatro entradas y cuatro salidas
verifica 32 unidades por salida en un ciclo. Permanecen pendientes el
historial por salida, GUI/variables de slots ilimitados, cargos custom y
rehidratación runtime completa desde SAV; `#329` continúa abierto.

Actualización #329-INDUSTRY-SPECIAL-EFFECT-043 (2026-09-03, commit `6e3ad37a`):
`CBID_INDUSTRY_SPECIAL_EFFECT` (`0x3B`) ya se evalúa en el ciclo nativo de
256 ticks para `PlantFields` (`param2=0`) y `CutTrees` (`param2=1`), pasando
`Random()` en `param1` y escribiendo de vuelta los registros `7C`. Un resultado
válido usa `ConvertBooleanCallback`; `CALLBACK_FAILED` conserva el fallback
(`Chance16(1,8)` para campos y periodo de 512 ticks para árboles). Los campos
reutilizan `PlantFarmField`; los árboles recorren la espiral 40×40, exigen la
huella terminada, eliminan el primer árbol adulto y agregan 45 unidades al
primer cargo. Esto no cierra #329: faltan `PlantOnBuild`, escalas/sonidos y
goldens integrales.

Actualización #329-INDUSTRY-PLANT-ON-BUILD-044 (2026-09-03, commit `628d1fb9`):
la colocación manual vanilla de `Farm`/`FarmTropic` y la ruta de fundación
NewGRF que declara `IndustryBehaviour::PlantOnBuild` ejecutan los 50 intentos
de `PlantRandomFarmField` después de materializar la industria. Coordenadas,
geometría, cercas, límites climáticos y consumo del RNG global comparten la
implementación de generación/efecto periódico; los campos quedan ligados al
`IndustryID` en MAP2. Falta reproducir el hook de afterload para partidas SAV,
además de escalas/sonidos y goldens tick-a-tick.

Actualización #329-INDUSTRY-PLANT-ON-BUILD-045 (2026-09-03, commit `56aa7858`):
el importador SAV conserva temporalmente la identidad, posición, tipo y tamaño
de las industrias de versiones `< SLV_32`. El afterload se consume una sola vez:
limpia todos los campos legacy como `MakeClear(CLEAR_GRASS, 3)`, ejecuta los 50
intentos de `PlantRandomFarmField` para cada industria con `PlantOnBuild`, vuelve
a asociar los campos al `IndustryID`, marca las teselas para remap y preserva el
RNG global. Las definiciones custom se resuelven al aplicar el catálogo NewGRF;
sin ese catálogo se usa sólo el fallback vanilla y la reatachación económica
completa queda pendiente. Esto no cierra #329: siguen faltando slots vacíos
legacy, cargos custom, historiales/GUI, escalas/sonidos y goldens integrales.

Actualización #329-INDUSTRY-CARGO-TYPES-046 (2026-09-03, commit `9f2ecc31`):
la ruta CB14B/CB14C replica el caso legacy que OpenTTD conserva en
`Industry::accepted`/`Industry::produced`: un `INVALID_CARGO` no aborta ni
desplaza los slots posteriores. El parser deja de filtrar `0xFF`, el catálogo
mantiene etiquetas alineadas y la instancia expone
`newgrf_input_cargo_slots`/`newgrf_output_cargo_slots` con `None` para cada
hueco. El procesamiento usa sólo los cargos válidos y conserva el
`source_index` para tasas y multiplicadores; `CargoTypesUnlimited` mantiene la
semántica estricta de terminar ante un cargo inválido. Una regresión con hueco
en el slot 0 y COAL en el slot 1 verifica la lista visible y el multiplicador
128 del slot original. Esto no cierra #329: cargos custom, rehidratación
runtime SAV, historiales/GUI, escalas/sonidos y goldens siguen pendientes.

Actualización #329-INDUSTRY-SAV-047 (2026-09-03, commit `eaa3473d`):
al aplicar el catálogo NewGRF después de importar un SAV, las filas `INDY` se
vuelven a asociar a su `IndustrySpecDef` mediante `IndustryType` y overrides.
La pasada no ejecuta callbacks de fundación: usa las listas serializadas
`accepted`/`produced`, conserva huecos `INVALID_CARGO`, recompone cargos,
tasas, multiplicadores por índice estático y reubica stocks/fechas de espera.
La regresión cubre una industria custom con hueco en la primera salida y
verifica `newgrf_type_id`, slots, stock, espera aceptada y multiplicador. Si el
GRF o un cargo custom no está instalado se conserva el fallback y la fila
opaca para un intento posterior; #329 sigue abierto por esa ausencia,
`DeliverGoodsToIndustry`, historiales/GUI, escalas/sonidos y goldens.

Actualización #329-INDUSTRY-DELIVERY-048 (2026-09-03, commit `12e6c751`):
la descarga normal materializa `DeliverGoodsToIndustry` antes de contabilizar
la entrega. Las industrias cubiertas se ordenan por `DistanceMax`, se excluye
la huella de origen y se respeta el límite `uint16` de `accepted[].waiting`;
`CBID_INDUSTRY_REFUSE_CARGO` decide cada lote y se actualizan fecha/flag de
aceptación. La producción se difiere hasta después de `load_vehicles`, con
CB1 de llegada, CB2 exclusivo diferido y matriz vanilla sin callback; la
regresión cubre exclusión, fecha, diferimiento y producción. Monitor,
exclusividad/neutral stations, aceptación exacta de estaciones, historiales de
salida y cargos custom siguen pendientes; `#329` continúa abierto.

Actualización #329-INDUSTRY-HISTORY-049 (2026-09-03, commit `a4dba228`):
`INDY.accepted[].history` ya se conecta al runtime. La entrega incrementa el
mes actual y `last_accepted`; el barrido diario acumula waiting y el rollover
calcula el promedio, rota 61 registros nativos y actualiza `valid_history`.
Importación y exportación hidratan/reemiten historial, acumulador y máscara;
las regresiones cubren entrega, rollover, SAV y chunk `INDY`. Los historiales
de producción por salida, monitor/exclusividad y cargos custom permanecen
parciales; `#329` continúa abierto.

Corrección vigente del scope de industrias: en `26a915db`,
`INDY.accepted[].history`, `accepted[].accumulated_waiting`,
`INDY.produced[].history` y `valid_history` ya alimentan el runtime para
cargos representables y vuelven al chunk `INDY` al exportar. Las menciones
anteriores que los describen como sólo conservados en round-trip son
históricas; permanecen parciales los cargos custom y los callbacks que todavía
no tienen call site.

Actualización #329-INDUSTRY-PRODUCED-HISTORY-050 (2026-09-03, commit
`26a915db`): `INDY.produced[].history` se hidrata y se actualiza por salida
durante la transferencia a estaciones o la carga directa. El rollover gira la
ventana nativa completa de 61 registros y el writer reemite los contadores
runtime; la regresión cubre transferencia, rollover e inspección del chunk.
Quedan parciales cargos custom y callbacks sin call site; `#329` sigue abierto.

Actualización #329-INDUSTRY-CARGO-MONITOR-051 (2026-09-03, commit
`036fda1f`): el runtime implementa `_cargo_pickups` y `_cargo_deliveries`
con el layout de bits de `CargoMonitorID`, activación explícita, contadores
saturantes de 32 bits y reset al consultar. La descarga registra porciones
aceptadas por industria y por el pueblo de la estación, y la recogida se
acredita sólo en la entrega final; `GameState` expone las consultas y limpieza.
La API Squirrel/GameScript equivalente, `exclusive_supplier`/neutral stations
y cargos custom aún requieren trabajo.

Actualización #329-INDUSTRY-EXCLUSIVE-SUPPLIER-052 (2026-09-03, commit
`470499ea`): `DeliverGoodsToIndustry` compara `exclusive_supplier` con
`Station::owner` y `station.serve_neutral_industries=false` restringe las
industrias con `neutral_station` a su estación asociada. `STNN.base.owner`,
`INDY.neutral_station` y `INDY.exclusive_supplier` se hidratan y se reemiten
con las referencias nativas; la configuración se conserva en `PATS`, con el
fallback histórico anterior a `SLV_SERVE_NEUTRAL_INDUSTRIES`. Las regresiones
cubren selección de estación, exclusión por compañía y round-trip. El binding
Squirrel/GameScript del monitor, cargos custom, scopes restantes y callbacks
sin call site permanecen parciales.

Actualización #329-SCRIPT-CARGO-MONITOR-053 (2026-09-03, commit
`8be6bbc6`): la fachada `ScriptCargoMonitor` publica los cinco métodos de la
API (`GetTown/IndustryDeliveryAmount`, `GetTown/IndustryPickupAmount` y
`StopAllMonitoring`). Las cuatro consultas devuelven `-1` para una compañía,
cargo o entidad fuera de rango y no activan una clave inválida; los monitores
válidos mantienen reset, saturación y estado efímero nativos. El binding VM
Squirrel/GameScript todavía no existe, ni se modelan cargos custom y scopes
restantes.

Actualización #329-INDUSTRY-SAV-054 (2026-09-03, commit `eb6bd78d`): el writer
de `INDY` reemite las filas `accepted` y `produced` cuyo `cargo_slot` no puede
resolverse con el catálogo vanilla del clima. Se conservan de forma opaca el
slot, waiting/stock, rate, fecha, acumulador y todos los historiales, incluso
cuando el GRF no está instalado; al volver a aparecer el catálogo la pasada de
rehidratación puede asociar la fila sin haber perdido datos. Esto resuelve la
interoperabilidad de round-trip, no la simulación del cargo custom: stocks,
entrega, producción y callbacks siguen fuera del modelo hasta disponer de una
representación dinámica.

Actualización #329-INDTILE-ANIMATION-055 (2026-09-03, commit `c88518c4`):
`IndustryAnimationTrigger` modela `ConstructionStageChanged`, `TileLoop`,
`IndustryTick`, `CargoReceived` y `CargoDistributed`. El scheduler real ya
dispara CB25 sólo para `TileLoop` (visitas), `IndustryTick` (intervalo de
producción) y `CargoReceived` (entrega confirmada); la pasada visual separada
avanza CB26/CB27 únicamente en teselas activas. La regresión evita que un tick
visual active CB25. `CargoDistributed`, `ConstructionStageChanged`, sonido y
los scopes restantes siguen parciales.

Actualización #329-INDTILE-ANIMATION-056 (2026-09-03, commit `aa289076`):
`CargoDistributed` ya tiene call site después de una transferencia efectiva a
estaciones; sólo se dispara cuando `TransportIndustryGoods` devuelve unidades
movidas y conserva el contexto parent de la huella. La regresión comprueba que
la máscara `CargoDistributed` no responde a `IndustryTick`. `ConstructionStageChanged`,
sonido y scopes restantes siguen parciales.

Actualización #329-INDTILE-ANIMATION-057 (2026-09-03, commit `ca2939a7`):
`ConstructionStageChanged` ya tiene call sites en la construcción inicial y en
los cambios de etapa observados por `TileLoop`. La primera llamada conserva el
flag upstream `var 18 |= 0x100`; las transiciones posteriores usan el ordinal
sin extensión. Ambos caminos hidratan el parent/PSA de la industria y tienen
regresión del callback. Quedan sonido, scopes restantes, cargos custom y la
generación automática fuera de este call site.

Actualización #329-INDTILE-CARGO-ACCEPTANCE-058 (2026-09-03, commit `67ef8101`):
las máscaras `0x2B`/`0x2C` de `IndustryTileSpecDef` ya tienen evaluación runtime.
CB2C selecciona tres slots locales de cargo de 5 bits y CB2B sus cantidades de
4 bits, usando el contexto completo de tesela e industria padre, CTT y writeback
de PSA. La tabla exacta alimenta `station_coverage_at_with_newgrf` y el call site
normal de `unload_vehicles`; un resultado cero no cae al proxy genérico de
`Goods`. `CALLBACK_FAILED`, máscara ausente y las APIs legacy mantienen el
fallback estático. La regresión cubre slots/cantidades y aceptación efectiva;
los cargos custom no resolubles, la reatachación económica y callbacks restantes
siguen pendientes, por lo que #329 continúa abierto.

Actualización #329-CUSTOM-CARGO-RUNTIME-059 (2026-09-04, commit `bd613e2a`):
`CargoSpec` conserva el `local_id` del GRF y asigna un ID global estable
(`31..62`) para que producción, aceptación y carga resuelvan
`CargoType::Custom`. Hasta 32 slots atraviesan stocks, packets, cobertura,
pagos, ratings, cargodist, refit y autoreplace cuando el catálogo está instalado.
El SAV nativo todavía no rehidrata esos slots, los cargos `63+` quedan opacos y
la CTT/GUI completa y los callbacks sin call site siguen parciales; una fila sin
`CargoSpec` se conserva para round-trip pero no se ejecuta.

Actualización #329-SAV-GLOBAL-CARGO-060 (2026-09-04, commit `566ce56a`):
la hidratación nativa aplica `SLV_55`: `STNN.goods`, `INDY.accepted/produced`,
`VEHS.common.cargo_type` y `LGRP.cargo` leen slots climáticos en saves legacy
y IDs globales en saves modernos. Los IDs globales `31..62` reactivan
`CargoType::Custom` para stocks, packets, historiales y vehículos aunque el
catálogo no esté instalado; `63+` permanece opaco. El writer convierte saves
legacy al formato moderno de 64 IDs. La semántica de peso, CTT, textos y
callbacks económicos todavía requiere `CargoSpec` y mantiene esta matriz
parcial para cargos sin definición NewGRF.

Actualización #329-SCRIPT-CARGO-MONITOR-061 (2026-09-04, commit `6266171f`):
las cuatro consultas de `ScriptCargoMonitor` aceptan `CargoType::Custom` cuando
el catálogo activo contiene su `CargoSpec`, y rechazan con `-1` los IDs custom
sin spec, como `ScriptCargo::IsValidCargo` de OpenTTD. El monitor conserva el
layout `CargoMonitorID` global de seis bits y registra entregas/recogidas custom
con activación, reset y saturación nativos. La fachada Squirrel/GameScript y
los callbacks económicos dependientes del catálogo siguen parciales.

Actualización #329-CARGO-WEIGHT-062 (2026-09-04, commit `fd573da5`):
la física vial recibe el catálogo de `CargoSpec` y aplica `prop 0x0F` a la masa
cargada antes de calcular potencia efectiva, esfuerzo tractor y aceleración.
Esto cubre cargos custom con peso explícito en buses, camiones y tranvías; el
fallback vanilla permanece para APIs sin catálogo. La masa de consist
ferroviario, el multiplicador `freight_trains` y las propiedades de GUI/CTT aún
no están cerrados.

- Resto de CBs houses / airports / industries / objects (incluidos los huecos que aún no tienen call site), cargo (excepto CB39/CB145). Stations aún requieren scopes completos y sonidos propios de tesela; el callback de sonido de vehículo ya cubre salida (incluido `sound_effect` de Action0), marcha, avería, túnel, efecto visual, carga/descarga y despegue/aterrizaje. RoadStops resuelve `45`/`46`/`47`, `60`–`65`/`69` y `66`/`67`/`68`/`6A`/`6B` al renderizar, en CB140–142 y en la randomización con pools de mundo. La importación `.sav` conserva el mapeo nativo `(GRFID, localidx)` y el estado de cada tesela; la API legacy sin catálogo mantiene fallback vanilla y un GRF ausente no puede reatajarse a una vista ejecutable.
- Scopes parent determinista/random, offsets relativos básicos, el tramo especial del primer vehículo contiguo con el mismo motor, la consulta `61→62` con segundo offset, el conteo `61→60` y los badges de vehículo/vía `0x64`/`0x65`/`0x7A` ya están cubiertos mediante GlobalVar `0x18`; los scopes parent de casa y objeto ya reciben el PSA del pueblo por GRFID cuando `CITY.psa_list` los asocia. Siguen pendientes los scopes parent completos de estación/industria y variables de casa/objeto que no sean ese storage.
- Storage persistente de industria/aeropuerto/pueblo: `INDY.psa`, `STNN.normal.airport.psa`, `CITY.psa_list` y `PSAC` se importan, hidratan sus referencias y exportan para los registros `7C` conocidos; casas y objetos leen el PSA del pueblo desde su scope parent y CB17/CB157 de construcción ya hacen writeback por GRFID. Los callbacks CB25/26/27 y la re-randomización de `IndustryTile` también escriben el PSA de la industria en `TileLoop`, `IndustryTick` y `CargoReceived`; siguen pendientes la invalidación tras mutaciones y los callbacks PSA de pueblo restantes. CB140–142 preserva `7C` de la estación pero no los scopes/áreas completos de `BaseStation`; CB14 aún no aporta el scope/regs de `BaseStation` ni layout 16-bit exacto; CB149 aún no aporta scope/vecinos ni strings GRF.
- Goldens tick-a-tick vs OpenTTD 15.3 para todos los features.
- Textos GRF de string (`0x40F` / `regs100`) en CB31: denegación genérica `NewGrfCallbackDenied`.

Actualización #329-CARGO-TRAIN-WEIGHT-063 (2026-09-04, commit `b32b87f4`):
`ConsistChanged` suma `CargoSpec::weight` por unidad cargada y actualiza
`cached_weight_t` y `cached_max_te_n` para la cabeza del tren después de
`LoadUnloadStation`. El catálogo activo se propaga tanto a motores vanilla como
a vagones NewGRF y cargos custom; la física ferroviaria usa esa masa en
aceleración y esfuerzo tractor. El refresh se hace después de cargar para no
alterar el lookahead de señales; `freight_trains`, CTT completa y otros
settings/propiedades económicos permanecen parciales.

Actualización #329-CARGO-FREIGHT-SETTING-064 (2026-09-04, commit `15c8bfcf`):
el setting `vehicle.freight_trains` se persiste en `GameState`/JSON y en
`PATS` como `UINT8`, con rango `1..=255`, default 1 y frontera de compatibilidad
`SLV_39`. El peso ferroviario aplica el multiplicador sólo cuando la
`CargoSpec` activa marca `is_freight`; las cargas no freight conservan su peso
normal. Todos los call sites de reconstrucción de consist y el refresh posterior
a carga/descarga reciben el valor persistido. La toolbar expone presets, pero la
edición arbitraria tipo slider, la CTT completa y los demás settings económicos
aún no tienen paridad.

Actualización #329-CARGO-FREIGHT-UI-065 (2026-09-04, commit `5e0938ff`):
`SetFreightTrains` valida y aplica el setting en runtime, reconstruyendo todos
los consist ferroviarios sin modificar el mapa. Ajustes muestra y cicla los
presets `1/2/4/8/16/32/64/128/255`, sincronizados con `GameState`; quedan
pendientes el control avanzado de valores arbitrarios y la UI del resto de
settings.

Actualización #329-CARGO-CTT-066 (2026-09-04, commit `b25a2362`):
`local_cargo_id_with_catalog`/`cargo_from_local_id_with_catalog` invierten la
CTT usando el label real del `CargoSpec`, incluidos cargos custom. Las vars de
carga `60`–`65`/`69` de estaciones y paradas viales recorren esos cargos cuando
el contexto de mundo lleva el catálogo activo; las APIs legacy siguen usando
el fallback sin catálogo. Los callbacks de animación, `AirportTiles`,
industria y variables ilimitadas todavía necesitan la misma propagación, por
lo que #329 continúa abierto.

Actualización #329-CARGO-CTT-067 (2026-09-04, commit `7782568d`): las rutas
runtime de CB140–142 para estaciones ferroviarias y waypoints reciben el
catálogo `CargoSpec` activo. `NewCargo`/`CargoTaken` codifican `param2` mediante
la CTT del GRF, y el contexto Action2 de cada tesela publica las variables
`60`–`69` con ese mismo catálogo; construcción, carga de vehículos, eventos de
economía y `TileLoop` usan las variantes catálogo-aware. La regresión `TOFU`
comprueba el índice CTT local 6 en toda la plataforma. AirportTiles, industria
y GUI/variables ilimitadas permanecen pendientes.
