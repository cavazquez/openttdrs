# Oráculo externo PBS/señales

El golden `train_pbs_golden.json` y la traza
`train_pbs_tick_golden.json` son regresiones **internas** de openttdrs. Este
documento define el segundo productor independiente: OpenTTD 15.3, fijado en
[`parity/openttd-reference.json`](parity/openttd-reference.json).

## Contrato JSONL v1

El parche `patches/openttd-15.3-snapshot-export/` añade un exportador que se
activa con:

```bash
OPENTTDRS_PBS_TRACE_OUT=/tmp/openttd-pbs.jsonl \
OPENTTDRS_PBS_TRACE_TICKS=40 \
./reference/openttd-upstream/build/openttd -D -g partida.sav
```

La primera fila es metadata (`producer: "openttd"`). Sigue una muestra
`initial` tras cargar el save y antes de avanzar; las filas `tick` se capturan
después de `StateGameLoop`:

```json
{"kind":"initial","tick":122,"trains":[{"vehicle":17,"x":2,"y":2,"progress":51,"speed":73,"subspeed":52,"direction":1}],"rail_reservations":[{"x":3,"y":2,"track_bits":1}]}
```

Los IDs de vehículo son locales a cada motor y el comparador no los usa. El
contrato compara la colección ordenada de `(x, y, progress, speed, subspeed,
direction)` de trenes y las reservas `(x, y, track_bits)` por muestra.
`track_bits` corresponde a
`GetRailReservationTrackBits` en OpenTTD y a la reserva `m2_hi` decodificada
en openttdrs.

## Generación reproducible

1. Obtener e integrar OpenTTD 15.3:

   ```bash
   ./scripts/fetch-openttd-reference.sh
   ./patches/openttd-15.3-snapshot-export/integrate.sh
   cmake -B reference/openttd-upstream/build -S reference/openttd-upstream -DOPTION_DEDICATED=ON
   cmake --build reference/openttd-upstream/build -j
   ```

2. El fixture versionado
   `crates/openttdrs-core/tests/fixtures/train_pbs_15_3.sav` fue creado desde
   **OpenTTD 15.3**. Tiene un tren, una path signal eléctrica unidireccional,
   una recta y una estación de destino. No incluye NewGRF, cruces ni más
   vehículos. Para reemplazarlo, validar primero:

   ```bash
   ./scripts/validate_sav_openttd.sh crates/openttdrs-core/tests/fixtures/train_pbs_15_3.sav
   ```

3. Exportar el oráculo y el candidato desde **el mismo save**:

   ```bash
   ./scripts/export_openttd_pbs_trace.sh \
     crates/openttdrs-core/tests/fixtures/train_pbs_15_3.sav \
     crates/openttdrs-core/tests/fixtures/parity/train_pbs_15_3_openttd.jsonl 40

   cargo run -p openttdrs-core --bin sav_pbs_runner -- \
     crates/openttdrs-core/tests/fixtures/train_pbs_15_3.sav \
     --ticks 40 --out /tmp/train_pbs_openttdrs.jsonl
   python3 scripts/compare_pbs_traces.py \
     crates/openttdrs-core/tests/fixtures/parity/train_pbs_15_3_openttd.jsonl \
     /tmp/train_pbs_openttdrs.jsonl
   ```

## Estado

El exportador, normalizador, validador y comparador están implementados y el
exportador fue compilado contra el commit OpenTTD 15.3 fijado. El fixture y su
oráculo de 40 ticks están versionados.

**Paridad cerrada** para este escenario (un tren, path signal, `AM_REALISTIC`):
`initial` y los 40 ticks coinciden en tesela, `progress` físico, `cur_speed`,
`subspeed`, dirección y reservas PBS (`tests/pbs_openttd_oracle.rs`).

### Fixture dual (curva + PBS + plataformas)

- Save: `crates/openttdrs-core/tests/fixtures/train_dual_pbs_curve_15_3.sav`
- Oráculo: `tests/fixtures/parity/train_dual_pbs_curve_15_3_openttd.jsonl`
- Tests: `tests/pbs_dual_curve_oracle.rs`

Contenido: 2 trenes Ginzu A4, 2 estaciones duales, path / path-oneway, curva en
`(25–26, 8)`, depósito `(24, 9)`. **Paridad `initial` cerrada** (cinemática +
reserva en `(26,7)`). El tick 1 aún diverge (tren en plataforma no avanza
`progress` como OpenTTD); el test `first_tick_still_diverges_from_openttd`
documenta el gap.

Contrato rail (solo trenes):

- `DoUpdateSpeed` devuelve distancia (`GetAdvanceSpeed` + remanente).
- Umbral `GetAdvanceDistance` (192 axial / 256 corner); sobrante en `progress`.
- Un tick de juego = 2× `TrainLocoHandler`; 16 pasos de píxel por tesela.
- Aceleración realista al importar `.sav` (`train_acceleration_model = Realistic`).
- Render: `rail_pixel / 16` → progreso visual 0..=255 (no usar el remanente físico).
- Import: no teletransportar vehículos ya sobre su red aunque YAPF falle (path signal).
