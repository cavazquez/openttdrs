# Colocación de vía ferroviaria — comportamiento del cliente

Referencia rápida para **Horz / Vert / X / Y**, **autorraíl**, **uniones** y **señales** (junio 2026).

Fuentes OpenTTD: `rail_gui.cpp` (`GenericPlaceSignals`, `GenericPlaceSignals`), `viewport.cpp` (`_tile_fract_coords`).

---

## Herramientas y comandos

| Toolbar | Comando | Comportamiento |
|---------|---------|----------------|
| **Autorraíl** | `PlaceRail` | Infiere pieza con `rail_trackbits_from_neighbors`; **refresca vecinos** (`refresh_rail_neighbors`). |
| **X / Y** | `PlaceRailBits` | Coloca bits fijos (`0x01` / `0x02`) en la **tesela del cursor**. |
| **Horz / Vert** | `PlaceRailBits` | Un carril paralelo por clic (`rail_horz_lane_bit` / `rail_vert_lane_bit` según `fract_x/y`). |
| **Quitar vía** | `RemoveRailBits` | Quita bits; actualiza vecinos y señales compatibles. |

---

## Regla principal (Horz / Vert / X / Y)

**La vía se escribe solo en la tesela bajo el cursor** — la misma que muestra el fantasma.

- **No** se redirige el clic a teselas vecinas con vía existente.
- **Sí** se actualizan uniones en vecinos al colocar:
  - **X / Y** (`PlaceRailBits` con diagonal): cruce perpendicular en la tesela vecinal (`propagate_rail_diag_to_neighbors`).
  - **Horz / Vert** (carril paralelo): empalme T en la vía E–O / N–S vecina (`refresh_rail_neighbors_after_place` + `junction_merge_for_neighbor`).

Las curvas en la **misma tesela** siguen siendo por fusión de bits al clic; ramificar con **autorraíl** infiere la pieza completa.

Implementación: `place_rail_bits` en `crates/openttdrs-core/src/command/transport/rail.rs`, `propagate_rail_diag_to_neighbors` en `shared.rs`.

---

## Autorraíl y vecinos

`PlaceRail` y `RemoveRail` sí ejecutan `refresh_rail_neighbors`:

1. `refresh_rail_neighbors_after_place` — fusiona piezas de unión en vecinos **perpendiculares** cuando corresponde (`junction_merge_for_neighbor`).
2. `refresh_rail_trackbits` — re-infiere X/Y/cruces; **no** toca teselas solo paralelas (Horz/Vert sin diagonal) para no destruir carriles al arrastrar.

---

## Señales (Sprint 5)

| Aspecto | Detalle |
|---------|---------|
| Pick | `world_pos_to_rail_signal_pick` — vecindario 5×5; hover unificado en `HoveredTileCoord` |
| Dibujo | `rail_signal_subtile_offset` — tabla `SignalPositions` (`DrawSingleSignal`, OpenTTD) |
| Colocación en cruce | `write_normal_rail_tile` conserva señales al fusionar Y+X → cruce |

**Bug abierto (jun 2026):** en diagonal X/Y el fantasma puede verse bien pero el clic coloca en tesela vecina — ver [SENALES_FERROVIARIAS.md §11](SENALES_FERROVIARIAS.md#11-bug-abierto-fantasma-vs-colocación-en-vía-diagonal-jun-2026).

Ver [SENALES_FERROVIARIAS.md](SENALES_FERROVIARIAS.md).

---

## Tests de regresión

```bash
cargo test -p openttdrs-core parallel_
cargo test -p openttdrs-core place_rail_bits_preserves_signal
```

Casos cubiertos: extensión Horz en línea; segundo carril en tesela vacía; Vert al este de Y sin modificar la Y; señal conservada al añadir segunda diagonal en la misma tesela.

---

## Backlog

- Junctions en pendiente — ✅ S3 (`sp3_visual_checklist_sloped_junction_sprite_ids`).
- Sim de señales en carriles paralelos (hoy probada sobre todo en X/Y).
