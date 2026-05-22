# Informe de arquitectura: OpenTTD upstream (referencia para openttdrs)

> Basado en análisis directo del código fuente en `reference/openttd-upstream/` (clon shallow, rama principal, abril 2026).  
> Licencia upstream: GPL-2.0 (`COPYING.md`). Ver advertencias de licencia al final.

El **plan de trabajo del port** (incrementos I0–I8 y fases SP de solitario) está en [DISENO_INCREMENTAL.md](DISENO_INCREMENTAL.md), enlazado con este informe por tabla y referencias por incremento. **Prioridad actual:** cerrar 0.1 en un jugador; **I8 (red)** solo después.

---

## 1. Visión general

OpenTTD es un motor de simulación **monolítico en C++** con build CMake. Los pilares son:

1. Un **mapa discreto de teselas** con layout en memoria muy comprimido.
2. Un **bucle de tick determinista** sobre el que se ejecutan todos los subsistemas.
3. Un **sistema de comandos** serializable que es la única fuente de cambios al estado del mundo (y la base del multijugador).
4. Una capa de **NewGRF** que extiende casi todos los aspectos del juego.
5. Un sistema de **saveload** fuertemente versionado con compatibilidad hacia atrás desde v0.1.

---

## 2. El mapa y las teselas (`src/map_func.h`, `src/tile_type.h`, `src/map.cpp`, `src/tile_map.h`)

### 2.1 Representación en memoria

Cada tesela está representada por **10 bytes** divididos en dos arrays paralelos de tamaño `Map::size`:

```
TileBase (8 bytes):
  type    u8  — bits 4-7: TileType (4 bits), bits 2-3: puentes, bits 0-1: zona trópico
  height  u8  — altura de la esquina norte (0-255)
  m1      u8  — principalmente propiedad (owner)
  m2     u16  — índice a town, industry o station según tipo
  m3      u8  — uso general
  m4      u8  — uso general
  m5      u8  — uso general

TileExtended (4 bytes):
  m6      u8  — uso general
  m7      u8  — principalmente soporte NewGRF
  m8     u16  — uso general
```

Hay **un solo** array por el mundo; no hay objetos `Tile` heap-allocated. `class Tile` es un **wrapper sin datos propios** que recibe un `TileIndex` y accede los arrays globales estáticos. El compilador lo elimina completamente en builds optimizados.

### 2.2 `TileType` — tipos de tesela reales

```cpp
enum class TileType : uint8_t {
    Clear,        // pasto, rocas, campos de granja
    Railway,      // vía férrea
    Road,         // carretera y/o tranvía
    House,        // edificio de pueblo
    Trees,        // árboles
    Station,      // estación o aeropuerto
    Water,        // agua
    Void,         // borde invisible del mapa
    Industry,     // parte de una industria
    TunnelBridge, // entrada de túnel o cabeza de puente
    Object,       // objetos: transmisores, tierra propia
};
```

Los 4 bits del tipo permiten hasta 16 tipos; hay 11 activos más `End` como marcador.

### 2.3 Dimensiones del mapa

- Mínimo: 64×64 (`2^6`), Máximo: 4096×4096 (`2^12`).
- Las dimensiones **deben ser potencias de 2**: el índice lineal se calcula con shift en lugar de multiplicación para máxima velocidad.
- El índice de una tesela en `(x, y)` es `y * Map::SizeX() + x`.
- Hay `TileAddWrap` para detectar cuando un desplazamiento cruza el borde del mapa (devuelve `INVALID_TILE`).
- La altura máxima es 255 niveles; cada nivel equivale a 8 píxeles en el render base.

### 2.4 Consecuencias para openttdrs

- La clase `Tile` en Rust puede ser un struct de valor; lo que OpenTTD llama `m1..m8` en Rust conviene modelar como campos con nombre según el subsistema.
- El mapa actual de openttdrs (`Vec<Tile>` de structs) es correcto; para escala real habría que medir si el layout comprimido (SoA en lugar de AoS) importa en rendimiento.
- **Los bits del tipo son 4 en OpenTTD** (usa `(tile.type() >> 4) & 0xF`); en Rust un `enum TileKind` con `u8` o `repr(u8)` modela esto limpiamente.

---

## 3. El reloj de tick (`src/timer/timer_game_tick.h`)

```cpp
class TimerGameTick {
    using TickCounter = uint64_t;  // monotónico global
    static TickCounter counter;
};
```

Constantes **literales del código**:

