# Diseño incremental — openttdrs

## Por qué incremental y no por fases

El plan original organizaba el trabajo en **fases secuenciales** (mapa → economía → pathfinding → vehículos → …). Eso tiene un problema grave: **no hay nada jugable ni observable hasta muy tarde**, y las decisiones de diseño de las capas inferiores se toman sin retroalimentación real de las capas superiores.

El diseño incremental resuelve esto con una idea simple:

> Cada incremento entrega una rebanada delgada que atraviesa **todas las capas** (tipos en core, lógica de simulación, test, representación en Bevy) y deja el sistema en un estado completamente funcional y observable.

Esto significa:

- Siempre hay algo que corre y muestra progreso.
- Los errores de diseño aparecen pronto, cuando el coste de cambiarlos es bajo.
- Cada incremento es un PR razonable, pequeño y revisable.
- No hay "trabajo de infraestructura invisible" que dure semanas.

---

## Relación con OpenTTD upstream

El [informe de arquitectura](INFORME_ARQUITECTURA_OPENTTD.md) resume el código de `reference/openttd-upstream/` (Clases tile bit-packed, `TimerGameTick`, `CargoPacket`, YAPF, comandos, saveload, red). Estos incrementos **no copian** el upstream línea a línea; la tabla siguiente enlaza conceptos para cuando conviene mirar el original:

| Concepto upstream | Dónde en OpenTTD | Cómo se traduce aquí |
|-------------------|------------------|----------------------|
| `TileBase` + `TileExtended` (~10 B/tile), accessor `Tile` | `map_func.h` | MVP: `Vec<Tile>` o equivalente simple; optimización tipo SoA si el profiler lo pide. |
| `TileType` (11 variantes en 4 bits) | `tile_type.h` | **I1:** `TileKind` propio con subset jugable; no replicar todos los tipos C++ de entrada. |
| `TimerGameTick`, constantes `Ticks::*` | `timer/timer_game_tick.h` | **I0:** `GameTick`; **I2:** usar **`INDUSTRY_PRODUCE_TICKS = 256`** como periodo por defecto de producción (literal del upstream). `DAY_TICKS = 74` si más adelante se calibra UI “por día”. |
| Industria `ProducedCargo` / `AcceptedCargo`, `PRODLEVEL_*` | `industry.h` | **I2:** MVP con `stock` y tasas fijas; historia mensual y cierre por abandono después. |
| `CargoPacket` (origen, `periods_in_transit`, …) | `cargopacket.h` | **I4:** balances `u32` bastan para el primer ciclo; packets completos cuando importe rating/pago realista. |
| Jerarquía `Vehicle`, movimiento sub-tile | `vehicle_base.h` | **I3:** salto tesela a tesela; `progress` y distancias axiales (`TILE_AXIAL_DISTANCE`) cuando el MVP lo exija. |
| Órdenes `Order`, listas compartidas | `order_base.h` | **I4:** cola mínima “ir a estación”; órdenes compartidas más tarde. |
| YAPF (siguiente tramo, cachés, regiones agua) | `pathfinder/yapf/` | **I5:** BFS que devuelve **`Vec` de teselas** es deliberado y más simple; migrar a heurística tipo A* si el mapa crece. |
| Registro masivo de comandos `*_cmd` | `command.cpp` | **I6:** mismo patrón abstracto (`Command` + `apply`), sin el árbol enorme del upstream. |
| `SaveLoadVersion` (`SLV_*`) | `saveload/saveload.h` | **I7:** formato propio versionado; **parcial:** `scripts/parse_sav.py` lee `.sav` → `.ottdmap` (solo mapa, no economía). |
| Red = replay de comandos + hash estado | `network/` / `openttdrs-net` | **I8 MVP (jul 2026):** lockstep TCP, dedicated, host migration — [ADR 0001](adr/0001-multiplayer-v1.md), [ADR 0004](adr/0004-host-migration-post-v1.md). |

---

## Estado actual del código (jul 2026)

Los incrementos **I0–I7** y el **MVP de I8** están en `main`. El hito **0.1** = partida en solitario jugable; la red ya no bloquea ese cierre.

