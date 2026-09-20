# V1-AIRH: primer servicio de pasajeros de RoadHaul

Contrato acotado de [#597](https://github.com/cavazquez/openttdrs/issues/597).
Certifica una sola ruta urbana de pasajeros de la IA propia; no intenta
certificar NoAI, Squirrel, estrategia general ni mapas arbitrarios.

## Fixture y límites

`v1_roadhaul_delivery.rs` crea un mapa Temperate plano de 64×64 con seed
`0x5970_0064`, dos pueblos en `(24,20)` y `(32,20)` y seis casas normales por
pueblo para que exista demanda urbana legítima. RoadHaul recibe 2.000.000 de
capital inicial; no hay vehículos, estaciones, infraestructura, colas de
construcción, carga ni ingresos prefabricados. La distancia fija respeta el
rango de planificación de RoadHaul y deja un objetivo pequeño: una ruta, un
bus y la primera entrega antes de 40.000 ticks.

El fixture fija desastres desactivados y `vehicle_breakdowns=0` para excluir
eventos externos aleatorios, conserva `selectgoods` y llama exclusivamente a
`GameState::step()`. El scheduler construye carretera, paradas y depósito,
compra el bus y crea sus órdenes mediante los comandos productivos; la prueba
no inyecta comandos, vehículos, carga, pagos ni ingresos para reemplazar sus
decisiones.

## Resultado reproducible

Dos ejecuciones con el mismo seed producen exactamente el mismo reporte:

```text
seed: 0x5970_0064
ruta construida: tick 2568
bus comprado y órdenes preparadas: tick 2592
primera entrega pagada: tick 3990
costo neto de RoadHaul: 9948
ingreso de la entrega: 24
órdenes: (25,20) -> (31,20)
canonical_hash: 17524054488457107959
```

El reporte conserva el seed y los ticks de construcción, compra, órdenes y
primera entrega como log compacto del recorrido. En cada tick compara dinero,
vehículos, estaciones e infraestructura de la compañía humana contra una
ejecución gemela con la IA desactivada, por lo que una mutación de RoadHaul en
activos ajenos falla inmediatamente.

Dos regresiones forman parte del recorrido: el servicio vial conserva una sola
orden implícita de depósito mientras el depósito sigue siendo alcanzable, y un
bus que llega vacío a una parada registra su intento de carga. Este último
evento habilita la generación de pasajeros con `selectgoods`, sin fabricar
carga para la IA.

Desde la raíz del repositorio:

```sh
cargo test --locked --offline -p openttdrs-core --test v1_roadhaul_delivery -- --exact --nocapture
cargo test --locked --offline -p openttdrs-core --lib vehicle::reliability::tests::road_service_order_is_removed_when_no_depot_is_reachable -- --exact --nocapture
cargo test --locked --offline -p openttdrs-core --lib sim_step::cargo_transfer::tests::empty_station_arrival_records_load_attempt_before_town_supply_exists -- --exact --nocapture
```

El cierre de #597 exige CI remota verde para el SHA publicado. Quedan fuera
competitividad, beneficio neto inicial, climas/seeds/mapas alternativos,
competencia humana activa y todo runtime NoAI/Squirrel.
