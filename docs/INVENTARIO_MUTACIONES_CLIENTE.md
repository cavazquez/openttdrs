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

### Deuda I8 (debería ser `Command` o settings replicados)

| Grupo | Archivos representativos | Qué muta |
|-------|--------------------------|----------|
| Pathfinding / PBS UI | `ui/pathfinding_settings_window.rs` | `state.pathfinding.*` |
| CargoDist UI | `ui/cargo_dist_settings_window.rs` | `cargo_dist` + `rebuild_station_flows` |
| Selectores vía/estación | `ui/toolbar/*_type_selector.rs`, airport/station pickers | `current_rail_type`, road/tram/station/airport |
| Color / AI / story | ventanas de compañía / AI / GS | espejos y settings de partida |
| Editor / sandbox | `state/editor_session.rs`, escenarios heightmap | clima, seed, clear entidades, cheats |
| Drag helper | `ui/toolbar/build_input/drag.rs` → `finalize_road_drag_line` | post-proceso mapa tras colocar |

### Neutro / revisar

- Helpers que envuelven `apply_command` (p.ej. `apply_order_edit`) → **no** cuentan como violación.
- Tests del cliente con mutación directa → fuera del inventario prod.

## Impacto en #21

Para listen-server / cliente-only, todo lo marcado **Deuda I8** que altere estado persistido debe pasar por el log de comandos (o prohibirse en cliente remoto). Lo **legítimo** permanece local o se orquesta por el host (step/load).

## Follow-up sugeridos (no bloquean cierre de #114)

1. Commands o mensajes de settings para `pathfinding` / `cargo_dist` / tipos de vía activos.
2. Editor/sandbox solo en modo single-player o como comandos de host.
3. Asegurar que `finalize_road_drag_line` no duplique efectos ya cubiertos por el comando de colocación.
