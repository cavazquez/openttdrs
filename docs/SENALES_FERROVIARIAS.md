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
- `m5` bit 4: estado de reserva PBS en la tesela.

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
| `m5` | 4 | reserva PBS (tesela) |
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
| Preview + toolbar + RMB dirección | ✅ | `preview/rail_signal.rs`, `rotate.rs` |
| Carriles X/Y/Upper/Lower/Left/Right | ✅ | `resolve_signal_track`, `fract_x/y` |
| Render presente + rojo/verde | ✅ | `sprites/rail.rs`, `collect_signal_sprite_ids` |
| Sim block simple (bloque hasta siguiente señal, 1 ocupación) | 🟡 | `rail_signals.rs`, `sim_step.rs` — solo X/Y bien probado; Horz/Vert parcial |
| Two-way / one-way block | ❌ | Falta `CycleSignalSide` + UI clic |
| Semaphore vs electric | ❌ | Siempre `variant=0` |
| Tipos Entry / Exit / Combo | ❌ | Solo `sig_type=0` en `m2` |
| Path / PathOneWay + reserva PBS | ❌ | Hito 0.2 — ver `ROADMAP_SPRINTS.md` |
| Presignal `UpdateSignalsOnSegment` | ❌ | Requiere port de `signal.cpp` |
| Arrastre línea + densidad | ❌ | |
| Signal convert + Ctrl ciclo tipos | ❌ | |
| Import `.sav` con PBS/presignals | 🟡 | Render si bits correctos; sim ignora tipo |

---

## 7. Plan de implementación por fases

Orden sugerido alineado con [ROADMAP_SPRINTS.md](ROADMAP_SPRINTS.md) y [PARIDAD_OPENTTD.md](PARIDAD_OPENTTD.md).

### Fase A — Block completo (S5 cierre)

**Objetivo:** paridad jugable en líneas doble vía con block signals.

1. **Sim en Horz/Vert:** extender `signal_bits_for_exit`, `rail_block_ahead`, `rail_traversal_bits` para Upper/Lower/Left/Right.  
2. **Two-way / one-way:** portar `CycleSignalSide`; clic en señal existente alterna lados (cliente + comando).  
3. **Merge `m2` al añadir 2.ª señal** en misma tesela (tipos distintos por carril).  
4. Tests: doble vía Horz, two-way estación fin de línea.

**Archivos:** `rail_signals.rs`, `command/transport.rs`, `build_input/click.rs`, tests en `rail_signals.rs` / `command/tests.rs`.

### Fase B — UX construcción

1. Arrastre con densidad N (estado en toolbar, como OpenTTD).  
2. Bulldozer quita señal sin quitar vía (`remove_rail_signal_bit` ya existe).  
3. Semaphore automático por año (opcional; leer game setting).  
4. Ctrl+clic ciclo de tipo (limitado a tipos ya simulados).

### Fase C — Presignals

**Objetivo:** saves antiguos y estaciones legacy.

1. Codificar `SignalType` 1–3 en `m2` al colocar/convertir.  
2. Port simplificado de `signal.cpp`:  
   - `ProbeSigSeg` / flags de bloque  
   - `UpdateSignalsOnSegment` + buffer `_globset`  
   - Regla entry: rojo si `Exit && !Green`  
3. Sprites entry/exit/combo (ya en OpenGFX vía `signal_type > 3`).  
4. Tests: estación 2 vías con entry + 2 exits (caso wiki).

**No replicar** bugs upstream (lost train ignora exit) salvo paridad explícita.

### Fase D — Path signals (PBS) — Hito 0.2

**Objetivo:** comportamiento moderno por defecto.

1. **Reserva:** estructura de reservas por tren (teselas + track bits); escribir `m2` bits 8–11, `m5` bit 4.  
2. **Antes de mover tren:** `TryReservePath` hasta safe waiting position (`yapf/` o módulo `rail_pbs.rs`).  
3. **Estado señal path:** verde solo con reserva válida; desreservar al salir del camino.  
4. **Pathfinder:** penalización pasar PBS por detrás; cruce de reserva (`ReservationCost`).  
5. **Cliente:** overlay rutas reservadas (setting).  
6. **PathOneWay:** `HasOnewaySignalBlockingTrackdir` en movimiento.

Dependencias: pathfinder trenes más fiel (YAPF simplificado o extensión de `pathfinder.rs`).

### Fase E — Import y regresión

1. Fixture save OpenTTD con mezcla block + path + presignals.  
2. Golden render señales (`tests/fixtures/`).  
3. Documentar limitaciones en § import de [TILES_Y_SAVEGAMES_OPENTTD.md](TILES_Y_SAVEGAMES_OPENTTD.md).

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
| PBS | — | `yapf`, reserva en `train_cmd.cpp` / PBS core |

---

## 9. Criterios de aceptación (definición de “hecho”)

| Nivel | Criterio |
|-------|----------|
| **S5** | Block unidireccional en todos los track bits; sim evita entrar a bloque ocupado; preview/colocación/quitar en Horz/Vert. |
| **Block full** | Two-way en terminal; arrastre densidad 4; semáforo/eléctrico visible. |
| **Presignal** | Entry/exit/combo cambian color como en OpenTTD en layout wiki estación; CI con save fixture. |
| **PBS** | Cruce doble vía con trenes en paralelo sin deadlock; reserva visible; path one-way bloquea sentido contrario. |

---

## 10. Enlaces internos

- Codificación tiles vía: [TILES_Y_SAVEGAMES_OPENTTD.md §7.1](TILES_Y_SAVEGAMES_OPENTTD.md#71-mp_railway--teselas-con-señales)  
- Sprint plan: [ROADMAP_SPRINTS.md § S5](ROADMAP_SPRINTS.md)  
- Render / assets: [SP3_AUDIT_SUMMARY.md](SP3_AUDIT_SUMMARY.md), [archive/SESION_OTTDMAP_SIGNALS_SIM_2026-04-28.md](archive/SESION_OTTDMAP_SIGNALS_SIM_2026-04-28.md)  
- Paridad global: [PARIDAD_OPENTTD.md](PARIDAD_OPENTTD.md)