| Constante | Valor | Significado |
|-----------|-------|-------------|
| `DAY_TICKS` | 74 | Ticks por día de juego |
| `TICKS_PER_SECOND` | ~37 | Ticks por segundo real |
| `INDUSTRY_PRODUCE_TICKS` | 256 | Cada cuántos ticks producen las industrias |
| `STATION_RATING_TICKS` | 185 | Ciclo de rating de estación |
| `CARGO_AGING_TICKS` | 185 | Ciclo de envejecimiento de carga |
| `TOWN_GROWTH_TICKS` | 70 | Ciclo de crecimiento de pueblos |

El reloj tiene además `TimerGameCalendar` (fecha legible para el jugador) y `TimerGameEconomy` (año económico, más lento), separados del tick puro.

### Para openttdrs

- `GameTick(u64)` ya modela el contador monotónico correctamente.
- Para Incremento 2 (industrias): producir cada **256 ticks** es la referencia directa del upstream.

---

## 4. Los comandos (`src/command.cpp`, `src/command_func.h`)

Uno de los patrones de diseño más importantes de OpenTTD. `command.cpp` importa **todos** los `*_cmd.h` del juego y los registra. Cada acción del jugador es un `Command` con:

- Un `CommandType` (enum).
- Parámetros serializados en un `EndianBuffer`.
- `CommandFlags` que indican si es válido offline, si lo ejecuta el servidor, etc.
- Un resultado: `CommandCost` (el coste en dinero y si fue exitoso).

El servidor retransmite comandos a todos los clientes; cada cliente los ejecuta independientemente sobre su copia del estado. Esto garantiza sincronización sin enviar el estado completo.

### Comandos existentes (lista parcial de `command.cpp`)

`rail_cmd`, `road_cmd`, `train_cmd`, `roadveh_cmd`, `water_cmd`, `station_cmd`, `town_cmd`, `industry_cmd`, `terraform_cmd`, `tunnelbridge_cmd`, `order_cmd`, `timetable_cmd`, `vehicle_cmd`, `engine_cmd`, `group_cmd`, `company_cmd`, `settings_cmd`, `object_cmd`, `waypoint_cmd`, `depot_cmd`, `goal_cmd`, `story_cmd`, `subsidy_cmd`, `signs_cmd`, `news_cmd`, `misc_cmd`, etc.

### Para openttdrs

- El patrón `enum Command + fn apply(state, cmd) -> Result<Cost, Error>` en Rust es un **mapeo directo** del sistema de comandos de OpenTTD.
- Que los comandos sean datos serializables es lo que habilita el multijugador (Incremento 8) y el replay/undo.

---

## 5. Economía y carga (`src/economy_type.h`, `src/cargo_type.h`, `src/cargopacket.h`)

### 5.1 `Money` y precios

```cpp
typedef OverflowSafeInt64 Money;
```

OpenTTD tiene 64 tipos de precio (`enum Price`): construcción de vías, edificación de puentes, costes de corrida de vehículos, terraforming, etc. Hay inflación acumulada con parte fraccional de 16 bits.

### 5.2 Tipos de carga

`CargoType` es un `uint8_t`. Los tipos **originales** (Temperate) son:

`PASS`, `COAL`, `MAIL`, `OIL`, `LVST`, `GOOD`, `GRAI`, `WOOD`, `IORE`, `STEL`, `VALU` (11 tipos, más hasta 64 con NewGRF). Los climas Arctic, Tropic y Toyland añaden tipos alternativos.

Los **labels son 4 bytes ASCII** (FourCC): `'COAL'`, `'WOOD'`, etc.

### 5.3 `CargoPacket` — la unidad de transporte

```cpp
struct CargoPacket {
    uint16_t count;             // unidades de carga
    uint16_t periods_in_transit;// envejecimiento
    Money    feeder_share;      // parte del pago a feeder
    TileIndex source_xy;        // origen geográfico
    Source source;              // industria o pueblo origen
    StationID first_station;    // primera estación de paso
    StationID next_hop;         // próximo destino
};
```

Un `CargoPacket` guarda el **origen y tiempo de tránsito** de un lote de carga. Esto determina el pago: cuanto más rápido llega, más paga (la función de pago de OpenTTD penaliza el envejecimiento). Hay dos listas: `VehicleCargoList` (en el vehículo) y `StationCargoList` (en la estación).

### 5.4 Tipos de economía

