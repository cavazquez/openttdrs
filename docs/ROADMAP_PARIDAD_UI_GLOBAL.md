# Roadmap global de paridad UI con OpenTTD

Fecha base: **2026-07-10**  
Estado: **plan canónico vivo para toolbar, menús, ventanas y subventanas**  
Referencia original: `reference/openttd-upstream/src/`

Este documento convierte la auditoría del código original y del cliente Rust en
un plan ejecutable. Su objetivo no es copiar cada píxel de OpenTTD, sino lograr
la misma **capacidad funcional, descubribilidad y profundidad de navegación**.

Documentos relacionados:

- [ROADMAP_PARIDAD_ESTRUCTURAL.md](ROADMAP_PARIDAD_ESTRUCTURAL.md): dependencias
  de simulación.
- [ROADMAP_MENUS_UI.md](ROADMAP_MENUS_UI.md): detalle histórico de flota.
- [parity/ui_windows_parity.md](parity/ui_windows_parity.md): comparación
  detallada de depósito, vehículo, órdenes y horario.
- [ROADMAP_NEWS_STATUSBAR.md](ROADMAP_NEWS_STATUSBAR.md): noticias/statusbar.
- [ROADMAP_MAIN_MENU.md](ROADMAP_MAIN_MENU.md): menú principal.

---

## 1. Objetivo y definición de paridad

Una feature UI alcanza paridad cuando:

1. **Es descubrible:** existe una ruta visible desde toolbar, menú, lista o
   ventana padre; no exige conocer una tecla o encontrar una entidad en el mapa.
2. **Es operable:** la acción real pasa por `Command`/`apply_command` cuando
   modifica la simulación.
3. **Conserva contexto:** ventanas de entidad y subventanas saben qué compañía,
   vehículo, estación, pueblo o industria están mostrando.
4. **Tiene lifecycle completo:** setup, apertura, sincronización, interacción,
   cierre, Esc y salida de `ClientScreen::InGame`.
5. **Tiene pruebas:** lógica pura, handler ECS y al menos una prueba de flujo de
   apertura para las superficies principales.
6. **Es equivalente funcionalmente:** el layout puede diferir de OpenTTD si no
   elimina información, acciones o rutas de navegación.

No se considera paridad:

- Mostrar datos en el HUD cuando OpenTTD permite gestionarlos en una ventana.
- Tener una variante en un enum sin botón o ruta de apertura.
- Tener una ventana de solo lectura cuando el original permite editar.
- Reemplazar una lista global por «buscar la entidad en el mapa».
- Exponer una acción solo mediante variable de entorno o hotkey no visible.

---

## 2. Baseline medido

Auditoría realizada contra:

- `reference/openttd-upstream/src/widgets/toolbar_widget.h`
- `reference/openttd-upstream/src/toolbar_gui.cpp`
- `reference/openttd-upstream/src/window_type.h`
- `crates/openttdrs-client/src/ui.rs`
- `crates/openttdrs-client/src/ui/toolbar/`
- `crates/openttdrs-client/src/ui/floating_window.rs`

### 2.1 Inventario

| Métrica | OpenTTD | openttdrs |
|---|---:|---:|
| Botones de toolbar en layout completo | 30 | 8 grupos + 3 botones fijos |
| Botones que abren dropdown | ~23 | 0 (no hay dropdown genérico) |
| Herramientas de construcción visibles | ~40 | 50 botones, reorganizados |
| Clases/IDs de ventana principales | 108 `WindowClass` | 17 `FloatingWindowId` |
| Paneles/modales adicionales | sistema Window unificado | ~5 paneles + SaveWindow |
| Acciones de herramienta | widgets por toolbar | 53 `BuildMenuAction` |
| Acciones del panel Ajustes | menú Settings ~22 ítems máximos | 12 `SaveMenuAction` + 16 colores |

### 2.2 Cobertura estructural estimada

Método:

- Inventariar las 108 clases útiles de `WindowClass`.
- Puntuar equivalente completo = 1, parcial = 0,5, solo lectura = 0,15,
  ausente = 0.
- Separar categorías para no ocultar que construcción está mucho más avanzada
  que navegación o gráficos.

| Categoría | Cobertura base | Diagnóstico |
|---|---:|---|
| Ventanas de entidad | ~62 % | fuerte en vehículo/órdenes/horario |
| Toolbars de construcción | ~55 % | rail/road fuertes; faltan tram y selectores |
| Toolbar principal | ~38 % | funciones condensadas en 8 grupos |
| **Global WindowClass** | **~24 %** | baseline del roadmap |
| Settings/meta | ~18 % | audio, noticias, save, PBS, NewGRF RO |
| Dropdowns/submenús | ~8 % | no existe primitiva reusable |
| Directorios/listas | ~5 % | casi todos ausentes |
| Gráficos/league | 0 % | sin equivalente |

Estas cifras son una métrica de planificación, no una afirmación de precisión
estadística. Deben recalcularse al finalizar cada hito.

---

## 3. Fortalezas actuales que deben conservarse

### 3.1 Arquitectura cliente

- `ClientUiPlugin` centraliza recursos y sistemas.
- `FloatingWindowPlugin` ya resuelve drag, z-order, cierre y foco visual.
- `InGameLifecyclePlugin` desmonta UI y reinicia recursos al salir.
- La UI usa comandos de core para acciones jugables.
- Hay paneles funcionales de órdenes, estación e industria.
- Guardar/cargar, statusbar, noticias, audio y menú principal tienen flujo real.

### 3.2 Construcción

- Paneles rail/road/water/air/landscape visibles y operativos.
- Pickers de estación ferroviaria y puente.
- Ghost previews e interacción drag.
- Señales y PBS integrados, aunque sin sub-toolbar visual equivalente.

### 3.3 Flota

- Ventana de vehículo con preview real.
- Órdenes editables y horario.
- Depósito, compra, venta, clonado y consist MVP.
- Finanzas, pueblo y noticias tienen ventanas funcionales.

El roadmap debe ampliar esta base; no reemplazarla con una copia literal del
sistema `Window` de C++.

---

## 4. Problema principal: navegación, no render de ventanas

OpenTTD usa esta cadena:

```text
MainToolbar
  → DropdownMenu
    → directorio/lista/BuildToolbar
      → vista de entidad
        → subventana (órdenes, detalles, refit, horario)
```

openttdrs usa principalmente:

```text
ToolbarGroup
  → panel inline de herramientas

o:

clic en mapa
  → ventana contextual/panel
```

