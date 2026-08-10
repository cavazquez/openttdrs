# Checkpoint de pausa: paridad y oráculo

Base: `5b0023b` (`main` publicado).

Esta rama conserva el lote de trabajo local de renderer, carga de SAV,
simulación, assets y herramientas de paridad. Se mantiene fuera de `main` para
poder retomarlo y separarlo en cambios revisables.

La metodología, evidencia disponible y procedimiento reproducible están en
[`METODOLOGIA_RENDER_SAV.md`](../parity/METODOLOGIA_RENDER_SAV.md).

## Validado

- `cargo fmt --all -- --check`
- 839 tests del cliente (2 ignorados)
- 1.468 tests principales del core, salvo los fallos listados abajo
- Mutaciones de `compare_world_raw`, `compare_world_semantic` y
  `compare_world_draw`
- La regresión PBS de consist de tres unidades vuelve a pasar: la actualización
  incremental ahora poda el extremo de reserva que ya no pertenece al path y
  conserva la huella física de la cola.
- El oráculo de OpenTTD rebasado en el fork `cavazquez/OpenTTD`, rama
  `openttdrs/oracle-parity`, compila y exporta una traza `world-draw`.

## Pendiente conocido

`cargo test --workspace --no-fail-fast` tiene tres fallos introducidos por este
lote respecto de `5b0023b`:

1. `newgrf_actions::tests::truncated_badge_list_emits_diagnostics_and_inspect_warning`
   no recibe el diagnóstico esperado.
2. `pbs_dual_curve_oracle::rust_matches_openttd_oracle_for_forty_ticks` diverge
   en la cinemática del primer tren desde la muestra 2.
3. `sav_load_stationlist::stationlist_depot_row_connects_to_rail` decodifica
   `Grass` donde el fixture espera `Rail`.

No integrar esta rama a `main` hasta resolver y volver a ejecutar la suite.
