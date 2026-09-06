# #383 — perfil PATS de CargoDist por clase de carga

Actualizado: **2026-09-05**. Sub-issue de [#328][parent].

[parent]: https://github.com/cavazquez/openttdrs/issues/328

## Divergencia medida

OpenTTD 15.3 guarda diez valores `linkgraph.*` en `PATS`: intervalo y
presupuesto en segundos, cuatro modos de distribución y cuatro parámetros del
pipeline. El core reducía ese estado a un modo global y un intervalo de días.
Así, por ejemplo, una partida con pasajeros simétricos, correo asimétrico y
mercancía manual perdía tres decisiones al cargarse.

El oracle define `Manual=0`, `Asymmetric=1`, `Symmetric=2`; decide por
clase con la precedencia pasajeros → correo → blindado → resto. El scheduler
hace módulo sobre `recalc_interval / EconomyTime::SECONDS_PER_DAY`, donde
OpenTTD 15.3 usa dos segundos económicos por día.

## Corrección acotada

`CargoDistPerCargoSettings` conserva el perfil exacto importado:

- `recalc_interval` y `recalc_time` como `u16` segundos;
- `distribution_pax`, `mail`, `armoured` y `default`;
- `accuracy`, `demand_size`, `demand_distance` y
  `short_path_saturation`.

El parser y writer declaran los widths nativos en `PATS`. El scheduler
convierte el intervalo sólo al operar, mediante división entera por dos, por
lo que un valor válido impar como cinco segundos conserva tanto sus bytes como
su cadencia nativa de dos días. Los jobs resuelven cada cargo por su
`CargoSpec` activo; los cargos NewGRF usan `CargoSpecDef::classes`.
`accuracy`, demanda y saturación alimentan el pipeline ya portado.

Los JSON propios anteriores conservan su selector global mientras no tengan un
perfil PATS. El comando UI global vuelve explícitamente a ese modo, para no
ocultar un cambio de intención detrás de cuatro campos importados.

## Regresiones y oracle

Las pruebas cubren:

- bytes enum y precedencia de clases, incluido un `CargoSpecDef` NewGRF;
- parser de widths, límites y perfil completo PATS;
- conversión del intervalo impar al scheduler y entrega de los knobs al job;
- round-trip `SavGame`/`GameState`, wire y mutación no relacionada de PATS;
- carga y re-guardado por OpenTTD dedicated del fixture rico no-default.

Oracle OpenTTD 15.3, commit
`c2661164bcb6cbf5ab97b56ccbee7506a3b26833`:

- `src/table/settings/linkgraph_settings.ini`;
- `src/settings_type.h` (`GetDistributionType`);
- `src/linkgraph/linkgraph_type.h`;
- `src/linkgraph/linkgraphschedule.cpp`;
- `src/timer/timer_game_common.h`.

Artefactos de la corrida final:

- candidato Rust:
  `6bf412550cdebb289acc84f87362e134e205442ff0b381cfd4b053dae81489c8`;
- re-guardado por OpenTTD:
  `aca7e503e4071ac5181e43d13e84857cf4cddb010f5b5a5d1a64f0749942702f`.

## Pendiente real

`recalc_time` se conserva fielmente, pero el core todavía ejecuta los jobs
de forma síncrona: no simula el presupuesto temporal, threads ni pausa de
OpenTTD. El link graph base, rutas, demanda de estación y entrega siguen
siendo un subconjunto documentado. Esto no cierra #328 ni afirma paridad total
de CargoDist, NewGRF o economía.