La segunda estrategia funciona bien para construir, pero escala mal para
gestionar cientos de vehículos, estaciones o industrias. La mayor mejora de
paridad vendrá de añadir **rutas globales de acceso**.

---

## 5. Arquitectura UI objetivo

### 5.1 Enfoque híbrido

No copiar la toolbar de 30 botones de forma rígida. Adoptar:

1. **Acciones directas:** pausa, velocidad, guardar/cargar.
2. **Menús de navegación:** mapa, mundo, compañía, flota, ajustes.
3. **Launchers de construcción:** rail, road, water, air, landscape.
4. **Ventanas persistentes:** listas, vistas de entidad, detalles.
5. **Popovers efímeros:** selección de menú, tipo, filtro o acción secundaria.

### 5.2 Jerarquía propuesta

```text
Toolbar
├─ Tiempo
│  ├─ Pausa
│  └─ Velocidad
├─ Archivo
│  ├─ Guardar
│  ├─ Cargar
│  └─ Menú principal
├─ Mapa
│  ├─ Minimapa
│  ├─ Mapa ampliado
│  ├─ Extra viewport
│  ├─ Flujo de carga
│  └─ Carteles
├─ Mundo
│  ├─ Pueblos
│  ├─ Industrias/cadenas
│  ├─ Estaciones
│  └─ Subvenciones
├─ Compañía
│  ├─ Finanzas
│  ├─ Infraestructura
│  ├─ Gráficos
│  └─ Apariencia
├─ Flota
│  ├─ Trenes
│  ├─ Vehículos carretera
│  ├─ Barcos
│  ├─ Aeronaves
│  ├─ Grupos
│  └─ Autoreemplazo
├─ Construcción
│  ├─ Rail
│  ├─ Road/tram
│  ├─ Water
│  ├─ Air
│  └─ Landscape
├─ Ajustes
│  ├─ Juego/display
│  ├─ Transparencia
│  ├─ Noticias
│  ├─ Pathfinding/PBS
│  └─ NewGRF
└─ Sonido / Noticias / Ayuda
```

La toolbar debe adaptarse a ancho reducido mediante grupos o una segunda fila,
sin ocultar rutas funcionales.

### 5.3 Primitivas nuevas

#### `UiRoute`

Ruta tipada para desacoplar botones de ventanas:

```rust
enum UiRoute {
    TownDirectory,
    IndustryDirectory,
    StationList { company: CompanyId },
    VehicleList { company: CompanyId, kind: VehicleKind },
    Finances { company: CompanyId },
    Graph(GraphKind),
    Build(BuildToolbarKind),
    Settings(SettingsPage),
}
```

#### `MenuSpec`

- Lista de entradas.
- Enabled/checked.
- Separadores e indentación.
- Tooltip/hotkey.
- Resultado `UiRoute` o acción cliente.
- Filtro opcional para railtype/roadtype.

#### `WindowKey`

Permitir múltiples ventanas de la misma clase:

```rust
struct WindowKey {
    kind: WindowKind,
    instance: u32,
}
```

Ejemplo: una ventana de órdenes para vehículo 17 y otra para vehículo 42.

#### `ListWindow`

Componente reutilizable para:

- scroll/virtualización;
- sort ascendente/descendente;
- filtro de texto y categoría;
- selección;
- doble clic para abrir;
- centrar cámara;
- acciones masivas.

TownDirectory, IndustryDirectory, StationList y VehicleList deben compartir esta
infraestructura.

### 5.4 Organización sugerida de archivos

```text
ui/
├─ navigation.rs
├─ menu/
│  ├─ mod.rs
│  ├─ model.rs
│  ├─ setup.rs
│  ├─ sync.rs
│  └─ handlers.rs
├─ list_window/
│  ├─ mod.rs
│  ├─ model.rs
│  ├─ setup.rs
│  ├─ sync.rs
│  └─ handlers.rs
├─ directories/
│  ├─ towns.rs
│  ├─ industries.rs
│  ├─ stations.rs
│  └─ vehicles.rs
└─ windows/
   ├─ station_view.rs
   ├─ vehicle_details.rs
   ├─ refit.rs
   └─ ...
```

No es obligatorio mover inmediatamente los archivos actuales. La migración
puede ser incremental.

---

## 6. Clasificación de dependencias

### A — Solo cliente/UI

El backend ya permite una primera versión:

- Dropdown/popover genérico.
- TownDirectory.
- IndustryDirectory básica.
- StationList.
- VehicleList por tipo.
- Abrir `DestinationPicker` existente.
- Vista de detalles con edad/peso/potencia/coste/fiabilidad disponibles.
- Refit con lista para vehículo completo.
- Lista de subvenciones (`subsidy.rs` ya existe).
- Mejoras de depósito: sprites, scroll >8, drag visual.
- Llegadas/salidas esperadas derivadas del horario.
- Más filtros/sorts en compra.
- Transparencia/display sobre flags ya existentes.

### B — Comando/campo acotado en core

- Renombrar depósito y grupo.
- Borrar/renombrar grupos.
- Refit por unidad y refit como orden.
- Variantes full-load y unload forzado.
- Más variables/comparadores de orden condicional.
- Velocidad máxima por tramo en horario.
- Acciones masivas globales de flota.
- Dar vuelta a vehículo road.
- Tractive effort en `EngineDef`.
- Históricos económicos mínimos por mes/año.

### C — Dependencia estructural

- Multi-compañía editable/seleccionable.
- Servicio, averías, unbunch y beneficio por vehículo.
- CargoDist/link graph completo.
- Paradas intermedias/non-stop semántico.
- NewGRF runtime Action0–14 y station/rail/road specs.
- Multijugador.
- Scenario editor.
- GameScript/AI.

Las fases no deben bloquearse por C si existe un MVP útil con datos actuales.

---

## 7. Fases del roadmap

## UI-0 — Baseline, gobernanza y harness

Prioridad: **P0**  
Objetivo de cobertura: mantener ~24 %, hacerla medible.

### Entregables

- [ ] Convertir este inventario en checklist versionado por feature.
- [ ] Añadir test que enumere `ToolbarGroup`, `BuildMenuAction`,
      `SaveMenuAction` y `FloatingWindowId`.
- [x] Detectar rutas huérfanas: ventana registrada pero nunca abierta.
- [x] Corregir `DestinationPicker`: «Ir a» abre la lista y desde ella el picker
      sobre mapa.
