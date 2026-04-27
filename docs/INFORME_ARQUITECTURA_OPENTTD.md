# Informe de arquitectura: OpenTTD upstream (referencia para openttdrs)

Este documento resume la organización del código fuente de **OpenTTD** tal como aparece en un clon local (por ejemplo `reference/openttd-upstream/`). Sirve como mapa mental para un port incremental a Rust; **no** es una guía línea a línea ni una especificación formal.

- **Repositorio oficial**: [https://github.com/OpenTTD/OpenTTD](https://github.com/OpenTTD/OpenTTD)  
- **Licencia**: GPL-2.0 (ver `COPYING.md` en el upstream). Cualquier reutilización de diseño o código debe respetar compatibilidad de licencias y atribución.

---

## 1. Visión general del árbol

OpenTTD es un motor monolítico en **C++** con CMake, centrado en:

1. Un **bucle de juego** determinista (ticks, comandos, economía).
2. Un **mapa** discreto por teselas con muchos tipos de ocupación (vías, carretera, agua, edificios, industrias).
3. **Vehículos** con órdenes, carga y routing.
4. **Extensibilidad** (NewGRF, scripts, basesets).
5. **Persistencia** (guardados con versionado y compatibilidad hacia atrás).
6. **Multijugador** (cliente/servidor, sincronización por comandos).

Carpetas de primer nivel relevantes en el clon:

| Ruta | Rol |
|------|-----|
| `src/` | Casi toda la lógica del juego, GUI, red, saveload, pathfinding. |
| `bin/` | Herramientas auxiliares / datos de arranque según plataforma. |
| `media/` | Recursos, basesets, documentación de medios. |
| `regression/` | Pruebas de regresión del simulador. |
| `docs/` | Documentación del proyecto upstream. |
| `cmake/`, `os/` | Build y adaptación por SO. |

---

## 2. Mapa y mundo (`src/` — teselas, terreno, infraestructura)

La simulación gira en torno a **teselas** (tiles) y estructuras derivadas.

### 2.1 Conceptos

- **Coordenadas y tipos de tesela**: definiciones dispersas en cabeceras como `tile_type.h`, `tile_map.h`, `tile_map.cpp`, `map_type.h`, `map_func.h`, `map.cpp`.
- **Comandos de modificación del mapa**: muchos archivos `*_cmd.cpp` / `*_cmd.h` (por ejemplo `clear_cmd`, `water_cmd`, `tunnelbridge_cmd`, `road_cmd`, `rail_cmd`, etc.) encapsulan acciones del jugador o de la IA que mutan el mundo a través del sistema de comandos.
- **Capas semánticas del mapa**: archivos `*_map.h` describen qué hay “en” una tesela para un subsistema (por ejemplo `rail_map.h`, `road_map.h`, `water_map.h`, `station_map.h`, `industry_map.h`, `town_map.h`).
- **Terraformado y pendientes**: lógica relacionada con altura y superficie (por ejemplo `clear_map.h`, `autoslope.h`, generación procedural en `tgp.cpp` / `tgp.h`).

### 2.2 Implicaciones para openttdrs

- Extraer primero un **modelo de mapa puro** (dimensiones, tesela, altura o nivel simplificado) en un crate sin Bevy, como ya hace `openttdrs-core`, y crecer hacia tipos de ocupación y comandos validados.
- Los `*_cmd` del upstream son la referencia natural para diseñar una **capa de comandos** serializable (útil más adelante para red y replays).

---

## 3. Economía y carga (`economy*`, `cargo*`, `cargopacket*`)

Archivos representativos:

- `economy.cpp`, `economy_base.h`, `economy_cmd.h`, `economy_func.h`, `economy_type.h`: flujo de dinero, costes, subsidios y reglas económicas de alto nivel.
- `cargo_type.h`, `cargotype.cpp`, `cargotype.h`: definición de tipos de carga.
- `cargopacket.cpp`, `cargopacket.h`: paquetes de carga transportados y metadatos asociados.
- `cargoaction.cpp`, `cargoaction.h`, `cargomonitor.cpp`, `cargomonitor.h`: acciones y monitorización ligadas a carga.

### 3.1 Implicaciones

- La economía depende fuertemente del **tick** y de las **estaciones/industrias**. Conviene modelar **carga** y **producción** como datos y reglas en `openttdrs-core` antes de pintar sprites.
- Los tests de regresión del upstream (`regression/`) pueden inspirar casos de prueba mínimos (producción, transferencia, decay de carga, etc.) cuando el port avance.

---

## 4. Vehículos, órdenes y estaciones

### 4.1 Vehículos

Familias por modo, con GUI y comandos separados:

- Trenes: `train_cmd.cpp`, `train_gui.cpp`, `train.h`, etc.
- Carretera: `roadveh_*`, `road_cmd`, autobuses/camiones.
- Aviones: `aircraft_*`, `airport_*`.
- Barcos: `ship_*`.

Patrones comunes: `vehicle_base.h`, `vehicle.cpp`, `vehicle_cmd.cpp`, `vehicle_gui.cpp`, `articulated_vehicles.*`.

### 4.2 Órdenes

- `order_base.h`, `order_cmd.cpp`, `order_cmd.h`, `order_gui.cpp`, `order_type.h`, `order_backup.*`: colas de órdenes, tipos de parada, copias de respaldo para UI y simulación.

### 4.3 Estaciones y waypoints

- `station_*`, `waypoint_*`, `base_station_base.h`: recepción de carga, plataformas, layouts.

### 4.4 Implicaciones

- Es uno de los bloques **más grandes** del port: máquina de estados por vehículo + interacción con vías y señales.
- Para Bevy: mantener la **simulación** fuera del ECS de render; el motor solo refleja estado y eventos (por ejemplo cambio de posición o sprite).

---

## 5. IA y scripts (`src/ai`, `src/script`)

- **`src/ai`**: motor de AIs de compañía (`ai_core`, `ai_instance`, configuración, escaneo de scripts, GUI).
- **`src/script`**: infraestructura de **Game Script** y API expuesta a Squirrel (`script_instance`, `script_scanner`, `api/` con definiciones de funciones expuestas al lenguaje de script).

### 5.1 Implicaciones

- La compatibilidad con scripts del upstream es **opcional** en fases tempranas; sustituir por reglas nativas en Rust o un DSL propio es válido si se documenta la ruptura.
- Si en el futuro se desea compatibilidad, habría que aislar una **capa de API** estable similar a `script/api`.

---

## 6. Pathfinding (`src/pathfinder`, YAPF)

- Directorio `src/pathfinder/` con utilidades compartidas (`pathfinder_type.h`, `follow_track.hpp`, regiones de agua `water_regions.*`).
- Subcarpeta **`pathfinder/yapf/`**: Yet Another Pathfinder — implementación principal por modos de transporte (`yapf_rail.cpp`, `yapf_road.cpp`, `yapf_ship.cpp`, plantillas en `.hpp` para nodos, costes y cachés).

OpenTTD históricamente también tuvo componentes NPF; en el código actual YAPF domina el routing de red ferroviaria y otros modos según configuración.

### 6.1 Implicaciones

- El pathfinding está **acoplado** al mapa y al tipo de vía; conviene introducir primero un **grafo abstracto** o grid reducido en Rust antes de copiar heurísticas.
- Cachés y costes (archivos `yapf_cost*.hpp`) son críticos para rendimiento; perfilar en Rust con benchmarks desde etapas medias.

---

## 7. NewGRF y extensiones (`src/newgrf*`)

Gran cantidad de archivos `newgrf_*.cpp/.h`: industrias, casas, aeropuertos, estaciones, textos, almacenamiento de variables, grupos de sprites, etc.

- `newgrf.cpp`, `newgrf.h`: núcleo de carga y resolución.
- `newgrf_config.*`, `newgrf_gui.*`: interfaz y configuración de GRF.
- `newgrf_spritegroup.*`: selección de sprites según variables (jumpíng del spec).

### 7.1 Implicaciones

- Bloque **de mayor riesgo** para un port: formato binario, evolución del spec y compatibilidad con contenidos de la comunidad.
- Estrategia razonable: primero **baseset fijo** y datos propios; luego un subconjunto documentado de NewGRF si el proyecto lo prioriza.

---

## 8. Red (`src/network`)

Archivos como `network.cpp`, `network_client.*`, `network_server.*`, `network_command.cpp`, `network_crypto.*`, `network_content.*`, coordinación (`network_coordinator.*`), administración (`network_admin.*`), etc.

### 8.1 Ideas clave del diseño upstream

- Los clientes ejecutan la misma simulación que el servidor aplicando la **misma secuencia de comandos**.
- Cualquier divergencia sutil produce **desync**; por eso tipos flotantes, iteración sobre hash maps no deterministas y condiciones de carrera son enemigos históricos.

### 8.2 Implicaciones

- Diseñar openttdrs con **determinismo** explícito (RNG sembrado, orden estable, tests de replay).
- La red debería ser una **última fase** tras comandos y estado estable.

---

## 9. Guardados (`src/saveload`)

Decenas de archivos `*_sl.cpp` (saveload por subsistema): `map_sl`, `company_sl`, `engine_sl`, `newgrf_sl`, `game_sl`, etc., más directorio `saveload/compat` para versiones antiguas.

### 9.1 Implicaciones

- El formato de save es un **contrato longitudinal**; reimplementarlo en Rust con paridad total es costoso.
- Alternativas: formato propio versionado + herramientas de importación parcial, o limitar compatibilidad a un subconjunto de partidas.

---

## 10. Vista, render y UI nativa

- **`viewport.*`**: cámara del mundo, ordenación de sprites, interacción con el mapa.
- **`video/`**: drivers (SDL2, OpenGL, null, dedicado, etc.).
- **`window.*`, `widget_*`**: sistema de ventanas y controles propio del juego.
- **`blitter/`**: composición de píxeles en distintos modos.

### 10.1 Implicaciones para Bevy

- En openttdrs, **Bevy** sustituye `video` + parte de `viewport` + UI progresivamente.
- Mantener separación: **estado del mapa** (core) frente a **presentación** (sprites, cámara isométrica más adelante, UI Bevy).

---

## 11. Temporización (`src/timer`)

Archivos `timer_game_tick.*`, `timer_game_calendar.*`, `timer_game_economy.*`, `timer_game_realtime.*`, `timer_manager.h`: separan el reloj de tick de juego, calendario, economía y tiempo real.

### 11.1 Implicaciones

- El `GameTick` de `openttdrs-core` puede evolucionar hacia varios contadores alineados con estos conceptos.

---

## 12. Tabla orientativa: OpenTTD (C++) → openttdrs (Rust)

| Zona upstream (indicativa) | Prioridad sugerida | Crate / capa Rust |
|----------------------------|--------------------|-------------------|
| `tile_*`, `map_*`, pendiente/terraform | Alta (fundamentos) | `openttdrs-core` |
| `timer/timer_game_*` | Alta | `openttdrs-core` |
| `economy_*`, `cargo_*` | Media | `openttdrs-core` |
| `pathfinder/yapf` | Media-alta | `openttdrs-core` o crate `openttdrs-path` |
| `vehicle_*`, `order_*`, `station_*` | Media | `openttdrs-core` + tests |
| `newgrf_*` | Baja al inicio | crate aparte o feature opcional |
| `network_*` | Baja | crate `openttdrs-net` (futuro) |
| `saveload/*` | Baja | crate `openttdrs-save` (futuro) |
| `viewport`, `video`, `window` | Paralela al core | `openttdrs-client` (Bevy) |

---

## 13. Orden de lectura recomendado en el clon

1. `README.md` y `docs/` del upstream para contexto de build y diseño.
2. `src/map_func.h`, `src/tile_map.h`, `src/command.cpp` (flujo de comandos).
3. `src/timer/timer_game_tick.cpp` y economía ligera.
4. `src/pathfinder/yapf/yapf.hpp` (visión general YAPF).
5. `src/network/network_internal.h` (solo cuando abordes multijugador).
6. `src/saveload/game_sl.cpp` (visión de serialización global).

---

## 14. Conclusión

OpenTTD concentra décadas de reglas de negocio en un solo árbol `src/`. Un port sano a Rust separa **simulación determinista**, **contenido** (GRF opcional) y **presentación** (Bevy). Este informe debe actualizarse cuando el clon de referencia cambie de versión o cuando openttdrs incorpore nuevos subsistemas con nombres propios.
