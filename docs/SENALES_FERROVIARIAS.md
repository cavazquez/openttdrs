# Señales ferroviarias — referencia OpenTTD e implementación en openttdrs

Guía de **tipos, comportamiento, codificación en mapa y plan de paridad** para señales ferroviarias. Fuentes oficiales y código upstream; estado actual del crate `openttdrs-core` / cliente.

**Referencias oficiales**

| Fuente | URL / ruta |
|--------|------------|
| Manual jugador — tipos y uso | [wiki.openttd.org/Manual/Signals](https://wiki.openttd.org/en/Manual/Signals) |
| Tutorial construcción (toolbar, arrastre, convertir) | [wiki.openttd.org/Manual/Building signals](https://wiki.openttd.org/en/Manual/Building%20signals) |
| Codificación en tesela (`m2`–`m5`) | [docs/landscape.html](https://github.com/OpenTTD/OpenTTD/blob/master/docs/landscape.html) (clase Railway) |
| Tipos y estados (`SignalType`, `SignalVariant`, `SignalState`) | [`src/signal_type.h`](https://github.com/OpenTTD/OpenTTD/blob/master/src/signal_type.h) |
| API mapa ferroviario | [`src/rail_map.h`](https://github.com/OpenTTD/OpenTTD/blob/master/src/rail_map.h) |
| Actualización rojo/verde (bloques y presignals) | [`src/signal.cpp`](https://github.com/OpenTTD/OpenTTD/blob/master/src/signal.cpp) |
| Colocación bajo cursor | [`src/rail_gui.cpp`](https://github.com/OpenTTD/OpenTTD/blob/master/src/rail_gui.cpp) (`GenericPlaceSignals`) |
| Dibujo sprites | [`src/rail_cmd.cpp`](https://github.com/OpenTTD/OpenTTD/blob/master/src/rail_cmd.cpp) (`DrawSignals`) |
| PBS en pathfinder | [`src/pathfinder/yapf/yapf_costrail.hpp`](https://github.com/OpenTTD/OpenTTD/blob/master/src/pathfinder/yapf/yapf_costrail.hpp) |
| Histórico YAPP (conceptos PBS) | [wiki Yet Another PBS Patch](https://wiki.openttd.org/en/Archive/Manual/Yet%20Another%20PBS%20Patch) |

> El manual de jugador advierte que **path signals son el estándar desde hace años**; block y presignals se mantienen por compatibilidad con saves antiguos ([Signals](https://wiki.openttd.org/en/Manual/Signals)).

---

## 1. Resumen para el jugador (OpenTTD oficial)

Las señales evitan colisiones y ayudan a elegir ramas hacia el destino. **Waypoints** sirven para forzar rutas concretas; las señales no sustituyen órdenes de ruta.

| Familia | Tipos | Idea central |
|---------|-------|--------------|
| **Path (PBS)** — recomendado | Path, PathOneWay | Reserva un **camino** hasta la siguiente posición segura de espera; varios trenes pueden compartir el mismo “bloque” si sus rutas no chocan. |
| **Block** — legado TTD | Block (two-way / one-way) | Un **bloque** = todo el tramo alcanzable hasta la siguiente señal; rojo si **cualquier** parte del bloque está ocupada. |
| **Presignal** — legado | Entry, Exit, Combo | Block + lógica extra: la **Entry** solo verde si hay al menos una **Exit** verde en el bloque siguiente. |

**Posiciones seguras de espera** (path signals): delante de otra señal, depósito o fin de vía — **no** inmediatamente detrás de un cruce (bloquearía la unión).

**Variante visual** (no cambia la lógica): eléctrica vs semáforo (`SignalVariant`); el juego puede colocar semáforos automáticamente antes de cierto año.

---

## 2. Tipos en código (`signal_type.h`)

### 2.1 `SignalType` — comportamiento

| Valor | Nombre | `m2` (3 bits) | Función |
|------:|--------|---------------|---------|
| 0 | **Block** | `000` | Señal de bloque clásica. Verde si el bloque **posterior** (hasta la siguiente señal en esa vía) está libre. Soporta **two-way** (bidireccional) y **one-way** (solo entra por el lado que mira). |
| 1 | **Entry** | `001` | Presignal de **entrada**: verde solo si el bloque siguiente tiene al menos una **Exit/Combo** verde además de estar libre según reglas de bloque. |
| 2 | **Exit** | `010` | Presignal de **salida**: como block hacia delante, pero su estado **alimenta** entradas/combos aguas arriba. |
| 3 | **Combo** | `011` | Presignal **combo**: actúa como exit del bloque anterior y entry del siguiente; permite árboles de presignals en estaciones ramificadas. |
| 4 | **Path** | `100` | Path signal **bidireccional por detrás**: por la espalda se ignora o se penaliza en pathfinder; roja hasta reservar camino. |
| 5 | **PathOneWay** | `101` | Path signal **de sentido único**: no se puede pasar por detrás (equivalente a señal permanente en rojo en sentido contrario). |

Helpers en `rail_map.h`:

- `IsPbsSignal` → Path o PathOneWay  
- `IsPresignalEntry` → Entry o Combo  
- `IsPresignalExit` → Exit o Combo  
- `IsOnewaySignal` → todo excepto Path (block one-way, presignals, path one-way)

### 2.2 `SignalVariant` — apariencia

| Valor | Nombre | Bits `m2` |
|------:|--------|-----------|
| 0 | Electric | bit 3 (señales 2–3) o bit 7 (señales 0–1): **0** |
| 1 | Semaphore | mismo bit: **1** |

En teselas Horz/Vert hay **dos grupos** de tipo/variante: pistas Upper/Left/X/Y usan bits 0–3; Lower/Right usan bits 4–7 (`GetSignalType` / `GetSignalVariant` en `rail_map.h`).

### 2.3 `SignalState`

| Valor | Significado en simulación |
|------:|---------------------------|
| Red | El tren no puede cruzar la señal en ese sentido. |
| Green | Puede cruzar (block: bloque libre; presignal: condiciones cumplidas; path: reserva válida). |

---

## 3. Comportamiento detallado por tipo

### 3.1 Block signal (`SignalType::Block`)

**Bloque:** todas las teselas de vía **alcanzables** desde la señal, siguiendo la vía, hasta la **próxima señal** en esa dirección (incluye ramas del bloque aunque el tren vaya por otra rama).

- **Verde:** ningún tren ocupa ninguna tesela del bloque.  
- **Rojo:** cualquier tren en cualquier rama del bloque.  
- **Limitación:** en un cruce, aunque la rama que tomará el tren esté libre, si otra rama del mismo bloque está ocupada → rojo (motivo principal de path signals).

**Two-way vs one-way** (solo block y presignals, no path):

- Clic repetido sobre señal existente (sin Ctrl): alterna two-way → one-way → one-way invertida → two-way ([Building signals](https://wiki.openttd.org/en/Manual/Building%20signals)).  
- Codificado en `m3` con `CycleSignalSide` (`rail_map.h`): path signals solo 2 lados; block/presignal 3 (ambos sentidos posibles).

**One-way block:** tren que llega por el lado “prohibido” se para y puede invertir (configurable en advanced settings).

### 3.2 Presignals (Entry / Exit / Combo)

Flujo típico en estación multivía ([Signals § Legacy Pre-signals](https://wiki.openttd.org/en/Manual/Signals)):

```text
[Entry] ──► bifurcación ──► [Exit] rama A
                         └──► [Exit] rama B
```

| Tipo | Regla de color |
|------|----------------|
| **Exit** | Igual que block hacia adelante; además propaga su estado a entries/combos anteriores. |
| **Entry** | Verde si el bloque inmediato posterior permite entrada **y** existe al menos un exit verde en ese bloque posterior. Si no hay exits designados, se comporta como block normal. |
| **Combo** | Exit para bloques anteriores + entry para bloques posteriores; encadena árboles (entry → combos → exits). |

**Limitaciones oficiales:**

- Un exit verde puede hacer verde la entry aunque **topológicamente** el tren no pueda llegar a esa exit (layout en T).  
- Trenes “perdidos” ignoran exit signals (bug conocido “will not be solved”).  
- En la práctica moderna se sustituye por **path signals**.

Implementación upstream: barrido de bloque en `signal.cpp` (`ProbeSigSeg`, flags `Exit`, `MultiExit`, `Green`, `Train`, …) y `UpdateSignalsAroundSegment` — entry roja si hay exit pero ninguna verde.

### 3.3 Path signals (`Path`, `PathOneWay`) — PBS / YAPP

**Reserva de camino:** antes de entrar, el tren reserva teselas hasta la siguiente **posición segura** (señal, depósito, fin de vía). Otro tren puede entrar al mismo “bloque” si reserva un camino **disjunto**.

| Aspecto | Path | PathOneWay |
|---------|------|------------|
| Pasar por detrás | Permitido (pathfinder penaliza) | Prohibido (bloqueo duro) |
| Color por defecto | Rojo hasta reserva exitosa | Igual |
| Uso recomendado | Casi todo | Cuando hay que prohibir sentido contrario (p. ej. salida estación) |

**Datos extra en tesela** (`landscape.html`):

- `m2` bits 8–10: pista reservada para PBS (`GetRailReservationTrackBits`).  
- `m2` bit 11: reserva también la pista opuesta (Horz/Vert).  
- `m5` bit 4: reserva PBS en **cruces a nivel** (`HasCrossingReservation`); en vía plana la reserva vive solo en `m2`.

**Pathfinder (YAPF):** penalizaciones por cruzar reserva ajena, pasar path signal por detrás, estación reservada (`yapf_costrail.hpp`: `ReservationCost`, `SignalCost`, `rail_pbs_cross_penalty`, …).

**Advanced settings** (wiki Signals / YAPP): resaltar rutas reservadas, tipo por defecto al construir, ciclo Ctrl+clic, `wait_for_pbs_path`, intervalos de reintento, etc.

---

## 4. Codificación en el mapa (paridad save / `.ottdmap`)

Tesela `MP_RAILWAY` con `RailTileType::Signals` (`m5` bits 6–7 = `01`).

### 4.1 Hasta 4 señales por tesela

Cada **signal bit** 0..3 corresponde a direcciones concretas según el `Track` (X, Y, Upper, Lower, Left, Right). Tabla completa en [landscape.html — Railway signals](https://github.com/OpenTTD/OpenTTD/blob/master/docs/landscape.html) y en `collect_signal_sprite_ids` (`crates/openttdrs-client/src/sprites/rail.rs`).

| Campo | Bits | Significado |
|-------|------|-------------|
| `m5` | 0–5 | `TrackBits` — qué piezas de vía hay |
| `m5` | 6–7 | `RailTileType` (= Signals) |
| `m5` | 4 | reserva PBS **solo en cruce a nivel** (`HasCrossingReservation`) |
| `m3` | 7–4 | **presente** — bit 1 = señal `n` existe (`GetPresentSignals`) |
| `m4` / `m3hi` | 7–4 | **estado** — bit 1 = verde (`GetSignalStates`; en `.ottdmap` el chunk `M3HI` carga en `m4()`) |
| `m2` | 2–0 | tipo señales 2 y 3 |
| `m2` | 6–4 | tipo señales 0 y 1 |
| `m2` | 3, 7 | variante semáforo/eléctrico (grupos 2–3 y 0–1) |
| `m2` | 8–10 | reserva PBS (track reservado) |
| `m2` | 11 | reserva pista opuesta |

En openttdrs, `Tile.m3hi` = nibble alto de estados (equivalente a `m4()` para señales).

### 4.2 Colocación: una señal, un carril

En doble vía **Horz** (`UPPER|LOWER`) o **Vert** (`LEFT|RIGHT`), **un clic = una señal** en el carril bajo el cursor. OpenTTD elige la pieza con `GenericPlaceSignals` según `fract_x`, `fract_y`:

| Layout | Regla |
|--------|--------|
| Vert | `RIGHT` si `fract_x <= fract_y`, si no `LEFT` |
| Horz | `UPPER` si `fract_x + fract_y <= 256`, si no `LOWER` |
| X / Y / pieza única | esa pieza directamente |

En openttdrs: `resolve_signal_track` en `rail_signals.rs` (misma lógica). Comando: `PlaceRailSignal(coord, orientation, fract_x, fract_y)`.

**No señales** en: cruces con más de Horz o Vert mezclado incompatible (`tracks_overlap`), puentes, túneles, pasos a nivel (OpenTTD).

### 4.3 Sprites

`DrawSingleSignal` → IDs OpenGFX; bases `1275` (block eléctrico clásico) y alternativa (`1352` / `OPENTTDRS_SIGNAL_ALT_BASE`) para presignals y PBS. Cliente: `signal_sprite_id`, `collect_signal_sprite_ids`, precarga en `rail_sprite_ids_for_preload` (8 IDs PBS sin PNG en OpenGFX — ver `SP3_AUDIT_SUMMARY.md`).

---

## 5. Herramientas de construcción (OpenTTD)

| Acción | Comportamiento oficial |
|--------|------------------------|
| Clic | Coloca señal del tipo seleccionado en toolbar |
| Clic en existente | Block: cicla two-way / one-way / dirección; con Ctrl: cicla **tipo** (block → entry → exit → combo → path → …) |
| Arrastre | Línea de señales espaciadas (`signal density`, default cada 4 teselas); desde presignal arrastrando → block en la misma dirección |
| Ctrl + arrastre | Autocolocación hasta estación/señal/bifurcación |
| Bulldozer / R | Quitar señales |
| Signal convert | Convierte tipo al seleccionado en toolbar |
| RMB (openttdrs) | Rotar orientación de colocación (`cycle_signal_facing`) |

Toolbar avanzada vs simplificada: la simplificada solo muestra path signals ([Building signals](https://wiki.openttd.org/en/Manual/Building%20signals)).

---

## 6. Estado actual en openttdrs

| Capacidad | Estado | Módulo |
|-----------|--------|--------|
| Colocar / quitar señal **block eléctrica** unidireccional | ✅ | `command/transport.rs`, `PlaceRailSignal` |
| Preview + toolbar + RMB dirección | 🟡 | Pick/colocación en diagonal — ver §11 |
| Carriles X/Y/Upper/Lower/Left/Right | ✅ | `resolve_signal_track`, `fract_x/y` |
| Render presente + rojo/verde | ✅ | `sprites/rail.rs`, `collect_signal_sprite_ids` |
| Sim block simple (bloque hasta siguiente señal, 1 ocupación) | ✅ | `rail_signals.rs` — X/Y + Horz/Vert (exit por carril) |
| Two-way / one-way block | ✅ | `cycle_signal_side_m3` vía `PlaceRailSignal` (clic en señal existente) |
| Semaphore vs electric | ✅ | `default_signal_variant` por año (`SEMAPHORE_BUILD_BEFORE_YEAR` = 1950); setting `gui.semaphore_build_before` no expuesto |
| Tipos Entry / Exit / Combo | ✅ | Colocación + Ctrl ciclo 6 tipos; sim entry exige bloque propio libre **y** algún exit/combo verde |
| Path / PathOneWay + reserva PBS | ✅ | Safe wait, wait/giro, UI `PBS...`, TryReserve BFS. YAPF nativo completo opcional |
| Presignal `UpdateSignalsOnSegment` | 🟡 | 2 pasadas (`compute_exit_signal_greens` + entry/combo); sin `ProbeSigSeg` / `_globset` upstream |
| Arrastre línea + densidad | ✅ | `signal_density` default 4; Shift+RMB cicla |
| Bulldozer quita señal (conserva vía) | ✅ | `RemoveRailSignal` vía herramienta Demoler |
| Signal convert + Ctrl ciclo tipos | ✅ | Ctrl+clic: block→entry→exit→combo→path→path oneway (`CycleRailSignalType`) |
| Import `.sav` con PBS/presignals | 🟡 | Encoding y render OK; reservas PBS runtime se recalculan; árboles combo multi-nivel frágiles |

---

## 7. Plan de implementación por fases

Orden sugerido alineado con [ROADMAP_SPRINTS.md](ROADMAP_SPRINTS.md) y [PARIDAD_OPENTTD.md](PARIDAD_OPENTTD.md).

### Fase A — Block completo (S5 cierre) ✅

**Objetivo:** paridad jugable en líneas doble vía con block signals.

1. **Sim en Horz/Vert:** `signal_bits_for_exit` / `signal_exit_dir` por carril; bloque sigue el corredor HORZ/VERT (ocupación por tesela).  
2. **Two-way / one-way:** `cycle_signal_side_m3` embebido en `PlaceRailSignal` (clic en señal del mismo carril).  
3. **Merge `m2` al añadir 2.ª señal** en misma tesela (otro carril).  
4. Tests: Upper≠Lower en Horz, Vert Left, ciclo one/two-way, two-way terminal.

**Archivos:** `rail_signals.rs`, `command/transport.rs`, `build_input/click.rs`, tests en `rail_signals.rs` / `command/tests.rs`.

### Fase B — UX construcción ✅

1. Arrastre con densidad N (`StationBuildState.signal_density`, default 4; Shift+RMB cicla 1/2/4/8/12/16).  
2. Bulldozer (`Clear`) sobre tesela con señal → `RemoveRailSignal` (conserva vía).  
3. Semaphore automático por año — ✅ `default_signal_variant` (1950); setting GUI opcional pendiente.  
4. Ctrl+clic ciclo de tipo (6 tipos OpenTTD) — ✅.

### Fase C — Presignals 🟡 (jul 2026)

**Objetivo:** saves antiguos y estaciones legacy.

1. Codificar `SignalType` 1–3 en `m2` al colocar/convertir — ✅ `PlaceRailSignal` acepta 0–5; Ctrl cicla los 6.  
2. Port simplificado de `signal.cpp`:  
   - `ProbeSigSeg` / flags de bloque — ❌ (sigue `rail_block_ahead` v1)  
   - `UpdateSignalsOnSegment` + buffer `_globset` — 🟡 2 pasadas entry/exit/combo  
   - Regla entry: rojo si bloque propio ocupado **o** ningún exit verde — ✅  
3. Sprites entry/exit/combo (ya en OpenGFX vía `signal_type > 3`) — ✅.  
4. Tests: ciclo 6 tipos + colocación entry/exit/combo; demo estación 2 vías — ✅ encoding; dinámico wiki parcial.

**No replicar** bugs upstream (lost train ignora exit) salvo paridad explícita.

### Fase D — Path signals (PBS) — Hito 0.2 🟡 (parcial, jul 2026)

**Objetivo:** comportamiento moderno por defecto.

1. **Reserva:** ✅ estructura por tren (`reserved_steps` + track bits); `m2` bits 8–11 vía `m2_hi`; `m5` bit 4 en **cruces a nivel** (`HasCrossingReservation`). En vía plana la reserva no usa `m5` bit 4 (paridad OpenTTD: ahí va en `m2`).  
2. **Antes de mover tren:** ✅ extensión de reserva a lo largo del `path` hasta **posición segura** (`is_safe_waiting_position`: depósito, block, delante de path, fin de vía) o conflicto.  
3. **Estado señal path:** ✅ verde solo con reserva completa hasta safe wait (`pbs_exit_has_complete_reservation`); path **no** exige verde previa para extender reserva; movimiento exige reserva completa.  
4. **Pathfinder:** ✅ penalización PBS por detrás (`YAPF_PBS_BEHIND_PENALTY`); cruce de reserva (`YAPF_RESERVATION_CROSS_PENALTY`).  
5. **Cliente:** ✅ overlay rutas reservadas (tecla R / `show_pbs_reservations`); default toolbar `SIGTYPE_PATH`.  
6. **PathOneWay:** ✅ bloqueo sentido contrario (`DeadEnd` / `train_blocked_by_signal`).  
7. **Espera / giro:** ✅ `PathfindingSettings` (`wait_for_pbs_path` default 30 días, `path_backoff_interval` 20, `reverse_at_signals`); stuck + giro al timeout (`tick_pbs_wait_and_maybe_reverse`).  
8. **UI settings:** ✅ toolbar **engranaje** (Ajustes) → `Pathfinding / PBS...` (`pathfinding_settings_window.rs`).  
9. **TryReservePath:** ✅ Dijkstra con costes YAPF (`find_path_to_safe_wait`: tile + `YAPF_RESERVATION_CROSS_PENALTY` + sesgo off-path) hasta safe wait; desactivable con `path_backoff_interval = 255`.

**Pendiente:** — (TryReservePath usa Dijkstra con costes YAPF: tesela, cruce reserva, sesgo path órdenes).

Dependencias: pathfinder trenes más fiel (YAPF simplificado o extensión de `pathfinder.rs`).

### Fase E — Import y regresión ✅ (jul 2026)

1. Fixture save OpenTTD con mezcla block + path + presignals → `rail_signals_mixed.sav` (`scripts/gen_rail_signals_sav.py`).  
2. Golden render/encoding señales → `tests/fixtures/parity/rail_signals_golden.json` + `golden_rail_signals.rs` + test cliente `golden_rail_signal_sprite_texture_ids`.  
3. Limitaciones § import documentadas en [TILES_Y_SAVEGAMES_OPENTTD.md](TILES_Y_SAVEGAMES_OPENTTD.md) (§17).  
4. Escenario parity `rail_signals_mixed` + roundtrip JSON en `parity/scenario.rs`.

---

## 8. Mapa código openttdrs ↔ upstream

| Responsabilidad | openttdrs | OpenTTD |
|-----------------|-----------|---------|
| Tipos | (implícito `sig_type=0`) | `signal_type.h` |
| Colocación | `resolve_signal_track`, `place_rail_signal` | `GenericPlaceSignals`, `CmdBuildSignal` |
| Codificación tile | `signal_placement_for_track`, `m2_for_signal` | `SetSignalType`, `SetPresentSignals` |
| Estado verde/rojo block | `update_rail_signal_states` | `UpdateSignalsOnSegment` |
| Bloqueo tren | `train_blocked_by_signal` | `CheckTrainOwnership`, PBS checks |
| Dibujo | `collect_signal_sprite_ids` | `DrawSignals` |
| PBS | `rail_pbs.rs` + `sim_step` | `yapf`, reserva en `train_cmd.cpp` / PBS core |

---

## 9. Criterios de aceptación (definición de “hecho”)

| Nivel | Criterio |
|-------|----------|
| **S5** | Block unidireccional en todos los track bits; sim evita entrar a bloque ocupado; preview/colocación/quitar en Horz/Vert. |
| **Block full** | Two-way en terminal; arrastre densidad 4; semáforo/eléctrico visible. |
| **Presignal** | Entry/exit/combo cambian color como en OpenTTD en layout wiki estación; CI con save fixture. |
| **PBS** | Cruce doble vía con trenes en paralelo sin deadlock; reserva visible; path one-way bloquea sentido contrario. |

---

## 11. Bug abierto: fantasma vs colocación en vía diagonal (jun 2026)

**Estado:** ✅ cerrado jul 2026 · tap ancla al press + preferencia seed en pick.

### Síntoma (reporte usuario)

En vías **X/Y diagonales** (tesela plana):

1. El **fantasma** (preview) aparece **sobre el riel**, donde el jugador espera colocar.
2. Al **clic**, la señal queda en una **tesela vecina** (a menudo una casilla al este/sudeste en pantalla), a veces en hierba o con apariencia de “vía nueva”.
3. En casos extremos parece colocarse **doble** (fantasma correcto + resultado en vecino).

OpenTTD usa `GetTileBelowCursor()` + `GenericPlaceSignals` sobre **esa** tesela (`rail_gui.cpp`); no hay búsqueda 5×5.

### Fix aplicado

1. **Tap vs arrastre:** en `RailSignals`, si el cursor se movió ≤10 px entre press y release, se coloca en `start_tile` + `signal_drag_fract` del press (misma fuente que el fantasma), sin re-pickear el vecino isométrico.
2. **Pick:** si el seed geométrico ya es vía válida y el cursor está cerca del ancla, se prefiere ese seed; desempate favorece el seed sobre vecinos.
3. Test: `pick_mid_diagonal_rail_segment_stays_on_track_tile` en `iso/coords.rs`.

### Intentos previos (insuficientes solos)

| Cambio | Archivos | Resultado |
|--------|----------|-----------|
| Snap al riel vecino | `world_pos_to_rail_signal_pick` (`iso/coords.rs`) | Mejor cerca del borde; sigue desalineado |
| Offset sub-tesela OpenTTD | `rail_signal_subtile_offset`, `signal_draw_pos` (`sprites/rail.rs`) | Fantasma más alineado al riel; clic sigue en otra tesela |
| Fuente única hover | `HoveredTileCoord` en cursor + preview + click | Misma tesela en teoría; usuario confirma bug persiste |
| Orden ECS | `cursor → ghost → click` (`ui.rs`) | Evita frame distinto; no corrige pick erróneo |
| Desempate vecinos | `rail_signal_pick_better` en pick 5×5 | Empates por métrica isométrica |

### Hipótesis para la próxima sesión

1. **Pick isométrico vs OpenTTD** — `world_pos_to_tile_coord` puede devolver tesela A mientras el riel visible está en B; el vecindario 5×5 elige B con métrica similar a C.
2. **Fract en tesela equivocada** — `PlaceRailSignal(coord, …, fract_x, fract_y)` calculado respecto a tesela B pero el jugador apunta a A; el core escribe en B (datos) mientras el fantasma se dibuja bien por offset visual.
3. **Paridad `GetTileFromScreenXY`** — portar lógica exacta de `viewport.cpp` (no solo inversa de `iso` + rombo relajado).
4. **Proyección al carril** — tras elegir tesela, proyectar `world_pos` al segmento X/Y dentro del rombo antes de `resolve_signal_track` (como hace el cliente con `_tile_fract_coords` tras fijar tile).
5. **Regresión visual** — captura `OPENTTDRS_MAP_SHOT` con herramienta señales + test que compare `HoveredTileCoord` vs tile bajo sprite fantasma.

### Archivos clave

| Rol | Ruta |
|-----|------|
| Pick | `crates/openttdrs-client/src/iso/coords.rs` — `world_pos_to_rail_signal_pick` |
| Hover unificado | `crates/openttdrs-client/src/ui/toolbar/build_input/cursor.rs` |
| Preview | `crates/openttdrs-client/src/ui/toolbar/preview/rail_signal.rs`, `preview/mod.rs` |
| Clic | `crates/openttdrs-client/src/ui/toolbar/build_input/click.rs` |
| Comando | `crates/openttdrs-core/src/command/transport.rs` — `place_rail_signal` |
| Dibujo | `crates/openttdrs-client/src/sprites/rail.rs`, `render/tiles/transport.rs` |
| Upstream | `OpenTTD/src/viewport.cpp` (`GetTileFromScreenXY`), `rail_gui.cpp` (`GenericPlaceSignals`) |

### Criterio de cierre

- Fantasma y señal colocada en la **misma tesela** y **misma posición en pantalla** al clicar sobre un tramo X o Y diagonal (caso GIF usuario jun 2026).
- Test cliente: mapa 3×3 con una diagonal; simular `world_pos` en centro del riel → `HoveredTileCoord` == tile del comando `PlaceRailSignal`.

### Repro local

```bash
cargo run -p openttdrs-client
# Toolbar → Señales → vía X o Y en diagonal → hover sobre riel → clic
# Comparar tesela del fantasma vs tesela donde aparece el sprite sólido
```

---

## 12. Enlaces internos

- Codificación tiles vía: [TILES_Y_SAVEGAMES_OPENTTD.md §7.1](TILES_Y_SAVEGAMES_OPENTTD.md#71-mp_railway--teselas-con-señales)  
- Sprint plan: [ROADMAP_SPRINTS.md § S5](ROADMAP_SPRINTS.md)  
- Render / assets: [SP3_AUDIT_SUMMARY.md](SP3_AUDIT_SUMMARY.md), [archive/SESION_OTTDMAP_SIGNALS_SIM_2026-04-28.md](archive/SESION_OTTDMAP_SIGNALS_SIM_2026-04-28.md)  
- Paridad global: [PARIDAD_OPENTTD.md](PARIDAD_OPENTTD.md)