```cpp
enum class EconomyType {
    Original, // imita TT original: cambios bruscos
    Smooth,   // cambios más frecuentes y pequeños
    Frozen,   // sin cambios: para scenarios controlados
};
```

### 5.5 Para openttdrs

- En el Incremento 2, una industria puede producir N unidades de un cargo por cada 256 ticks: número exacto del upstream.
- El envejecimiento del `CargoPacket` (`periods_in_transit`) en Rust sería un campo `u16` en la entidad de carga que crece cada N ticks.
- Para el Incremento 4 (ciclo económico mínimo), el pago simplificado puede ser: `income = count` sin envejecimiento; se puede sofisticar después.

---

## 6. Industrias (`src/industry.h`)

### 6.1 Estructura real

```cpp
struct Industry {
    struct ProducedCargo {
        CargoType cargo;      // tipo de cargo producido
        uint16_t  waiting;    // stock esperando ser recogido
        uint8_t   rate;       // tasa de producción
        HistoryData<ProducedHistory> history; // historial 24 meses
    };
    struct AcceptedCargo {
        CargoType cargo;      // cargo que acepta (insumo)
        uint16_t  waiting;    // stock esperando ser procesado
        Date      last_accepted;
    };

    TileArea  location;      // área de teselas que ocupa
    Town*     town;          // pueblo asociado
    uint8_t   prod_level;    // PRODLEVEL 0x00-0x80
    IndustryControlFlags control_flags; // control por GameScript
};
```

Constantes clave:

| Constante | Valor | Descripción |
|-----------|-------|-------------|
| `PRODLEVEL_MINIMUM` | 0x04 | Por debajo: la industria se cierra |
| `PRODLEVEL_DEFAULT` | 0x10 | Nivel al crearse |
| `PRODLEVEL_MAXIMUM` | 0x80 | Producción plena |
| `PROCESSING_INDUSTRY_ABANDONMENT_YEARS` | 5 | Años sin producir → cierre |

Una industria puede **producir Y aceptar** cargos (ejemplo: una acería acepta mineral de hierro y produce acero).

### 6.2 Para openttdrs (Incremento 2)

- El struct mínimo útil en Rust es `Industry { pos: TileCoord, kind: IndustryKind, produced: [(CargoKind, u32)], accepted: [(CargoKind, u32)] }`.
- `rate` en OpenTTD es `u8` (0–255); en un MVP basta con tasa fija hardcodeada por tipo.
- La producción cada 256 ticks se puede implementar como `if tick.get() % 256 == 0 { stock += rate }`.

---

## 7. Vehículos (`src/vehicle_base.h`)

### 7.1 Jerarquía

OpenTTD tiene una jerarquía de herencia en C++:

```
Vehicle (base)
├── GroundVehicle
│   ├── Train (consta de wagons articulados)
│   └── RoadVehicle
├── Aircraft
└── Ship
```

El `Vehicle` base tiene más de 1200 líneas de definición con campos como:

```cpp
VehicleID   index;         // ID único en el pool
VehicleType type;          // Train, Road, Air, Ship
Order       current_order; // orden actual ejecutada
OrderList  *orders;        // lista de órdenes
TileIndex   tile;          // posición en el mapa
uint8_t     x_pos, y_pos;  // coordenadas sub-tile
Direction   direction;     // dirección de movimiento
uint8_t     progress;      // progreso dentro de un tile (0-255)
VehicleCargoList cargo;    // lote de carga a bordo
uint8_t     cargo_cap;     // capacidad máxima
VehStates   vehstatus;     // estado (parado, oculto, chocado…)
```

### 7.2 Movimiento sub-tile

Los vehículos no saltan de tesela a tesela: tienen `progress` (0-255) dentro de cada tesela, incrementado por velocidad. `TILE_AXIAL_DISTANCE = 192` es la distancia lógica de cruzar una tesela en diagonal.

### 7.3 Para openttdrs (Incrementos 3-4)

- Para el Incremento 3 (movimiento naive), basta con saltar de tesela a tesela por tick; el `progress` sub-tile se puede agregar después.
- Para el Incremento 4 (carga/descarga), la regla real es: el vehículo llega a la `TileIndex` de la estación, transfiere `CargoPacket`s entre `VehicleCargoList` y `StationCargoList`.

---

## 8. Órdenes (`src/order_base.h`)