| Capa | Qué hay hoy |
|------|-------------|
| `openttdrs-core` | Mapa TNBP/JGR, comandos road/rail, PBS/YAPF parcial, economía multi-compañía, NewGRF parse/Action2, IA TransCargo, save JSON + `.sav` parcial. |
| `openttdrs-client` | Vista isométrica OpenGFX, toolbar, menús, noticias, `--server` / `--client`. |
| `openttdrs-net` | TCP lockstep + `openttdrs-dedicated`. |
| Scripts | `parse_sav.py`, `descargar_assets.sh`, `doctor.sh`, validación TNBP en CI. |
| Docs | [docs/README.md](README.md), [SIGUIENTES_PASOS.md](SIGUIENTES_PASOS.md), [PARIDAD_OPENTTD.md](PARIDAD_OPENTTD.md). |

**Carreteras en mapas reales:** orientación desde `mapt` + `m5` (normal, cruce a nivel,
depósito, túnel/puente carretera). Los PNG `road_tx` / `road_ty` se asignan **cruzados**
respecto a `RoadDir` para alinear la textura con la proyección del cliente (~90° respecto
a “nombre de archivo = eje”); validado en pantalla.

Detalle operativo y hallazgos: [SIGUIENTES_PASOS.md](SIGUIENTES_PASOS.md). Gaps: [PARIDAD_OPENTTD.md](PARIDAD_OPENTTD.md).

---

## Roadmap y prioridades

### Principio

1. **Solitario jugable** (construir, simular, guardar/cargar) — prioridad de producto del 0.1.
2. **I8 red** — MVP ya mergeado; pulido desync/UI es trabajo posterior, no bloquea el 0.1.

### Hito 0.1 — vertical slice en solitario

| Fase | Objetivo | Ejemplos de trabajo |
|------|----------|---------------------|
| **SP1 — Ciclo jugable** | Partida local con bucle claro: industria → estación → vehículo → carga/entrega → economía visible | Feedback HUD (sin ruta, dinero, órdenes), coherencia estación en mapa vs `state.stations`, pausa/velocidad, pruebas de integración comando↔sim |
| **SP2 — Construcción y herramientas** | **Cerrado** (SP2.6 manual 2026-05-22) — [archive/SP2_CHECKLIST.md](archive/SP2_CHECKLIST.md) | Mensajes HUD, preview, transporte, paradas, tren, industria, órdenes |
| **SP3 — Presentación del mapa** | Que el mapa **se lea** como OpenTTD, sin exigir paridad total | ✅ S3 cerrado (jul 2026): slope/junctions, culling, industrias 0–174 — [archive/ROADMAP_PARIDAD_VISUAL.md](archive/ROADMAP_PARIDAD_VISUAL.md) |
| **SP4 — Pulido y deuda** | Estabilidad antes de abrir nuevas grandes features | Migraciones de save si hace falta, `check.sh` alineado con CI, bootstrap demo sin inconsistencias tile/estación, documentación al día |

**Criterio de “0.1 hecho”:** una sesión en solitario de ~15–30 minutos donde se puede **construir red y estaciones**, **asignar órdenes**, **ver vehículos y economía evolucionar**, **guardar y reanudar** sin pasos manuales raros — sin necesidad de red ni segundo cliente.

### I8 Red — MVP hecho (jul 2026)