- [ ] Corregir documentación divergente de `RailConvert`, depósito y órdenes.
- [ ] Ampliar `OPENTTDRS_WINDOWS_SHOT` a todas las ventanas/paneles.
- [ ] Capturas de referencia 1280×720 y 1920×1080.

### Criterios de aceptación

- No hay `WindowState` registrado sin ruta de apertura documentada.
- Cada ventana tiene owner/lifecycle identificado.
- `check.sh` valida inventario y pruebas.
- El porcentaje baseline puede recalcularse con el mismo método.

---

## UI-1 — Infraestructura de navegación

Prioridad: **P0**  
Objetivo de cobertura global: **~28–30 %**.

### Entregables

- [x] `UiRoute` tipado y primer popover reusable (`Mundo`).
- [x] Generalizar las entradas a `MenuSpec` declarativo.
- [x] Anclaje al botón, z-order y posicionamiento dentro del viewport.
- [x] Checked/disabled/divider/hotkey.
- [x] Cierre por selección, Esc, clic externo y cambio de pantalla.
- [x] Navegación teclado arriba/abajo/Enter/Esc.
- [x] Protección contra click-through al mapa (`BuildMenuUi` + `UiToolState.block_map_click`).
- [x] `ListWindow` base con sort, filtro, scroll y selección.
- [x] Migrar tres menús piloto: Mapa, Mundo e Industrias (también Flota/Economía sobre la misma base).

### Criterios de aceptación

- [x] Tres botones toolbar usan la misma primitiva de menú.
- [x] No existen handlers duplicados por cada menú.
- [x] Abrir/cerrar repetidamente no deja entidades ni estados huérfanos.
- [x] Tests cubren foco, click externo y Esc.

---

## UI-2 — Directorios y listas globales

Prioridad: **P0**  
Objetivo de cobertura global: **~36–40 %**.

### UI-2A — Pueblos

- [x] TownDirectory ordenable por nombre/población.
- [x] Añadir sort por rating.
- [x] Centrar cámara directamente desde la fila.
- [x] Clic en fila abre `TownWindow`.
- [ ] Acción «Fundar pueblo» si el backend lo permite.

### UI-2B — Industrias

- [x] IndustryDirectory ordenable por tipo/stock.
- [x] Clic abre `IndustryPanel`.
- [x] Vista inicial de cadenas input/output.
- [x] Integrar construcción/fundación desde el menú.

### UI-2C — Estaciones

- [x] StationList global ordenable por nombre/rating/carga waiting.
- [x] Clic abre `StationCargoPanel`.
- [x] Filtro por compañía.
- [x] Filtro por facility/carga.
- [x] Waiting cargo y rating disponibles.

### UI-2D — Flota

- [x] VehicleList para tren, road, ship y aircraft.
- [x] Sort por nombre, edad, velocidad (beneficio pendiente de core).
- [x] Start/stop, enviar a depósito y centrar.
- [x] Clic abre `VehicleWindow`.
- [x] Selección por compañía; inicialmente compañía activa.

### Criterios de aceptación

- Cualquier entidad principal se abre sin buscarla en el mapa.
- Listas funcionan con 500 elementos sin caída perceptible.
- Acciones masivas usan comandos de core.
- Tests abren la vista de entidad desde cada directorio.

---

## UI-3 — Mundo, estaciones y subvenciones

Prioridad: **P1**  
Objetivo de cobertura global: **~44–47 %**.

### StationView completa

- [x] Nombre/rename (comando `RenameStation` + UI en StationPanel).
- [x] Carga waiting por tipo y rating.
- [x] Vehículos que visitan la estación.
- [x] Botón centrar.
- [x] Acceso a lista filtrada de vehículos.
- [x] WaypointView equivalente (vista simplificada).

### TownView / autoridad

- [x] Rating por compañía (rating global de autoridad mostrado).
- [x] Acciones de autoridad local.
- [x] Crecimiento, población e historial básico.
- [x] Historial temporal / gráfico. *(series mensuales + sparkline en TownWindow)*

### IndustryView

- [x] Producción/transportado por cargo (stock + cadena I/O).
- [x] Inputs/outputs.
- [x] Gráfico básico cuando exista histórico. *(series mensuales + sparkline en IndustryPanel)*
- [x] Mantener preview actual.
- [x] Panel jugable: ritmo de producción, Centrar, sin texto debug.

### SubsidyList

- [x] Ofertas y contratos activos.
- [x] Tiempo restante.
- [x] Clic origen/destino centra mapa.
- [x] Abrir entidad relacionada (industria + estación + centrar destino).

### StationView polish

- [x] Owner, ingresos, tiles unidas, cobertura y días sin recogida en panel.
- [x] Directorio de estaciones centra cámara al seleccionar fila.
- [x] Directorio de industrias centra cámara al seleccionar fila.

### Criterios de aceptación

- [x] Las ventanas dejan de ser meros paneles contextuales.
- [x] Subsidios del core son visibles y navegables.
- [x] Las relaciones estación↔vehículos e industria↔cargos son accesibles.

---

## UI-4 — Flota y subventanas de vehículo

Prioridad: **P1**  
Objetivo de cobertura global: **~50–55 %**.

### VehicleDetails

- [x] Edad/vida útil (edad + aviso renovar; sin max lifespan en core).
- [x] Peso, potencia, coste y fiabilidad (runtime + diseño).
- [x] Detalle por unidad del consist (conteo + tira horizontal de sprites).
- [x] Pestañas cargo/info/capacidad/totales.
- [ ] Beneficio cuando exista backend.

### RefitWindow

- [x] Lista de cargas.
- [x] Capacidad y coste (capacidad actual; coste gratis en core).
- [x] Refit de vehículo completo.
- [ ] Selección parcial de consist cuando exista comando.

### Orders

- [x] Abrir/cablear `DestinationPicker` o eliminarlo en favor de pick directo.
- [x] Reordenar (botones ↑/↓; drag nativo de órdenes pendiente).
- [x] Variantes full-load/unload.
- [x] Parar en depósito (toggle).
- [ ] Refit en orden.
- [x] Lista de órdenes compartidas.
- [x] Condicionales crear/editar (carga sobre/bajo umbral + ciclar).

### Depot / BuyVehicle

- [x] Sprites reales por fila + tira horizontal del consist (hasta 8 unidades).
- [x] Scroll >8 (hasta 24 filas).
- [x] Reorden de filas ↑/↓ y drag nativo (`DepotReorderVehicleSlot`).
- [x] Autoreplace global.
- [x] Filtros/sorts + búsqueda (texto, Tram, loco/vagón).