```cpp
struct Order {
    uint8_t   type;          // tipo de orden (GoToStation, GoToDepot, etc.)
    uint8_t   flags;         // load/unload, non-stop, etc.
    DestinationID dest;      // ID de la estación/depot/waypoint
    CargoType refit_cargo;   // refit automático
    uint16_t  wait_time;     // cuánto esperar (ticks)
    uint16_t  travel_time;   // tiempo estimado de viaje (ticks)
};
```

Las órdenes se agrupan en `OrderList` con back-references a todos los vehículos que comparten la misma lista (para edición masiva). El vehículo itera la lista cíclicamente.

### Para openttdrs (Incremento 4)

- Una orden mínima: `GoTo(StationId)`. Lista como `Vec<Order>` con un índice actual.
- El sistema de **órdenes compartidas** se puede diferir hasta el Incremento 9+.

---

## 9. Pathfinding — YAPF (`src/pathfinder/yapf/`)

### 9.1 Arquitectura YAPF

YAPF (Yet Another Pathfinder) es un **A\*** genérico parametrizado por plantillas C++. Los componentes son:

```
yapf_base.hpp      — bucle principal A*, gestión de nodos
yapf_costbase.hpp  — función heurística base
yapf_costrail.hpp  — costes específicos de vía férrea (señales, pendientes, curvas)
yapf_costcache.hpp — cachés de segmentos de vía para evitar recalcular rutas
yapf_node.hpp      — nodo del grafo con key+parent
yapf_node_rail.hpp / _road.hpp / _ship.hpp — nodos especializados
yapf_rail.cpp      — punto de entrada tren
yapf_road.cpp      — punto de entrada vehículo de carretera
yapf_ship.cpp      — punto de entrada barco
```

La API hacia el resto del juego (en `yapf.h`) es por funciones libres:

```cpp
Track YapfTrainChooseTrack(...);     // devuelve el siguiente track del tren
Trackdir YapfRoadVehicleChooseTrack(...);  // devuelve el siguiente trackdir del RV
Track YapfShipChooseTrack(...);      // devuelve el siguiente track del barco
```

El resultado **no es una ruta completa** sino **la siguiente dirección a tomar**. YAPF recalcula en cada intersección o cuando es necesario, con cachés de segmentos que se invalidan al cambiar el mapa.

### 9.2 Agua y regiones

Los barcos usan un sistema extra de `WaterRegion` (bloques de 16×16 teselas) para pathfinding de largo alcance, ya que el grafo de agua es mucho menos estructurado que las vías.

### 9.3 Para openttdrs (Incremento 5)

- YAPF no devuelve rutas completas; el Incremento 5 sí puede devolver `Vec<TileCoord>` completo (BFS simple) porque los mapas del MVP serán pequeños.
- Para escala real, habría que migrar a un A* con heurística Manhattan o similar, con cachés de segmentos.
- `follow_track.hpp` es la parte que sabe qué teselas son accesibles desde una dada en una dirección; en Rust sería una función `neighbors(map, coord, kind) -> Vec<TileCoord>`.

---

## 10. Saveload (`src/saveload/saveload.h`)

La versión de saveload actual supera **SLV_300+** (cada PR que agrega un campo al estado sube la versión). El sistema es:

- Cada subsistema registra su descriptor (`SaveLoadTable`) con los campos a serializar.
- Hay archivos `*_sl.cpp` por subsistema: `map_sl`, `company_sl`, `engine_sl`, `vehicle_sl`, etc., más un directorio `compat/` con conversores de versiones antiguas.
- `afterload.cpp` tiene fixups para versiones muy antiguas que cambiaron semántica de campos.

La **cadena de versiones es un contrato inmutable**: jamás se reutiliza un número de versión ni se reordena el enum `SaveLoadVersion`.

### Para openttdrs

- Compatibilidad binaria con OpenTTD es impráctica sin reimplementar el parseador completo.
- La estrategia sensata (Incremento 7): formato propio con `serde_json` o `bincode`, con **versión semántica** en el archivo.

---

## 11. Red (`src/network/`)

El diseño central: **solo se transmiten comandos, no el estado**. Cada cliente arranca desde el mismo estado inicial y aplica la misma secuencia de comandos, produciendo el mismo resultado.

Archivos clave:

| Archivo | Rol |
|---------|-----|
| `network_server.*` | Acepta clientes, distribuye `CommandPacket` |
| `network_client.*` | Recibe comandos del servidor, los ejecuta |
| `network_command.cpp` | Serializa/deserializa comandos para la red |
| `network_coordinator.*` | Servidor central de coordinación (NAT traversal) |
| `network_crypto.*` | Handshake y autenticación |

