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
| `0B` | Cargoes | runtime parcial | pendiente | catálogo `cargo_spec` |
| `0C` | Sound effects | pendiente | pendiente | — |
| `0D` | Airports | pendiente | pendiente | — |
| `0E` | Signals | ignorada por spec (null en OTTD 15.3) | N/A | gráficos: RailTypes `RTSG_SIGNALS` + Action5 `0x04` |
| `0F` | Objects | runtime parcial | pendiente | catálogo `object_spec` |
| `10` | Rail types | runtime parcial | runtime por sprite type | construcción/render + techo velocidad |
| `11` | Airport tiles | pendiente | pendiente | — |
| `12` | Road types | runtime parcial | runtime | construcción/render + techo velocidad |
| `13` | Tram types | runtime parcial (mismo parser que Road) | pendiente | catálogo road (clase tram) |
| `14` | Road stops | runtime parcial | pendiente | catálogo `road_stop` |
| `15` | Badges | runtime parcial | no aplica | catálogo `badge` |

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
| `23` powered wagon weight | **runtime** |
| `27` misc flags (bit0 `RailTilts`) | **runtime** (`rail_tilts`) |
| `2E` curve speed mod | **runtime** |
| `0E`, `08`, `0A`, `0C`, `0F`–`11`, `18`–`1A`, `1C`–`22`, `25`–`26`, `28`–`2D`, `2F`–`31` | consumidas (ancho fijo / CTT / callbacks) |

## Road vehicles (`01`)

Fuente: `newgrf_act0_roadvehs.cpp`.

| Props | Estado |
|---|---|
| `08`, `15` velocidad | **runtime** |
| `09` running cost factor | **runtime** |
| `0F` capacidad | **runtime** |
| `10` carga default | **runtime**; pasajeros selecciona Bus, el resto Truck |
| `11` cost factor | **runtime** |
| `13` potencia (×10 HP) | **runtime** |
| `14` peso (cuartos de tonelada) | **runtime** |
| `05`, `0A`, `0E`, `12`, `16`–`1F`, `21`–`29` | consumidas cuando tienen ancho fijo; semántica pendiente |
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
| `08`, `09`, `10`–`1D`, `20`–`26` restantes | consumidas si tienen ancho fijo; semántica pendiente |
| `1E`, `1F` | pendiente: listas CTT variables |

## Aircraft (`03`)

Fuente: `newgrf_act0_aircraft.cpp`.

| Props | Estado |
|---|---|
| `0B` cost factor | **runtime** |
| `0C` velocidad (conversión `×128/10`) | **runtime** |
| `0E` running cost factor | **runtime** |
| `0F` capacidad de pasajeros | **runtime** |
| `08`–`0A`, `0D`, `11`–`1C`, `1F`–`24` restantes | consumidas si tienen ancho fijo; semántica pendiente |
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
| `14` max speed | **runtime** (`GameState.runtime.rail_type_max_speed` → techo tren) |
| resto tamaños fijos usados para avanzar el bloque | consumidas |

## Road types (`12`) / Tram types (`13`)

Fuente: `newgrf_act0_roadtypes.cpp` (TramTypes reutiliza el handler).

| Props | Estado |
|---|---|
| `08` short label | **runtime** |
| `14` max speed | **runtime** (`RoadTypeDef.max_speed` → techo RV) |
| `16` intro year (extensión local WORD) | **runtime** |
| `09` flags tram (extensión local en RoadTypes) | **runtime** (bit0); en TramTypes `0x09` es string WORD consumido |
| feature `13` | **runtime** parcial: parse/apply al catálogo road con `RoadTramType::Tram` |

## Cargoes (`0B`)

Fuente: `newgrf_act0_cargo.cpp`.

| Props | Estado |
|---|---|
| `08` bitnum BYTE | **runtime** (catálogo) |
| `17` label 4 chars | **runtime** (catálogo) |
| `FE` nombre C-string (extensión local) | **runtime** (catálogo) |
| resto | pendiente |

## Objects (`0F`)

Fuente: `newgrf_act0_objects.cpp`.

| Props | Estado |
|---|---|
| `08` class label 4 chars | **runtime** (catálogo) |
| `0C` size BYTE | **runtime** (catálogo) |
| `FE` nombre C-string (extensión local) | **runtime** (catálogo) |
| Action1/3 views | **runtime** parcial (si el GRF las trae) |
| resto | pendiente |

## Road stops (`14`)

Fuente: `newgrf_act0_roadstops.cpp`.

| Props | Estado |
|---|---|
| `08` class label 4 chars | **runtime** (catálogo) |
| `09` stop type BYTE (`0` bus / `1` truck) | **runtime** (catálogo) |
| `FE` nombre C-string (extensión local) | **runtime** (catálogo) |
| Action1/3 views | **runtime** parcial (si el GRF las trae) |
| resto | pendiente |

## Badges (`15`)

Fuente: `newgrf_act0_badges.cpp`.

| Props | Estado |
|---|---|
| `08` label 4 chars (scaffold; OTTD usa C-string) | **runtime** (catálogo) |
| `09` flags DWORD | **runtime** (catálogo) |
| `FE` nombre C-string (extensión local) | **runtime** (catálogo; sustituye label) |

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