### Criterios de aceptación

- Flujo `VehicleList → Vehicle → Orders/Details/Refit/Timetable`.
- Flujo `Depot → BuyVehicle/Autoreplace`.
- Subventanas conservan el `VehicleID` correcto.
- **Política single-instance (MVP):** una ventana por `FloatingWindowId`
  (Vehicle, Orders, Refit, Timetable, Depot…). Al seleccionar otro vehículo se
  reemplaza el `Option<vehicle_id>` compartido; no coexisten dos VehicleDetails.
  Multi-instance (`WindowKey`) queda documentado en §12.2 como trabajo futuro.

---

## UI-5 — Economía, mapas, opciones y gráficos

Prioridad: **P1/P2**  
Objetivo de cobertura global: **~58–62 %**.

### Economía

- [x] Finances con histórico y categorías. *(beneficio operativo + infra; series mensuales en core)*
- [x] CompanyInfrastructure. *(conteos vía/carretera/vehículos/estaciones en Finances)*
- [x] Income/Operating Profit mínimos. *(GraphWindow + menú Economía)*
- [x] Delivered cargo y company value. *(entregas en Finances; CompanyValue en gráficos)*
- [x] Graph legend (swatches por serie). *(filtro por compañía en GraphWindow)*

### Mapas

- [x] SmallMap expandible con capas. *(toggles Ind/Due/Veh + Ampliar)*
- [x] Leyenda y filtros. *(leyenda compacta IDV+)*
- [x] ExtraLargeMap. *(botón Ampliar / Esc para cerrar)*
- [x] ExtraViewport. *(MVP: sigue cámara principal, zoom alejado)*
- [x] SignList. *(lista real + PlaceSign / Rename / Remove)*
- [x] LinkGraph. *(observacional: flujos estación→estación; sin routing CargoDist)*
- [x] LinkGraphLegend. *(ventana con top aristas + filtro cargo; overlay mapa OOS)*

### Opciones/display

- [x] Ventana Game/Display Options. *(Ajustes → Display…)*
- [x] Nombres de pueblos/estaciones/facilities. *(toggles pueblos + estaciones)*
- [x] Full animation/detail. *(toggles + gate paleta; faroles/árboles/cercas vía FullDetail; faro/estadio)*
- [x] Transparencia e invisibilidad por categorías. *(matriz TO_* en Display Options)*
- [x] Persistencia en `ClientPreferences`.
- [x] Mantener TO_CATENARY actual dentro de esta UI.

### Criterios de aceptación

- [x] Toolbar Graphs abre al menos Income y Operating Profit reales.
- [x] Preferencias afectan render y sobreviven reinicio. *(Display Options → ClientPreferences)*
- [x] Mapa ampliado permite navegar y entender capas. *(ExtraLargeMap + capas Ind/Due/Veh)*

### Extra UI-5 (este corte)

- [x] Ring buffer mensual `EconomyHistory` en core (`ECONOMY_HISTORY_MONTHS = 36`).
- [x] Ventana `CargoPaymentRates` (tarifas base / tránsito).
- [x] CompanyValue mensual en series + GraphKind.
- [x] ExtraLargeMap (minimapa centrado, celda 8px).
- [x] ExtraViewport MVP + Display Options.
- [x] SignList real (UI-6b adelantado).
- [x] LinkGraph: observacional (`LinkGraphStats` + UI; routing CargoDist OOS).
- [x] NewGRF editable (config-only: ON/OFF, ↑/↓, quitar; sin Action0–14).

**UI-5 cerrado** (criterios + extras jugables). Parámetros NewGRF / runtime Action0–14 → OOS (UI-7 cerró stack config-only).
Routing CargoDist completo (next_hop / modos) sigue OOS.

---

## UI-6 — Completitud de construcción

Prioridad: **P2**  
Objetivo de cobertura global: **~65–68 %**.

### UI-6a (corte jugable — backend ya existe)

- [x] Sub-toolbar / panel flotante de señales (tipo + densidad).
- [x] Selector de railtype en toolbar Rail (`current_rail_type`) + HUD.
- [x] BuildTrees / `PlantTree` en Landscape.
- [x] Fuera de alcance documentado (ver abajo).

### UI-6b (carteles)

- [x] Entidad `Sign` + `PlaceSign` / `RemoveSign` / `RenameSign`.
- [x] Herramienta Paisaje → Cartel.
- [x] SignList real (centrar / renombrar / borrar) + etiquetas en mapa.
- [x] ClearTile quita carteles de la tesela.

### UI-6c (tranvía visual)

- [x] `RoadType` + `PlaceTramBits` (m3/m8).
- [x] Construir carretera preserva overlay de tranvía.
- [x] Toolbar Road: Tranvía X/Y/Cruce + preview/HUD.
- [x] Vehículos: ver UI-6e.

### UI-6d (JoinStation MVP)

- [x] `Command::JoinStations` para paradas bus/camión 1×1 adyacentes.
- [x] `Station.joined_tiles` + `covers_tile`.
- [x] Reescritura de órdenes / subsidios / pools compartidos.
- [x] Herramienta Road «Unir estaciones» (2 clics) + botón panel «Unir…».
- [x] Rail: huellas adyacentes + mismo eje; `station_at_tile`; toolbar Rail.

### UI-6e (vehículos de tranvía MVP)

- [x] `VehicleKind::Tram` + motor vanilla `ENGINE_TRAM_MPS`.
- [x] `PathNetwork::Tram` sobre bits m3 (sin fallback 0x0F).
- [x] Compra en depósito de carretera + salida con overlay tram.
- [x] Paradas bus / lista Road / sprites bus como placeholder.
- [x] Fuera de alcance: depósito tram dedicado, NewGRF, sprites propios.

### UI-6f (selectores roadtype/tramtype)

- [x] Catálogo `RoadTypeDef` + `list_road_types(class, filter)` (hook NewGRF).
- [x] `current_road_type` / `current_tram_type` en `GameState` + escritura en m8.
- [x] Dropdowns filtrables en toolbar Road (C:… / T:…) + HUD.
- [x] Fuera de alcance: tipos NewGRF reales con sprites. *(Action0 RoadTypes metadatos ✅)*

### UI-6g (station classes / layouts)

