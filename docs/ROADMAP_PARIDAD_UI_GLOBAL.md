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
- [ ] Generalizar las entradas a `MenuSpec` declarativo.
- [ ] Anclaje al botón, z-order y posicionamiento dentro del viewport.
- [ ] Checked/disabled/divider/hotkey.
- [x] Cierre por selección, Esc, clic externo y cambio de pantalla.
- [ ] Navegación teclado arriba/abajo/Enter/Esc.
- [ ] Protección contra click-through al mapa (`BuildMenuUi`/focus).
- [ ] `ListWindow` base con sort, filtro, scroll y selección.
- [ ] Migrar tres menús piloto: Mapa, Mundo e Industrias.

### Criterios de aceptación

- Tres botones toolbar usan la misma primitiva de menú.
- No existen handlers duplicados por cada menú.
- Abrir/cerrar repetidamente no deja entidades ni estados huérfanos.
- Tests cubren foco, click externo y Esc.

---

## UI-2 — Directorios y listas globales

Prioridad: **P0**  
Objetivo de cobertura global: **~36–40 %**.

### UI-2A — Pueblos

- [x] TownDirectory ordenable por nombre/población.
- [ ] Añadir sort por rating.
- [ ] Centrar cámara directamente desde la fila.
- [x] Clic en fila abre `TownWindow`.
- [ ] Acción «Fundar pueblo» si el backend lo permite.

### UI-2B — Industrias

- [x] IndustryDirectory ordenable por tipo/stock.
- [x] Clic abre `IndustryPanel`.
- [ ] Vista inicial de cadenas input/output.
- [ ] Integrar construcción/fundación desde el menú.

### UI-2C — Estaciones

- [x] StationList global ordenable por nombre/rating/carga waiting.
- [x] Clic abre `StationCargoPanel`.
- [ ] Filtro por compañía.
- [ ] Filtro por facility/carga.
- [x] Waiting cargo y rating disponibles.

### UI-2D — Flota

- [ ] VehicleList para tren, road, ship y aircraft.
- [ ] Sort por nombre, edad, velocidad, beneficio cuando exista.
- [ ] Start/stop, enviar a depósito y centrar.
- [ ] Doble clic abre `VehicleWindow`.
- [ ] Selección por compañía; inicialmente compañía activa.

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

- [ ] Nombre/rename.
- [ ] Carga waiting por tipo y rating.
- [ ] Vehículos que visitan la estación.
- [ ] Botón centrar.
- [ ] Acceso a lista filtrada de vehículos.
- [ ] WaypointView equivalente.

### TownView / autoridad

- [ ] Rating por compañía.
- [ ] Acciones de autoridad local.
- [ ] Crecimiento, población e historial básico.

### IndustryView

- [ ] Producción/transportado por cargo.
- [ ] Inputs/outputs.
- [ ] Gráfico básico cuando exista histórico.
- [ ] Mantener preview actual.

### SubsidyList

- [ ] Ofertas y contratos activos.
- [ ] Tiempo restante.
- [ ] Clic origen/destino centra mapa.
- [ ] Doble clic abre entidad relacionada.

### Criterios de aceptación

- Las ventanas dejan de ser meros paneles contextuales.
- Subsidios del core son visibles y navegables.
- Las relaciones estación↔vehículos e industria↔cargos son accesibles.

---

## UI-4 — Flota y subventanas de vehículo

Prioridad: **P1**  
Objetivo de cobertura global: **~50–55 %**.

### VehicleDetails

- [ ] Edad/vida útil.
- [ ] Peso, potencia, coste y fiabilidad.
- [ ] Detalle por unidad del consist.
- [ ] Pestañas cargo/info/capacidad/totales.
- [ ] Beneficio cuando exista backend.

### RefitWindow

- [ ] Lista de cargas.
- [ ] Capacidad y coste.
- [ ] Refit de vehículo completo.
- [ ] Selección parcial de consist cuando exista comando.

### Orders

- [ ] Abrir/cablear `DestinationPicker` o eliminarlo en favor de pick directo.
- [ ] Drag para reordenar.
- [ ] Variantes full-load/unload.
- [ ] Refit en orden.
- [ ] Lista de órdenes compartidas.
- [ ] Más condicionales.

### Depot / BuyVehicle

- [ ] Sprites reales por fila/unidad.
- [ ] Scroll >8 y horizontal para consist.
- [ ] Drag nativo A→B y drag a vender.
- [ ] Autoreplace global.
- [ ] Más filtros/sorts y búsqueda.

