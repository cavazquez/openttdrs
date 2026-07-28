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
| `04` | Stations | runtime parcial | runtime | picker, construcción, render |
| `05` | Canals | pendiente | pendiente | — |
| `06` | Bridges | pendiente | pendiente | — |
| `07` | Houses | pendiente | pendiente | — |
| `08` | Global variables | runtime parcial | no aplica | tablas rail/road/tram |
| `09` | Industry tiles | runtime parcial | runtime | construcción/render industria |
| `0A` | Industries | pendiente | pendiente | — |
| `0B` | Cargoes | pendiente | pendiente | — |
| `0C` | Sound effects | pendiente | pendiente | — |
| `0D` | Airports | pendiente | pendiente | — |
| `0E` | Signals | pendiente | pendiente | señales usan hoy RailTypes |
| `0F` | Objects | pendiente | pendiente | — |
| `10` | Rail types | runtime parcial | runtime por sprite type | construcción/render |
| `11` | Airport tiles | pendiente | pendiente | — |
| `12` | Road types | runtime parcial | runtime | construcción/render |
| `13` | Tram types | pendiente (alias incompleto) | pendiente | — |
| `14` | Road stops | pendiente | pendiente | — |
| `15` | Badges | pendiente | no aplica | — |

## Propiedades comunes de vehículos

Fuente: `newgrf_act0.cpp::CommonVehicleChangeInfo`.

| Prop | Semántica 15.3 | Estado |
|---:|---|---|
| `00` | fecha de introducción | **runtime** (año derivado de días) |
| `02` | decay speed | **runtime** (`reliability_spd_dec`) |
| `03` | vida del vehículo | **runtime** |
| `04` | vida del modelo | **runtime**; retira el modelo del catálogo salvo `0xFF` |
| `06` | climas disponibles | consumida; falta filtro de landscape |
| `07` | velocidad de carga | consumida; falta campo/callback de carga |

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

## Action3 de vehículos

Los features `00`–`03` comparten el grafo Action1 → Action2 → Action3. El
selector de cargo se consulta primero y cae al grupo default si no existe. El
renderer hace esta selección para trenes, vehículos de carretera, barcos y
aviones; cambiar/refitear la carga cambia también el grupo seleccionado.

## Action5

| Tipo | Bloque | Estado |
|---:|---|---|
| `05` | catenaria | **runtime**, 36 slots acotados |
| `0D` | costa | **runtime**, 18 slots y mapping especial 10/16 sprites |
| `04`, `06` | foundations | pendiente |
| `07` | GUI | pendiente |
| `08` | previews de aeropuerto | pendiente |
| `09` | road stops | pendiente |
| `0A` | one-way roads | pendiente |
| `0B` | bridges | pendiente |
| `0C` | grass | pendiente |

Un bloque soportado se recorta al rango de su propia tabla; nunca puede escribir
en slots vecinos. Los tipos pendientes se inspeccionan, pero no se aplican.