El **desync** ocurre cuando dos instancias divergen. OpenTTD lo detecta comparando hashes del estado; cuando pasa, el cliente se desconecta. Las causas históricas de desync incluyen: uso de floats, iteración sobre containers no deterministas, dependencias de puntero.

### Para openttdrs (Incremento 8)

- El hash de estado en Rust puede ser un `u64` derivado de `std::hash::Hasher` sobre campos deterministas del `GameState`.
- El primer protocolo puede ser tan simple como: servidor → `Vec<Command>` serializado como JSON; cliente → aplica, avanza ticks.

---

## 12. Vista y render (`src/viewport.cpp`, `src/video/`, `src/blitter/`)

En OpenTTD el render **no está separado del estado**. Los objetos del juego tienen campos de sprite directamente en `Vehicle`, `Station`, etc. El viewport ordena sprites por capa (suelo → estructuras → vehículos → efectos visuales) usando una k-d tree para culling.

Los `blitter/` son implementaciones de composición de píxeles en distintos modos (8bpp paleta, 32bpp RGBA, acelerado OpenGL).

### Para openttdrs

- Bevy **sí separa** render de simulación. Los sistemas de Bevy leen el `GameState` y producen entidades/sprites. Esto es mucho más limpio que el enfoque de OpenTTD.
- No hay nada del render de OpenTTD que valga copiar: la separación ECS de Bevy es superior.

---

## 13. Tabla completa: OpenTTD → openttdrs

| Subsistema upstream | Archivos clave | Incremento openttdrs | Complejidad de port |
|---------------------|---------------|---------------------|---------------------|
| Layout de mapa / teselas | `map_func.h`, `tile_type.h`, `tile_map.h` | I1 | Baja — ya hay base |
| Reloj de tick | `timer_game_tick.h` | I0 (hecho) | Baja — ya hecho |
| Tipos de carga | `cargo_type.h` | I2 | Baja — solo enums |
| Industrias | `industry.h`, `industry_cmd.*` | I2 | Media — producción + rate |
| Vehículos (base) | `vehicle_base.h` | I3–I4 | Alta — sub-tile, sprites, pool |
| Órdenes | `order_base.h` | I4 | Media — lista cíclica |
| Estaciones | `station_base.h` | I4 | Media — GoodsEntry, rating |
| Comandos | `command.cpp`, `*_cmd.*` | I6 | Media — patrón es directo |
| Pathfinding (BFS) | `pathfinder/yapf/yapf_base.hpp` | I5 | Media — YAPF es A*, overkill para MVP |
| Saveload | `saveload/saveload.h`, `*_sl.cpp` | I7 | Alta — versionado extenso |
| Red | `network_*` | I8 (backlog) | Post-0.1 — determinismo estricto; no prioritaria frente a solitario |
| NewGRF | `newgrf*.cpp` | Pospuesto | Muy alta — spec binaria 20+ años |
| Render / viewport | `viewport.*`, `video/`, `blitter/` | No portar | — Bevy lo reemplaza |

---

## 14. Advertencia de licencia

El upstream OpenTTD es **GPL-2.0**. Este informe cita diseños y constantes del código fuente con fines de documentación interna. Cualquier copia literal de código C++ en openttdrs requiere que el proyecto openttdrs mantenga compatibilidad con GPL-2.0 y cite el origen. Para tipos de datos, interfaces y algoritmos independientes reimplementados en Rust, la interpretación de la licencia es más permisiva (ideas no son copyrightables), pero conviene consultar si el alcance crece hacia compatibilidad binaria o copia extensiva.

---

## 15. Orden de lectura recomendado en el clon

Para cada incremento del diseño, el mapa de lectura es:

| Incremento | Leer en el clon |
|-----------|-----------------|
| I1 | `tile_type.h`, `tile_map.h`, `clear_map.h` |
| I2 | `industry.h`, `industrytype.h`, `timer_game_tick.h` (constantes) |
| I3 | `vehicle_base.h` (primeras 200 líneas), `transport_type.h` |
| I4 | `order_base.h`, `station_base.h`, `cargopacket.h` |
| I5 | `pathfinder/yapf/yapf.h`, `pathfinder/follow_track.hpp` |
| I6 | `command.cpp` (primeras 80 líneas), `command_type.h` |
| I7 | `saveload/saveload.h` (primeras 100 líneas), cualquier `*_sl.cpp` |
| I8 | `network/network_internal.h`, `network/network_command.cpp` |