### Criterios de aceptación

- Flujo `VehicleList → Vehicle → Orders/Details/Refit/Timetable`.
- Flujo `Depot → BuyVehicle/Autoreplace`.
- Subventanas conservan el `VehicleID` correcto.
- Pueden coexistir ventanas de vehículos distintos o se documenta la política
  single-instance.

---

## UI-5 — Economía, mapas, opciones y gráficos

Prioridad: **P1/P2**  
Objetivo de cobertura global: **~58–62 %**.

### Economía

- [ ] Finances con histórico y categorías.
- [ ] CompanyInfrastructure.
- [ ] Income/Operating Profit mínimos.
- [ ] Delivered cargo y company value.
- [ ] Graph legend y filtros por compañía.

### Mapas

- [ ] SmallMap expandible con capas.
- [ ] Leyenda y filtros.
- [ ] ExtraViewport.
- [ ] SignList.
- [ ] LinkGraphLegend cuando exista CargoDist.

### Opciones/display

- [ ] Ventana Game/Display Options.
- [ ] Nombres de pueblos/estaciones/facilities.
- [ ] Full animation/detail.
- [ ] Transparencia e invisibilidad por categorías.
- [ ] Persistencia en `ClientPreferences`.
- [ ] Mantener TO_CATENARY actual dentro de esta UI.

### Criterios de aceptación

- Toolbar Graphs abre al menos Income y Operating Profit reales.
- Preferencias afectan render y sobreviven reinicio.
- Mapa ampliado permite navegar y entender capas.

---

## UI-6 — Completitud de construcción

Prioridad: **P2**  
Objetivo de cobertura global: **~65–68 %**.

- [ ] Toolbar de tranvías.
- [ ] Selectores filtrables de railtype/roadtype/tramtype.
- [ ] Sub-toolbar visual de señales y densidad.
- [ ] Station classes/layout NewGRF cuando el runtime exista.
- [ ] JoinStation.
- [ ] BuildWaypoint completo road/rail.
- [ ] Buoys, rivers y aqueduct según backend.
- [ ] Airport picker extensible.
- [ ] BuildTrees / BuildObject.
- [ ] Place sign + QueryString.
- [ ] Separar herramientas sandbox/editor de economía normal.

### Criterios de aceptación

- Cada herramienta upstream base tiene botón, selector o decisión explícita de
  fuera de alcance.
- Tipo seleccionado se conserva y se muestra.
- Pickers tienen coste, disponibilidad y preview.

---

## UI-7 — Settings avanzados y modding

Prioridad: **P2/P3**  
Objetivo de cobertura global: **~70–75 %**.

- [ ] NewGRF editable: activar/desactivar/reordenar.
- [ ] Parámetros NewGRF.
- [ ] Presets de settings.
- [ ] Sandbox/cheats si se decide soportarlos.
- [ ] Consola y diagnostics para desarrollo.
- [ ] About/help y mapa de hotkeys.
- [ ] Posiciones/tamaños de ventana persistentes.

NewGRF editable está bloqueado por el runtime Action0–14; la UI puede diseñarse
antes, pero no debe simular que aplica cambios inexistentes.

---

## UI-8 — Modos opcionales

Prioridad: **P3**  
Objetivo: posterior a la paridad single-player.

- [ ] Multi-compañía completa en toolbar/listas/finanzas.
- [ ] Multijugador: lobby, clientes, chat, join/spectate.
- [ ] Scenario editor y toolbar de 19 botones.
- [ ] GameScript: story, goals, league.
- [ ] AI settings/debug.
- [ ] Highscore/endscreen.
- [ ] Herramientas dev: framerate, sprite aligner, inspección NewGRF.

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

Hoy muchos estados usan un único `Option<ID>`. Antes de implementar listas y
subventanas, decidir:

- single-instance por tipo (más simple), o
- `WindowKey(kind, instance)` (más cercano a OpenTTD).

Recomendación: multi-instance para Vehicle/Orders/Details/Station; single para
directorios, settings, gráficos y audio.

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
4. Implementar `ListWindow` genérico.
5. ~~Probar con TownDirectory.~~ ✅
6. ~~Migrar IndustryDirectory y StationList.~~ ✅
7. Construir VehicleList ×4 sobre la misma base.

Este corte aumenta la paridad global más que seguir agregando ventanas
contextuales aisladas y reduce el coste de todas las fases posteriores.
