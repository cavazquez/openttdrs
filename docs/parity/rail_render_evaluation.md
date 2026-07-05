# Evaluación render/interpolación ferroviaria (Fase Rail 3E)

Fecha: 2026-07-04 · Complementa `rail_status.md` y `rail_debugging_plan.md`.

## Objetivo

Comparar posición lógica (traza `train_line` / `parity_runner`) con lo que dibuja
el cliente (`OPENTTDRS_RENDER_TRACE`), medir saltos de interpolación y documentar
gaps frente a `_vehicle_subcoord` y `_tunnel_visibility_frame`.

## Hallazgos

| Aspecto | Traza lógica (`parity`) | Render (`vehicle_subtile` + cliente) | Estado |
|---|---|---|---|
| Sub-tesela en JSONL | `rail.parts[0].subtile_x/y` | Misma función `vehicle_subtile` a `tick_alpha=0` | **Alineado** — chequeo `train_render_subtile_consistency` |
| Interpolación entre ticks | Solo cambia a 5 Hz | `extrapolate_vehicle_pose` + `tick_alpha` | **Sin retrocesos** en recta (`train_line_extrapolation_subtile_is_monotonic`) |
| Sprite del tren | `dir` lógico | Capa según pose extrapolada | **Alineado** — `sprite_selection_uses_extrapolated_pose_for_train` |
| CSV de render | — | Columnas `logical_subtile_*` / `extrap_subtile_*` añadidas | **Listo** para diff manual vs JSONL |
| `_vehicle_subcoord` por pieza | Golden 3A (`vehicle_subcoord_matches_rust_copy`) | Render usa `train_straight_subtile` (eje central) | **Divergencia cosmética** en piezas diagonales puras (`train_diagonal_subcoord_approximation`) |
| Ocultamiento en túnel | Constante `{12,8,8,12}` portada | Sin ocultar sprite en túnel | **Pendiente** — `tunnel_hides_train_at_progress` solo evalúa umbral |

## Cómo reproducir

```bash
# Traza lógica
cargo run -p openttdrs-core --bin parity_runner -- \
    --scenario train_line --ticks 300 --out /tmp/train_line.jsonl

# Traza de render (cliente con escenario cargado o mapa propio)
OPENTTDRS_RENDER_TRACE=/tmp/render_trace.csv cargo run -p openttdrs-client
```

Comparar `rail.parts[0].subtile_*` del JSONL (por tick) con
`logical_subtile_*` del CSV en el mismo tick (columnas `tick` + `vehicle`).

Tolerancia recomendada: `0.51` (medio píxel), igual que `parity_diff --subtile-epsilon`.

## Decisiones (Rail 3E)

1. **No portar** `_vehicle_subcoord` completo al render en esta fase: en vías `X`/`Y`
   el eje central coincide con la entrada OpenTTD; en `UPPER`/`LOWER`/`LEFT`/`RIGHT`
   el sprite puede desplazarse ~1 px respecto al original.
2. **No implementar** ocultamiento por `_tunnel_visibility_frame` hasta tener render
   de wormhole/túnel con capas dedicadas (sigue en `rail_unknown_features.md` ítem 9).
3. **Mantener** extrapolación genérica de carretera para trenes: sin stutter medible
   en `train_line` con física Rail 3B.

## Tests de regresión

- `train_render_subtile_consistency` en `parity/report.rs`
- `train_line_divergences_are_absent_after_rail_3b` (incluye consistencia render)
- `train_line_extrapolation_subtile_is_monotonic`
- `sprite_selection_uses_extrapolated_pose_for_train`
- `tunnel_hides_train_matches_visibility_frame`