- [x] Catálogo `StationClass` / `StationSpec` + `list_station_*` filtrable (hook NewGRF).
- [x] `current_station_class` / `current_station_spec` + `Station.station_spec`.
- [x] Picker rail: dropdowns clase/tipo + tamaños deshabilitados por spec.
- [x] Layout vía `station_spec_layout` (vanilla = `rail_station_layout`).
- [x] Action0 Stations (0x04) metadatos → catálogo dinámico (layouts gfx NewGRF OOS).

### UI-6h (boyas / acueducto)

- [x] `StopKind::Buoy` + `PlaceBuoy` (waypoint acuático, sin carga).
- [x] `PlaceAqueduct` (rampas en pendiente enfrentadas + vano con `bridge_above`).
- [x] Toolbar Agua: Boya / Acueducto + preview/drag.
- [x] Render `buoy.png` + tablero de acueducto vía `spawn_bridge_middle`.
- [x] Rivers: `WaterClass::River` + `PlaceRiver` + generación en `world_gen`.

### Pendiente / fuera de alcance (requiere más backend)

- [~] JoinStation tipos mixtos (road+rail / aeropuerto / dock). *(OOS)*
- [x] BuildWaypoint road MVP (`PlaceRoadWaypoint` + botón toolbar; órdenes road vía `VehicleOrder::Waypoint`).
- [x] Airport picker extensible.
- [~] BuildObject genérico. *(OOS UI-6: BuyLand = objeto jugable; faro/transmisor = worldgen/saves; NewGRF runtime → OOS post UI-7)*
- [~] Separar herramientas sandbox/editor. *(etiquetadas «editor» en Economía/Agua; modo editor UI-8)*

### Criterios de aceptación

- [x] Cada herramienta upstream base tiene botón, selector o decisión explícita de
  fuera de alcance.
- [x] Tipo seleccionado se conserva y se muestra.
- [x] Pickers tienen coste, disponibilidad y preview.

**UI-6 cerrado** para el corte jugable documentado.

---

## UI-7 — Settings avanzados y modding

Prioridad: **P2/P3**  
Objetivo de cobertura global: **~70–75 %**.

**UI-7 cerrado** (corte jugable). Runtime Action0–14 completo (sprites/callbacks) /
parámetros NewGRF / consola REPL / cheats formales → OOS o UI-8.

- [x] NewGRF editable config-only: ON/OFF/↑↓/quitar + **Añadir…** desde disco
      (`assets/opengfx/…` / `OPENTTDRS_NEWGRF_DIR`).
- [x] Parse-only Action0–14 (histograma Inspeccionar) + Action0 RoadTypes / Stations / Trains metadatos.
- [ ] Parámetros NewGRF. *(OOS: sin runtime de params)*
- [x] Presets de settings (Clásico / Rendimiento / Dev en Display Options).
- [ ] Sandbox/cheats si se decide soportarlos. *(OOS)*
- [x] Consola y diagnostics para desarrollo. *(UI-8: Consola/Dev + overlay; REPL/cheats OOS)*
- [x] About/help y mapa de hotkeys (F1 / ? + Ajustes → Ayuda…).
- [x] Posiciones de ventana persistentes (`ClientPreferences.window_layouts`).

NewGRF editable **completo** (sprites Action1/3/5 + callbacks) sigue OOS;
RoadTypes / Stations / Trains metadatos alimentan catálogos.
**Preview + in-world trenes Action1/3** (8bpp sin comprimir, sin Action2/5) ✅
compra depósito + mapa (fallback OpenGFX si no hay vistas).
**Preview RoadTypes Action1/3** ✅ en selector carretera/tranvía (sin gfx in-world).
**Preview Stations Action1/3** ✅ en picker rail (sin gfx in-world / layouts 0x0E).
**Action5 parse + Inspeccionar** ✅ slots (tipo/offset/count + primer sprite); runtime tiles OOS.
     
---

## UI-8 — Modos opcionales

Prioridad: **P3**  
Objetivo: posterior a la paridad single-player.

**UI-8 (cortes tools-dev + highscore/endscreen) cerrados.** Resto de modos
(multi, MP, editor, GS/AI) siguen pendientes.

- [x] Multi-compañía mínima: rival IA opcional, selector activa, HUD/listas.
- [x] Multi-compañía polish ownership: `m1` en vía/carretera/depósitos, render por owner, guards flota. *(resto: segunda humana, demolish ownership, MP)*
- [x] Multi-compañía demolish/build: no pisar/demoler infra ajena (`TileNotOwned`). *(resto: segunda humana, MP)*
- [ ] Multi-compañía completa en toolbar/listas/finanzas. *(resto: segunda humana, …)*
- [ ] Multijugador: lobby, clientes, chat, join/spectate.
- [ ] Scenario editor y toolbar de 19 botones.
- [ ] GameScript: story, goals, league.
- [ ] AI settings/debug.
- [x] Herramientas dev: framerate / consola corta, tile inspect, NewGRF inspect,
      sprite-bounds lite (gizmos + tile seleccionado).
- [x] Highscore/endscreen. *(retiro + quiebra ×3 meses; tabla local en prefs)*
- [ ] Consola REPL completa / cheats formales. *(OOS; cmds help/fps/…/endgame/clear)*

No usar UI-8 para bloquear UI-1 a UI-6.

---

## 8. Roadmap por prioridad

### P0 — Próximos trabajos

1. UI-0 baseline/harness.
2. UI-1 dropdown + navegación.
3. UI-2 VehicleList.
4. UI-2 Town/Industry/Station directories.
5. StationView mínima.

### P1 — Paridad operativa

1. SubsidyList.
2. VehicleDetails/Refit.
3. Depot polish y autoreplace.
4. Órdenes avanzadas.
5. Finanzas/gráficos mínimos.
6. Opciones visuales completas.

### P2 — Completitud clásica

1. Trams y selectores de tipo.
2. JoinStation/WaypointView.
3. SmallMap/ExtraViewport.
4. Construcción trees/objects/signs.
5. NewGRF editable cuando sea viable.

### P3 — Modos avanzados

1. Red/multi-compañía.
2. Editor.
3. GS/AI.
4. Debug/console/highscore.

---

## 9. Hitos y metas de cobertura

| Hito | Fases | Meta orientativa |
|---|---|---:|
| UI 0.1 | UI-0 | baseline reproducible |
| UI 0.2 | UI-1 | 28–30 % |
| UI 0.3 | UI-2 | 36–40 % |
| UI 0.4 | UI-3 | 44–47 % |
| UI 0.5 | UI-4 | 50–55 % |
| UI 0.6 | UI-5 | 58–62 % |
| UI 0.7 | UI-6 | 65–68 % |
| UI 0.8 | UI-7 | 70–75 % |
| UI 1.0 | UI-8 seleccionado | 80–85 %+ |

