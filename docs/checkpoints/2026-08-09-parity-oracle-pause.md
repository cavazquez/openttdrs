# Checkpoint de pausa: paridad y oráculo

Base: `5b0023b` (`main` publicado).

Esta rama conserva el lote de trabajo local de renderer, carga de SAV,
simulación, assets y herramientas de paridad. Se mantiene fuera de `main` para
poder retomarlo y separarlo en cambios revisables.

La metodología, evidencia disponible y procedimiento reproducible están en
[`METODOLOGIA_RENDER_SAV.md`](../parity/METODOLOGIA_RENDER_SAV.md).

## Validado

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps`
- Mutaciones de `compare_world_raw`, `compare_world_semantic` y
  `compare_world_draw`
- La regresión PBS de consist de tres unidades vuelve a pasar: la actualización
  incremental ahora poda el extremo de reserva que ya no pertenece al path y
  conserva la huella física de la cola.
- El oráculo de OpenTTD rebasado en el fork `cavazquez/OpenTTD`, rama
  `openttdrs/oracle-parity`, compila y exporta una traza `world-draw`.

## Fallos del checkpoint resueltos

Los tres fallos observados al congelar el checkpoint ya no bloquean la suite:

1. `newgrf_actions::tests::truncated_badge_list_emits_diagnostics_and_inspect_warning`
   vuelve a pasar tanto aislado como dentro de la suite; no requirió un cambio
   de comportamiento y se mantiene vigilado por el test existente.
2. `pbs_dual_curve_oracle::rust_matches_openttd_oracle_for_forty_ticks` diverge
   en la cinemática del primer tren desde la muestra 2. La causa era limitar a
   una ruta por tick: un tren activo sin path frenaba mientras esperaba. La
   ruta es una precondición de movimiento, por lo que el límite se retiró; una
   futura amortización debe conservar un paso local válido o ejecutarse fuera
   del tick.
3. `sav_load_stationlist::stationlist_depot_row_preserves_clear_gap` ahora
   valida el dato real: el oráculo `world-raw` de OpenTTD confirma
   `Rail — Clear — Clear — RailDepot`; Rust coincidía y el test anterior
   inventaba una conexión ferroviaria inexistente.

## Estado de integración

El checkpoint queda verde, pero sigue siendo un lote amplio de renderer,
simulación, assets y herramientas. Debe separarse en commits revisables antes
de proponer su integración a `main`.
