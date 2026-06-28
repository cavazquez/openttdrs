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
- **No** se añaden piezas de unión automáticas en vecinos al colocar (`PlaceRailBits` no llama a `refresh_rail_neighbors`).

Las curvas/T visuales al **ramificar** requieren hoy **autorraíl** o colocar en la tesela que ya tiene vía. Unir dos tramos paralelos perpendicularmente deja dos teselas independientes hasta que el jugador conecte explícitamente (paridad parcial respecto a OpenTTD; ver backlog S3).

Implementación: `place_rail_bits` en `crates/openttdrs-core/src/command/transport.rs`.

---

## Autorraíl y vecinos

`PlaceRail` y `RemoveRail` sí ejecutan `refresh_rail_neighbors`:

1. `refresh_rail_neighbors_after_place` — fusiona piezas de unión en vecinos **perpendiculares** cuando corresponde (`junction_merge_for_neighbor`).
2. `refresh_rail_trackbits` — re-infiere X/Y/cruces; **no** toca teselas solo paralelas (Horz/Vert sin diagonal) para no destruir carriles al arrastrar.

---

## Señales (Sprint 5)

| Aspecto | Detalle |
|---------|---------|
| Pick | `world_pos_to_rail_signal_pick` — busca vía en vecindario 5×5; `fract` respecto a esa tesela. |
| Dibujo | `rail_signal_track_offset` — mismo desplazamiento que overlays 1007–1010 (carriles paralelos). |
| Colocación en cruce | `write_normal_rail_tile` conserva señales al fusionar Y+X → cruce (`trackbits_to_signal_present`). |

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

- Uniones automáticas con Horz/Vert sin usar autoraíl (opcional, paridad OpenTTD).
- Junctions en pendiente (Sprint 3 — `ROADMAP_SPRINTS.md` § S3).
- Sim de señales en carriles paralelos (hoy probada sobre todo en X/Y).
