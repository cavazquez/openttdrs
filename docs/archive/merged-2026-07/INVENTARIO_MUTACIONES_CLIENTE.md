# Inventario mutaciones cliente fuera de `Command` (#114)

Fecha: 2026-07-16. Crate: `openttdrs-client`. ADR red: [adr/0001-multiplayer-v1.md](adr/0001-multiplayer-v1.md).

## Resumen

- ~40 archivos con `ResMut<SimWorld>`; ~138 usos de `apply_command` (UI mayormente canalizada).
- **~25–30 archivos productivos** con mutación directa (~45–65 sitios).
- DoD de este issue: **inventario clasificado**. Migrar a `Command` es deuda I8 (hijos / follow-up).

## Clasificación

### Legítimo (no debe ser `Command`)

| Grupo | Archivos representativos | Motivo |
|-------|--------------------------|--------|
| Tick de sim | `simulation.rs` → `sim.state.step()` | Reloj de partida; en red lo dispara el protocolo |
| Persistencia | `persistence.rs` → `sim.state = loaded` | Reemplazo de mundo al cargar |
| Drenaje UI runtime | `ui/statusbar/sync.rs` (news/display) | Colas efímeras de `runtime` |
| Bootstrap pre-partida | `state/bootstrap/*`, población procedural | Antes de que exista log de red |

### Migrado a `Command` (I8 settings)

| Grupo | Archivos | Comando |
|-------|----------|---------|
| Pathfinding / PBS UI | `ui/pathfinding_settings_window.rs` | `SetPathfindingSettings` |
| CargoDist UI | `ui/cargo_dist_settings_window.rs` | `SetCargoDistDistribution` |
| Color compañía | `ui/toolbar/settings.rs` | `SetCompanyColour` |
| Selectores vía/road/tram | `rail_type_selector`, `road_type_selector` | `SetCurrentRailType` / `Road` / `Tram` |
| Estación / aeropuerto | `rail_station_window`, `airport_picker_window` | `SetCurrentStation*` / `Airport*` |
| AI TransCargo | `ui/ai_settings_window.rs` | `SetAiSettings` |
| Drag carretera | `ui/toolbar/build_input/drag.rs` | `FinalizeRoadDragLine` |
| Editor GenLand | `ui/genland_window.rs` / `editor_session` | `RegenerateLandscape` |
| Editor sandbox cheats | `apply_editor_sandbox` | `CheatSetEnabled` / toggles |

### Estado local (no debe ser `Command`)

| Grupo | Archivos | Motivo |
|-------|----------|--------|
| Story page nav | `ui/story_window.rs` → `StoryWindowState.page_index` | Navegación por cliente; no afecta sim |

### Deuda I8 restante

Ninguna mutación productiva pendiente del inventario #114 (tick/load/bootstrap siguen legítimos).

### Neutro / revisar

- Helpers que envuelven `apply_command` (p.ej. `apply_order_edit`) → **no** cuentan como violación.
- Tests del cliente con mutación directa → fuera del inventario prod.

## Impacto en #21

Para listen-server / cliente-only, todo lo marcado **Deuda I8** que altere estado persistido debe pasar por el log de comandos (o prohibirse en cliente remoto). Lo **legítimo** permanece local o se orquesta por el host (step/load).

## Follow-up sugeridos (no bloquean cierre de #114)

1. ~~Commands settings / selectores / color / AI.~~ Hecho.
2. ~~Editor GenLand + sandbox cheats vía Command.~~ Hecho.
3. ~~`FinalizeRoadDragLine` en el log (no solo local).~~ Hecho.
4. ~~`story_index` → `StoryWindowState.page_index` local.~~ Hecho.
