# Matriz Action0 / Action3 / Action5 (OpenTTD 15.3)

Referencia fijada: commit `14ec60f248547d4d062a1160f0fc26d742319888`, en
`reference/openttd-upstream/src/newgrf/`.

Callbacks (CBID) y storage/triggers: ver
[`newgrf-callback-matrix.md`](newgrf-callback-matrix.md) (#228).

> Esta matriz mide propiedades de catálogo, compra, construcción y render. La
> única fuente para afirmar que un callback se **ejecuta** es la matriz de
> callbacks: que aquí una máscara figure como runtime o almacenada no declara
> un call site. En particular, CB17, CB28 y CB25–27 ya tienen call sites; los
> demás callbacks de esas features siguen siendo parciales u OOS.

Estados:

- **runtime**: la propiedad modifica un catálogo usado por compra, construcción o render.
- **consumida**: se valida su ancho para continuar el bloque, pero el port no anuncia su semántica.
- **ignorada por spec**: OpenTTD la acepta sólo por compatibilidad o no tiene efecto aplicable.
- **pendiente**: el feature todavía no tiene un catálogo runtime equivalente.

## Features

| ID | Feature 15.3 | Action0 | Action3 | Consumidor |
|---:|---|---|---|---|
| `00` | Trains | runtime parcial | runtime + cargo | compra, render, save/load |
| `01` | Road vehicles | runtime parcial | runtime + cargo | compra, render, save/load |
| `02` | Ships | runtime parcial | runtime + cargo | compra, render, save/load |
| `03` | Aircraft | runtime parcial | runtime + cargo | compra, render, save/load |
| `04` | Stations | runtime (salvo `09`) | runtime + cargo fallback | picker, construcción, render |
| `05` | Canals | runtime | runtime parcial | catálogo `canal_feature` + Action5 `0x08` |
| `06` | Bridges | runtime | N/A (sin Action3) | catálogo `bridge_spec` (13 slots in-place) |
| `07` | Houses | runtime | runtime | catálogo `house_spec` + overrides; crecimiento/render |
| `08` | Global variables | runtime parcial | no aplica | tablas rail/road/tram |
| `09` | Industry tiles | runtime | runtime | catálogo `industry_tile_spec` + overrides; render/place |
| `0A` | Industries | runtime | N/A (sprites vía tiles 09) | catálogo `industry_spec` + layouts/I/O; place |
| `0B` | Cargoes | runtime | runtime | catálogo `cargo_spec` → pagos/capacidad/UI + Action3 views |
| `0C` | Sound effects | runtime | no aplica | catálogo `sound_effect` + cola play; samples Action11 |
| `0D` | Airports | runtime parcial | runtime | catálogo `airport_spec_catalog` (≥10); Action3 purchase/default; build+save; FTA/callbacks bloqueados (#260) |
| `0E` | Signals | ignorada por spec (null en OTTD 15.3; #255) | N/A | gráficos: RailTypes `RTSG_SIGNALS` + Action5 `0x04`; estilo en `m2` save/load |
| `0F` | Objects | runtime | runtime | catálogo `object_spec`; build+render multitile |
| `10` | Rail types | runtime | runtime (signals/underlay/overlay) | construcción/coste/compat + techo velocidad |
| `11` | Airport tiles | runtime parcial | runtime | catálogo `airport_tile_spec_catalog` (≥74); Action3 views/subst; layouts `0xFE`; callbacks/anim consumidos (#260) |
| `12` | Road types | runtime | runtime | construcción/coste + techo velocidad |
| `13` | Tram types | runtime (feature propio + catálogo road) | runtime | IDs/clase tram separados |
| `14` | Road stops | runtime parcial | runtime parcial | auto-select / construcción / render (`road_stop_spec`) |
| `15` | Badges | runtime | no aplica | catálogo `badge` (merge por label); asociaciones roadstops/objects |

## Propiedades comunes de vehículos

Fuente: `newgrf_act0.cpp::CommonVehicleChangeInfo`.

| Prop | Semántica 15.3 | Estado |
|---:|---|---|
| `00` | fecha de introducción | **runtime** (año derivado de días) |
| `02` | decay speed | **runtime** (`reliability_spd_dec`) |
| `03` | vida del vehículo | **runtime** |
| `04` | vida del modelo | **runtime**; retira el modelo del catálogo salvo `0xFF` |
| `06` | climas disponibles | **runtime**; filtra el catálogo por landscape activo |
| `07` | velocidad de carga | **runtime**; limita carga y descarga gradual por tick |

## Trains (`00`)

Fuente: `newgrf_act0_trains.cpp`.

| Props | Estado |
|---|---|
| `05` track type BYTE | **runtime** (`required_rail_type` 0..3; `engine_compatible_with_rail`) |
| `09` velocidad WORD | **runtime** |
| `0B` potencia WORD | **runtime** |
| `0D` running cost factor | **runtime** |
| `12` sprite / image index | **runtime** (`train_image_index`) |
| `13` dual-headed | **runtime** |
| `14` capacidad | **runtime** |
| `15` cargo default | **runtime** |
| `16`/`24` peso BYTE/high | **runtime** (`weight_t`) |
| `17` cost factor | **runtime** |
| `1B` powered wagon power | **runtime** |
| `1D` refit mask WORD | **runtime** (`refit_mask` → `refittable_cargo_types_for_engine`) |
| `1F` tractive effort BYTE | **runtime** (`tractive_effort` → `engine_tractive_effort`) |
| `20` air drag BYTE | **runtime** (`air_drag` → `engine_air_drag` / consist) |
| `21` shorten factor BYTE | **runtime** (almacenado en `EngineDef.shorten_factor`) |
| `23` powered wagon weight | **runtime** |
| `27` misc flags (bit0 `RailTilts`) | **runtime** (`rail_tilts`) |
| `2E` curve speed mod | **runtime** |
| `1E` callback mask BYTE / `31` additional mask BYTE | **runtime** (`EngineDef.vehicle_callback_mask`; bit 7 habilita CB33) |
| `0E`, `08`, `0A`, `0C`, `0F`–`11`, `18`–`1A`, `1C`, `22`, `25`–`26`, `28`–`2D`, `2F`–`30` | consumidas (ancho fijo / CTT) |

## Road vehicles (`01`)

Fuente: `newgrf_act0_roadvehs.cpp`.

| Props | Estado |
|---|---|
| `08`, `15` velocidad | **runtime** |
| `09` running cost factor | **runtime** |
| `0F` capacidad | **runtime** |
| `10` carga default | **runtime**; pasajeros selecciona Bus, el resto Truck |
| `11` cost factor | **runtime** |
| `12` sound effect BYTE | **runtime** (`sound_effect`; `0`/`0xFF` = default) |
| `17` callback mask BYTE / `28` additional mask BYTE | **runtime** (`EngineDef.vehicle_callback_mask`; bit 7 habilita CB33) |
| `13` potencia (×10 HP) | **runtime** |
| `14` peso (cuartos de tonelada) | **runtime** |
| `05`, `0A`, `0E`, `16`, `18`–`1F`, `21`–`27`, `29` | consumidas cuando tienen ancho fijo; semántica pendiente |
| `20`, `24`, `25`, `2A` | pendiente: extended/listas variables |

## Ships (`02`)

Fuente: `newgrf_act0_ships.cpp`.

| Props | Estado |
|---|---|
| `0A` cost factor | **runtime** |
| `0B`, `23` velocidad BYTE/WORD | **runtime** |
| `0C` carga default | **runtime** |
| `0D` capacidad WORD | **runtime** |
| `0F` running cost factor | **runtime** |
| `10` sound effect BYTE | **runtime** (`sound_effect`) |
| `12` callback mask BYTE / `22` additional mask BYTE | **runtime** (`EngineDef.vehicle_callback_mask`; bit 7 habilita CB33) |
| `14` ocean speed fraction | **runtime** (`ocean_speed_frac` → `ship_speed_for_tile`) |
| `15` canal speed fraction | **runtime** (`canal_speed_frac` → `ship_speed_for_tile`) |
| `08`, `09`, `11`, `13`, `16`–`1D`, `20`–`21`, `23`–`26` restantes | consumidas si tienen ancho fijo; semántica pendiente |
| `1E` CTT include (lista variable) | **runtime** (`refit_mask` → `refittable_cargo_types_for_engine`; #274) |
| `1F` CTT exclude (lista variable) | consumida (bytes); exclude aún no resta de la máscara (#274 residual) |

## Aircraft (`03`)

Fuente: `newgrf_act0_aircraft.cpp`.

| Props | Estado |
|---|---|
| `09` helicopter flag | **runtime** (`is_helicopter` → `aircraft_is_helicopter_def`) |
| `0A` large aircraft flag | **runtime** (`is_large_aircraft`) |
| `0B` cost factor | **runtime** |
| `0C` velocidad (conversión `×128/10`) | **runtime** |
| `0E` running cost factor | **runtime** |
| `0F` capacidad de pasajeros | **runtime** |
| `12` sound effect BYTE | **runtime** (`sound_effect`) |
| `14` callback mask BYTE / `22` additional mask BYTE | **runtime** (`EngineDef.vehicle_callback_mask`; bit 7 habilita CB33) |
| `08`, `0D`, `11`, `13`, `15`–`1C`, `1F`–`21`, `23`–`24` restantes | consumidas si tienen ancho fijo; semántica pendiente |
| `1D`, `1E` | pendiente: listas CTT variables |

## Stations (`04`)

Fuente: `newgrf_act0_stations.cpp`.

| Props | Estado |
|---|---|
| `08` class label | **runtime** |
| `0A` copy sprite layout | consumida (extended-byte id) |
| `0B` callback mask | **runtime parcial**: se conserva en el spec; `Avail` ejecuta CB13, `DrawTileLayout` ejecuta CB14 en el renderer, `AnimationNextFrame`/`AnimationSpeed` habilitan CB141/CB142 y `SlopeCheck` ejecuta CB149 por tesela al consultar/construir. Restan otros bits, scopes/layouts dinámicos completos, strings y compatibilidad GRF <8 del CB149. |
| `0C` disallowed platforms bitmask | **runtime** |
| `0D` disallowed lengths bitmask | **runtime** |
| `0E` custom tile layout | **runtime** |
| `0F` copy custom layout | **runtime** |
| `13` general flags | **runtime parcial**: se conserva `Cb141RandomBits` (bit 2) y entrega bits aleatorios a CB141; los demás flags no tienen consumidor todavía |
| `16` animation info | **runtime parcial**: frames y estado de loop alimentan el scheduler CB140–142 por tesela (`m7`) |
| `17` animation speed | **runtime parcial**: velocidad base `2^speed` del scheduler CB140–142 |
| `18` animation triggers | **runtime parcial**: máscara CB140 para `Built`, `TileLoop`, `NewCargo`, `CargoTaken`, `VehicleLoads`, `VehicleArrives` y `VehicleDeparts` de tren (`TA_PLATFORM` al entrar en `BeginLoading` y al ejecutar `LeaveStation`), `AcceptanceTick` (`TA_WHOLE`, cada 250 ticks escalonado por StationID) y `PathReservation` (`TA_PLATFORM` al reservar por primera vez una tesela de estación). El disparador entrega el ordinal correcto en el byte bajo de `param2`; para carga, el byte alto usa la CTT Action0 GlobalVar `0x09` (o el bitnum/clima según versión GRF). Restan scopes de estación y sonidos propios de tesela. |
| `09` sprite layouts | pendiente (variable; no bloquea AC de catálogo/construcción) |
| short label del spec | derivado del nombre (no hay prop Action0 15.3) |
| Action3 cargo group / default | **runtime** (`views_for_local_id_cargo_ctx`; fallback verificable) |
| vars/CB dinámicas (`40`/`42`/`43`/`4A`/`5F`/`10`/`67`, CB24) | **runtime** vía `Action2EvalCtx` compartido; `4A` lee el frame `m7`; CB24 call site (#228) |

## Rail types (`10`)

Fuente: `newgrf_act0_railtypes.cpp`.

| Props | Estado |
|---|---|
| `08` label | **runtime** (señales / tablas) |
| `0E`/`0F` compatible / powered lists | **runtime** (`rail_type_props` → convert/vehicles) |
| `10` flags | **runtime** |
| `11` curve speed | **runtime** (almacenado) |
| `13` construction cost | **runtime** (`rail_build_cost_factored`) |
| `14` max speed | **runtime** (`rail_type_max_speed` / `rail_type_props`) |
| `17` introduction date | **runtime** (almacenado) |
| `1C` maintenance cost | **runtime** (almacenado) |
| Action3 signals / underlay / overlay | **runtime** (slots por `RailType`; fallback OpenGFX) |
| resto tamaños fijos | consumidas |

## Road types (`12`) / Tram types (`13`)

Fuente: `newgrf_act0_roadtypes.cpp` (TramTypes reutiliza el handler).

| Props | Estado |
|---|---|
| `08` short label | **runtime** |
| `0F` powered list | **runtime** (`RoadTypeDef.powered_mask`) |
| `10` flags | **runtime** |
| `13`/`1C` cost / maintenance | **runtime** (coste build factored) |
| `14` max speed | **runtime** (`RoadTypeDef.max_speed` → techo RV) |
| `16` intro year (extensión local WORD) | **runtime** |
| `09` flags tram (extensión local en RoadTypes) | **runtime** (bit0); en TramTypes `0x09` es string WORD consumido |
| feature `13` | **runtime**: `from_tramtypes_feature` + clase `Tram`; IDs ≠ road |
| Action3 views | **runtime** (grupo local; fallback OpenGFX) |

## Cargoes (`0B`)

Fuente: `newgrf_act0_cargo.cpp`.

| Props | Estado |
|---|---|
| `08` bitnum BYTE | **runtime** (catálogo) |
| `0F` weight | **runtime** (catálogo) |
| `10`/`11` transit periods | **runtime** → pagos |
| `12` initial payment | **runtime** → `payment_spec_for_cargo` |
| `13`/`14` colours | **runtime** (catálogo) |
| `15` freight | **runtime** (catálogo) |
| `16` classes | **runtime** (catálogo) |
| `17` label 4 chars | **runtime** (identidad; lookup case-insensitive) |
| `1A` callback mask BYTE | **parcial runtime**: bit `ProfitCalc` → CB39 al pagar cada packet; bit `StationRatingCalc` → CB145 en el barrido periódico de rating; demás bits OOS |
| `1D` capacity multiplier | **runtime** → compra vehículo |
| `FE` nombre C-string (extensión local) | **runtime** (UI / inspect) |
| strings WORD `09`–`0D` / `1B`/`1C` | consumidas |
| resto (`18`/`1E`/`1F`) | consumidas (ancho fijo) |
| Action1/3 views | **runtime** (`newgrf_views` en `CargoSpecDef`) |

## Signals (`0E`) — N/A Action0 en 15.3

OpenTTD 15.3 registra `GSF_SIGNALS` como `nullptr`. Gráficos:

| Fuente | Estado |
|---|---|
| Action5 `0x04` (240 slots) | **runtime** (no altera slots no escritos; vanilla si vacío) |
| RailTypes Action3 `RTSG_SIGNALS` | **runtime** (rojo/verde × tipo/variante/PBS) |
| tipo/variante en `Tile.m2` | **runtime** (save/load) |
| Action0 `0E` | **ignorada por spec** |

## Canals (`05`)

Fuente: `newgrf_act0_canals.cpp` / `newgrf.h` (`CanalFeature` / `CF_*`).

IDs `0..8` (`CF_END=9`). Action3 opcional adjunta vistas al feature.
Action5 tipo `0x08` (65 slots; `SPR_LOCK_*` desde offset 4). Construcción
`PlaceCanal`/`PlaceLock` no depende del catálogo; el override se observa en
`canal_feature_catalog` + `canal_action5_newgrf_sprites`.

| Props | Estado |
|---|---|
| `08` callback_mask BYTE | **runtime** (catálogo) |
| `09` flags BYTE | **runtime** (catálogo) |
| Action1/3 views | **runtime** parcial (`newgrf_views` por local_id) |
| Action5 `0x08` | **runtime** (65 slots; no clobber vecinos) |

## Bridges (`06`)

Fuente: `newgrf_act0_bridges.cpp`. OpenTTD muta los 13 slots vanilla in-place;
no hay Action3 de bridges. Último GRF del stack gana por `local_id`. Sin
override → costes/disponibilidad iguales a `_orig_bridge` / `BRIDGE_SPECS`.

| Props | Estado |
|---|---|
| `08` year BYTE (`0`=siempre; else `1920+year`) | **runtime** |
| `09` min length BYTE | **runtime** |
| `0A` max length BYTE (`>16` → unlimited) | **runtime** |
| `0B` price BYTE | **runtime** |
| `0C` speed WORD (`0`→`u16::MAX`) | **runtime** |
| `0D` sprite tables | consumida (ancho fijo; `has_custom_sprites`) |
| `0E` flags BYTE | consumida |
| `0F` year DWORD | **runtime** |
| `10`–`12` string IDs WORD | consumidas |
| `13` price WORD | **runtime** |
| `15` pillar extended list | consumida |
| `FE` nombre C-string (extensión local) | **runtime** |

## Houses (`07`)

Fuente: `newgrf_act0_houses.cpp` / GRFSpecs Action0/Houses.

Catálogo runtime `house_spec_catalog` (ids ≥ `NEW_HOUSE_OFFSET` = 110). El pool de
crecimiento (`pick_town_house_id_with_catalog`) combina vanilla + NewGRF 1×1/norte
multitile filtrando zona+clima+año; `callback_mask` (`0x14`/`0x1D`) se almacena
y CB17 se ejecuta al decidir construcción. Los demás callbacks de casas siguen
sin call site (ver `newgrf-callback-matrix.md`). Action3 adjunta `newgrf_views`; dibujo usa vistas
o fallback `subst_id` / `% 110` (`resolve_house_draw_id`). Overrides `0x15` →
`house_overrides[vanilla]`. Multitile: footprint N/E/W/S con ids consecutivos.

| Props | Estado |
|---|---|
| `08` substitute BYTE | **runtime** (obligatorio; define el slot) |
| `09` building flags BYTE | **runtime** (tamaño / church / stadium) |
| `0A` availability years WORD | **runtime** (`1920+lo/hi`; `>150` → max) |
| `0B` population BYTE | **runtime** |
| `0C` mail BYTE | **runtime** (catálogo) |
| `0D`–`0F` acceptance BYTE | consumidas |
| `10` WORD / `11` BYTE / `12` WORD | consumidas |
| `13` availability mask WORD | **runtime** (zonas+climas) |
| `14` callback lo BYTE | **runtime parcial**: CB17 se ejecuta al construir; resto sin call site |
| `15` override BYTE | **runtime** (`house_overrides`) |
| `16`–`17`, `19`–`1C` | consumidas |
| `18` probability BYTE | **runtime** |
| `1D` callback hi BYTE | **runtime parcial**: CB17 se ejecuta al construir; resto sin call site |
| `1E` DWORD / `1F` BYTE | consumidas |
| `20` watch list / `23` tile acceptance | consumidas (listas) |
| `21`/`22` long years WORD | **runtime** |
| `24` badge list | consumida |
| `FE` nombre C-string (extensión local) | **runtime** |
| Action1/3 views | **runtime** (`newgrf_views` / subst fallback) |

## Airports (`0D`) / Airport tiles (`11`)

Fuente: `newgrf_act0_airports.cpp`.

Catálogo tiles `airport_tile_spec_catalog` (gfx ≥74) y aeropuertos
`airport_spec_catalog` (ids ≥10). Apply: tiles antes que airports; layouts
`0x0A` resuelven `0xFE`→tile local. Construcción via
`SetCurrentAirportNewgrfSpec` + `PlaceAirportArea`. FTA y callbacks **bloqueados**
explícitamente (#260 / #228): `station_uses_airport_fta` es false si hay
`airport_newgrf_spec_id`. Action3 adjunta `newgrf_views` (tiles) y
purchase (`0xFF`)/default (airports); dibujo NewGRF o fallback `subst_id`
(`resolve_airport_tile_draw_gfx`); piezas de construcción usan siempre subst
(`resolve_airport_tile_piece_gfx`).

| Feature | Props runtime | Props consumidas |
|---|---|---|
| Airport tiles `11` | `08` subst, `09` override, `0E` callback_mask (almacenado); Action1/3 views | `0F`–`11` anim, `12` badges |
| Airports `0D` | `08` subst/disable, `0A` layouts, `0C` years, `0E` catchment, `0F` noise, `10` name, `11` maintenance; Action3 purchase/default | `0D` TTDP type (almacenado), `12` badges |

## Industry tiles (`09`)

Fuente: `newgrf_act0_industries.cpp` (`IndustrytilesChangeInfo`).

Catálogo runtime `industry_tile_spec_catalog` (gfx ≥ `NEW_INDUSTRYTILEOFFSET` = 175).
Overrides `0x09` → `industry_tile_overrides[vanilla]`. Acceptance y
`callback_mask` se almacenan; CB25/CB26/CB27 se ejecutan en tick de tiles y los
demás callbacks de industria siguen residuales (#228). Cargos vía
`GetCargoTranslation` / `cargo_spec` (#224). Action3 adjunta `newgrf_views`;
dibujo NewGRF o fallback `subst_id` (`resolve_industry_tile_draw_gfx`).

| Props | Estado |
|---|---|
| `08` substitute BYTE | **runtime** (obligatorio; define el slot) |
| `09` override BYTE | **runtime** (`industry_tile_overrides`) |
| `0A`–`0C` acceptance WORD | **runtime** (índice+amt; labels vía `cargo_spec`) |
| `0D` land shape flags BYTE | consumida |
| `0E` callback mask BYTE | almacenado; CB25/CB26/CB27 ejecutados, resto sin call site |
| `0F` anim info WORD | consumida |
| `10`–`12` anim/special BYTE | consumidas |
| `13` acceptance list | **runtime** (variable; labels vía `cargo_spec`) |
| `14` badge list | consumida (WORD count + n×WORD) |
| Action1/3 views | **runtime** (`newgrf_views` / subst fallback) |

## Industries (`0A`)

Fuente: `newgrf_act0_industries.cpp` (`IndustriesChangeInfo`).

Catálogo runtime `industry_spec_catalog` (ids ≥ `NEW_INDUSTRYOFFSET` = 37).
Apply: tiles (`09`) antes que industries. Layouts `0x0A` resuelven `gfx==0xFE`
→ tile local del mismo GRF a gfx global. Produced/accepted (`0x10`/`0x11` o
`0x25`/`0x26`) como índices+labels vía `cargo_spec`. `callback_mask`
(`0x21`/`0x22`) almacenado; CB28 se ejecuta al colocar (#228). Place: `place_industry_spec_def_sandbox`
escribe footprint con gfx NewGRF; producción/aceptación leen def cuando existe.

| Props | Estado |
|---|---|
| `08` substitute BYTE | **runtime** (obligatorio; define el slot) |
| `09` override BYTE | **runtime** (`industry_overrides`) |
| `0A` layouts | **runtime** (`0xFE` → tile local del GRF) |
| `0B` life type BYTE | consumida |
| `0C`–`0E` string IDs WORD | consumidas |
| `0F` fund cost BYTE | **runtime** (`cost_multiplier`) |
| `10` produced cargos (2 B) | **runtime** (índices+labels) |
| `11` accepted cargos (3 B +1) | **runtime** (índices+labels) |
| `12`/`13` production rates BYTE | **runtime** |
| `14` minimal cargo BYTE | consumida |
| `15` random sounds list | consumida |
| `16` conflicting (3 B) | consumida |
| `17`–`19` probs/colour BYTE | consumidas |
| `1A` behaviour DWORD | consumida |
| `1B`/`1F`/`24` string WORD | consumidas |
| `1C`–`1E` input multipliers DWORD | **runtime** (almacenados) |
| `20` prospecting DWORD | consumida |
| `21`/`22` callback mask BYTE | almacenado; CB28 ejecutado al colocar, resto sin call site |
| `23` removal cost DWORD | consumida |
| `25`/`26` cargo lists variables | **runtime** (índices+labels) |
| `27` production rates list | **runtime** |
| `28` multiplier table | **runtime** (almacenada) |
| `29` badge list | consumida |
| `FE` nombre C-string (extensión local) | **runtime** |

## Objects (`0F`)

Fuente: `newgrf_act0_objects.cpp`.

Consumidor (#261): `BuildObject` valida footprint W×H, clima y coste antes de
mutar; escribe todas las teselas (`m5` = id ≥ `NEW_OBJECT_OFFSET`, `m2` = offset);
`ClearTile` demuele el footprint completo. Si la máscara `0x15` activa
`SlopeCheck`, evalúa CB157 por tesela antes de mutar (`param1=slope`,
`param2=dy<<4|dx`). Action3 dibuja `views[i % len]` en cada tesela.
`grfid`/`local_id` persisten; NewGRF se re-aplica tras load.

| Props | Estado |
|---|---|
| `08` class label 4 chars | **runtime** (catálogo) |
| `0B` climate mask BYTE | **runtime** (filtra en `check_build_object`) |
| `0C` size BYTE | **runtime** (footprint W×H en build/clear/render) |
| `0D` build cost multiplier BYTE | **runtime** (`build_object_cost_factored`) |
| `15` callback mask WORD | **parcial runtime**: bit `SlopeCheck` → CB157 en query+execute; sin scopes completos/error string/fallback de pendiente upstream |
| `FE` nombre C-string (extensión local) | **runtime** (catálogo) |
| `FD` badge associations (extensión local: BYTE count + N× label 4 chars) | **runtime** (`associated_badges` + diagnósticos) |
| Action1/3 views | **runtime** (render `views[i % len]` por tesela del footprint) |
| resto | pendiente |

## Road stops (`14`)

Fuente: `newgrf_act0_roadstops.cpp` / `newgrf_roadstop.h`.

| Props | Estado |
|---|---|
| `08` class label 4 chars | **runtime** (catálogo) |
| `09` stop type BYTE (`0` bus / `1` truck / `2` all) | **runtime** (catálogo; validado en query+execute) |
| `0C` draw_mode BYTE (`Road`/`Overlay`/`WaypGround`) | **runtime** (catálogo; bits en `road_stop_spec`) |
| `0D` cargos de random triggers DWORD | **runtime parcial**: preserva la máscara local y la traduce con CTT/versión Action8; habilita re-randomización Action2 en NewCargo, CargoTaken, carga, llegada y salida vial. Falta estado independiente por cada tesela de una parada compuesta. |
| `12` flags DWORD (`DriveThroughOnly` bit3, `RoadOnly` bit5, `TramOnly` bit6, …) | **runtime** (validado en query+execute; resto almacenado) |
| `11` callback mask BYTE | **runtime parcial**: bit `Avail` ejecuta CB13 en picker/query+execute; `AnimationNextFrame`/`AnimationSpeed` habilitan CB141/CB142 y CB140 usa la máscara Action0 `0x10`. El render reevalúa la rama Action3/Action2 con el contexto local persistente; restan scopes completos de estación. |
| `0E` animation info | **runtime parcial**: frames/loop alimentan CB140–142, con frame/activo persistidos por parada |
| `0F` animation speed | **runtime parcial**: espera base `2^speed` del scheduler CB140–142 |
| `10` animation triggers | **runtime parcial**: `Built`, `TileLoop`, `NewCargo`, `CargoTaken`, `VehicleLoads`, llegada/salida vial y `AcceptanceTick`; CB140 recibe el ordinal y el id CTT de cargo en `param2`. Una parada compuesta/importada todavía no conserva estado separado por tesela. |
| `FE` nombre C-string (extensión local) | **runtime** (catálogo) |
| `FD` badge associations (extensión local: BYTE count + N× label 4 chars) | **runtime** (`associated_badges` + diagnósticos) |
| Action1/3 views | **runtime** parcial: bahía `0..3`; DT `4`/`5` si hay vistas; el renderer resuelve Action2 por parada con random/triggers, vista/tipo/terreno, road/tram, frame, `param[]` y los scopes vecinos `66`/`67`/`68`/`6A`/`6B`, y cachea también `(var,param)`; si no hay vista usa Action5 `0x11` / OpenGFX. Restan vars BaseStation `60`–`65`/`69`. |
| drive-through `m5`=`RSV_*` 4/5 | **runtime** (colocación + connect eje X/Y) |
| `grfid` + `newgrf_local_id` | **runtime** (save/load + rebind tras re-apply multi-GRF) |
| resto (`0x0A`–`0x0B`, `0x13`–`0x16`) | consumidas (ancho fijo) / pendiente |

## Sound effects (`0C`)

Fuente: `newgrf_act0_sound.cpp` / Action11 (`newgrf_sound.cpp`).

Action0 `0C` solo ajusta volume/priority/override sobre samples registrados vía
Action11. Identidad runtime: `(grfid, local_id)` — dos GRFs con el mismo
`local_id` no se contaminan. Formato fixture Action11: `0x11`, count, luego
N× (`WORD` size LE + PCM mono u8). Action0 sin sample o Action11 truncado →
`GameState.runtime.newgrf_diagnostics`. Reproducción observable:
`play_newgrf_sound` / `pending_newgrf_sounds`; el cliente Bevy drena la cola,
empaqueta el PCM como WAV 8-bit/11.025 kHz y lo reproduce por el mixer mundial
(el callback de sonido de vehículo cubre salida, marcha, avería, túnel,
efecto visual, carga/descarga y despegue/aterrizaje; los motores vanilla
conocidos conservan su muestra específica).
`override_old` (`0x0A`) rellena `runtime.sound_overrides[SoundId]`.

| Props | Estado |
|---|---|
| Action11 samples | **runtime** (PCM + `has_sample`) |
| `08` relative volume BYTE | **runtime** (default 128; clamp `0..=128`) |
| `09` priority BYTE | **runtime** |
| `0A` override old SoundId BYTE | **runtime** (`sound_overrides` si `< SOUND_COUNT`) |

## Badges (`15`)

Fuente: `newgrf_act0_badges.cpp`.

Catálogo global por label (case-insensitive): el mismo label en varios GRFs
produce un solo `BadgeDef` (sin colisión). Identidad: preferir `0xFE` C-string;
si no hay, `0x08` 4-char. `BadgeDef.grfid` se persiste (primer registrador).

Asociaciones: roadstops/objects referencian badges por label (`prop 0xFD` local);
apply resuelve contra `badge_catalog` (mismo GRF primero). Listas `0xFD`
truncadas / labels inválidos → `GameState.runtime.newgrf_diagnostics` y
`GrfInspectReport.warnings`. Inspector lista labels y asociaciones.

| Props | Estado |
|---|---|
| `08` label 4 chars | **runtime** (catálogo; fallback si no hay `FE`) |
| `09` flags DWORD | **runtime** (catálogo; merge actualiza flags) |
| `FE` nombre C-string (extensión local) | **runtime** (identidad preferida) |
| `FD` badge associations (en roadstops/objects) | **runtime** (`associated_badges` + diagnósticos) |

## Action3 de vehículos

Los features `00`–`03` comparten el grafo Action1 → Action2 → Action3. El
selector de cargo se consulta primero y cae al grupo default si no existe. El
renderer hace esta selección para trenes, vehículos de carretera, barcos y
aviones; cambiar/refitear la carga cambia también el grupo seleccionado.

## Action5

IDs y tamaños según `newgrf_act5.cpp` / `table/sprites.h` de OpenTTD 15.3.
Un bloque soportado se recorta al rango de su propia tabla; nunca puede escribir
en slots vecinos. Antes de aplicar un bloque se siguen los saltos `ActionD` +
`Action7`/`Action9` + `Action10` que dependen de `param[]` y del paisaje
(`0=temp`, `1=arctic`, `2=tropic`, `3=toyland`); así los bancos alternativos
de OpenGFX no se pisan entre sí. Las condiciones que exigen estado externo
(otros GRFs, cargos o tipos) aún no fuerzan una rama. Los tipos
`A5BLOCK_INVALID` se inspeccionan, pero no se aplican.

| Tipo | Bloque 15.3 | Slots | Estado |
|---:|---|---:|---|
| `04` | signal graphics | 240 | **runtime** (#255; Action0 `0E` N/A en 15.3) |
| `05` | catenary | 36 | **runtime** |
| `06` | foundations | 90 | **runtime** |
| `07` | TTDP GUI | — | ignorada por spec (no usada por OTTD) |
| `08` | canals | 65 | **runtime** (#259; `canal_action5_newgrf_sprites`) |
| `09` | one-way roads | 18 | **runtime** |
| `0A` | 2CC colour maps | 256 | **runtime** (#274; merge no-clobber; slots en `twocc_action5_newgrf_sprites`) |
| `0B` | tramway | 119 | **runtime** (#274; merge no-clobber; slots en `tramway_action5_newgrf_sprites`) |
| `0C` | snowy temperate tree | — | ignorada por spec (`A5BLOCK_INVALID`; no es «grass» — #250 N/A en 15.3) |
| `0D` | shore | 18 | **runtime** |
| `11` | road stop graphics | 8 | **runtime** (no Action0 RoadStops) |
| `15` | OpenTTD GUI | 192 | **runtime** |
| `16` | airport preview | 9 | **runtime** |
| `1B` | bridge decks | 24 | **runtime** |