Las metas porcentuales no sustituyen los criterios de aceptación. Una fase no
está completa por añadir IDs de ventana: debe cerrar un flujo de usuario.

---

## 10. Estrategia de pruebas

### 10.1 Pruebas por ventana

Cada ventana/panel nuevo debe incluir:

- test de `setup_*`;
- test de apertura desde su ruta real;
- test de `sync_*` con datos vacíos y poblados;
- test de cada handler;
- test de cierre y limpieza del state;
- test de Esc/topmost;
- test de salida/reentrada a `InGame`.

### 10.2 Pruebas de navegación

- Toolbar → menú → ventana.
- Directorio → entidad → subventana.
- Ventana hija conserva contexto del padre.
- Clic sobre UI no llega al mapa.
- Menú no sale del viewport.
- Navegación por teclado.
- Cierre de parent cierra o desacopla children según política.

### 10.3 Pruebas visuales

Ampliar `windows_shot`:

- todas las `FloatingWindowId`;
- SaveWindow;
- OrderPanel, StationCargoPanel, IndustryPanel, Minimap;
- toolbar y cada panel de grupo;
- viewport 1280×720 y 1920×1080;
- datos mínimos y datos con scroll.

### 10.4 Rendimiento

- 500 vehículos en VehicleList.
- 200 estaciones.
- 200 industrias.
- filtros y sorts sin asignaciones por frame innecesarias.
- sync solo ante cambios relevantes/caché snapshot.

---

## 11. Métricas de seguimiento

Actualizar al cerrar cada fase:

| Métrica | Baseline |
|---|---:|
| Cobertura global WindowClass | ~24 % |
| Toolbar principal | ~38 % |
| Construcción | ~55 % |
| Ventanas entidad | ~62 % |
| Dropdowns/submenús | ~8 % |
| Directorios/listas | ~5 % |
| Gráficos | 0 % |
| FloatingWindow con ruta de apertura | 17/17 tras DestinationPicker + 3 directorios globales |
| Ventanas con test de apertura real | bajo; medir en UI-0 |

Métricas adicionales:

- número de rutas huérfanas;
- número de acciones UI que mutan sim sin `Command`;
- número de ventanas sin test handler;
- número de features A/B/C cerradas;
- tiempo medio para encontrar una entidad sin usar el mapa.

---

## 12. Riesgos y decisiones

### 12.1 Copiar layout vs copiar capacidad

Decisión recomendada: copiar **capacidad y jerarquía**, no dimensiones exactas.
Los paneles Bevy actuales pueden mantenerse si:

- no ocultan features;
- tienen ruta visible;
- soportan scroll/filtro;
- conservan relación padre-hijo.

### 12.2 Single-instance vs multi-instance

**Decisión MVP (UI-4):** single-instance por `FloatingWindowId`. Los resources
(`VehicleWindowState`, `OrderEditState`, `RefitWindowState`,
`TimetableWindowState`, …) guardan un único `Option<ID>`; abrir otro contexto
reemplaza el anterior. Documentado en `floating_window.rs` y criterios UI-4.

Futuro (más cercano a OpenTTD): `WindowKey { kind, instance }` para
Vehicle/Orders/Details/Station. Directorios, settings, gráficos y audio siguen
single-instance.

### 12.3 Documentación histórica divergente

`ROADMAP_MENUS_UI.md` y `parity/ui_windows_parity.md` contienen snapshots de
distintas fechas. Este archivo es la fuente canónica para prioridad global.
Los documentos históricos conservan detalle técnico, pero no deben usarse para
inferir cobertura global sin verificar el código.

### 12.4 Backend incompleto

No crear controles que aparenten funcionar si el comando no existe. Marcar:

- disabled con explicación, o
- no exponer hasta que exista un MVP real.

---

## 13. Definition of Done por fase

Una fase se marca ✅ cuando:

- [ ] Todos sus criterios de aceptación están cubiertos.
- [ ] `scripts/check.sh` pasa.
- [ ] Documentación y porcentaje se actualizan.
- [ ] No quedan states/acciones huérfanos nuevos.
- [ ] Hay pruebas de apertura e interacción.
- [ ] Hay captura visual de referencia.
- [ ] Las mutaciones usan comandos.
- [ ] Se documentan divergencias UX deliberadas.

---

## 14. Siguiente corte recomendado

**UI-0 + UI-1A: infraestructura de navegación**

1. Inventario automatizado de rutas.
2. ~~Arreglar `DestinationPicker`.~~ ✅
3. ~~Implementar popover reusable inicial.~~ ✅
4. ~~Implementar `ListWindow` genérico.~~ ✅
5. ~~Probar con TownDirectory.~~ ✅
6. ~~Migrar IndustryDirectory y StationList.~~ ✅
7. ~~Construir VehicleList ×4 sobre la misma base.~~ ✅

**Siguiente:** station in-world / Action5 shore-catenary, o segunda humana / editor.

Progreso UI-8 (multi-compañía mínima):

1. ~~`set_active_company` + sync espejo.~~ ✅
2. ~~Nueva partida: toggle Rival IA (TransCargo).~~ ✅
3. ~~Selector compañía en toolbar + nombre en statusbar.~~ ✅
4. ~~Finanzas título + vehicle list Mía/Todas.~~ ✅
5. ~~Color swatch → pool (`sync_active_from_mirrors`).~~ ✅
6. ~~Ownership tiles (`m1`) PlaceRail/PlaceRoad/depósitos.~~ ✅
7. ~~Render estaciones/depósitos/vehículos por owner (caché multi-paleta).~~ ✅
8. ~~Guards flota (`VehicleNotOwned`) + feedback UI.~~ ✅
9. ~~Demolish/build ownership (`TileNotOwned` + preview).~~ ✅
10. Segunda humana / tram `m3` / MP → OOS.

Progreso NewGRF Action0–14 (parse + metadatos + preview + in-world):

