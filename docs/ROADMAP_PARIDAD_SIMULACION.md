# Roadmap — Paridad de simulación vs OpenTTD 15.3

Plan de cierre de divergencias **de comportamiento** entre `openttdrs-core` y OpenTTD.
Origen: auditoría de julio 2026 que leyó el C++ original y el port en paralelo, subsistema
por subsistema, en vez de fiarse de los documentos del repo.

**Referencia usada:** `reference/openttd-upstream` en **15.3** (`14ec60f`), contrastada con
`OpenTTD/` en master (`2effec96e8`). Todas las referencias `archivo:línea` de la columna
*Original* apuntan a `reference/openttd-upstream/src/`; las del *Port* a
`crates/openttdrs-core/src/`.

**Alcance:** bucles y reglas de simulación. Fuera: UI ([ROADMAP_PARIDAD_UI_GLOBAL.md](ROADMAP_PARIDAD_UI_GLOBAL.md)),
NewGRF más allá de los callbacks citados, red, scripts de IA/GS y formato de guardado salvo
donde afecta a la simulación ([ROADMAP_SAV_EXPORT.md](ROADMAP_SAV_EXPORT.md)).

**Relacionado:** [PARIDAD_OPENTTD.md](PARIDAD_OPENTTD.md) (vista corta) ·
[ROADMAP_PARIDAD_ESTRUCTURAL.md](ROADMAP_PARIDAD_ESTRUCTURAL.md) (fases 1–7 ya cerradas) ·
[parity/status.md](parity/status.md) (niveles de madurez) ·
[ROADMAP_INDUSTRIAS_PARIDAD.md](ROADMAP_INDUSTRIAS_PARIDAD.md) (render de industrias).

---

## 1. Diagnóstico de fondo

El port tiene buena paridad en **datos y fórmulas aisladas**, y poca en **los bucles que las
ejecutan**. El mismo patrón aparece en los seis subsistemas auditados: `DoUpdateSpeed`,
`GetCurveSpeedLimit`, el algoritmo `Randomizer`, las tarifas de carga templada o la codificación
de señales están portados con exactitud y con tests golden; pero el orden del tick, las
cadencias, el consumo de azar y las máquinas de estado que los invocan son reconstrucciones
propias. Por eso el juego se comporta *parecido* sin ser reproducible contra el original.

Consecuencia práctica: **ningún subsistema puede alcanzar el nivel 5 de
[parity/status.md](parity/status.md)** (traza equivalente tick a tick) mientras no se cierren las
entradas de la prioridad **P2**.

---

## 2. Cómo leer este documento

| Prioridad | Criterio | Contenido |
|-----------|----------|-----------|
| **P0** | Corrección puntual, coste S, cambia comportamiento observable ya | Constantes y condiciones mal portadas |
| **P1** | Reglas que hacen que el juego *se sienta* OpenTTD | Rating, industrias, ciudades, dinero, averías |
| **P2** | Rediseño de bucle; habilita la reproducibilidad | Relojes, orden del tick, RNG, controladores, órdenes |
| **P3** | Mundo y contenido que hoy no existe | Generación procedural, inundación, casas, extras |

Formato de ficha en P0/P1:

- **Problema** — qué hace hoy el port y por qué importa.
- **Original** — la regla real, con `archivo:línea`.
- **Solución** — el cambio concreto propuesto.
- **Coste** — S horas · M días · L una semana o más · XL rediseño de subsistema.

En P2 y P3 el mismo contenido va en tabla por volumen. La auditoría catalogó 82 divergencias; aquí
aparecen como 71 entradas porque varias comparten solución y se agrupan (las tres fórmulas
financieras de P1.18, los flags de orden de P1.20, las dos generaciones procedurales de P3.1).
Las entradas marcadas **✔ verificado**
se comprobaron abriendo los dos ficheros durante la auditoría; el resto proviene de la
exploración automatizada con referencia de línea, así que conviene confirmar la línea exacta al
abrir el issue.

---

## 3. Camino recomendado

