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

## Contrato JSONL v2

`schema_version: 2` añade `units[]` por cabeza de tren (recorrido `Next()`),
sin romper fixtures v1: el comparador solo exige unidades cuando el oráculo
las declara.

Cada unidad exporta `index` (0 = cabeza), tile `x`/`y`, `rail_pixel` (misma
convención que `rail_pixel_from_openttd_pos` en Rust) y `direction`. La cabeza
conserva los campos v1 de velocidad/progreso.

```json
{"kind":"initial","tick":4685,"trains":[{"vehicle":2,"x":46,"y":37,"progress":51,"speed":73,"subspeed":52,"direction":1,"units":[{"index":0,"x":46,"y":37,"rail_pixel":5,"direction":1},{"index":1,"x":47,"y":37,"rail_pixel":13,"direction":1},{"index":2,"x":47,"y":37,"rail_pixel":5,"direction":1}]}],"rail_reservations":[{"x":43,"y":37,"track_bits":1}]}
```

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

## Viewer de traza (tiles + reservas + señal)

Para ver por dónde pasa el tren y qué reservas PBS hay en cada muestra:

```bash
# Oráculo corto (paridad, 40 ticks)
python3 scripts/view_pbs_trace.py \
  crates/openttdrs-core/tests/fixtures/parity/train_pbs_15_3_openttd.jsonl \
  /tmp/pbs_trace.html

# Recorrido completo del mismo save (400 ticks → estación destino)
python3 scripts/view_pbs_trace.py \
  crates/openttdrs-core/tests/fixtures/parity/train_pbs_15_3_400_openttd.jsonl \
  /tmp/pbs_trace_400.html
# HTML versionado: docs/parity/train_pbs_15_3_400_trace.html
```

Abre el HTML en el navegador: scrubber de muestras, mapa del corredor,
lista de tiles visitados y anotación de señal (`--signal X,Y,label`; el
fixture `train_pbs_15_3` usa por defecto la path signal en `(46,37)`).

La traza de **400 ticks** es solo para inspección visual (path
`47,37 → … → 42,37`). El golden de paridad sigue siendo la de **40 ticks**.

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
`(25–26, 8)`, depósito `(24, 9)`. **Paridad cerrada** (`initial` + 40 ticks:
cinemática y reservas PBS) en `tests/pbs_dual_curve_oracle.rs`.

### Fixture multi-vagón (consist + PBS, schema v2)

- Save: `crates/openttdrs-core/tests/fixtures/train_consist_2wagon_pbs_15_3.sav`
- Oráculo: `tests/fixtures/parity/train_consist_2wagon_pbs_15_3_openttd.jsonl`
- Tests: `tests/consist_pbs_openttd_oracle.rs`

Contenido: locomotora Ginzu A4 + 2 Goods Van sobre la recta PBS de
`train_pbs_15_3`, sin NewGRF. La cola ocupa otra tesela/píxel que la cabeza.
Regeneración:

```bash
./scripts/gen_consist_2wagon_fixture.sh 40
```

El generador engancha vagones en AfterLoad (`OPENTTDRS_FIXTURE_ATTACH_WAGONS`)
sobre `train_pbs_15_3.sav`, guarda el `.sav` materializado y exporta el JSONL
desde ese save (sin re-enganchar).

Contrato rail (solo trenes):

- `DoUpdateSpeed` devuelve distancia (`GetAdvanceSpeed` + remanente).
- Umbral `GetAdvanceDistance` (192 axial / 256 corner); sobrante en `progress`.
- Un tick de juego = 2× `TrainLocoHandler`; 16 pasos de píxel por tesela.
- Aceleración realista al importar `.sav` (`train_acceleration_model = Realistic`).
- Render: `rail_pixel / 16` → progreso visual 0..=255 (no usar el remanente físico).
- Import: no teletransportar vehículos ya sobre su red aunque YAPF falle (path signal).
