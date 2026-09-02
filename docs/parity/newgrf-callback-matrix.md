# Matriz de callbacks NewGRF (CBID) — OpenTTD 15.3

Actualizada: **2026-09-02** (scope/metadata nativa de industrias).

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
`resolve_vehicle_modify_property_callback`,
`trigger_vehicle_randomisation_chain`,
`decode_vehicle_articulated_part`, `resolve_vehicle_articulated_part_callback`,
`resolve_vehicle_visual_effect_callback`, `vehicle_visual_effect_kind`,
`resolve_industry_tile_animation_callback`,
`resolve_industry_tile_random_trigger`
(`crates/openttdrs-core/src/newgrf_callback.rs`).

AirportTile animation uses `trigger_newgrf_airport_tile_animation`,
`trigger_newgrf_airport_animation_for_station` and
`step_newgrf_airport_tiles` (`crates/openttdrs-core/src/map/station_tile_anim.rs`).

## Por feature

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
| Houses (`07`) | `0x17` `CBID_HOUSE_ALLOW_CONSTRUCTION` | **soportado** (#266) | Call site: crecimiento físico del pueblo (`try_build_town_house`), antes de reservar el footprint; respeta su máscara y booleano de 8 bits |
| Houses (`07`) | `0x150` `CBID_HOUSE_DRAW_FOUNDATIONS` | **parcial runtime** | El renderer evalúa el callback con el scope de casa (etapa, edad, random, pueblo y parámetros GRF) antes de `FOUNDATION_LEVELED`; `CALLBACK_FAILED` conserva la fundación y un resultado cero la suprime, como `ConvertBooleanCallback` upstream. La regresión `newgrf_house_draw_foundations_callback_can_suppress_default` cubre una casa inclinada custom sin parent vanilla. Falta el resto de callbacks de casa y layouts/rotaciones avanzados. |
| Houses | resto `0x1A`–`0x1C`, `0x1E`–`0x21`, … | **almacenado** | `HouseSpecDef.callback_mask` |
| Houses | Action2 `TileLayoutSpriteGroup` (`TileSeq`) | **parcial runtime** | El catálogo conserva el grafo aunque la casa no tenga callbacks variables. El renderer resuelve la etapa/edad/terreno/random de la tesela y `0x42` consulta la zona del pueblo cuyo `TownID` está persistido en `MAP2` (con fallback al más cercano en mapas legacy); `0x44`, `0x60`/`0x61` calculan conteos por `HouseID` global y por pueblo desde una instantánea del mapa, y `0x62`/`0x63` consultan información/frame de teselas vecinas con wrap. Sustituye `s1`/`s2` cuando el layout es completo y materializa ground, parents y children con cajas `M(...)`, incluyendo la superficie de una fundación nivelada; layouts incompletos, sprites base y paletas especiales mantienen fallback vanilla atómico. Faltan conteos por clase, aceptación de estaciones y layouts 16-bit completos.
| Industry tiles (`09`) | `0x25` trigger, `0x26` next frame, `0x27` speed | **soportado** (#293) | `phase_tile_animation` ejecuta los tres con coordenada real, `param2=IndustryTick`, máscara Action0 y fallback `CALLBACK_FAILED` |
| Industry tiles (`09`) | `0x150` `CBID_INDTILE_DRAW_FOUNDATIONS` | **parcial runtime** | El renderer evalúa el callback con el scope de tesela/industria padre (etapa, random, terreno, pueblo, posición y parámetros GRF) antes de `FOUNDATION_LEVELED`; `CALLBACK_FAILED` conserva la fundación y cero la suprime, igual que `DrawNewIndustryTile` upstream. La regresión `newgrf_industry_draw_foundations_callback_can_suppress_default` cubre una tesela inclinada custom sin cimiento vanilla. Faltan callbacks de sonido, slope/autoslope y scopes avanzados. |
| Industry tiles | Action2 vars `0x40`–`0x44`, `0x60`–`0x62`, `0x7A` | **parcial runtime** | El renderer construye el scope por tesela con `m3` como random (incluidos triggers), etapa de obra, terreno, zona del pueblo más cercano, posición relativa, frame completo `m3hi`, información/frame/id de teselas vecinas y presencia de badges mediante GlobalVar `0x18`; el mismo contexto alimenta vistas planas y layouts `TileSeq`, con caché por fingerprint. `0x62` conserva los sentinelas `0xFFFF`/`0xFFFE` y traduce el local del mismo GRF. Faltan el resto de variables específicas del tile y callbacks de sonido/slope/autoslope. |
| Industry tiles | `0x2B`–`0x2C`, … | **almacenado** | `IndustryTileSpecDef.callback_mask` |
| Industries (`0A`) | `0x28` `CBID_INDUSTRY_LOCATION` | **parcial runtime** (#266) | Call site: `place_industry_spec_def_layout_sandbox` (la variante histórica usa layout 0); respeta el bit `Location`, valida y materializa el layout elegido, carga Action3→Action2, pasa `IACT_USERCREATION` (`param2=2`) y expone el scope de construcción con `0x7A` (badges), `0x80`/`0x81` (TileIndex), `0x82` (pueblo), `0x86` (layout cero-based), `0x87` (terreno), `0x88` (zona), `0x89`/`0x8D` (distancia), `0x8A` (altura), `0x8B` (distancia a agua) y `0x8F` (random). La instancia conserva `selected_layout` uno-based, `random`, fundador, fecha/tipo de construcción, año de última producción y flags para el scope padre y `INDY`; se permiten sólo `FAILED`/`0x400`. Siguen pendientes otros tipos de creación, strings de error GRF y la semántica de callbacks de GameScript |
| Industries | `0x29` `CBID_INDUSTRY_PRODUCTION_CHANGE`; `0x35` `CBID_INDUSTRY_MONTHLYPROD_CHANGE`; `0x15F` `CBID_INDUSTRY_PROD_CHANGE_BUILD` | **parcial runtime** | CB29 se ejecuta en el cambio diario de una industria NewGRF y CB35 después de actualizar las estadísticas mensuales; se decodifican `no-op`, halve/double, divide/multiply, increment/decrement, cierre, cambio estándar y set de `prod_level` vía registro `0x100`. CB15F fija el nivel inicial al fundar si el resultado está en `PRODLEVEL_MINIMUM..MAXIMUM`. Los Action2 `IndustryProductionSpriteGroup` v0/v1/v2 se parsean, se conservan y se resuelven a través de Action3→Action2 (incluidos grupos random/variational). |
| Industries | `0x01` `CBID_INDUSTRY_PRODUCTION_CARGO_ARRIVAL`; `0x02` `CBID_INDUSTRY_PRODUCTION_256_TICKS` | **parcial runtime** | Los grupos v0/v1/v2 se ejecutan al procesar entradas de estación y en el ciclo de producción; consumen las colas de cargos aceptados, agregan las dos salidas legacy y transportan salidas adicionales v2 a estaciones. Se respeta `again` con límite de 65536 iteraciones y los valores indirectos `7D` de v1/v2. Faltan el scope completo de industria, `ProdMultiHandling`/escala de cargo, traducción exacta de cargos custom, mensajes y storage SAV nativo de las colas. |
| Industries (cambio de nivel) | `0x29` `CBID_INDUSTRY_PRODUCTION_CHANGE`; `0x35` `CBID_INDUSTRY_MONTHLYPROD_CHANGE`; `0x15F` `CBID_INDUSTRY_PROD_CHANGE_BUILD` | **parcial runtime** | CB29 se ejecuta en el cambio diario de una industria NewGRF y CB35 después de actualizar las estadísticas mensuales; se decodifican `no-op`, halve/double, divide/multiply, increment/decrement, cierre, cambio estándar y set de `prod_level` vía registro `0x100`. CB15F fija el nivel inicial al fundar si el resultado está en `PRODLEVEL_MINIMUM..MAXIMUM`. `CALLBACK_FAILED` conserva no-op y no cae silenciosamente al algoritmo vanilla. Faltan scopes completos, mensajes y storage persistente de industria. |
| Industries | Scope `0x40`–`0xB4` y callbacks restantes | **parcial runtime** | El scope padre de las teselas expone esperas, distancia al agua, layout seleccionado/posición/dimensiones, cargos/salida, producción mensual, nivel, color aleatorio, contador y random persistente; los valores WORD conservan sus bits altos (`0x8A`/`0x8C`, `0x94`…`0xA5`, `0xAA`). También expone fundador (`0x45`/`0xA7`), fecha absoluta (`0x46`), flags (`0x47`), último año de producción (`0xA9`), entrega (`0xAC`), fecha relativa (`0xB0`) y tipo de construcción (`0xB3`), con `INVALID_OWNER` y cero como fallbacks de saves antiguos. Siguen fuera del modelo el PSA, `last_accepted`/historiales anidados y cargos custom no representables; `IndustrySpecDef.callback_mask` conserva callbacks aún no conectados. |
| Industries | `0x22`, `0x37`–`0x3B`, `0x3D`, `0x14A`+, … | **almacenado** | `IndustrySpecDef.callback_mask` |
| Airport tiles (`11`) / Airports (`0D`) | Action2 vars `0x41`/`0x42`/`0x43`/`0x44`/`0x60`–`0x62`/`0x7A` + `0x150`/`0x152`–`0x154` | **parcial runtime** / **almacenado** / **OOS** | El renderer resuelve por tesela la posición relativa, terreno, frame, layout padre, random y vecinos con caché por fingerprint; Action1/3 puede seleccionar una vista distinta por esos valores y conserva fallback vanilla. Action0 conserva frames/status/speed/triggers y las listas de badges traducidas por GlobalVar `0x18`; `0x42` consulta la zona del pueblo más cercano y `0x7A` devuelve presencia o `UINT_MAX` para un índice local fuera de tabla. `CB0x150` decide si se conserva la fundación nivelada en pendientes (`CALLBACK_FAILED`/no nulo = conservar, cero = suprimir); `CB0x152` se ejecuta para `Built`/`TileLoop`/`NewCargo`/`CargoTaken`/`AcceptanceTick`, recibe el cargo local en `var18` usando la CTT del GRF, `CB0x153` selecciona el próximo frame y `CB0x154` regula la cadencia `2^speed`, con `AnimatedTileList` persistida en JSON y secuencia vanilla como fallback. `AirplaneTouchdown` se conecta al cruce de fases de aterrizaje (y al nodo FTA cuando existe). Siguen pendientes foundations de compositor (`0x150` Action5/rotación de cimiento), rotaciones runtime y sonidos (#260) |
| Canals (`05`) | `0x147` sprite offset | **almacenado** | `CanalSpecDef.callback_mask` |
| RoadStops (`14`) | `0x13` `CBID_STATION_AVAILABILITY` | **soportado** | Máscara Action0 `0x11`, Action2/3 y call site query+execute de `PlaceBusStop`/`PlaceTruckStop`; `CALLBACK_FAILED` o booleano 8-bit no nulo permite |
| RoadStops | `0x140`–`0x142` animación + Action2 random | **parcial runtime** | Action0 `0x0D`/`0x0E`/`0x0F`/`0x10`; CB140 cubre `Built`/`TileLoop`, `NewCargo`/`CargoTaken`, `VehicleLoads`/`VehicleArrives`/`VehicleDeparts` vial y `AcceptanceTick`; CB141/CB142 conservan frame/activo y `7C` por parada. Cada tesela custom creada o recuperada desde JSON tiene spec/frame/activo/random propios, preservados al unir paradas; eventos de vehículo usan la tesela exacta y los de carga/aceptación recorren el stop completo. La ruta Action2 conserva la máscara `0x0D`, CTT/versión, bits de estación (16) + tesela (8), triggers pendientes y grupos random `any`/`all` alcanzables; ejecuta NewCargo, CargoTaken, carga, llegada y salida antes de CB140. El render y el scheduler resuelven Action3/Action2 por tesela con random/triggers, vista/tipo/terreno, road/tram (incluidos tipos externos del catálogo), frame, `param[]`, las vars de carga `60`–`65`/`69`, las vars de mundo `45` (zona/distancia a pueblo), `46` (distancia cuadrática) y `47` (propietario/AI/colores), y los offsets `66` (frame misma estación), `67` (land), `68` (info), `6A` (GRFID) y `6B` (id local): nibbles firmados, wrap de mapa y caché por `(var,param)` evitan congelar una variante vecina. `param2` lleva el ordinal del trigger y, para carga, el id CTT (o fallback histórico) en el byte alto. La randomización del scheduler recibe los pools de town/company/industries cuando corre desde `sim_step`; las APIs históricas sin catálogo conservan fallback vanilla, mientras que la ruta explícita `...with_catalog_and_road_types` y el renderer usan tipos externos. `69` conserva y rota los cuatro flags de `GoodsEntry`, mientras `65` usa el catchment vivo con pools de mundo y conserva el fallback estático en APIs legacy. La importación `.sav` ya decodifica `roadstopspeclist`/`roadstoptiledata`, conserva `(GRFID, localidx)`, frame y random por tesela y reata la spec cuando el catálogo NewGRF está disponible; los GRF no instalados siguen siendo no resolubles. |
| RoadStops / Stations | Action2 `TileLayoutSpriteGroup` (`TileSeq`) | **parcial runtime** | Se parsean `type`, ground, origen/extents, children, referencias custom Action1 y los índices de registros. Action3/2 se resuelve por vista, random y variables de la tesela. El renderer materializa layouts de road stops, waypoints y estaciones rail con suelo propio, cajas parent `M(...)`, children relativos y caché por fingerprint, incluyendo pendientes niveladas; el procesador aplica `DODRAW`, offsets de sprite, offsets de cajas/children, `var10` y draw mode `0x100`. Sprites base y paletas custom siguen en fallback vanilla atómico; callbacks avanzados y paletas fuera de la rampa de compañía continúan pendientes. |
| Objects | Action2 `TileLayoutSpriteGroup` (`TileSeq`) | **parcial runtime** | El catálogo conserva el grafo cuando el objeto publica layouts aunque no tenga callbacks variables. El renderer resuelve la tesela del footprint, materializa suelo y secuencia de parents/children con cajas `M(...)`, consulta `0x42` (fecha), `0x45` (zona/distancia Manhattan) y `0x46` (distancia euclídea al cuadrado), y expone `0x47` (color) y `0x48` (vista) desde la instancia `OBJS`; `0x60`/`0x61`/`0x62`/`0x63` consultan id, random, información y frame de teselas vecinas del mismo footprint, y `0x64` devuelve cantidad/distancia de instancias por tipo. La asociación objeto↔pueblo usa `Object::town` importado o el pueblo más cercano como fallback; los offsets solicitados se cachean por fingerprint de registros. Sprites base, paletas custom y layouts incompletos usan fallback vanilla atómico. Faltan callbacks de objeto (salvo el slope check), conteos por clase/catchment y layouts 16-bit completos. |
| Industry tiles | Action2 `TileLayoutSpriteGroup` (`TileSeq`) | **parcial runtime** | El catálogo conserva layouts aunque la tesela no tenga callbacks variables. El renderer resuelve Action2 con etapa, random y parámetros GRF, reemplaza el suelo/agua cuando el layout es completo, emite parents/children con cajas `M(...)` sobre la superficie que decide `CB0x150` y cachea cada pieza por fingerprint y color de compañía. Sprites base, paletas especiales y layouts incompletos mantienen fallback vanilla atómico; faltan scopes avanzados, callbacks de sonido/slope y variantes de animación/layout por etapa. |
| Objects (`0F`) | `0x157` `CBID_OBJECT_LAND_SLOPE_CHECK` | **parcial runtime** | Máscara Action0 `0x15` WORD, Action3→Action2 y call site query+execute de `BuildObject` por tesela. `param1=slope`, `param2=dy<<4\|dx`; el resultado aplica la inversión de bit 10 para GRF <8. El objeto conserva ahora town/build date/color/view y las variables de alcance `0x40`–`0x48`, `0x60`–`0x64` durante el render. Faltan callbacks de objeto adicionales, scopes/vecinos, string de error GRF y el fallback de pendiente completo de OpenTTD. |
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
| Industry tile `m3` random bits + `m6` triggers (reseed) | **Parcial runtime**: la ruta con catálogo conserva `m3`/triggers por tesela y deja vanilla en no-op; el fallback histórico sin catálogo sigue disponible para herramientas legacy |
| `ResolveRerandomisation` / Action2 random sprite groups por trigger | **Parcial runtime** (#266): los grupos alcanzables consumen sólo sus triggers y reseedean la máscara declarada; layouts/variables no random y callbacks de sonido/slope/autoslope siguen pendientes |
| Vehicle/station random Action2 (`0x80`/`0x83`/`0x84`) en resolve de sprites | **Parcial** (eval con `random_bits` de 16 bits; reseed gameplay vía `trigger_vehicle_randomisation_chain`; vehículos consumen `Callback32`, `NewCargo` y `Empty`, y conservan triggers pendientes) |
| RoadStops Action0 `0x0D` + Action2 random | **parcial runtime**: CTT/versión, eventos `NewCargo`/`CargoTaken`/llegada/salida/carga vial, `any`/`all`, reseed de bits base/tesela, JSON y selección visual Action2 dinámica con el contexto persistente. El renderer y los CB140–142 resuelven vars de mundo `45`/`46`/`47`, vars de carga `60`–`65`/`69` y `66`/`67`/`68`/`6A`/`6B` por offset; la randomización del scheduler también usa esos scopes y recibe los pools del mundo en `sim_step`. Spec/frame/activo/random son independientes por tesela en stops compuestos creados, JSON o importados desde `.sav` (`roadstopspeclist` + `roadstoptiledata`). El cliente reproduce los samples `NewGRF` de la cola global y los callbacks de sonido de vehículo cubren salida, marcha, avería, túnel, efecto visual, pago carga/descarga y despegue/aterrizaje; quedan las APIs legacy sin catálogo y GRF ausentes.
| `CBID_RANDOM_TRIGGER` genérico | **OOS** |

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
9. Industry tiles CB25/CB26/CB27 — trigger, next frame y velocidad en `phase_tile_animation` (FAILED observable).
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
23. Industries CB1/CB2 — consumo iterativo de entradas/salidas y `again` para cargos vanilla representables; faltan scopes/escala/cargos custom y persistencia SAV nativa.

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

- Resto de CBs houses / airports / industries / objects (incluidos los huecos que aún no tienen call site), cargo (excepto CB39/CB145). Stations aún requieren scopes completos y sonidos propios de tesela; el callback de sonido de vehículo ya cubre salida (incluido `sound_effect` de Action0), marcha, avería, túnel, efecto visual, carga/descarga y despegue/aterrizaje. RoadStops resuelve `45`/`46`/`47`, `60`–`65`/`69` y `66`/`67`/`68`/`6A`/`6B` al renderizar, en CB140–142 y en la randomización con pools de mundo. La importación `.sav` conserva el mapeo nativo `(GRFID, localidx)` y el estado de cada tesela; la API legacy sin catálogo mantiene fallback vanilla y un GRF ausente no puede reatajarse a una vista ejecutable.
- Scopes parent determinista/random, offsets relativos básicos, el tramo especial del primer vehículo contiguo con el mismo motor, la consulta `61→62` con segundo offset, el conteo `61→60` y los badges de vehículo/vía `0x64`/`0x65`/`0x7A` ya están cubiertos mediante GlobalVar `0x18`; siguen pendientes los scopes parent de estación/industria/casa/objeto con sus storage propios.
- Storage persistente en industria/casa y callbacks de estación que sí tengan scope de estación; CB140–142 preserva `7C` de la estación pero no los scopes/áreas completos de `BaseStation`; CB14 aún no aporta el scope/regs de `BaseStation` ni layout 16-bit exacto; CB149 aún no aporta scope/vecinos ni strings GRF.
- Goldens tick-a-tick vs OpenTTD 15.3 para todos los features.
- Textos GRF de string (`0x40F` / `regs100`) en CB31: denegación genérica `NewGrfCallbackDenied`.