| Incremento | Estado | Notas |
|------------|--------|--------|
| **I8** | ✅ MVP | TCP lockstep, dedicated, `--server` / `--client`, host migration (#171). Spec histórica en [§ Incremento 8](#incremento-8--dos-instancias-comparten-el-mundo-backlog). Pendiente: UX desync / lobby. |

Cadena técnica [#14](https://github.com/cavazquez/openttdrs/issues/14)–[#21](https://github.com/cavazquez/openttdrs/issues/21): **I0–I8 MVP hechos**. Priorizar SP / UI / gaps en [PARIDAD_OPENTTD.md](PARIDAD_OPENTTD.md).

---

## Los incrementos

Cada incremento tiene esta estructura:

- **Qué añade al core**: tipos nuevos o extensiones de los existentes (sin romper tests previos).
- **Qué añade a los tests**: invariantes nuevas sobre lo que se agrega.
- **Qué muestra el cliente**: cambio visible en la ventana de Bevy.
- **Frontera clara**: qué queda explícitamente fuera de ese incremento.

---

### Incremento 1 — "Una tesela tiene tipo"

**Objetivo**: el mapa deja de ser solo alturas y adquiere semántica mínima.

**Core — qué añadir:**

```
map.rs
  Tile { height: u8, kind: TileKind }

  enum TileKind {
      Grass,
      Water,
      Forest,
      CoalField,
  }
```

`Map::new_flat` produce un mapa de `TileKind::Grass`. Métodos `set_kind` / `get_kind` simétricos a `set_height`.

**Tests:**
- `tile_kind_default_is_grass()`
- `tile_kind_roundtrip()`
- La altura sigue siendo independiente del tipo.

**Cliente Bevy:**
- Color de cada tesela depende de `TileKind` (verde → bosque, azul → agua, gris oscuro → carbón, verde claro → prado).
- Semilla pseudoaleatoria fija para distribuir tipos en la carga inicial (solo visual, sin RNG en core).

**Fuera de este incremento:** industrias, producción, vehículos, comandos del jugador.

**Referencia upstream:** `TileType` enum en `tile_type.h` (Clear, Railway, Road, House, Trees, Station, Water, Void, Industry, TunnelBridge, Object). Aquí los nombres y la granularidad son deliberadamente más simples para el MVP.

---

### Incremento 2 — "Una industria existe en el mapa"

**Objetivo**: introducir el primer objeto de simulación encima del mapa.

**Core — qué añadir:**

```
industry.rs
  enum IndustryKind { CoalMine, Forest }

  struct Industry {
      pos:   TileCoord,
      kind:  IndustryKind,
      stock: u32,   // unidades de cargo en almacén
  }

GameState {
    map:        Map,
    tick:       GameTick,
    industries: Vec<Industry>,   // ← nuevo
}
```

`GameState::step()` llama a `Industry::produce(&mut self, tick)` cada **256 ticks** (`INDUSTRY_PRODUCE_TICKS` en el upstream) incrementando `stock` en cantidad fija. Determinista, sin RNG.

**Tests:**
- `industry_produces_on_schedule()` — después de N ticks el stock aumenta la cantidad esperada.
- `industry_does_not_exceed_capacity()` — stock tiene tope configurable.
- Paso de dos mundos con misma seed → mismo stock (determinismo).

**Cliente Bevy:**
- Las industrias se dibujan como un punto o cuadrado de color sobre la tesela correspondiente.
- Texto o color de intensidad que refleja `stock` actual.

**Fuera:** transporte de cargo, estaciones, jugador.

**Referencia upstream:** struct `Industry` con `ProducedCargo` / `AcceptedCargo`, niveles `PRODLEVEL_*`, abandono tras años económicos (`industry.h`). MVP: solo producción periódica y tope de stock.

---

### Incremento 3 — "Un vehículo existe y se desplaza"

**Objetivo**: el primer objeto móvil, sin pathfinding real todavía.

**Core — qué añadir:**

```
vehicle.rs
  enum VehicleKind { Truck }

  struct Vehicle {
      id:       u32,
      kind:     VehicleKind,
      pos:      TileCoord,
      dest:     TileCoord,
      cargo:    u32,
  }

GameState {
    ...
    vehicles: Vec<Vehicle>,   // ← nuevo
}
```

Movimiento por pasos: cada tick el vehículo avanza **una tesela** en la dirección cardinal que reduce la distancia Manhattan al destino (sin buscar camino). Si llega, invierte destino. Sin colisiones por ahora.

**Tests:**
- `vehicle_moves_toward_dest()` — después de N ticks está más cerca.
- `vehicle_inverts_on_arrival()` — llega al destino, gira.
- Paso de dos mundos idénticos → misma posición en tick T (determinismo).

**Cliente Bevy:**
- Los vehículos se dibujan como puntos blancos en movimiento sobre el mapa.

**Fuera:** pathfinding real, vías, carga/descarga, estaciones.

**Referencia upstream:** pool global `_vehicle_pool`, campos `tile`, órdenes, `VehicleCargoList` (`vehicle_base.h`). MVP: vector simple y movimiento Manhattan sin sub-tile.

---

### Incremento 4 — "Un vehículo recoge y entrega cargo"

**Objetivo**: primer ciclo económico completo, aunque primitivo.

**Core — qué añadir:**

```
station.rs
  struct Station {
      pos:   TileCoord,
      stock: u32,
  }

GameState {
    ...
    stations: Vec<Station>,   // ← nuevo
}
```

Reglas de `step()`:
- Si un vehículo llega a la posición de una industria y `cargo == 0`: toma `min(stock_industria, capacidad_vehiculo)` de carga.
- Si un vehículo llega a la posición de una estación y `cargo > 0`: entrega toda la carga, la estación acumula `income` (simple contador).
- Vehículo invierte destino tras cada operación.

**Tests:**
- `vehicle_loads_from_industry()`.
- `vehicle_delivers_to_station()`.
- `economic_cycle_roundtrip()` — tras N ciclos el income de la estación es el esperado.

**Cliente Bevy:**
- Estaciones: cuadrado de color distinto.
- `income` de la estación visible como número flotante o intensidad.

**Fuera:** dinero del jugador, costes, UI de construcción.

**Referencia upstream:** `CargoPacket` con origen y envejecimiento (`cargopacket.h`); pagos con inflación (`economy_type.h`). MVP: contadores `u32` y `income`; sin rating ni feeder_share hasta que el modelo lo requiera.

---

### Incremento 5 — "El mapa tiene vías y el vehículo las sigue"

**Objetivo**: primer pathfinding real, acotado a grafos pequeños.

**Core — qué añadir / cambiar:**

```
map.rs
  TileKind añade: Road, Rail

pathfinder.rs
  fn find_path(map: &Map, from: TileCoord, to: TileCoord) -> Option<Vec<TileCoord>>
```

BFS sobre teselas adyacentes con `TileKind::Road` o `Rail`. El vehículo sigue el camino resultante tesela a tesela. Si no hay camino, se detiene.

**Tests:**
- `bfs_finds_path_on_straight_road()`.
- `bfs_returns_none_when_blocked()`.
- `vehicle_follows_path()` — posición en cada tick coincide con el camino devuelto.
- Benchmarks básicos con mapas de distinto tamaño (solo `#[bench]` o inline en test).

**Cliente Bevy:**
- Las vías se dibujan en la rejilla (color diferente).
- El vehículo solo se mueve si hay camino trazado.

**Fuera:** construcción de vías por el jugador, señales, múltiples modos de transporte.

**Referencia upstream:** YAPF elige **solo el siguiente tramo** (`yapf.h`, plantillas en `yapf_*.cpp`); barcos usan regiones de agua (`water_regions.*`). Aquí BFS devuelve una ruta explícita por simplicidad en mapas pequeños.

---

### Incremento 6 — "El jugador construye vías"

**Objetivo**: primer input del jugador que modifica el estado del core.

**Core — qué añadir:**

```
command.rs
  enum Command {
      PlaceRoad(TileCoord),
      PlaceStation(TileCoord),
  }

  fn apply(state: &mut GameState, cmd: Command) -> Result<(), CommandError>
```

`Command` es un tipo de datos **serializable** (sin dependencia de Bevy). `apply` valida (no se puede poner vía en agua, etc.) y muta el `GameState`.

**Tests:**
- `place_road_mutates_tile_kind()`.
- `place_road_on_water_returns_error()`.
- `command_sequence_is_deterministic()` — la misma lista de comandos produce el mismo estado.

**Cliente Bevy:**
- Click izquierdo sobre una tesela lanza `Command::PlaceRoad`.
- Click derecho: `Command::PlaceStation`.
- El mapa se actualiza en pantalla en el siguiente frame.

**Fuera:** coste económico del comando, deshacer, red.

**Referencia upstream:** `command.cpp` enlaza cientos de handlers `*_cmd`; flags offline/servidor (`command_func.h`). MVP: serialización + validación local como base para red posterior.

---

### Incremento 7 — "El estado persiste en disco"

**Objetivo**: save/load sin perder nada del estado actual.

**Core — qué añadir:**

Dependencia opcional: `serde` + `serde_json` (o `bincode`) solo en feature `save`.

```
save.rs
  fn save(state: &GameState, path: &Path) -> Result<(), SaveError>
  fn load(path: &Path)          -> Result<GameState, SaveError>
```

`GameState`, `Map`, `Industry`, `Vehicle`, `Station` derivan `Serialize` / `Deserialize`.

**Tests:**
- `save_load_roundtrip()` — estado antes y después de save/load es idéntico.
- `save_load_after_N_steps()` — determinismo no se rompe al reanudar.

**Cliente Bevy:**
- **Hecho:** `F5` / **Ctrl+S** guardan y `F9` / **Ctrl+L** cargan (ruta configurable); formato versionado en `openttdrs_core::save`.

**Fuera:** migraciones entre versiones de save; compatibilidad con `.sav` OpenTTD (sigue siendo `parse_sav` → `.ottdmap`).

**Referencia upstream:** `SaveLoadVersion` inmutable y tablas por subsistema (`saveload/saveload.h`, `*_sl.cpp`). MVP: campo `version` en el JSON del envoltorio (`save.rs`).

---

### Incremento 8 — "Dos instancias comparten el mundo" (backlog)

> **Prioridad:** la más baja del proyecto hasta cerrar el hito **0.1 en solitario** (fases SP arriba). No bloquea guardados, construcción ni simulación local.

**Arquitectura v1 (ADR):** [adr/0001-multiplayer-v1.md](adr/0001-multiplayer-v1.md) — lockstep de comandos, listen-server + dedicated headless; **sin host migration** en v1 (recuperación por save/restart o dedicated).

**Objetivo**: multijugador mínimo basado en replicación de comandos.

**Core — qué añadir / verificar:**

- `Command` ya es serializable (Incremento 6).
- ✅ `GameState::apply_command_log(cmds: &[Command])` — reproduce una lista (ticks aparte vía `step`).
- ✅ `GameState::canonical_hash()` — fingerprint persistido (#108).
- RNG del core debe ser semillado explícitamente si se introduce (hoy CargoDist RNG persistido #107).

**Infraestructura:**
- ✅ Crate `openttdrs-net` — TCP length-prefixed JSON, `ListenServer` / `ClientSession`.
- ✅ Bin `openttdrs-dedicated` — headless (`--bind HOST:PORT`).
```
TCP: servidor envía Commit / AdvanceTicks / HashCheck;
clientes aplican el log y avanzan ticks sincrónicos.
```

**Tests:**
- ✅ `two_worlds_same_log_same_state()` / `desync_detected_on_hash_mismatch()` en `tests/command_log_desync.rs`.
- ✅ `tests/tcp_lockstep.rs` en `openttdrs-net`.

**Cliente Bevy:**
- ✅ Args `--server [bind]` / `--client <addr>` (`network::parse_net_cli`).

**Fuera:** seguridad, cheating, latencia, reconexión, host migration (ADR 0001).

**Referencia upstream:** los clientes aplican la misma secuencia de comandos que el servidor; desync por divergencia de estado (`network_*`). MVP: misma disciplina determinista + hash opcional del `GameState`.

---

## Resumen visual

```mermaid
flowchart TB
  subgraph foundation ["Fundación I0–I7 (en main)"]
    direction LR
    I0["I0: Grid + tick"] --> I1["I1: Tipos"] --> I2["I2: Industria"]
    I2 --> I3["I3: Vehículo"] --> I4["I4: Cargo"] --> I5["I5: Vías BFS"]
    I5 --> I6["I6: Comandos"] --> I7["I7: Save"]
  end

  subgraph solo ["0.1 — Solitario (prioridad actual)"]
    direction LR
    SP1["SP1: Ciclo jugable"] --> SP2["SP2: Construcción UI"]
    SP2 --> SP3["SP3: Visual mapa"] --> SP4["SP4: Pulido"]
  end

  subgraph later ["Post-0.1 — baja prioridad"]
    I8["I8: Red / multijugador"]
  end

  I7 --> SP1
  SP4 -.->|"solo cuando 0.1 esté cerrado"| I8
```

La cadena **I0→I7** es la base técnica ya mergeada: cada incremento extiende los tipos anteriores. El trabajo **actual** avanza por **SP1→SP4** en paralelo donde tenga sentido (visual y gameplay no son estrictamente secuenciales). **I8** no forma parte del cierre del 0.1.

---

## Reglas de trabajo

1. **Un incremento = un PR** (o varios commits en la misma rama). Nunca mezclar dos incrementos.
2. **Los tests del incremento anterior no pueden romperse.** Si hay que cambiar un tipo, el cambio va en el mismo PR con la migración.
3. **Cada PR deja el cliente Bevy en un estado observable**, aunque sea solo con gizmos o texto en pantalla.
4. **No se diseña el incremento N+2** hasta que N está mergeado. El diseño concreto de cada módulo emerge del código que existe, no de la especificación.
5. La sección de cada incremento en este documento es la **spec mínima**; el código puede ser más simple si los tests pasan.
