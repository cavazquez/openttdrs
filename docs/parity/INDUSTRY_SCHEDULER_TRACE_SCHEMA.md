# Contrato de traza del scheduler de industrias

Actualizado: 2026-09-07. Sub-issues: #501 (oráculo), #502 (ejecución vanilla)
y #506 (candidato/comparador); padre de runtime: #499 / RMAP-158. #507
conserva la importación de los relojes `DATE` y el estado RNG de carga; #510
rechaza una ejecución dedicated degradada, #511 alinea el contador de industria
y #512 aísla el residual RNG posterior.

`OPENTTDRS_INDUSTRY_TRACE_OUT` habilita, exclusivamente en el binario OpenTTD
instrumentado, una traza JSONL de la rutina diaria
`_economy_industries_daily`. Sin la variable, el hook no abre archivos ni
consulta estado adicional y no consume `Random()`.

El objetivo es comparar la secuencia nativa que consume la candidata: `ECMY`,
`IBLD`, `ITBL`, decisiones `Chance16`, entidades y estado del RNG global. No
es una captura de pantalla ni un sustituto de la comparación raw por tesela
cuando una fundación haya materializado una industria. #502 usa el contrato
para la cadencia y una regresión física determinista desde 1960.

## Ejecución

```bash
./scripts/export_openttd_industry_trace.sh path/to/game.sav /tmp/industry.jsonl 40
./scripts/export_openttdrs_industry_trace.sh path/to/game.sav /tmp/industry-openttdrs.jsonl 40
python3 scripts/compare_industry_scheduler_traces.py \
  /tmp/industry.jsonl /tmp/industry-openttdrs.jsonl 40
```

El exportador arma el hook después del segundo `AfterLoadGame` por defecto,
porque el dedicado crea primero una partida temporal antes de cargar `-g`.
`OPENTTDRS_SNAPSHOT_MIN_CALL=1` sólo se usa para fixtures que no tienen esa
partida temporal.

El dedicated debe poder abrir su socket de red. El exportador rechaza una
traza incluso si su JSONL es sintácticamente válida cuando el log contiene
`Could not bind socket`: esa ruta de arranque puede consumir RNG distinto. En
un sandbox se debe ejecutar el oracle con sockets permitidos; no se normaliza
ni se compara esa salida degradada.

El segundo exportador carga el mismo `.sav` mediante el core Rust, escribe una
fila `initial` y captura cada fila `day` dentro del scheduler, inmediatamente
después del timer industrial. La instrumentación es efímera, no se serializa y
no consume `Random()`. El comparador valida primero ambos JSONL y luego exige
igualdad exacta de metadata contractual, reloj, RNG, `ECMY`, `ITBL`, industrias
y acciones; informa el primer campo que diverge.

## JSONL v1

La primera fila es `metadata`:

```json
{
  "kind": "metadata",
  "schema_version": 1,
  "trace": "industry_scheduler",
  "initial_sample_point": "after_load_game",
  "day_sample_point": "after_industry_daily_timer",
  "max_days": 40,
  "industry_type_count": 240
}
```

Le sigue una fila `initial` y exactamente `max_days` filas `day`. Cada muestra
incluye:

- `calendar` y `economy`: fecha, año y mes nativos;
- `random_state`: ambos `u32` de `_random` después de la muestra;
- `industry_daily_change_counter` e `industry_daily_increment` de `ECMY`;
- `wanted_inds` y las 240 filas ordenadas de `ITBL` (`probability`, mínimos,
  objetivo y backoff);
- `industries`, ordenada estrictamente por ID, con tipo, posición y campos que
  cambian por producción/fundación;
- `change_loop` y `actions` de la jornada. Cada acción conserva el ordinal,
  el porcentaje 3…9 evaluado por `Chance16`, la rama `foundation` o
  `production`, la industria elegida si aplica, y el tipo/resultado cuando una
  fundación alcanzó `PlaceIndustry`.

La fila `initial` tiene `change_loop = 0` y acciones vacías. En una fila `day`,
la cantidad de acciones coincide exactamente con `change_loop`; el contador
guardado ya contiene sólo la fracción baja de 16.16.

## Estado de comparación

El candidato traduce solamente la base interna relativa de `date` a la escala
absoluta del contrato; no relaja `year`, `month`, `tick` ni RNG. La corrección
de #507 conserva el `DATE` moderno completo —ambos relojes, sus fracciones y
el tick— y, en la corrida controlada sobre la partida de trabajo, iguala en
`initial` calendario, economía, tick y las dos palabras RNG que contiene el
SAV. #510 rechaza la salida dedicated que no puede abrir su socket, porque esa
ruta recorre un arranque distinto y no es un oracle de importación.

Con el estado inicial ya alineado, #508 conserva los 16 bits de `INDY.counter`
y #509 mantiene `INDY.location.tile` como origen aun con un footprint
incompleto. #511 reproduce el decremento con wrapping durante `OnTick_Industry`
y coloca `_economy_industries_daily` antes de `TimerGameTick`, como el loop
nativo. La corrida normal de tres días ya iguala tick, ambos relojes, `ECMY`,
`ITBL`, acciones y los 42 `INDY.counter`; por ejemplo el primero pasa de
`12730` a `12658` en ambos motores. El primer residual válido es ahora
`day[1].random_state` (#512): OpenTTD
`[703151878, 1259678577]` frente a openttdrs `[2833653817, 3599663225]`.
Este contrato no declara aún paridad runtime completa del scheduler.

## Límites explícitos

Esta traza observa la selección y el resultado del intento de fundación, pero
no sustituye la traza individual de los hasta 2.000 `CreateNewIndustry` de
`PlaceIndustry`. Las muestras post-timer tampoco permiten reconstruir por sí
solas los consumos RNG de subsistemas intermedios. El scheduler vanilla ya se
porta en #502; extender el oracle a los intentos y a callbacks NewGRF sigue
siendo necesario antes de reclamar paridad de #499.
