# Techo vial en curvas/reversa según modelo de aceleración

Actualizado: 2026-09-08.

Sub-issue [#532](https://github.com/cavazquez/openttdrs/issues/532) de
[#330](https://github.com/cavazquez/openttdrs/issues/330).

## Divergencia y corrección

`RoadVehicle::GetCurrentMaxSpeed` de OpenTTD 15.3
(`src/roadveh_cmd.cpp`) aplica el techo del 75 % en curvas y del 50 % durante
reversa sólo con `AM_REALISTIC`. La consulta Rust aplicaba ambos límites
también en `AM_ORIGINAL`, aunque el tick ya recibía el modelo desde la partida.
Para un MPS Regal con techo interno 112, una curva original quedaba limitada
a 84 y una reversa a 56; el original conserva el techo 112 en ambos casos.

La consulta productiva ahora recibe `road_vehicle_acceleration_model`. Los
wrappers sin partida usan `AM_ORIGINAL`, igual que los wrappers históricos
del controlador. La detección de reversa exige además un estado normal
`state <= RVSB_TRACKDIR_MASK`: los bits bajos de una parada, depósito o túnel
no constituyen por sí solos un trackdir de reversa. Los límites explícitos
de órdenes y la consulta de velocidad NewGRF CB36 siguen aplicándose.

## Oráculo reproducible

`scripts/road_curve_speed_oracle.py` extrae y ejecuta literalmente dos cuerpos
C++ del checkout OpenTTD 15.3: `RoadVehicle::GetCurrentMaxSpeed` e
`IsReversingRoadTrackdir`. El harness sólo sustituye almacenamiento de
vehículo/settings, búsqueda de puente y siguiente unidad; no reescribe la
regla de velocidad y no modifica `reference/`.

Referencia comprobada: commit
`c2661164bcb6cbf5ab97b56ccbee7506a3b26833`; ambos archivos nativos están sin
modificaciones locales. El resultado fue **128/128 casos reproducidos**.

```bash
python3 scripts/road_curve_speed_oracle.py reference/openttd-upstream --check
cargo test -p openttdrs-core --lib current_max_speed
cargo test -p openttdrs-core --lib road_tick_uses_the_active_model
```

La fixture `crates/openttdrs-core/tests/fixtures/parity/road_curve_speed.tsv`
conserva **128 casos nativos**: ambos modelos, ocho estados (recta, las cuatro
reversas, dos estados de parada y wormhole), dos direcciones, dos techos
(101 y 112, incluido redondeo entero) y dos límites de orden. El TSV expresa
el límite de orden en unidades internas para comparar el contrato de velocidad
sin atribuir al test paridad de la codificación SAV de órdenes.

`current_max_speed_matches_native_model_and_state_matrix` compara las APIs
inmutable y productiva con esa fixture. La regresión adicional
`road_tick_uses_the_active_model_for_curve_and_reverse_speed_limits` ejecuta
el tick con progreso insuficiente para mover otro frame: acredita que el
modelo alcanza realmente `UpdateSpeed`, aislando la penalización instantánea
del giro. La consulta CB36 conserva su regresión preexistente.

Validación del 2026-09-08: los seis tests focalizados de `current_max_speed`
y el test productivo del modelo activo pasaron, además del oráculo 128/128.
También pasaron la suite completa de core, `cargo fmt --all -- --check`,
Clippy core/client con `--all-targets -- -D warnings` y el chequeo de
documentación de paridad.

## Alcance pendiente

Esto acredita el techo de velocidad de una unidad vial. No completa #330:
siguen pendientes las trazas tick a tick amplias de tráfico/adelantamiento,
composición articulada, puentes, pendientes, YAPF y ferrocarril. La aplicación
de la penalización instantánea de curva y de `RoadZPosAffectSpeed` al modelo
realista requiere otra corrección; no se modifica en esta etapa ni se declara
paridad del controlador vial completo.
