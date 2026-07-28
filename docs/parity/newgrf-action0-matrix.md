# Matriz Action0 / Action3 / Action5 (OpenTTD 15.3)

Referencia fijada: commit `14ec60f248547d4d062a1160f0fc26d742319888`, en
`reference/openttd-upstream/src/newgrf/`.

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
| `05` | Canals | pendiente | pendiente | — |
| `06` | Bridges | pendiente | pendiente | — |
| `07` | Houses | pendiente | pendiente | — |
| `08` | Global variables | runtime parcial | no aplica | tablas rail/road/tram |
| `09` | Industry tiles | runtime parcial | runtime | construcción/render industria |
| `0A` | Industries | pendiente | pendiente | — |
| `0B` | Cargoes | runtime | pendiente | catálogo `cargo_spec` → pagos/capacidad/UI |
| `0C` | Sound effects | runtime | no aplica | catálogo `sound_effect` + cola play; samples Action11 |
| `0D` | Airports | pendiente | pendiente | — |
| `0E` | Signals | ignorada por spec (null en OTTD 15.3; #255) | N/A | gráficos: RailTypes `RTSG_SIGNALS` + Action5 `0x04`; estilo en `m2` save/load |
| `0F` | Objects | runtime | runtime | catálogo `object_spec`; build+render multitile |
| `10` | Rail types | runtime | runtime (signals/underlay/overlay) | construcción/coste/compat + techo velocidad |
| `11` | Airport tiles | pendiente | pendiente | — |
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
| `0E`, `08`, `0A`, `0C`, `0F`–`11`, `18`–`1A`, `1C`, `1E`, `22`, `25`–`26`, `28`–`2D`, `2F`–`31` | consumidas (ancho fijo / CTT / callbacks) |

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
| `13` potencia (×10 HP) | **runtime** |
| `14` peso (cuartos de tonelada) | **runtime** |
| `05`, `0A`, `0E`, `16`–`1F`, `21`–`29` | consumidas cuando tienen ancho fijo; semántica pendiente |
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
| `14` ocean speed fraction | **runtime** (`ocean_speed_frac` → `ship_speed_for_tile`) |
| `15` canal speed fraction | **runtime** (`canal_speed_frac` → `ship_speed_for_tile`) |
| `08`, `09`, `11`–`13`, `16`–`1D`, `20`–`26` restantes | consumidas si tienen ancho fijo; semántica pendiente |
| `1E`, `1F` | pendiente: listas CTT variables |

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
| `08`, `0D`, `11`, `13`–`1C`, `1F`–`24` restantes | consumidas si tienen ancho fijo; semántica pendiente |
| `1D`, `1E` | pendiente: listas CTT variables |

## Stations (`04`)

Fuente: `newgrf_act0_stations.cpp`.

| Props | Estado |
|---|---|
| `08` class label | **runtime** |
| `0A` copy sprite layout | consumida (extended-byte id) |
| `0B` callback mask | consumida |
| `0C` disallowed platforms bitmask | **runtime** |
| `0D` disallowed lengths bitmask | **runtime** |
| `0E` custom tile layout | **runtime** |
| `0F` copy custom layout | **runtime** |
| `09` sprite layouts | pendiente (variable; no bloquea AC de catálogo/construcción) |
| short label del spec | derivado del nombre (no hay prop Action0 15.3) |
| Action3 cargo group / default | **runtime** (`views_for_local_id_cargo_ctx`; fallback verificable) |
| vars/CB dinámicas (`40`/`42`/`43`/`5F`/`10`/`67`, CB24) | **runtime** vía `Action2EvalCtx` compartido (#228) |

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
| `1D` capacity multiplier | **runtime** → compra vehículo |
| `FE` nombre C-string (extensión local) | **runtime** (UI / inspect) |
| strings WORD `09`–`0D` / `1B`/`1C` | consumidas |
| resto (`18`/`1A`/`1E`/`1F`) | consumidas (ancho fijo) |

## Signals (`0E`) — N/A Action0 en 15.3

OpenTTD 15.3 registra `GSF_SIGNALS` como `nullptr`. Gráficos:

| Fuente | Estado |
|---|---|
| Action5 `0x04` (240 slots) | **runtime** (no altera slots no escritos; vanilla si vacío) |
| RailTypes Action3 `RTSG_SIGNALS` | **runtime** (rojo/verde × tipo/variante/PBS) |
| tipo/variante en `Tile.m2` | **runtime** (save/load) |
| Action0 `0E` | **ignorada por spec** |

## Objects (`0F`)

Fuente: `newgrf_act0_objects.cpp`.

Consumidor (#261): `BuildObject` valida footprint W×H, clima y coste antes de
mutar; escribe todas las teselas (`m5` = id ≥ `NEW_OBJECT_OFFSET`, `m2` = offset);
`ClearTile` demuele el footprint completo. Action3 dibuja `views[i % len]` en
cada tesela. `grfid`/`local_id` persisten; NewGRF se re-aplica tras load.

| Props | Estado |
|---|---|
| `08` class label 4 chars | **runtime** (catálogo) |
| `0B` climate mask BYTE | **runtime** (filtra en `check_build_object`) |
| `0C` size BYTE | **runtime** (footprint W×H en build/clear/render) |
| `0D` build cost multiplier BYTE | **runtime** (`build_object_cost_factored`) |
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
| `12` flags DWORD (`DriveThroughOnly` bit3, `RoadOnly` bit5, `TramOnly` bit6, …) | **runtime** (validado en query+execute; resto almacenado) |
| `FE` nombre C-string (extensión local) | **runtime** (catálogo) |
| `FD` badge associations (extensión local: BYTE count + N× label 4 chars) | **runtime** (`associated_badges` + diagnósticos) |
| Action1/3 views | **runtime** parcial (bahía `0..3`; DT `4`/`5` si hay vistas; si no Action5 `0x11` / OpenGFX) |
| drive-through `m5`=`RSV_*` 4/5 | **runtime** (colocación + connect eje X/Y) |
| `grfid` + `newgrf_local_id` | **runtime** (save/load + rebind tras re-apply multi-GRF) |
| resto (`0x0A`–`0x0B`, `0x0D`–`0x11`, `0x13`–`0x16`) | consumidas (ancho fijo) / pendiente |

## Sound effects (`0C`)

Fuente: `newgrf_act0_sound.cpp` / Action11 (`newgrf_sound.cpp`).

Action0 `0C` solo ajusta volume/priority/override sobre samples registrados vía
Action11. Identidad runtime: `(grfid, local_id)` — dos GRFs con el mismo
`local_id` no se contaminan. Formato fixture Action11: `0x11`, count, luego
N× (`WORD` size LE + PCM mono u8). Action0 sin sample o Action11 truncado →
`GameState.runtime.newgrf_diagnostics`. Reproducción observable:
`play_newgrf_sound` / `pending_newgrf_sounds` (cliente Bevy puede drenar después).
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
en slots vecinos. Los tipos `A5BLOCK_INVALID` se inspeccionan, pero no se aplican.

| Tipo | Bloque 15.3 | Slots | Estado |
|---:|---|---:|---|
| `04` | signal graphics | 240 | **runtime** (#255; Action0 `0E` N/A en 15.3) |
| `05` | catenary | 36 | **runtime** |
| `06` | foundations | 90 | **runtime** |
| `07` | TTDP GUI | — | ignorada por spec (no usada por OTTD) |
| `08` | canals | 65 | pendiente (#259) |
| `09` | one-way roads | 18 | **runtime** |
| `0A` | 2CC colour maps | — | pendiente |
| `0B` | tramway | — | pendiente |
| `0C` | snowy temperate tree | — | ignorada por spec (`A5BLOCK_INVALID`; no es «grass» — #250 N/A en 15.3) |
| `0D` | shore | 18 | **runtime** |
| `11` | road stop graphics | 8 | **runtime** (no Action0 RoadStops) |
| `15` | OpenTTD GUI | 192 | **runtime** |
| `16` | airport preview | 9 | **runtime** |
| `1B` | bridge decks | 24 | **runtime** |