| # | Entrada | Prioridad | Coste | Estado | Por qué en ese orden |
|---|---------|-----------|-------|--------|----------------------|
| 1 | [P0.1](#p01--la-quiebra-ignora-el-préstamo---hecho) Quiebra con préstamo | P0 | S | hecho | Una línea; hoy una compañía sobreendeudada es inmortal |
| 2 | [P0.2](#p02--periodo-de-tránsito-de-la-carga---hecho) Periodo de tránsito 185 | P0 | S | hecho | Recalibra todos los ingresos, que hoy caen 2,5× más rápido |
| 3 | [P0.4](#p04--rating-inicial-de-autoridad-local---hecho) Rating de autoridad local | P0 | S | hecho | Cambia la curva de arranque y es requisito de P1.9 |
| 4 | [P1.1](#p11--updatestationrating-completo--hecho) `UpdateStationRating` + [P0.3](#p03--rating-inicial-de-estación--hecho-con-p11) | P1 | XL | hecho | Es lo que hace que servir bien una estación importe |
| 5 | [P1.2](#p12--reparto-de-carga-entre-estaciones-competidoras--hecho) Reparto entre estaciones | P1 | L | hecho | Cierra la mitad que le faltaba a P1.1: el rating ya reparte la producción |
| 6 | [P1.4](#p14--prod_level-y-cierre-de-industrias--hecho) Industrias dinámicas + [P1.3](#p13--producción-industrial-por-spec--hecho) rates | P1 | XL | hecho | Sin esto el mundo económico es estático y las rutas no caducan |
| 7 | [P3.1](#tabla-p3) `GenerateTowns` / `GenerateIndustries` | P3 | XL | | Un mapa nuevo nace vacío: bloquea comparar una partida completa |
| 8 | [P2.1](#tabla-p2) Relojes calendario/economía | P2 | XL | hecho | Habilita los barridos escalonados de los que todo depende |

**P1 completo** (22/22). P3.1 se adelanta al resto de P3 porque sin mundo generado no hay
escenario de comparación. Los P0 puntuales también están cerrados (P0.7 reclasificado a P2).

---

## 4. P0 — Correcciones puntuales

Siete entradas. **Seis cerradas**: cinco como arreglos puntuales (P0.1, P0.2, P0.4, P0.5, P0.6) y
P0.3 más tarde, dentro de P1.1, porque exigía el modelo de rating entero. Queda P0.7, que choca
con la clasificación de segmentos de señal y está reclasificada a P2 (detalle en su ficha).

### P0.1 — La quiebra ignora el préstamo ✔ · hecho

- **Problema** — `check_bankruptcy` compara solo la caja contra el techo de préstamo, así que
  una compañía endeudada nunca quiebra. Con 50.000 en caja, 400.000 de préstamo y techo de
  300.000, OpenTTD la liquida y openttdrs la mantiene viva indefinidamente.
- **Original** — sobrevive si `money - current_loan >= -GetMaxLoan()` (`economy.cpp:556`).
  El chequeo es mensual y la línea temporal de bancarrota es aviso al mes 4, venta al 7 y
  quiebra al 10 (`economy.cpp:566-631`).
- **Port** — `money < -max_loan` (`economy/payments.rs:157-158`); racha de 3 meses en
  `sim_step/economy.rs:99-116`.
- **Hecho** — `check_bankruptcy(money, loan, max_loan)` resta el préstamo; los tres puntos de
  llamada le pasan el saldo pendiente. Tests `bankruptcy_counts_outstanding_loan`.
- **Pendiente** — la línea temporal de OpenTTD (aviso al mes 4, venta al 7, quiebra al 10) sigue
  siendo una racha de 3 meses; va con [P1.9](#p19--autoridad-local-por-compañía) y P2.1.

### P0.2 — Periodo de tránsito de la carga ✔ · hecho

- **Problema** — el port cuenta un periodo por día (74 ticks) cuando el original cuenta uno cada
  185. Para el mismo viaje acumula 2,5× más periodos, así que el factor de tiempo del pago cae
  mucho antes y todas las rutas largas rinden de menos.
- **Original** — `CARGO_AGING_TICKS = 185` (`timer/timer_game_tick.h:81`), unos 2,5 días; la API
  de scripts convierte con `days_in_transit * 2 / 5` (`script_cargo.cpp:78`).
- **Port** — `TICKS_PER_TRANSIT_DAY = 74` (`economy/time.rs:4`) usado para incrementar
  `periods_in_transit` (`cargo_packet/types.rs:197`, `313`).
- **Hecho** — la constante ambigua se partió en tres: `TICKS_PER_DAY = 74` (calendario),
  `CARGO_AGING_TICKS = 185` (envejecimiento a bordo) y `STATION_RATING_TICKS = 185` (barrido de
  rating de estación, que también corría por día). `ticks_to_transit_days` pasó a
  `ticks_to_transit_periods` y `age_one_day` a `age_one_period`, porque el nombre viejo era el
  origen de la confusión. Test `onboard_cargo_ages_every_185_ticks`.
- **Nota** — ningún test existente cubría la cadencia, así que la recalibración temida no ocurrió:
  la suite pasó sin tocar ningún valor esperado.

### P0.3 — Rating inicial de estación · hecho con P1.1

- **Problema** — una estación recién construida rinde como perfecta desde el primer día porque el
  rating derivado arranca en 255.
- **Original** — `INITIAL_STATION_RATING = 175` (`station_base.h:23`) y, sin carga entregada, sube
  de uno en uno hasta ese techo (`station_cmd.cpp:3988`).
- **Port** — `recompute_station_rating` dejaba 255 mientras no hubiera carga esperando.
- **Por qué no era S** — en el original el 175 es el valor inicial de un rating **persistente por
  cargo** (`GoodsEntry::rating`), no una constante suelta: hacía falta el modelo entero.
- **Hecho** — dentro de [P1.1](#p11--updatestationrating-completo--hecho): `GoodsEntry::rating`
  nace en 175 y, mientras la estación nunca haya movido ese tipo de carga, sube de uno en uno sin
  pasar de ahí. Test `new_station_starts_at_initial_rating`.

### P0.4 — Rating inicial de autoridad local ✔ · hecho

- **Problema** — los pueblos arrancan indiferentes (0) en vez de moderadamente favorables (500),
  lo que desplaza todos los umbrales de permisos.
- **Original** — `RATING_INITIAL = 500` (`town_type.h:45`), asignado por compañía en
  `town_cmd.cpp:2072`.
- **Port** — `local_authority_rating: 0` (`town.rs:85`, y en los constructores de
  `sav/entities.rs:293`, `sav/write/mod.rs:343`).
- **Hecho** — constante `TOWN_RATING_INITIAL = 500` usada por `Default for Town` y por el
  `serde(default)` del campo, así que también aplica a los pueblos importados de un `.sav` y a las
  partidas guardadas antes del cambio. Test `new_town_starts_at_initial_rating`.
- **Hecho (P1.9)** — el desglose por compañía está en `authority_ratings[]` con migración serde.

### P0.5 — Todas las industrias producen en el mismo tick · hecho

- **Problema** — el port dispara la producción de todas las industrias en `tick % 256`, lo que
  crea picos sincronizados de carga en el mundo entero.
- **Original** — cada industria decrementa su propio `counter` y produce cuando
  `counter % INDUSTRY_PRODUCE_TICKS == 0`, con fase distinta por industria
  (`industry_cmd.cpp:1177-1191`).
- **Port** — `tick.is_multiple_of(256)` global (`industry.rs:297-298`).
- **Hecho** — `Industry::counter` (12 bits, como `GB(r, 4, 12)`) sembrado al fundar con el RNG
  determinista de teselas; `produces_on_tick` desplaza la fase en vez de mirar el tick global.
  Test `industries_produce_on_their_own_phase`.
- **Pendiente** — las industrias que vienen de un `.sav` importado nacen con fase 0, porque el
  chunk `INDY` todavía no decodifica `counter`. Va con [ROADMAP_SAV_EXPORT.md](ROADMAP_SAV_EXPORT.md).

### P0.6 — La hierba crece ocho veces más rápido · hecho

- **Problema** — el tile loop sube la densidad de hierba en cada visita, saltándose el contador
  intermedio del original.
- **Original** — `TileLoop_Clear` lleva un contador 0..7 y solo al octavo incremento sube la
  densidad 0→3 (`clear_cmd.cpp:271-284`).
- **Port** — sube densidad directamente cuando `cycle & 7 == 7` (`map/tree_tile_loop.rs:251-267`).
- **Hecho** — contador 0..7 en `m5` bits 5–7 (`clear_counter` / `with_clear_counter`), como
  `GetClearCounter`. Cada parcela madura en su propio momento en vez de cambiar toda la franja del
  tile loop a la vez. Tests `grass_density_needs_eight_visits_per_step` y
  `grass_tiles_with_different_counters_ripen_apart`.
- **Pendiente** — los campos (`CoalField`) siguen con el ciclo global: sus bits de `m5` están
  ocupados por la etapa de cultivo.

### P0.7 — No existe el interruptor `reserve_paths` · reclasificada a P2

- **Problema** — el PBS está siempre activo, así que no se pueden reproducir partidas vanilla con
  el ajuste desactivado.
- **Original** — `_settings_game.pf.reserve_paths` ("always reserve paths regardless of signal
  type") **es `false` por defecto** (`table/settings/pathfinding_settings.ini:89`). Con el valor
  por defecto solo se reserva en segmentos que ya son PBS; el ajuste en `true` fuerza `SIGSEG_PBS`
  en todos (`train_cmd.cpp:2044`, `2324`, `2730`).
- **Port** — reserva incondicional (`sim_step/movement.rs:85-92`), es decir, el port se comporta
  como `reserve_paths = true`, que es el modo **no** vanilla.
- **Por qué no es S** — el interruptor por sí solo no sirve: para poder ponerlo en `false` hay que
  saber si el segmento del tren es PBS o de bloque, que es lo que resuelve `UpdateSignalsOnSegment`
  y el port no distingue. Añadir el flag con default `true` no cambiaría nada observable.
- **Solución** — junto al trabajo de segmentos de señal de P2: clasificar el segmento y reservar
  solo cuando sea PBS o el ajuste lo fuerce.
- **Coste** — M dentro de P2.

---

## 5. P1 — Reglas de comportamiento

Veintidós entradas, **todas cerradas** (P1.1 —con P0.3— a P1.22).
1
### P1.1 — `UpdateStationRating` completo · hecho

- **Problema** — el rating era `255 − días de espera`. No influían la velocidad del vehículo, la
  edad del servicio ni el stock acumulado, y no había convergencia suave: la señal económica que
  debería premiar servir bien una estación no existía.
- **Original** — `station_cmd.cpp:3973-4126`, cada `STATION_RATING_TICKS = 185`
  (`timer_game_tick.h:78`, disparo en `station_cmd.cpp:4335-4339`). Términos: `last_speed - 85`
  desplazado 2; escalones de espera en 21/12/6/3 (+25/+25/+45/+35); base −90; umbrales de stock
  1500/1000/600/300/100; +26 por estatua; edad del vehículo <3/<2/<1 (+10/+10/+13); convergencia
  `clamp(objetivo - actual, -2, 2)`; truncado aleatorio si el rating queda bajo con mucho stock.
- **Hecho** — `station/goods_entry.rs` introduce `GoodsEntry` por carga (rating persistente,
  `has_rating`, `last_speed`, `last_age`, `max_waiting_cargo`) y `station/cargo_rating.rs` porta
  la función: objetivo con todos los términos, convergencia ±2 y los dos truncados aleatorios con
  el RNG de partida (`cargo_rng`), más el recorte progresivo por encima de 4096 unidades. La
  estación recuerda al último vehículo que cargó (`Vehicle::station_visit`, con las unidades de
  velocidad de cada tipo como en `economy.cpp:1745-1765`) y el tipo de vehículo, que es lo que
  hace que los barcos esperen cuatro veces más antes de penalizar. Save v22 con migración que
  reparte el rating agregado antiguo a las cargas que la estación estaba moviendo. Tests
  `new_station_starts_at_initial_rating`, `good_service_raises_rating_two_points_per_sweep`,
  `unserved_cargo_creeps_back_to_initial_rating`, `station_rating_decays_with_waiting_cargo` y
  `abandoned_station_loses_rating_over_time`.
- **Cambio de comportamiento** — el rating por compañía deja de existir como tal: en el original
  es de la estación e igual para todos, y la competencia se resuelve al repartir la producción
  ([P1.2](#p12--reparto-de-carga-entre-estaciones-competidoras--hecho)). `company_time_since_pickup` se
  conserva porque ese reparto lo va a necesitar.
- **Pendiente** — el +26 por estatua (no hay acciones de ayuntamiento todavía, va con
  [P1.10](#p110--efecto-real-de-la-publicidad), igual que `ModifyStationRatingAround`) y el número
  real de destinos: hoy `num_dests = 1`, que equivale al reparto manual del original.

### P1.2 — Reparto de carga entre estaciones competidoras · hecho

- **Problema** — no había competencia: cada estación recogía lo suyo sin repartir con las demás que
  cubrían la misma industria, y la producción primaria ni siquiera pasaba por el andén.
- **Original** — `TransportIndustryGoods` saca el stock de la industria y `MoveGoodsToStation`
  pondera por `rating + 1` y reparte entre estaciones y compañías con
  `amount * company_best * station_rating / best_sum / company_sum`
  (`station_cmd.cpp:4599-4660`, disparo en `industry_cmd.cpp:541`). El rating también decide
  cuánto llega de verdad (`UpdateStationWaiting` hace `>> 8` sobre el escalado).
- **Hecho** — `station/move_goods.rs` porta `CanMoveGoodsToStation`, `UpdateStationWaiting`
  (con `amount_fract` en `GoodsEntry`) y `MoveGoodsToStation`. Tras cada ciclo de producción,
  `transport_industry_goods` mueve hasta 255 unidades del stock a las estaciones en cobertura.
  Los pueblos generan pax/correo por el mismo camino, así que el rating también recorta lo que
  llega al andén. Con `selectgoods`, una estación nunca visitada no recibe nada: el stock se
  queda en la industria (o no se genera en el pueblo) hasta que un vehículo intenta cargar
  (`note_station_load_attempt`, aunque el andén esté vacío). El camión en la tesela de la mina
  puede cargar del andén de la estación que la cubre. Tests
  `two_stations_compete_for_mine_output_by_rating`, `mine_production_splits_between_competing_stations`,
  `single_station_receives_rating_fraction`, `better_rated_station_gets_more_cargo`,
  `companies_compete_by_best_rating`, `unvisited_station_leaves_stock_on_industry`.
- **Pendiente** — el reparto entre paradas de bus que se pisan las mismas casas exige la
  producción por casa de [P1.7](#p17--producción-de-pasajeros-por-casa); hoy cada parada genera
  por su propio catchment. Tampoco está el consumidor exclusivo de industria
  (`exclusive_consumer`).

### P1.3 — Producción industrial por spec · hecho

- **Problema** — toda industria sumaba 8 unidades por ciclo, sin relación con su tipo.
- **Original** — cada spec tiene su `production_rate`, escalado por `prod_level` con
  `CeilDiv(rate * prod_level, PRODLEVEL_DEFAULT)` (`industry_cmd.cpp:1160-1230`, `2592-2600`).
- **Hecho** — `IndustrySpec::production_rate` con los valores de `build_industry.h` (carbón 15,
  bosque 13, pozos 12, minas 10/7, etc.; procesadoras 0). `Industry::produce_amount` aplica
  `CeilDiv(rate * prod_level, 16)`. Las fábricas de goods siguen transformando insumos y
  escalan su salida de 8 con el mismo `prod_level`. Hecho junto a [P1.4](#p14--prod_level-y-cierre-de-industrias--hecho).
- **Pendiente** — segundo cargo de granja (livestock) y rates de NewGRF.

### P1.4 — `prod_level` y cierre de industrias · hecho

- **Problema** — las industrias nunca cambiaban de producción ni cerraban, así que el mapa
  económico era estático y una ruta rentable lo era para siempre.
- **Original** — `prod_level` con `PRODLEVEL_DEFAULT = 0x10`, mínimo `0x04`, máximo `0x80` y
  cierre en `0x00` (`industry.h:35-38`); bucles diario y mensual con umbrales de transporte
  `PERCENT_TRANSPORTED_60 = 153` y `_80 = 204`, duplicando o partiendo el nivel y cerrando en el
  mínimo (`industry_cmd.cpp:2872-3148`). El bucle diario también crea industrias nuevas.
- **Hecho** — `Industry::prod_level` (default 0x10, serde para saves antiguos).
  `change_industry_production` porta el modo **original** (extractive/organic): cada día de
  calendario una industria al azar puede subir o bajar según el % transportado del mes pasado;
  al bajar desde el mínimo se marca cierre. El mes siguiente `remove_closed_industries` la
  borra del mapa y publica noticia (`NewsType::IndustryClose`). Sin historial mensual todavía
  no hay cambios (hace falta un mes cerrado). Pozos temperate solo bajan. Tests
  `coal_mine_produces_fifteen_at_default_level`, `doubling_prod_level_doubles_output`,
  `poor_service_closes_mine_from_minimum`, `closed_industries_are_removed_next_month`.
- **Pendiente** — economía smooth, creación diaria de industrias nuevas, abandono de
  procesadoras a los 5 años, noticias de subida/bajada de producción (hoy solo cierre).

### P1.5 — Transformación de insumos · hecho

- **Problema** — solo las fábricas de `Goods` consumen algo; acería, refinería y el resto de
  cadenas no transforman.
- **Original** — al aceptar carga, `produced.waiting += accepted.waiting * input_cargo_multiplier / 256`
  con la matriz de multiplicadores del spec (`economy.cpp:1156-1165`).
- **Hecho** — `IndustrySpec::processing_inputs` con multiplicadores 256 para temperate:
  aserradero (madera→goods), refinería (petróleo→goods), acería (hierro+carbón→acero),
  fábrica (madera+carbón→goods). `produce_from_nearby_stations` consume desde estaciones en
  cobertura y aplica `out += in * multiplier / 256`. Corregido `output_cargo` de aserradero y
  refinería. Tests `sawmill_consumes_wood_for_goods`, `steel_mill_consumes_iron_and_coal_for_steel`,
  `oil_refinery_consumes_oil_for_goods` y fábrica actualizada.
- **Pendiente** — colas `accepted[]`/`produced[]` persistentes en la industria (hoy se consume
  del andén cada ciclo); specs de otros climas y segundo cargo de granja.

### P1.6 — Pago diferido de transferencias · hecho

- **Problema** — cada descarga cobra el ingreso completo en el acto, también cuando es una
  transferencia intermedia. Eso hace rentables cadenas de feeder que en el original no lo son y
  rompe el sentido de `feeder_share`.
- **Original** — `PayTransfer` solo acumula el valor en `feeder_share` del paquete y el comentario
  es explícito en que no hay cobro; `PayFinalDelivery` paga `profit - GetFeederShare` al entregar
  (`economy.cpp:1217-1248`).
- **Port** — ingreso completo por descarga y 75 % a la primera estación
  (`sim_step/cargo_transfer.rs:76-128`).
- **Hecho** — `CargoUnloadAction::Transfer` solo acumula `feeder_share` (75 % del tramo) sin
  `credit_company`; `Deliver` liquida el acumulado al owner de `first_station` y paga al
  entregador `income - feeder_share`. Tests `feeder_share_paid_on_unload_preserves_packet_flags`
  (sin cobro en trasbordo) y `final_delivery_liquidates_accumulated_feeder_share`.
- **Pendiente** — el freight en hub sigue clasificándose siempre como `Transfer` hasta portar el
  staging de OpenTTD ([P2.19](#p219--clasificación-de-carga)); hoy no hay ingreso en andén
  intermedio, solo en entrega final de pax/mail.

### P1.7 — Producción de pasajeros por casa · hecho

- **Problema** — el port genera 2 pasajeros y 1 correo por casa dentro de la cobertura de la
  parada. La producción no depende del tipo de casa ni existe fuera del alcance de una estación.
- **Original** — `TileLoop_Town` genera por casa con `hs->population` y `hs->mail_generation`,
  vía `TownGenerateCargoOriginal` o binomial con RNG (`town_cmd.cpp:602-667`, `751-778`).
- **Hecho** — `produce_town_cargo` recorre casas del mapa, usa `HOUSE_POPULATION` y
  `HOUSE_MAIL_GENERATION` (generado desde `town_land.h`), escala al ciclo de 256 ticks y reparte
  con `MoveGoodsToStation` entre paradas que comparten cobertura. Tests
  `produce_uses_house_spec_population`, `competing_bus_stops_split_house_passengers_by_rating` y
  `produce_adds_cargo_when_houses_in_coverage` actualizado.
- **Pendiente** — algoritmo original/binomial con RNG por tesela; `TileLoop_Town` completo con
  edad de casas ([P3.6](#p36--renovación-de-casas)).

### P1.8 — Tasa de crecimiento urbano · hecho

- **Problema** — las ciudades suman 10 de población por pulso, sin depender de cuántas estaciones
  las sirven de verdad ni de su tamaño.
- **Original** — `GetNormalGrowthRate` indexa `_grow_count_values[2][6]` (normal
  `{320,420,300,220,160,100}`, financiado `{120,120,120,100,80,60}`) por `CountActiveStations`,
  ajusta por ciudad y divide por `num_houses/50+1` (`town_cmd.cpp:3819-3856`). Sin estaciones y
  sin financiación solo crece con `Chance16(1,12)` (`town_cmd.cpp:3876-3911`).
- **Port** — `grow_town_if_served` con paso fijo (`town.rs:222`, `293-331`).
- **Hecho** — `growth_rate` y `grow_counter` por pueblo; cadencia `TOWN_GROWTH_TICKS = 70`;
  `get_normal_growth_rate` desde estaciones activas y tablas `_grow_count_values`; expansión física
  sin step abstracto de población; `Chance16(1,12)` sin estaciones; metas invierno/desierto con
  altura (`DEF_SNOW_LINE_HEIGHT`) y desierto tropical. Tests `growth_rate_scales_with_active_stations`,
  `unserved_town_growth_requires_chance_without_funding`.
- **Coste** — L.

### P1.9 — Autoridad local por compañía · hecho

- **Problema** — hay un único rating compartido, así que dos compañías en el mismo pueblo son
  indistinguibles para el ayuntamiento y no hay penalización ni recuperación con el tiempo.
- **Original** — `ratings[MAX_COMPANIES]` (`town_type.h`), evolución mensual con
  `RATING_GROWTH_UP_STEP = 5` hasta `RATING_GROWTH_MAXIMUM = 200` y ±12/−15 por estaciones bien o
  mal servidas (`town_cmd.cpp:3766-3794`); `CheckforTownRating` exige umbrales según
  `town_council_tolerance` para tocar carretera, túnel o puente municipal (`town_cmd.cpp:4077-4104`).
- **Port** — un `i16` y un umbral fijo de −200 solo para estaciones (`town.rs:43-45`, `355-363`).
- **Hecho** — `authority_ratings[]` por `CompanyId` (serde + save v23); `update_town_rating` mensual;
  estaciones y financiación usan `active_company`; `check_town_rating` con `TownCouncilTolerance` en
  demolición municipal (`OWNER_TOWN_M1`). Tests `authority_ratings_are_per_company`,
  `update_town_rating_recovers_and_penalizes_by_station_service`.
- **Coste** — M el modelo, L con los chequeos.

### P1.10 — Efecto real de la publicidad · hecho

- **Problema** — la campaña sube el rating del ayuntamiento, cuando en el original no lo toca.
- **Original** — la publicidad llama a `ModifyStationRatingAround` con +0x40/+0x70/+0xA0 según el
  tamaño (`town_cmd.cpp:3412-3445`). Lo que sí afecta al ayuntamiento es financiar edificios
  (`fund_buildings_months = 3`), la estatua, el soborno y reconstruir carreteras.
- **Port** — `TOWN_ADVERTISE_RATING_BOOST` sobre el rating del pueblo (`command/town.rs:20-46`).
- **Hecho** — `station::modify_station_rating_around` (+0x70, radio 15, campaña mediana única) sobre
  cargas activas de estaciones del `active_company` en el radio; `town_advertise` ya no toca
  `local_authority_rating`. Test `town_advertise_boosts_nearby_station_rating_not_authority`.
- **Pendiente** — estatua (+26 rating estaciones y pending de P1.1), exclusividad 12 meses y
  soborno; tamaños small/large de publicidad si se exponen en UI.

### P1.11 — Modelo de averías · hecho

- **Problema** — la avería era determinista (`tick * id % 256` bajo un umbral de fiabilidad) y duraba
  siempre 3 días. No había acumulación de riesgo ni relación con la velocidad.
- **Original** — `breakdown_chance` acumulativo (+1 por tick, +25 con probabilidad 1/25) contra la
  tabla `_breakdown_chance[rel >> 10]`, con `breakdown_ctr = GB(r,16,6)+0x3F` y
  `breakdown_delay = GB(r,24,7)+0x80` sacados de `Random()`, y solo con `cur_speed >= 5`
  (`vehicle.cpp:1276-1324`). `HandleBreakdown` tiene fases (`vehicle.cpp:1332-1392`).
- **Hecho** — `vehicle/reliability.rs` porta la tabla, el acumulador diario (`check_vehicle_breakdown`
  con `cargo_rng`), las fases `HandleBreakdown` (`breakdown_ctr` / `breakdown_delay`) en cada tick de
  movimiento, bonus de fiabilidad para barcos y umbral `cur_speed >= 5`. El evento `SimEvent::Breakdown`
  se emite al entrar en avería activa (fase 2→1). Tests en `reliability.rs` y
  `check_breakdown_triggers_when_unreliable`.
- **Pendiente** — ajuste `vehicle_breakdowns` reducido/nulo y RNG global de [P2.3](#tabla-p2) para
  reproducibilidad bit a bit.

### P1.12 — Decaimiento de fiabilidad por motor · hecho

- **Problema** — todos los motores perdían 10 puntos cada 256 ticks; el catálogo no influaba.
- **Original** — `reliability -= reliability_spd_dec` propio del motor (`engine.cpp:785`,
  `vehicle.cpp:1294`), que además se duplica al pasar `max_age` (`vehicle.cpp:1410-1450`).
- **Hecho** — `EngineDef` incluye `reliability_spd_dec` y `lifelength_years` (valores del upstream en
  `catalog_data.rs`; barcos con `decay_speed` 5 → 20). Barrido diario con escala port 0..10 000,
  copia al vehículo al comprar, y `age_vehicle_calendar_day` duplica `reliability_spd_dec` en los
  años tras `max_age`. Tests `reliability_decays_by_engine_spd_dec` y
  `reliability_spd_dec_doubles_after_max_age_year_boundary`.
- **Hecho (P2.5)** — barrido diario de economía escalonado con `economy_timer.date_fract`.

### P1.13 — Autorenew · hecho

- **Problema** — solo existen las reglas de reemplazo `from → to`. Un vehículo viejo sin regla
  configurada envejece para siempre.
- **Original** — `GetNewEngineType` devuelve el mismo `engine_type` cuando `NeedsAutorenewing`
  (`age - max_age >= engine_renew_months * 30`), aparte de las reglas de grupo
  (`autoreplace_cmd.cpp:281-309`, `vehicle.cpp:156-171`).
- **Hecho** — `autoreplace.rs` renueva al mismo motor si `engine_renew` y
  `needs_autorenewing(tick, engine_renew_months)` con `max_age_days` del motor; reserva
  `engine_renew_money` en reemplazo y en `pending_autoreplace_for_service`. Tests
  `autorenew_same_engine_when_old_enough` y `autorenew_respects_engine_renew_money`.
- **Coste** — L.

### P1.14 — Servicio automático · hecho

- **Problema** — el intervalo solo se expresa en días y el vehículo no busca depósito por su
  cuenta.
- **Original** — `NeedsServicing` acepta intervalo en días **o** en porcentaje de fiabilidad,
  respeta `no_servicing_if_no_breakdowns` y puede mandar a depósito solo por un autoreplace
  pendiente si hay dinero para el doble del coste (`vehicle.cpp:201-276`);
  `CheckIfRoadVehNeedsService` inserta la orden vía pathfinder (`roadveh_cmd.cpp:1682-1718`).
- **Hecho** — `requires_service_for_company` / `requires_service_with` con intervalo en días o %,
  `no_servicing_if_no_breakdowns` + `vehicle_breakdowns`, y `check_road_vehicles_need_service`
  inserta `depot_pass_through` con pathfinder road. Tests `requires_service_by_percent_threshold`,
  `requires_service_by_day_interval` y `road_vehicle_inserts_depot_order_when_service_due`.
- **Coste** — L.

### P1.15 — Inflación · hecho

- **Problema** — inflación lineal inventada (`1024 + años × 4`), que diverge del original desde el
  primer año y no tiene tope.
- **Original** — compuesta mensual: `inflation_prices += (inflation_prices * infl_amount * 54) >> 16`
  y lo mismo para pagos con `infl_amount_pr = max(0, initial_interest - 1)`, solo entre 1920 y
  2090, con base `1 << 16` (`economy.cpp:704-737`, `909-922`).
- **Hecho** — `GlobalEconomy` en `economy/global.rs` con acumuladores `inflation_prices` /
  `inflation_payment`, actualización mensual en `sim_step/economy.rs`, inflación previa al año de
  arranque en `startup`, y `max_loan` escalado vía `scaled_max_loan`. Pagos y costes de
  construcción leen los acumuladores (no la fórmula lineal). Tests `compound_inflation_matches_openttd_monthly_step`,
  `inflation_stops_outside_original_year_window`, `max_loan_scales_with_inflation_prices`,
  `startup_applies_pre_1950_inflation`.
- **Pendiente** — multiplicadores NewGRF por índice (`SetPriceBaseMultiplier`).

### P1.16 — Sistema de precios base · hecho

- **Problema** — los costes eran constantes sueltas por acción, sin dificultad ni escala común.
- **Original** — tabla `PriceBase` escalada por dificultad (6/8/9) e inflación, consultada como
  `GetPrice(index, cost_factor, grf, shift)` (`economy.cpp:742-794`, `949-962`,
  `table/pricebase.h`).
- **Hecho** — `economy/pricebase.rs` con tabla `_price` de los índices principales,
  `get_price`/`base_price_at` escalando `GlobalEconomy.inflation_prices` y dificultad
  (`construction_cost`, `vehicle_costs`). Migrados a `GetPrice`: terraform, compra de terreno,
  objetos, vía, carretera, estación y waypoint (`economy/build_costs.rs` + comandos de
  construcción). Constantes en `game_state/mod.rs` alineadas a dificultad media sin inflación.
- **Pendiente** — depósitos, túneles/puentes, señales, clear-tile, compra de vehículos vía índice,
  infraestructura de mantenimiento y multiplicadores NewGRF.

### P1.17 — Costes de funcionamiento · hecho

- **Problema** — coste fijo por tipo y solo en movimiento; un vehículo parado no costaba nada.
- **Original** — `GetRunningCost()` desde el spec del motor, prorrateado como
  `coste * running_ticks / (365 * DAY_TICKS)` (`train_cmd.cpp:4195-4287`).
- **Hecho** — `running_cost_year` del catálogo, prorrateo con acumulador fraccional
  (`running_cost_accum`) en `economy/vehicle_costs.rs`; cobro en `sim_step/economy.rs` aunque el
  vehículo esté parado si sigue en servicio (`running` / tren con `cur_speed > 0`). Suma del
  consist ferroviario. Tests `running_cost_prorates_yearly_catalog_cost`,
  `stopped_bus_with_running_flag_still_costs`.

### P1.18 — Interés, valor de compañía e índice de rendimiento · hecho

- **Problema** — tres fórmulas financieras aproximadas que desalineaban las finanzas y el ranking.
- **Original** — interés: cuota anual prorrateada por mes, interés también sobre caja negativa y
  cargo fijo de `_price[PR_STATION_VALUE] >> 2` (`economy.cpp:800-827`). Valor:
  `instalaciones * PR_STATION_VALUE * 25 + Σ(v->value * 3/2) - préstamo + caja`
  (`economy.cpp:115-158`). Rendimiento: nueve componentes de `_score_part` con topes propios
  escalados a 1000 (`economy.cpp:91-102`, `202-314`).
- **Hecho** — `monthly_company_interest` + `monthly_station_maintenance_fee` en `economy/payments.rs`
  (mes 0..11 desde `calendar_month_index`); valor con `StationValue` vía `get_price` y
  `vehicle_asset_value` en `economy_quarterly.rs`; rating con componentes principales de
  `_score_part` (vehículos rentables, estaciones servidas, entregas, liquidez, préstamo).
  Tests `monthly_interest_on_100k_loan`, `monthly_interest_includes_negative_cash`,
  `company_value_uses_station_value_times_facilities`, `performance_rating_includes_profit_and_stations`.
- **Pendiente** — `v->value` con depreciación diaria; variedad real de cargas en `ScoreID::Cargo`;
  ingresos min/max desde 12 meses con desglose por tipo de cargo.

### P1.19 — Subsidios · hecho

- **Problema** — generación determinista cada 8 meses, solo industria a estación, sin límite de
  distancia y con multiplicador fijo ×2.
- **Original** — mensual con `RandomRange(16)`: pasajeros 1/8, carga de pueblo 1/16, carga de
  industria 1/16; filtros de población y porcentaje transportado; `SUBSIDY_MAX_DISTANCE = 70`;
  duración `subsidy_duration * 12` meses; multiplicador configurable +50 %/×2/×3/×4
  (`subsidy.cpp:425-497`, `507-572`, `economy.cpp:1124-1131`).
- **Hecho** — `subsidy.rs` genera mensualmente (pax/town/industry), aplica `SUBSIDY_MAX_DISTANCE`,
  población mínima, duración `subsidy_duration * 12` y multiplicador por `subsidy_multiplier`.
  Rutas pueblo→pueblo y carga con destino town/industry. Tests `passenger_subsidy_respects_distance_and_population`,
  `subsidy_award_duration_uses_subsidy_duration_years` y `award_on_first_delivery_uses_difficulty_multiplier`.
- **Coste** — L.

### P1.20 — Flags de orden que faltan · hecho

- **Problema** — la orden solo tenía `full_load` y `no_unload`, así que no existen transferencia
  forzada, no cargar, `FullLoadAny`, vías sin parar ni posición de parada. Es la carencia que más
  limita las rutas que el jugador puede montar.
- **Original** — `OrderLoadFlags` y `OrderUnloadFlags` en `order_type.h:67-82`;
  `OrderNonStopFlags` y `OrderStopLocation` en `order_type.h:87-123`; condicionales con ocho
  variables por ocho comparadores en `order_type.h:128-152`.
- **Hecho** — `VehicleOrder::Station` con `no_load`, `full_load_any`, `transfer`, `non_stop`
  (`OrderNonStop`); `station_flags_from_sav` / `station_flags_to_sav` sin colapsar flags;
  respeto en `cargo_transfer.rs` y `order_execution`/`movement.rs`. Tests
  `station_flags_preserve_transfer_no_load_and_timetable`, `full_load_any_flag_is_not_collapsed_to_full_load`,
  `no_load_order_skips_loading`.
- **Pendiente** — `OrderStopLocation` (andén inicio/medio/fin, ver P3.14) y condicionales
  extendidas (ocho variables).

### P1.21 — Horarios reales · hecho

- **Problema** — hay esperas por `wait_ticks` pero nadie mide el tiempo real de viaje, así que el
  retraso no es fiable y no se puede escalonar una línea.
- **Original** — `UpdateVehicleTimetable` resetea `current_order_time`, hace autofill con redondeo
  y ajusta `lateness_counter` (`timetable_cmd.cpp:469-575`); `timetable_start` y
  `CmdSetTimetableStart` reparten los vehículos de un grupo (`timetable_cmd.cpp:351-411`).
- **Hecho** — `current_order_time` + `update_vehicle_timetable` (lateness, autofill con redondeo
  a segundo); import ORDL conserva `wait_time`/`travel_time`; `timetable_start` y comando
  `SetVehicleTimetableStart`. Tests `timetable_clock_increments_current_order_time`,
  `autofill_sets_travel_ticks_on_arrival`, `lateness_increases_when_late`,
  `timetable_start_offsets_lateness_on_first_arrival`.
- **Pendiente** — reparto automático entre vehículos de órdenes compartidas (`timetable_all`);
  import de `timetable_start` desde chunk `VEHS`.

### P1.22 — Rama asintótica del factor de tiempo y fluctuaciones · hecho

- **Problema** — dos reglas menores del pago y del clima económico.
- **Original** — para tránsitos muy largos el factor cae por
  `max(2 * MIN_TIME_FACTOR * 16 * 16 / (exceso + 32), 1)` con desplazamiento 25
  (`economy.cpp:1010-1015`); `HandleEconomyFluctuations` genera recesiones con noticias
  (`economy.cpp:844-863`).
- **Hecho** — `cargo_time_factor` devuelve la rama asintótica con `TIME_FACTOR_FRAC = 16` y
  desplazamiento 25 en `transported_goods_income`. `GlobalEconomy::fluct` con
  `handle_monthly_fluctuations` y noticias `NewsType::Economy`. Tests
  `asymptotic_time_factor_for_very_long_transit`, `recession_cycle_emits_fluctuation_events`.
- **Pendiente** — efectos de recesión en producción industrial / pueblos (hoy solo el contador y
  las noticias).

---

## 6. P2 — Bucles de simulación

Rediseños que habilitan la reproducibilidad. Mientras estén abiertos, cualquier golden tick a
tick contra OpenTTD seguirá divergiendo aunque las fórmulas individuales sean correctas.

<a id="tabla-p2"></a>

| ID | Tema | Problema (port) | Original | Solución | Coste |
|----|------|-----------------|----------|----------|-------|
| **P2.1** | Relojes separados | Un único `GameTick`; el calendario se deriva como `tick / 74` (`tick.rs:16-31`) | Tres relojes independientes —calendario, economía y tick— con `date_fract` 0..73 (`timer_game_economy.cpp:131`, `timer_game_calendar.cpp:117`) | Introducir los tres timers con su `date_fract`; es la base de todos los barridos escalonados | XL · **hecho** |
| **P2.2** | Orden del tick | · hecho | `AnimateAnimatedTiles` → timers → `RunTileLoop` → `CallVehicleTicks` (carga y luego movimiento) → `CallLandscapeTick` (`openttd.cpp:1257-1265`) | Secuencia OpenTTD; PBS grueso post-move (elección atómica en B4) | XL · **hecho** |
| **P2.3** | RNG global | · hecho | `_random` global consumido por toda la simulación y persistido en el save, con `_interactive_random` aparte (`random_func.cpp:36-48`, `misc_sl.cpp:96`) | `random` + `interactive_random` en `GameState`; alias serde `cargo_rng` → `random`; consumido en rating/subsidios/averías/desastres | XL |
| **P2.4** | `CallLandscapeTick` | · hecho | Orden town → trees → station → industry → companies → linkgraph (`landscape.cpp:1727-1740`) | `sim_step/landscape.rs::call_landscape_tick`; linkgraph stub hasta P2.21 | L · **hecho** |
| **P2.5** | Barridos diarios de vehículos | · hecho | `RunVehicleCalendarDayProc` y `RunEconomyVehicleDayProc` recorren 1/74 de la flota por día, con costes y averías cada 8 días (`vehicle.cpp:907-951`) | Barrido escalonado por `date_fract`; calendar day envejece, economy day averías; cada tick en `sim_step` | M · **hecho** |
| **P2.6** | `RunTileLoop` | · hecho | LFSR de Galois con feedback según tamaño de mapa y estado en `_cur_tileloop_tile` (`landscape.cpp:798-835`) | LFSR portado; una pasada por tick; tesela 0 especial cada 256 ticks; estado persistido | M |
| **P2.7** | `TrainController` | · hecho | `ChooseTrainTrack` decide el ramal con YAPF y reserva de forma atómica al entrar en la tesela; los vagones siguen `_connecting_track` (`train_cmd.cpp:3289-3487`, `2727-2888`) | `choose_train_track_on_enter` + YAPF en `advance_one_tile` | XL · **hecho** |
| **P2.8** | Liberación de reservas | · hecho | `FreeTrainTrackReservation` recorre la reserva tesela a tesela y pone en rojo las señales PBS al liberarlas (`train_cmd.cpp:2419-2477`) | `free_train_track_reservation` walk + PBS rojo en sync | L · **hecho** |
| **P2.9** | Costes de YAPF | A* con tesela 1, señal roja 100, cruce de reserva 80; sin caché ni look-ahead (`pathfinder/yapf.rs:24-194`) | Segmentos con caché y penalizaciones calibradas: tesela 100, esquina 71, primera roja 1000, cruce de reserva 300, coste de estación y plataforma (`yapf_costrail.hpp:59-615`) | Portar la escala de costes y la segmentación; hoy las elecciones de ramal divergen en redes densas | XL |
| **P2.10** | Reversa en señal | · hecho | `wait_oneway_signal`, `wait_twoway_signal` y `reverse_at_signals` actúan durante el movimiento (`train_cmd.cpp:3375-3422`) | `tick_signal_wait_and_maybe_reverse` en el bucle de movimiento | L · **hecho** |
| **P2.11** | Geometría del consist | Las poses de los vagones se proyectan desde el historial de la cabeza (`train_consist/pose.rs:25-78`) | Cada unidad avanza en su propio paso con `CalcNextVehicleOffset` dentro de `TrainController` (`train_cmd.cpp:1903-1966`) | Simular unidad a unidad; afecta a trenes largos en curvas y túneles | XL |
| **P2.12** | PBS en cruces | · hecho | Con `Split` o `MultiEnter` la señal va a rojo aunque no haya tren (`signal.cpp:410-458`) | Flags en `explore_sig_segment` → PBS rojo sin reserva | M · **hecho** |
| **P2.13** | Buffer de señales | · hecho | `_globset` guarda pares (tesela, `DiagDirection`) y fuerza actualización a las 64 entradas (`signal.cpp:589-610`) | `SignalGlobEntry` + flush a 64 | M · **hecho** |
| **P2.14** | Controlador de carretera | · hecho | `IndividualRoadVehicleController` avanza frame a frame con `_road_drive_data` y los estados `RVSB_*` (`roadveh_cmd.cpp:1201-1576`, `roadveh.h:38-57`) | FSM en `road_movement/controller.rs` + tablas `drive_data` | XL · **hecho** |
| **P2.15** | Tráfico en carretera | · hecho | `RoadVehFindCloseTo` sincroniza velocidad con el de delante y `blocked_ctr > 1480` permite atravesarlo (`roadveh_cmd.cpp:627-694`) | `road_movement/traffic.rs` | XL · **hecho** |
| **P2.16** | Pasos por tick | · hecho | Bucle `while (j >= adv_spd)` que consume varios sub-pasos en el mismo tick (`roadveh_cmd.cpp:1610-1645`) | `road_vehicle_tick` con remanente en `progress` | L · **hecho** |
| **P2.17** | Órdenes implícitas | Un solo `current_order`; el índice implícito del save se descarta (`sav/array_legacy.rs:245-246`) | `OT_IMPLICIT` con `cur_implicit_order_index` y `cur_real_order_index`, insertadas al visitar estaciones (`base_consist.h:47-48`, `vehicle.cpp:2152-2275`) | Portar los dos índices y la inserción automática | XL |
| **P2.18** | `ProcessOrders` | Avance lineal módulo longitud de lista (`order_execution.rs:65-78`) | Máquina que interrumpe por depósito, resuelve vías, avanza índices y busca depósito más cercano (`order_cmd.cpp:1949-2159`) | Portar `ProcessOrders` y `UpdateOrderDest`; depende de P2.17 | XL |
| **P2.19** | Clasificación de carga | Decisión directa al descargar, sin fase previa ni reserva (`cargo_packet/operations.rs:31-48`) | `PrepareUnload` llama a `Stage`, que clasifica cada paquete en `TRANSFER`, `DELIVER`, `KEEP` o `LOAD` usando los flujos (`cargopacket.cpp:406-526`) | Portar el staging; es lo que hace correcto el pago diferido de P1.6 | XL |
| **P2.20** | Cola de estación | Cola FIFO por tipo de carga (`cargo_packet/types.rs:74-76`) | `StationCargoList` es un `MultiMap` indexado por `next_hop`, con cantidad reservada (`cargopacket.h:513-608`) | Reindexar la cola por destino | L |
| **P2.21** | Planificador del linkgraph | El MCF portado se alimenta de estadísticas de viajes y se reconstruye al mes (`cargodist/legacy/link_graph.rs`) | `OnTick_LinkGraph` lanza y recoge jobs asíncronos sobre una copia del grafo de estaciones cuando `date_fract == 21`, con `recalc_interval` (`linkgraphschedule.cpp:202-216`) | Construir el grafo desde estaciones y planificar jobs; el MCF ya está portado en `cargodist/parity/` | XL |
| **P2.22** | Siguiente parada del flujo | Siguiente estación distinta de la lista (`vehicle/order.rs:134-152`) | `GetNextStoppingStation` recorre la lista de forma recursiva con vías y condicionales (`order_cmd.cpp:363-409`) | Portar la resolución recursiva; depende de los flags de P1.20 | M |

**P2.1 · hecho** — `timer/mod.rs`: `CalendarTimer` y `EconomyTimer` con `date_fract` 0..73,
`elapsed_tick()` → `TimerTriggers` (día/mes/año), persistidos en `GameState` con migración serde
desde `tick`. `sim_step` avanza ambos relojes tras `tick.advance()`; economía mensual e intereses
usan `economy_timer`, noticias y edad de vehículo usan `calendar`. Por defecto alineados (sin
wallclock).

**P2.5 · hecho** — `process_vehicle_calendar_day` / `process_vehicle_economy_day` recorren
`index % DAY_TICKS == date_fract` cada tick (`vehicle/reliability.rs`); cadencia OpenTTD 1/74.

**P2.2 · hecho** — `sim_step`: animación → tile loop → paths (sin PBS) → load/unload → move →
PBS post-move → landscape. El routing PBS completo ya no precede a la carga.

**P2.4 · hecho** — `call_landscape_tick`: town → trees → station → industry → companies →
linkgraph (stub).

**P2.7 · hecho** — `choose_train_track_on_enter` elige ramal con `next_rail_trackdir_yapf`
en cruces y anota reserva atómica al entrar; `advance_one_tile` lo invoca antes de consumir
el `path`.

**P2.8 · hecho** — `free_train_track_reservation` recorre `reserved_steps`, pone PBS a rojo y
limpia `m2_hi`; `sync_reservations_to_map` también enrojece PBS al liberar teselas.

**P2.10 · hecho** — `wait_oneway_signal` / `wait_twoway_signal` (defaults 15/41 días ×2) y
`reverse_at_signals` en el bucle de movimiento vía `tick_signal_wait_and_maybe_reverse`.

**P2.12 · hecho** — `SigSegmentProbe::{split,multi_enter}` en `explore_sig_segment`; PBS sin
reserva permanece roja con Split/MultiEnter.

**P2.13 · hecho** — `_globset` como `SignalGlobEntry { tile, enter_dir }` con
`SIG_GLOB_UPDATE = 64` y flush inmediato al encolar en movimiento.

**P2.14–P2.16 · hecho** — controlador road con `road_state`/`frame`, tablas drive, bucle
`while j >= adv_spd` y `RoadVehFindCloseTo` (`blocked_ctr`).

---

## 7. P3 — Mundo y contenido

<a id="tabla-p3"></a>

| ID | Tema | Problema (port) | Original | Solución | Coste |
|----|------|-----------------|----------|----------|-------|
| **P3.1** | Generación de pueblos e industrias | Ausente en la generación de mundo; solo fundación manual (`command/town.rs:49-121`) | `GenerateTowns` coloca `{5,11,23,46}` pueblos escalados por tamaño con layout y proporción de ciudades (`town_cmd.cpp:2432-2485`); `GenerateIndustries` reparte por probabilidad, clima y proporción tierra/agua (`industry_cmd.cpp:2488-2540`) | Portar ambas al `world_gen`; sin esto no hay escenario para comparar una partida completa | XL |
| **P3.2** | Inundación | El agua nunca inunda | `TileLoop_Water` propaga en diagonal, arrasa la tesela y ahoga vehículos (`water_cmd.cpp:1074-1301`) | Portar `DoFloodTile` y `FloodVehicles` | XL |
| **P3.3** | Generación de terreno | Ruido por capas propio (`world_gen/mod.rs:35-130`) | TGP con Perlin y ajustes por `terrain_type`, `quantity_sea_lakes` y coberturas de nieve y desierto (`tgp.cpp`, `landscape.cpp:1606-1706`) | Portar TGP con sus parámetros de configuración | XL |
| **P3.4** | Expansión física del pueblo | Radio 12, tres intentos y solo hierba plana (`town_expand.rs:9-43`) | `GrowTownAtRoad` recorre el grafo de carreteras con iteraciones según `TownLayout` y respeta rejillas y puentes (`town_cmd.cpp:1793-1950`) | Portar el recorrido y los layouts | XL |
| **P3.5** | Elección de casa | Identificador por `seed % 110` (`town_expand.rs:228-235`) | `TryBuildTownHouse` filtra por `HouseZone`, años de validez y probabilidad ponderada, con edificios únicos (`town_cmd.cpp:2814-2935`) | Portar `_house_specs` completa al runtime | L |
| **P3.6** | Renovación de casas | Las casas no envejecen | Pasado `minimum_life` se demuelen y se reconstruyen con probabilidad 20/256 (`town_cmd.cpp:671-705`) | Portar la edad de casa dentro de `TileLoop_Town` (P1.7) | L |
| **P3.7** | Aceptación de carga urbana | Ausente | `AddAcceptedCargo_Town` acepta bienes, comida o agua según el spec (`town_cmd.cpp:805-851`) | Portar la aceptación por casa | L |
| **P3.8** | Radio de zonas del pueblo | Población abstracta, sin zonas | `UpdateTownRadius` con `_town_squared_town_zone_radius_data` según número de casas (`town_cmd.cpp:1956-1997`) | Portar la tabla de radios; requisito de P3.5 | M |
| **P3.9** | Propagación del desierto | Solo se pinta en la generación inicial | `TileLoopClearDesert` ajusta densidad según vecinos y convierte hierba en desierto (`clear_cmd.cpp:234-253`) | Portar la transición en el tile loop | M |
| **P3.10** | Adelantamiento en carretera | Ausente | `RoadVehCheckOvertake` con carril opuesto, aceleración 512 y `RV_OVERTAKE_TIMEOUT = 35` (`roadveh_cmd.cpp:806-857`) | Portar tras P2.14 y P2.15 | L |
| **P3.11** | Choques en tierra | Solo está implementado el choque de aviones (`aircraft_crash.rs`) | `Vehicle::Crash` con `crashed_ctr` hasta 2220 ticks y `RoadVehCheckTrainCrash` en pasos a nivel (`roadveh_cmd.cpp:524-553`, `vehicle.cpp:291-317`) | Portar el estado `Crashed` y el chequeo en cruces | L |
| **P3.12** | Reemplazo de cadena | Cambia `engine_id` sobre el mismo vehículo (`autoreplace.rs:61-77`) | `ReplaceChain` reconstruye el consist con articulados, dual-head y wagon removal (`autoreplace_cmd.cpp:739-816`) | Portar la reconstrucción; depende de P1.13 | XL |
| **P3.13** | Pendiente en carretera | El efecto solo existe en trenes | `RoadZPosAffectSpeed` aplica 232/256 al subir y +2 al bajar (`roadveh_cmd.cpp:859-868`) | Portar la corrección por altura | M |
| **P3.14** | Punto de parada en andén | Siempre en el centro del andén (`station/geometry.rs:142-197`) | `GetTrainStopLocation` coloca al principio, en medio o al final según orden y longitud (`train_cmd.cpp:263-299`) | Portar junto con `OrderStopLocation` (P1.20) | M |
| **P3.15** | Límite del tipo de vía | No se aplica durante el movimiento | `cached_max_track_speed` limita según el railtype de la tesela (`train_cmd.cpp:382-426`) | Consultar el railtype al avanzar | M |
| **P3.16** | Distancia de pago | Manhattan entre origen y estación de entrega (`sim_step/cargo_transfer.rs:81-82`) | El paquete acumula distancia recorrida y `GetDistance` la usa por tramos (`cargopacket.h:220-252`) | Añadir `travelled` y `source_xy` al paquete | M |
| **P3.17** | Truncado y división de paquetes | Truncado por tipo, sin prorrateo del feeder (`cargo_packet/types.rs:136-204`) | Truncado aleatorio por destino; `Split` reparte `feeder_share` proporcionalmente (`cargopacket.cpp:94-102`, `763-806`) | Portar ambas operaciones | M |
| **P3.18** | Reroute de carga | Ausente | `Reroute` reasigna `next_hop` cuando cambian los flujos (`cargopacket.cpp:663-667`) | Portar tras P2.21 | M |
| **P3.19** | Modo wallclock | Ausente | `TimerGameEconomy::UsingWallclockUnits` con meses de 30 días desacoplados del calendario (`timer_game_economy.cpp:98-103`) | Añadir el modo tras P2.1 | L |
| **P3.20** | Consist: vagones y railtypes | Sin powered wagons, límites de velocidad por vagón ni `compatible_railtypes` por unidad (`train_consist/topology.rs:68-150`) | `ConsistChanged` los calcula por unidad con callbacks GRF (`train_cmd.cpp:107-250`) | Extender el cacheo del consist | M |

---

## 8. Qué ya está en paridad

No reabrir sin evidencia nueva; varios puntos tienen tests golden.

| Área | Piezas verificadas |
|------|--------------------|
| **Tiempo y azar** | `DAY_TICKS = 74` y 27 ms/tick (`timer_game_tick.h:75` ↔ `economy/time.rs:4`) · algoritmo `Randomizer` con `0x1234567F` y test de secuencia (`random_func.cpp:47` ↔ `cargodist/parity/rng.rs:34`) · frecuencia 256 del tile loop y ciclo `11x + 9y + (tick >> 8)` (`tree_cmd.cpp:848` ↔ `map/tree_tile_loop.rs:38`) · `TREE_UPDATE_FREQUENCY = 16` |
| **Física de vehículos** | `DoUpdateSpeed` con `subspeed` y `tempmax` (`ground_vehicle.hpp:365` ↔ `engine/physics.rs:71`) · `GetAdvanceSpeed` y distancias 192/256 (`vehicle_base.h:412` ↔ `physics.rs:45`) · aceleración original de tren (×2, freno ×4) y carretera (256) · −25 % al girar · `GetCurveSpeedLimit` con golden 61/88/231 y tilt |
| **Ferrocarril** | Codificación de señales (tipos, colocación, variantes, ciclo de UI) · presignals entry/exit/combo en las topologías con test, incluido wormhole · modelo de reserva PBS por track bit con límite de 64 pasos · `CheckTrainStayInDepot` con espera de 37 ticks y rollback · `CalcNextVehicleOffset` y tablas de subcoordenadas |
| **Economía y carga** | Tarifas de los 11 cargos templados y núcleo del ingreso (`>> 21`, factores 31 y 255) · feeder share 75 % (`economy.cpp:1245` ↔ `company.rs:171`) · `INDUSTRY_PRODUCE_TICKS = 256` · truncado a 255 periodos · préstamo en tramos de 10.000 con techo 300.000 · pipeline MCF y cálculo de demanda con los tres modos de distribución |
| **Aviación y paisaje** | FTA de aeropuertos, `AirportFtaFlags` y crash de jet en pista corta (3276 sobre 2²²) · densidad de nieve `k = z − snowline + 1` (`clear_cmd.cpp:190` ↔ `map/tree_tile_loop.rs:426`) · radio de autoridad 20 y clamp de rating ±1000 · tabla de población de las 110 casas originales · generadores de nombres de pueblo |

---

## 9. Método y limitaciones

Seis exploraciones paralelas del C++ y del port, con la consigna de extraer primero la regla del
original y solo después buscar su equivalente en Rust. Las entradas marcadas **✔ verificado**
([P0.1](#p01--la-quiebra-ignora-el-préstamo---hecho) a [P0.4](#p04--rating-inicial-de-autoridad-local---hecho))
se comprobaron abriendo ambos ficheros. El resto conserva la referencia de línea que devolvió la
exploración: **confirmar la línea antes de abrir el issue**, porque el upstream se mueve.

El inventario corto [PARIDAD_OPENTTD.md](PARIDAD_OPENTTD.md) mide qué funciones existen; este
documento mide si el comportamiento coincide. Son ejes distintos y conviene no mezclarlos: al
cerrar la FTA de aeropuertos el primero pasó a ✅ mientras el segundo seguía teniendo divergencias
abiertas en el mismo subsistema.

---

*Auditoría: 2026-07-25 · referencia OpenTTD 15.3 (`14ec60f`) · 71 entradas (P0 7 · P1 22 · P2 22 · P3 20).*