1. ~~Walker parse-only Action0–14 + histograma Inspeccionar.~~ ✅
2. ~~Action0 RoadTypes (0x12) metadatos → catálogo dinámico + selector.~~ ✅
3. ~~Action0 Stations (0x04) metadatos → catálogo clase/spec + picker rail.~~ ✅
4. ~~Action0 Trains (0x00) metadatos → `engine_catalog` + compra depósito rail.~~ ✅
5. ~~Action1/3 trains → preview 8bpp en compra (sin Action2/5).~~ ✅
6. ~~Action1/3 trains → sprites in-world (dirs / 1 vista reutilizada; sin Action2).~~ ✅
7. ~~Action1/3 RoadTypes → preview en selector (sin in-world).~~ ✅
8. ~~Action1/3 Stations → preview en picker rail (sin in-world / 0x0E).~~ ✅
9. ~~Action5 → parse slots + resumen Inspeccionar (sin runtime tiles).~~ ✅
10. ~~Action1/3 RoadTypes → in-world plano (vista 0; sin pendientes/tram overlay).~~ ✅
11. Station in-world / Action5 shore-catenary / callbacks / 32bpp → OOS.

Progreso backend OOS (JoinStation rail + CargoDist observacional):

1. ~~JoinStation rail (huella/eje + `station_at_tile` + toolbar Rail).~~ ✅
2. ~~Link graph observacional (`LinkGraphStats`, save v18, UI LinkGraph).~~ ✅
3. Routing CargoDist completo (next_hop / modos) → OOS futuro.

Progreso UI-8 (highscore/endscreen):

1. ~~Trigger fin: quiebra ×3 meses + retiro (Ajustes / `endgame`).~~ ✅
2. ~~Endscreen modal + ranking local.~~ ✅
3. ~~Highscores persistidos en `ClientPreferences`.~~ ✅
4. ~~Menú «Mejores puntuaciones» + salida sin Continuar.~~ ✅

Progreso UI-8 (tools-dev):

1. ~~Consola / Dev (F3 / `` ` ``): FPS, frame ms, toggles overlay/gizmos, cmds.~~ ✅
2. ~~Inspector de tile (F2) + dump estructurado.~~ ✅
3. ~~NewGRF Inspeccionar (scan + validate_stack).~~ ✅
4. ~~Sprite bounds lite en tile seleccionado (gizmos ON).~~ ✅
5. ~~Help + menú Ajustes actualizados.~~ ✅

Progreso UI-7 (cierre):

1. ~~Ayuda / About + hotkeys (F1 / ? + menú Ajustes).~~ ✅
2. ~~Persistencia de posiciones de ventanas flotantes.~~ ✅
3. ~~NewGRF «Añadir…» desde disco (scan `.grf` + `AddNewGrfToStack`).~~ ✅
4. ~~Presets Clásico / Rendimiento / Dev.~~ ✅
5. ~~Parámetros NewGRF / Action0–14 / consola / cheats documentados OOS.~~ ✅

Progreso UI-6 (cierre):

1. ~~Road Waypoint MVP (`PlaceRoadWaypoint` + toolbar + preview).~~ ✅
2. ~~Etiquetas editor (Río / casa / industrias) + BuyLand como objeto.~~ ✅
3. ~~JoinStation / BuildObject genérico documentados como OOS.~~ ✅

Progreso UI-5 (cierre):

1. ~~Economía + Graph + Display + mapas (criterios).~~ ✅
2. ~~SignList real.~~ ✅
3. ~~LinkGraph stub documentado (fuera de alcance sin CargoDist).~~ ✅
4. ~~NewGRF editable config-only (comandos + UI ON/OFF/↑↓/quitar).~~ ✅

Progreso UI-1 (`MenuSpec`):

1. ~~Modelo `MenuSpec`/`MenuId`/`MenuAction` + specs Mapa/Mundo/Industrias/Flota/Economía.~~ ✅
2. ~~Chrome anclado + sync checked/disabled/focus.~~ ✅
3. ~~Teclado ↑↓ Enter Esc + dismiss outside + anti click-through.~~ ✅
4. ~~Handlers unificados (sin popover Mundo ad-hoc).~~ ✅
5. ~~Pulido visual: ancla real bajo botón, tipografía Caption, check/hotkey, focus≠checked, Flota/Economía alineados al borde.~~ ✅

Progreso Airport picker:

1. ~~Catálogo `AirportClass`/`AirportSpec` (Heliport…Commuter).~~ ✅
2. ~~`PlaceAirportArea` con `spec` + layouts parametrizados.~~ ✅
3. ~~Ventana picker (clase/tipo/eje/cobertura) + toolbar unificado.~~ ✅
4. ~~Preview footprint + halo de cobertura; RMB rota eje.~~ ✅
5. ~~Layouts grandes: City, Metropolitan, International, Intercontinental.~~ ✅

Progreso UI-0 (`ListWindow`):

1. ~~Módulo `ui/list_window` (SortDir, filtro texto, chrome compartido).~~ ✅
2. ~~TownDirectory + IndustryDirectory migrados (filtro + sort Asc/Desc).~~ ✅
3. ~~StationDirectory + VehicleList migrados al chrome compartido.~~ ✅

Progreso UI-5 (filtros Graph por compañía):

1. ~~Historial económico por `Company` (ingresos/costes/entregas).~~ ✅
2. ~~Cierre mensual por compañía + espejo en `stats`.~~ ✅
3. ~~GraphWindow: botones de compañía + series filtradas.~~ ✅
4. ~~Finances usa totales de la compañía activa.~~ ✅

Progreso UI-5e (matriz TO_*):

1. ~~Bitsets `transparency_opt` / `invisibility_opt` + helpers.~~ ✅
2. ~~Display Options: V/T/O por categoría (Signs…Text).~~ ✅
3. ~~Gates render: trees, houses, industries, buildings, bridges, structures, signs, text, catenary.~~ ✅
4. ~~Ciclo catenaria toolbar migrado a bitsets.~~ ✅

Progreso UI-5d (animación/detalle):

1. ~~`full_animation` / `full_detail` en ClientPreferences.~~ ✅
2. ~~Toggles Display Options.~~ ✅
3. ~~Gate paleta: agua, refinería, fizzy, TileAnimClock.~~ ✅
4. ~~Faroles de carretera detrás de FullDetail.~~ ✅
5. ~~Árboles en acera (`Roadside::Trees`) detrás de FullDetail.~~ ✅
6. ~~Cercas de vía (`DrawTrackDetails`) detrás de FullDetail.~~ ✅
7. ~~Animación faro/estadio (`lighthouse[4]`).~~ ✅

Progreso UI-4x (polish flota legible):

1. ~~Labels de estación + toggle Display Options.~~ ✅
2. ~~Leyenda GraphWindow (swatches).~~ ✅
3. ~~Condicionales crear/editar en order panel.~~ ✅
4. ~~BuyVehicle: búsqueda + Tram + loco/vagón + highlight sort/filtro.~~ ✅

Progreso UI-6g:

1. ~~Catálogo filtrable StationClass/StationSpec.~~ ✅
2. ~~Picker clase/tipo + disallowed sizes.~~ ✅
3. ~~Persistencia `current_station_*` + `Station.station_spec`.~~ ✅
4. ~~Action0 Stations metadatos (0x04) + picker dinámico.~~ ✅

Progreso UI-6h (boyas / acueducto / ríos):

1. ~~`PlaceBuoy` + pathfinding/órdenes de barco.~~ ✅
2. ~~`PlaceAqueduct` con rampas enfrentadas.~~ ✅
3. ~~Toolbar Agua + preview/drag.~~ ✅
4. ~~Render boya + deck de acueducto.~~ ✅
5. ~~`WaterClass` + `PlaceRiver` + carve en world_gen.~~ ✅
6. ~~Río en pendiente no navegable (hace falta esclusa).~~ ✅

Progreso UI-6f:

1. ~~Catálogo filtrable `list_road_types` + clases Road/Tram.~~ ✅
2. ~~`current_road_type` / `current_tram_type` + m8.~~ ✅
3. ~~Dropdowns filtrables toolbar + HUD.~~ ✅
4. ~~Sin tipos NewGRF reales (documentado; hook listo).~~ ✅

Progreso UI-6e:

1. ~~`VehicleKind::Tram` + `PathNetwork::Tram` (m3).~~ ✅
2. ~~Motor + compra en RoadDepot + toggle salida.~~ ✅
3. ~~Paradas bus / render placeholder bus.~~ ✅
4. ~~Sin depósito tram dedicado ni NewGRF (documentado).~~ ✅

Progreso UI-6d:

1. ~~JoinStations bus/camión 1×1 + joined_tiles.~~ ✅
2. ~~UI Unir (toolbar + panel).~~ ✅

Progreso UI-6c:

1. ~~RoadType + PlaceTramBits (m3/m8).~~ ✅
2. ~~Preservar tram al construir carretera.~~ ✅
3. ~~Toolbar Tranvía X/Y/Cruce.~~ ✅
4. ~~Sin vehículos de tranvía (documentado).~~ ✅

Progreso UI-6b:

1. ~~Sign + PlaceSign/RemoveSign/RenameSign en core.~~ ✅
2. ~~Herramienta Cartel + SignList + etiquetas mapa.~~ ✅
3. ~~Resto UI-6 documentado como fuera de alcance / backend.~~ ✅

Progreso UI-6a:

1. ~~Panel flotante Señales (tipo + densidad).~~ ✅
2. ~~Selector railtype en toolbar Rail + HUD.~~ ✅
3. ~~PlantTree en Landscape.~~ ✅
4. ~~Fuera de alcance documentado.~~ ✅

Progreso UI-5 (corte inicial):

1. ~~EconomyHistory mensual en core + GraphWindow Income/Operating Profit.~~ ✅
2. ~~Menú Economía (Finanzas / Gráficos / Tarifas).~~ ✅
3. ~~Finances enriquecido + conteos de infraestructura.~~ ✅
4. ~~Minimapa capas Ind/Due/Veh + leyenda.~~ ✅
5. ~~CargoPaymentRates.~~ ✅

Progreso UI-5b:

1. ~~CompanyValue en EconomyHistory + GraphWindow / menú.~~ ✅
2. ~~ExtraLargeMap (Ampliar + Esc).~~ ✅

Progreso UI-5c:

1. ~~Display Options (minimapa/PBS/gizmos/diagnóstico/catenaria/pueblos).~~ ✅
2. ~~ExtraViewport MVP (sigue cámara principal).~~ ✅
3. ~~Stubs SignList + LinkGraph (bloqueados por backend).~~ ✅

Pendiente UI-5: ninguno que bloquee cierre. LinkGraph observacional ✅;
routing CargoDist completo → OOS. NewGRF Action0–14 / parámetros → OOS.
NewGRF Action0–14 / parámetros → OOS hasta runtime (UI-7 cerró config-only).
Graph por compañía: ✅.

Progreso UI-3 (corte inicial):

1. ~~SubsidyList + menú Mundo.~~ ✅
2. ~~TownWindow rating/historial.~~ ✅
3. ~~IndustryPanel cadena I/O.~~ ✅
4. ~~StationView mínima (centrar, rating/cargo, waypoint, vehículos).~~ ✅
5. ~~Rename de estación + VehicleList filtrada por estación.~~ ✅

Progreso UI-3 (polish jugable):

1. ~~StationView: owner/ingresos/cobertura/días sin recogida/tiles unidas.~~ ✅
2. ~~IndustryPanel: texto jugador + ritmo producción + Centrar.~~ ✅
3. ~~Directorios estación/industria centran cámara.~~ ✅
4. ~~Town labels honestos (acumulado vs demanda teórica).~~ ✅
5. ~~Subsidy OpenRelated centra destino.~~ ✅
6. ~~`TownHistory` / `IndustryHistory` en core + push mensual.~~ ✅
7. ~~Sparklines en TownWindow e IndustryPanel.~~ ✅

Pendiente UI-3: historial de estación (opcional; no bloquea criterios).

Progreso UI-4:

1. ~~VehicleDetails enriquecido (edad/peso/potencia/fiabilidad).~~ ✅
2. ~~RefitWindow con lista de cargas.~~ ✅
3. ~~Reordenar órdenes ↑/↓.~~ ✅
4. ~~Scroll depósito >8 + clic abre órdenes.~~ ✅
5. ~~Pestañas Info/Carga/Capacidad/Totales.~~ ✅
6. ~~Shared orders UI (crear/vincular/desvincular + lista de pools).~~ ✅
7. ~~Autoreplace UI + mass replace desde depósito.~~ ✅
8. ~~DepotReorder ↑/↓ + toggle parar depósito.~~ ✅
9. ~~Tira horizontal de consist (depósito + VehicleDetails).~~ ✅
10. ~~Drag nativo reordenar filas de depósito.~~ ✅
11. ~~Política single-instance documentada (MVP).~~ ✅

Pendiente UI-4 (bloqueado o polish): beneficio (sin backend), refit parcial/en
orden (sin comando/`VehicleOrder::Refit`), drag nativo de órdenes.
